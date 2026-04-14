use crate::FileSummary;
use crate::analysis::{self, LetSite};
use crate::parser;
use anyhow::{Result, bail};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Payload for an upsert operation on a let binding.
pub struct BindingSpec {
    /// Binding name (required).
    pub name: String,
    /// Right-hand side expression (what goes after `=`).
    pub body: String,
    /// Type annotation text (after `:`). `None` = preserve on update.
    pub type_annotation: Option<String>,
    /// Function parameters. `None` = preserve on update.
    pub params: Option<Vec<String>>,
    /// `true` = remove existing signature on update.
    pub no_type: bool,
    /// Sibling positioning: insert/move after this sibling name.
    pub after: Option<String>,
    /// Sibling positioning: insert/move before this sibling name.
    pub before: Option<String>,
    /// Absolute file line to resolve ambiguity.
    pub line: Option<usize>,
}

// ---------------------------------------------------------------------------
// Public functions
// ---------------------------------------------------------------------------

/// Upsert a let binding within the named top-level declaration.
///
/// On update: replaces body, optionally replaces/removes type annotation and
/// params according to `spec`. On insert: appends (or positions via
/// `after`/`before`) in the outermost let block (or the let block containing
/// `spec.line`).
pub fn upsert_let_binding(
    source: &str,
    summary: &FileSummary,
    decl_name: &str,
    spec: &BindingSpec,
) -> Result<String> {
    let decl = summary
        .find_declaration(decl_name)
        .ok_or_else(|| anyhow::anyhow!("declaration '{}' not found", decl_name))?;

    // Parse the whole file to get a tree for analysis.
    let tree = parser::parse(source)?;
    let root = tree.root_node();
    let decl_node = find_value_decl_node(root, source, decl_name).ok_or_else(|| {
        anyhow::anyhow!("could not find value_declaration node for '{decl_name}'")
    })?;

    let all_sites = analysis::collect_let_sites(decl_node, source);
    let matching: Vec<&LetSite> = all_sites.iter().filter(|s| s.name == spec.name).collect();

    if matching.is_empty() {
        // INSERT path
        insert_let_binding(source, decl_node, &all_sites, spec, decl)
    } else {
        // UPDATE path — resolve ambiguity if needed
        let site = resolve_site(matching, spec.line, &spec.name, decl_name)?;
        update_let_binding(source, decl_node, site, &all_sites, spec)
    }
}

/// Remove a single let binding (and its type sig) from the named declaration.
pub fn remove_let_binding(
    source: &str,
    summary: &FileSummary,
    decl_name: &str,
    binding_name: &str,
    line_hint: Option<usize>,
) -> Result<String> {
    let _decl = summary
        .find_declaration(decl_name)
        .ok_or_else(|| anyhow::anyhow!("declaration '{}' not found", decl_name))?;

    let tree = parser::parse(source)?;
    let root = tree.root_node();
    let decl_node = find_value_decl_node(root, source, decl_name).ok_or_else(|| {
        anyhow::anyhow!("could not find value_declaration node for '{decl_name}'")
    })?;

    let all_sites = analysis::collect_let_sites(decl_node, source);
    let matching: Vec<&LetSite> = all_sites
        .iter()
        .filter(|s| s.name == binding_name)
        .collect();

    if matching.is_empty() {
        bail!(
            "let binding '{}' not found in '{}'",
            binding_name,
            decl_name
        );
    }

    let site = resolve_site(matching, line_hint, binding_name, decl_name)?;

    // Pre-check: refuse to leave an empty let block. Removing the sole
    // binding in a `let … in …` would produce `let\nin body`, which fails
    // the re-parse gate with an opaque error — diagnose it up front.
    if let Some(let_node) = find_let_containing(decl_node, site.node_span.0) {
        let siblings = bindings_in_let_block(let_node, source, &all_sites);
        if siblings.len() == 1 {
            bail!(
                "cannot remove '{binding_name}' from '{decl_name}': it is the only binding in its let block; \
                 rewrite the enclosing declaration with `set decl` to remove the let entirely"
            );
        }
    }

    remove_site_from_source(source, site, decl_node)
}

/// Remove multiple let bindings in one atomic all-or-nothing operation.
///
/// All names are resolved up front. If any name is missing or ambiguous the
/// function returns an error without modifying the source. Processing is
/// done rear-to-front by byte offset so earlier spans stay valid.
pub fn remove_let_bindings_batch(
    source: &str,
    summary: &FileSummary,
    decl_name: &str,
    names: &[String],
) -> Result<String> {
    let _decl = summary
        .find_declaration(decl_name)
        .ok_or_else(|| anyhow::anyhow!("declaration '{}' not found", decl_name))?;

    let tree = parser::parse(source)?;
    let root = tree.root_node();
    let decl_node = find_value_decl_node(root, source, decl_name).ok_or_else(|| {
        anyhow::anyhow!("could not find value_declaration node for '{decl_name}'")
    })?;

    let all_sites = analysis::collect_let_sites(decl_node, source);

    // Resolve all names up front; collect all errors.
    let mut errors: Vec<String> = Vec::new();
    let mut resolved: Vec<&LetSite> = Vec::new();

    for name in names {
        let matching: Vec<&LetSite> = all_sites.iter().filter(|s| &s.name == name).collect();
        match matching.len() {
            0 => errors.push(format!(
                "let binding '{}' not found in '{}'",
                name, decl_name
            )),
            1 => resolved.push(matching[0]),
            _ => {
                // Ambiguous in a batch call — require single-target retry.
                let candidates = format_candidates(&matching, decl_name);
                errors.push(format!(
                    "'{name}' is ambiguous in '{decl_name}': {} candidates:\n{candidates}\nretry with --line <N>",
                    matching.len()
                ));
            }
        }
    }

    if !errors.is_empty() {
        bail!("{}", errors.join("\n"));
    }

    // Pre-check: would this batch empty any let block? For each resolved
    // target, find its containing let_in_expr and count how many of that
    // block's sibling bindings are ALSO in the resolved set. If the count
    // equals the block's total sibling count, we'd leave an empty `let … in`.
    for site in &resolved {
        let Some(let_node) = find_let_containing(decl_node, site.node_span.0) else {
            continue;
        };
        let siblings = bindings_in_let_block(let_node, source, &all_sites);
        let siblings_in_targets = siblings
            .iter()
            .filter(|s| {
                resolved
                    .iter()
                    .any(|r| r.node_span == s.node_span && r.name == s.name)
            })
            .count();
        if siblings_in_targets == siblings.len() && !siblings.is_empty() {
            let names: Vec<String> = siblings.iter().map(|s| s.name.clone()).collect();
            bail!(
                "cannot remove {:?} from '{decl_name}': that batch would empty a let block; \
                 rewrite the enclosing declaration with `set decl` to remove the let entirely",
                names
            );
        }
    }

    // Sort rear-to-front by node_span start to avoid byte-offset drift.
    resolved.sort_by(|a, b| b.node_span.0.cmp(&a.node_span.0));

    let mut current = source.to_string();
    for site in resolved {
        // Re-parse on each pass because indentation helper needs the current tree.
        let tree2 = parser::parse(&current)?;
        let root2 = tree2.root_node();
        let decl_node2 = find_value_decl_node(root2, &current, decl_name).ok_or_else(|| {
            anyhow::anyhow!("could not find declaration node after edit ('{decl_name}')")
        })?;
        let sites2 = analysis::collect_let_sites(decl_node2, &current);
        // Find the site by name (already known unambiguous after front-loading).
        // After re-parsing, use the same name; there's guaranteed to be one.
        let site2 = sites2.iter().find(|s| s.name == site.name).ok_or_else(|| {
            anyhow::anyhow!("binding '{}' disappeared between batch steps", site.name)
        })?;
        current = remove_site_from_source(&current, site2, decl_node2)?;
    }

    Ok(current)
}

/// Rename a let binding and every reference to it within the enclosing declaration.
pub fn rename_let_binding(
    source: &str,
    summary: &FileSummary,
    decl_name: &str,
    old: &str,
    new: &str,
    line_hint: Option<usize>,
) -> Result<String> {
    let decl = summary
        .find_declaration(decl_name)
        .ok_or_else(|| anyhow::anyhow!("declaration '{}' not found", decl_name))?;

    let tree = parser::parse(source)?;
    let root = tree.root_node();
    let decl_node = find_value_decl_node(root, source, decl_name).ok_or_else(|| {
        anyhow::anyhow!("could not find value_declaration node for '{decl_name}'")
    })?;

    let all_sites = analysis::collect_let_sites(decl_node, source);
    let matching: Vec<&LetSite> = all_sites.iter().filter(|s| s.name == old).collect();

    if matching.is_empty() {
        bail!("let binding '{}' not found in '{}'", old, decl_name);
    }

    // Shadowing guard: if `old` is bound by more than one let binding in the
    // decl, the rename cannot be scoped to one of them without producing
    // incorrect output, because the identifier rewrite pass walks every
    // `lower_case_identifier` in the decl and can't tell which site's
    // references belong to which binding. Elm 0.19 disallows shadowing, so
    // reaching this branch at all means the source is invalid Elm that
    // `elm make` will reject anyway — `--line` cannot help here because the
    // rename's scope-independent rewrite has no way to respect scope.
    if matching.len() > 1 {
        let candidates = format_candidates(&matching, decl_name);
        let _ = line_hint; // documented: --line would not help; see message below
        bail!(
            "cannot rename '{old}' in '{decl_name}': {} bindings with that name exist at different scopes (Elm 0.19 disallows this shadowing):\n{candidates}\n`--line <N>` cannot disambiguate here because the rewrite cannot be scoped; fix the shadowing first, or rewrite the enclosing declaration with `set decl`",
            matching.len()
        );
    }

    let site = resolve_site(matching, line_hint, old, decl_name)?;

    // Collision check: is `new` already bound in scope at the binding's position?
    let in_scope = analysis::collect_binders_in_scope(decl_node, source, site.node_span.0);
    if in_scope.contains(new) {
        bail!("'{}' is already in scope in '{}'", new, decl_name);
    }

    // `decl` is unused now that we splice by byte offsets, but we keep the
    // lookup above for the `find_declaration` side-effect (it catches
    // decl_name mismatches before we reach into the tree).
    let _ = decl;

    // Rewrite references within the enclosing top-level decl using a
    // tree-sitter pass over `lower_case_identifier` nodes. The rewrite
    // operates on the `value_declaration` node only — the let binding and all
    // its references live inside the function body, so the top-level type
    // annotation (if present) does not need rewriting and is preserved
    // verbatim by splicing byte offsets instead of reassembling lines.
    let rewritten_decl = rewrite_identifier_in_decl(decl_node, source, old, new)?;

    let mut result = source.to_string();
    result.replace_range(
        decl_node.start_byte()..decl_node.end_byte(),
        &rewritten_decl,
    );
    Ok(result)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Find the `value_declaration` tree-sitter node for the named top-level decl.
fn find_value_decl_node<'a>(
    root: tree_sitter::Node<'a>,
    source: &str,
    name: &str,
) -> Option<tree_sitter::Node<'a>> {
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() == "value_declaration" {
            let child_name = {
                let fdl = child.child_by_field_name("functionDeclarationLeft")?;
                let mut c = fdl.walk();
                fdl.named_children(&mut c)
                    .find(|n| n.kind() == "lower_case_identifier")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .map(str::to_owned)
            };
            if child_name.as_deref() == Some(name) {
                return Some(child);
            }
        }
    }
    None
}

/// Format ambiguity candidates for the error message.
fn format_candidates(matching: &[&LetSite], _decl_name: &str) -> String {
    matching
        .iter()
        .map(|s| {
            let scope_hint = if s.scope_path.len() > 1 {
                format!(" (inside {})", s.scope_path.last().unwrap())
            } else {
                String::new()
            };
            format!("  line {}{}", s.line, scope_hint)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Resolve a single site from a slice of candidates.
///
/// If `line_hint` is provided, picks the candidate at that line.
/// If there is exactly one candidate, returns it.
/// Otherwise returns an ambiguity error.
fn resolve_site<'a>(
    matching: Vec<&'a LetSite>,
    line_hint: Option<usize>,
    binding_name: &str,
    decl_name: &str,
) -> Result<&'a LetSite> {
    if let Some(line) = line_hint {
        let found = matching.iter().find(|s| s.line == line).copied();
        return found.ok_or_else(|| {
            anyhow::anyhow!(
                "no binding '{}' at line {} in '{}'",
                binding_name,
                line,
                decl_name
            )
        });
    }
    if matching.len() == 1 {
        return Ok(matching[0]);
    }
    let candidates = format_candidates(&matching, decl_name);
    bail!(
        "'{binding_name}' is ambiguous in '{decl_name}': {} candidates:\n{candidates}\nretry with --line <N>",
        matching.len()
    );
}

/// Detect the indentation of let-bindings in the same scope level.
///
/// We look at sibling sites that share the same scope_path; if there are any,
/// we read their first line from `source` and take the leading whitespace.
/// Falls back to 4 spaces.
fn detect_indent(source: &str, sites: &[LetSite], scope_path: &[String]) -> String {
    let lines: Vec<&str> = source.lines().collect();
    for site in sites {
        if site.scope_path == scope_path {
            let line_idx = site.line - 1; // 0-indexed
            if let Some(line) = lines.get(line_idx) {
                let indent: String = line
                    .chars()
                    .take_while(|c| *c == ' ' || *c == '\t')
                    .collect();
                if !indent.is_empty() {
                    return indent;
                }
            }
        }
    }
    "    ".to_string()
}

/// Build the text for a binding from its components.
fn build_binding_text(
    name: &str,
    params: &[String],
    body: &str,
    type_annotation: Option<&str>,
    indent: &str,
) -> String {
    let mut text = String::new();
    if let Some(ann) = type_annotation {
        text.push_str(&format!("{indent}{name} : {ann}\n"));
    }
    if params.is_empty() {
        text.push_str(&format!("{indent}{name} =\n{indent}    {body}"));
    } else {
        let param_str = params.join(" ");
        text.push_str(&format!("{indent}{name} {param_str} =\n{indent}    {body}"));
    }
    text
}

/// Find the outermost `let_in_expr` descendant of `decl_node`.
fn find_outermost_let<'a>(decl_node: tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
    if let Some(body) = decl_node.child_by_field_name("body") {
        find_first_let(body)
    } else {
        None
    }
}

fn find_first_let<'a>(node: tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
    if node.kind() == "let_in_expr" {
        return Some(node);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(found) = find_first_let(child) {
            return Some(found);
        }
    }
    None
}

/// Find the `let_in_expr` that contains the given byte offset.
fn find_let_containing<'a>(
    decl_node: tree_sitter::Node<'a>,
    byte_offset: usize,
) -> Option<tree_sitter::Node<'a>> {
    if let Some(body) = decl_node.child_by_field_name("body") {
        find_let_containing_in(body, byte_offset)
    } else {
        None
    }
}

fn find_let_containing_in<'a>(
    node: tree_sitter::Node<'a>,
    byte_offset: usize,
) -> Option<tree_sitter::Node<'a>> {
    if node.kind() == "let_in_expr"
        && node.start_byte() <= byte_offset
        && byte_offset <= node.end_byte()
    {
        // Return this as a candidate but also check children for a tighter fit.
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if let Some(inner) = find_let_containing_in(child, byte_offset) {
                return Some(inner);
            }
        }
        return Some(node);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.start_byte() <= byte_offset
            && byte_offset <= child.end_byte()
            && let Some(found) = find_let_containing_in(child, byte_offset)
        {
            return Some(found);
        }
    }
    None
}

/// Collect the sibling binding sites (value_declaration + preceding
/// type_annotation pairs) within a specific `let_in_expr` node in source order.
fn bindings_in_let_block<'s>(
    let_node: tree_sitter::Node<'_>,
    source: &str,
    all_sites: &'s [LetSite],
) -> Vec<&'s LetSite> {
    // Filter all_sites to direct children only (not nested further).
    let direct: Vec<&'s LetSite> = direct_let_sites(let_node, source, all_sites);
    direct
}

/// Collect sites that are direct children of this let block (not nested inside
/// a deeper let within it).
fn direct_let_sites<'s>(
    let_node: tree_sitter::Node<'_>,
    _source: &str,
    all_sites: &'s [LetSite],
) -> Vec<&'s LetSite> {
    // Direct children are those value_declaration/type_annotation nodes that
    // are named children of let_node. We use the fact that LetSite.scope_path
    // length equals the nesting depth.
    // The let_node itself was walked by visit_let_block which sets scope_path =
    // parent_scope (the path *above* this let block).
    // We can determine the depth by finding the let_node's parent scope from
    // the tree structure, but it's simpler to use byte ranges: a site is a
    // direct child if no other let_in_expr that is a descendant of let_node
    // contains it.
    let let_start = let_node.start_byte();
    let let_end = let_node.end_byte();

    // Collect all nested let node ranges.
    let nested_lets = collect_nested_let_ranges(let_node);

    all_sites
        .iter()
        .filter(|s| {
            // Must be within this let block.
            if s.node_span.0 < let_start || s.node_span.1 > let_end {
                return false;
            }
            // Must NOT be inside any nested let block within this one.
            !nested_lets
                .iter()
                .any(|(ns, ne)| s.node_span.0 >= *ns && s.node_span.1 <= *ne)
        })
        .collect()
}

/// Collect (start_byte, end_byte) of every `let_in_expr` that is a direct or
/// indirect descendant of `let_node` (but not `let_node` itself).
fn collect_nested_let_ranges(let_node: tree_sitter::Node) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    // Descend into the body only (the `in` expression), not the bindings part.
    // Actually, nested lets can appear inside binding bodies too.
    // We collect all descendant let_in_expr nodes except let_node itself.
    let mut cursor = let_node.walk();
    for child in let_node.named_children(&mut cursor) {
        collect_nested_let_ranges_inner(child, &mut ranges);
    }
    ranges
}

fn collect_nested_let_ranges_inner(node: tree_sitter::Node, ranges: &mut Vec<(usize, usize)>) {
    if node.kind() == "let_in_expr" {
        ranges.push((node.start_byte(), node.end_byte()));
        // Don't descend further — those would be doubly-nested.
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_nested_let_ranges_inner(child, ranges);
    }
}

/// The INSERT path: add a new binding to a let block.
fn insert_let_binding(
    source: &str,
    decl_node: tree_sitter::Node,
    all_sites: &[LetSite],
    spec: &BindingSpec,
    _decl: &crate::Declaration,
) -> Result<String> {
    // Pick the target let block.
    let let_node = if let Some(line) = spec.line {
        // Find the let block that contains this line.
        // Convert line to a byte offset (start of that line).
        let byte_offset = line_to_byte_offset(source, line);
        find_let_containing(decl_node, byte_offset)
            .ok_or_else(|| anyhow::anyhow!("no let block found at line {line}"))?
    } else {
        find_outermost_let(decl_node)
            .ok_or_else(|| anyhow::anyhow!("no let block found in declaration"))?
    };

    let siblings = bindings_in_let_block(let_node, source, all_sites);

    // Determine the scope_path for this let block (used for indent detection).
    let scope_path: Vec<String> = if let Some(first) = siblings.first() {
        first.scope_path.clone()
    } else {
        vec![]
    };

    let indent = detect_indent(source, all_sites, &scope_path);

    let params: &[String] = spec.params.as_deref().unwrap_or(&[]);
    let new_text = build_binding_text(
        &spec.name,
        params,
        &spec.body,
        spec.type_annotation.as_deref(),
        &indent,
    );

    // Determine insertion position.
    let (insert_byte, needs_newline_before) =
        find_insert_position(source, let_node, &siblings, spec)?;

    let mut result = source.to_string();
    let insertion = if needs_newline_before {
        format!("\n{}", new_text)
    } else {
        new_text
    };
    result.insert_str(insert_byte, &insertion);
    Ok(result)
}

/// Compute the byte offset where we should insert the new binding text,
/// and whether we need to prepend a newline.
fn find_insert_position(
    source: &str,
    let_node: tree_sitter::Node,
    siblings: &[&LetSite],
    spec: &BindingSpec,
) -> Result<(usize, bool)> {
    if let Some(after_name) = &spec.after {
        // Find the sibling named `after_name` and insert after its last byte.
        let sibling = siblings
            .iter()
            .find(|s| &s.name == after_name)
            .ok_or_else(|| {
                anyhow::anyhow!("sibling '{}' not found for --after positioning", after_name)
            })?;
        let end = sibling.node_span.1;
        // Skip trailing whitespace/newline to position after the binding's last line.
        let end = skip_to_end_of_line(source, end);
        return Ok((end, true));
    }

    if let Some(before_name) = &spec.before {
        // Find the sibling named `before_name` and insert before its first byte.
        let sibling = siblings
            .iter()
            .find(|s| &s.name == before_name)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "sibling '{}' not found for --before positioning",
                    before_name
                )
            })?;
        // Insert at the start of the sibling (beginning of its line).
        let start = start_of_line_for_byte(source, sibling.node_span.0);
        return Ok((start, false));
    }

    // Default: append after last sibling (before the `in` keyword).
    if let Some(last) = siblings.last() {
        let end = last.node_span.1;
        let end = skip_to_end_of_line(source, end);
        return Ok((end, true));
    }

    // No siblings — insert at start of the let block's binding area.
    // Find the first named child of the let_in_expr that is a binding.
    // The let block structure: `let <bindings> in <body>`. The `let` keyword
    // is a keyword child. We insert at the position right after `let\n`.
    let let_start = let_node.start_byte();
    // Find `let` keyword — it's the first child bytes.
    let src_bytes = source.as_bytes();
    // Skip `let` + whitespace to find where to put our binding.
    let after_let = let_start + "let".len();
    // Skip any whitespace.
    let mut pos = after_let;
    while pos < src_bytes.len() && (src_bytes[pos] == b' ' || src_bytes[pos] == b'\t') {
        pos += 1;
    }
    if pos < src_bytes.len() && src_bytes[pos] == b'\n' {
        pos += 1; // skip the newline after `let`
    }
    Ok((pos, false))
}

/// Skip source bytes forward from `pos` until we're past the current line
/// (past the `\n`).
fn skip_to_end_of_line(source: &str, pos: usize) -> usize {
    let bytes = source.as_bytes();
    let mut p = pos;
    while p < bytes.len() && bytes[p] != b'\n' {
        p += 1;
    }
    if p < bytes.len() {
        p + 1 // skip the `\n`
    } else {
        p
    }
}

/// Return the byte offset of the start of the line containing `byte_pos`.
fn start_of_line_for_byte(source: &str, byte_pos: usize) -> usize {
    let bytes = source.as_bytes();
    let mut p = byte_pos;
    while p > 0 && bytes[p - 1] != b'\n' {
        p -= 1;
    }
    p
}

/// Convert a 1-indexed line number to a byte offset of the start of that line.
fn line_to_byte_offset(source: &str, line: usize) -> usize {
    let mut current_line = 1;
    let mut byte = 0;
    for (i, ch) in source.char_indices() {
        if current_line == line {
            byte = i;
            break;
        }
        if ch == '\n' {
            current_line += 1;
        }
    }
    byte
}

/// Dedent a multi-line string by removing the common leading whitespace prefix
/// from every non-empty line.
fn dedent(s: &str) -> String {
    let lines: Vec<&str> = s.lines().collect();
    let indent_len = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|l| {
            if l.len() >= indent_len {
                &l[indent_len..]
            } else {
                l.trim_start()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The UPDATE path: replace an existing binding.
fn update_let_binding(
    source: &str,
    decl_node: tree_sitter::Node,
    site: &LetSite,
    all_sites: &[LetSite],
    spec: &BindingSpec,
) -> Result<String> {
    // Extract the binding lines from source using line numbers (which capture
    // the leading whitespace of each line correctly), then dedent the block.
    let (span_start, span_end) = site.node_span;

    // Find the start of the first line of the binding (to include leading whitespace).
    let line_start = start_of_line_for_byte(source, span_start);

    // Find the end of the last line.
    let bytes = source.as_bytes();
    let mut line_end = span_end;
    while line_end < bytes.len() && bytes[line_end] != b'\n' {
        line_end += 1;
    }
    // Don't include the trailing newline — just up to end of last content line.
    let existing_lines_text = &source[line_start..line_end];
    let existing_text = dedent(existing_lines_text);

    // Parse the existing binding to extract current sig and params.
    let existing_info = parser::parse_let_binding(&existing_text)?;

    // Resolve type annotation for the new binding.
    let new_type = if spec.no_type {
        None
    } else if spec.type_annotation.is_some() {
        spec.type_annotation.as_deref()
    } else {
        existing_info.type_annotation.as_deref()
    };

    // Resolve params.
    let new_params: Vec<String> = if let Some(p) = &spec.params {
        p.clone()
    } else {
        existing_info.params.clone()
    };

    let indent = detect_indent(source, all_sites, &site.scope_path);
    let new_binding_text =
        build_binding_text(&spec.name, &new_params, &spec.body, new_type, &indent);

    // Check if we need to move the binding (--after / --before).
    let needs_move = spec.after.is_some() || spec.before.is_some();

    if !needs_move {
        // Simple replacement in place.
        // We replace the full line range (line_start..line_end), because the
        // new_binding_text already carries its own leading indent and replacing
        // only the content span would double-indent the first line.
        let mut result = source.to_string();
        result.replace_range(line_start..line_end, &new_binding_text);
        return Ok(result);
    }

    // Move: first remove the binding, then insert it at the new position.
    // Step 1: remove the binding's lines from source.
    let removed = remove_site_from_source(source, site, decl_node)?;

    // Step 2: re-parse and re-collect sites.
    let tree2 = parser::parse(&removed)?;
    let root2 = tree2.root_node();
    let decl_node2 = find_value_decl_node(root2, &removed, &all_sites[0].scope_path[0])
        .ok_or_else(|| anyhow::anyhow!("could not find declaration node after removal"))?;
    let all_sites2 = analysis::collect_let_sites(decl_node2, &removed);

    // Compute scope_path for the moved binding — same as original.
    let scope_path2 = site.scope_path.clone();
    let indent2 = detect_indent(&removed, &all_sites2, &scope_path2);
    let new_binding_text2 =
        build_binding_text(&spec.name, &new_params, &spec.body, new_type, &indent2);

    // Find the let block that contained the original site.
    let let_node2 = {
        let byte_offset = line_to_byte_offset(&removed, site.line.saturating_sub(1).max(1));
        // Use the scope_path's last element to identify the correct let block.
        // Fallback to outermost.
        find_let_for_scope(decl_node2, &removed, &scope_path2)
            .or_else(|| find_let_containing(decl_node2, byte_offset))
            .or_else(|| find_outermost_let(decl_node2))
            .ok_or_else(|| anyhow::anyhow!("no let block found after removal"))?
    };

    let siblings2 = bindings_in_let_block(let_node2, &removed, &all_sites2);

    // Make a temporary spec without the body (we use new_binding_text2 directly).
    let (insert_byte, needs_newline) = find_insert_position(&removed, let_node2, &siblings2, spec)?;

    let insertion = if needs_newline {
        format!("\n{}", new_binding_text2)
    } else {
        new_binding_text2
    };
    let mut result = removed;
    result.insert_str(insert_byte, &insertion);
    Ok(result)
}

/// Find the `let_in_expr` that corresponds to the given scope_path.
///
/// `scope_path[0]` is the top-level decl name (which `decl_node` already
/// represents). Each subsequent element is a binding name at progressively
/// deeper nesting. Navigation: descend into `decl_node`'s body to find the
/// outermost `let_in_expr`; if `scope_path.len() > 1`, find the
/// `value_declaration` child of that let block whose name matches
/// `scope_path[1]`, descend into ITS body to find its outermost let,
/// and repeat. Returns `None` if any step cannot be resolved (unknown
/// scope name, no let block at the expected depth).
fn find_let_for_scope<'a>(
    decl_node: tree_sitter::Node<'a>,
    source: &str,
    scope_path: &[String],
) -> Option<tree_sitter::Node<'a>> {
    let body = decl_node.child_by_field_name("body")?;
    let mut let_node = find_first_let(body)?;

    // scope_path[0] is the decl name — the let we just found is its outermost
    // let block, which is what we want if scope_path.len() == 1.
    for name in scope_path.iter().skip(1) {
        // Find the value_declaration in `let_node` with the given name.
        let target_binding = {
            let mut cursor = let_node.walk();
            let_node.named_children(&mut cursor).find(|c| {
                c.kind() == "value_declaration"
                    && c.child_by_field_name("functionDeclarationLeft")
                        .and_then(|fdl| {
                            let mut fc = fdl.walk();
                            fdl.named_children(&mut fc)
                                .find(|ch| ch.kind() == "lower_case_identifier")
                        })
                        .and_then(|id| id.utf8_text(source.as_bytes()).ok())
                        == Some(name.as_str())
            })
        };
        let binding = target_binding?;
        let binding_body = binding.child_by_field_name("body")?;
        let_node = find_first_let(binding_body)?;
    }
    Some(let_node)
}

/// Remove a binding site from source.
///
/// Removes the full line(s) that the binding occupies (including any blank
/// lines that separate it from the next binding, to keep the let block tidy).
fn remove_site_from_source(
    source: &str,
    site: &LetSite,
    _decl_node: tree_sitter::Node,
) -> Result<String> {
    let (span_start, span_end) = site.node_span;

    // Find the start of the first line of the binding.
    let line_start = start_of_line_for_byte(source, span_start);

    // Find the end of the last line of the binding, including the trailing \n.
    let bytes = source.as_bytes();
    let mut line_end = span_end;
    // Advance past the end of the last line.
    while line_end < bytes.len() && bytes[line_end] != b'\n' {
        line_end += 1;
    }
    if line_end < bytes.len() {
        line_end += 1; // include the \n
    }

    let mut result = source.to_string();
    result.replace_range(line_start..line_end, "");
    Ok(result)
}

/// Rewrite every `lower_case_identifier` node within `decl_node` whose text
/// equals `old` to `new`. Returns the rewritten declaration source string
/// (not the whole file).
fn rewrite_identifier_in_decl(
    decl_node: tree_sitter::Node,
    source: &str,
    old: &str,
    new: &str,
) -> Result<String> {
    let decl_start = decl_node.start_byte();
    let decl_end = decl_node.end_byte();
    let decl_bytes = &source.as_bytes()[decl_start..decl_end];

    // Collect all lower_case_identifier nodes with text == old, sorted by
    // byte offset within the decl (reversed for rear-to-front replacement).
    let mut replacements: Vec<(usize, usize)> = Vec::new(); // (rel_start, rel_end)
    collect_identifier_sites(
        decl_node,
        source.as_bytes(),
        old,
        decl_start,
        &mut replacements,
    );
    replacements.sort_by(|a, b| b.0.cmp(&a.0));
    replacements.dedup();

    let mut result = String::from_utf8(decl_bytes.to_vec())
        .map_err(|_| anyhow::anyhow!("decl source is not valid UTF-8"))?;

    for (rel_start, rel_end) in replacements {
        result.replace_range(rel_start..rel_end, new);
    }

    Ok(result)
}

/// Recursively walk `node` and collect `(rel_start, rel_end)` for every
/// `lower_case_identifier` whose text equals `old`.
fn collect_identifier_sites(
    node: tree_sitter::Node,
    source: &[u8],
    old: &str,
    decl_start: usize,
    out: &mut Vec<(usize, usize)>,
) {
    if node.kind() == "lower_case_identifier" {
        if let Ok(text) = node.utf8_text(source)
            && text == old
        {
            let rel_start = node.start_byte() - decl_start;
            let rel_end = node.end_byte() - decl_start;
            out.push((rel_start, rel_end));
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_identifier_sites(child, source, old, decl_start, out);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{extract_summary, parse};

    // Pass-through `indoc!` shim — test strings are already correctly laid out.
    macro_rules! indoc {
        ($s:expr) => {
            $s
        };
    }

    fn make_summary(source: &str) -> FileSummary {
        let tree = parse(source).unwrap();
        extract_summary(&tree, source)
    }

    // -----------------------------------------------------------------------
    // Scenario (a): body-only edit on a typed binding preserves the sig
    // -----------------------------------------------------------------------
    #[test]
    fn test_body_only_edit_preserves_sig() {
        let source = indoc!(
            "module M exposing (..)\n\
             update msg model =\n\
             \x20   let\n\
             \x20       helper : Int -> Int\n\
             \x20       helper n =\n\
             \x20           n + 1\n\
             \x20   in\n\
             \x20   helper 0\n"
        );
        let summary = make_summary(source);
        let spec = BindingSpec {
            name: "helper".to_string(),
            body: "n + 2".to_string(),
            type_annotation: None,
            params: None,
            no_type: false,
            after: None,
            before: None,
            line: None,
        };
        let result = upsert_let_binding(source, &summary, "update", &spec).unwrap();
        assert!(
            result.contains("helper : Int -> Int"),
            "sig should be preserved; got:\n{result}"
        );
        assert!(
            result.contains("n + 2"),
            "body should be updated; got:\n{result}"
        );
        assert!(
            !result.contains("n + 1"),
            "old body should be gone; got:\n{result}"
        );
    }

    // -----------------------------------------------------------------------
    // Scenario (b): --type on update replaces the sig
    // -----------------------------------------------------------------------
    #[test]
    fn test_type_on_update_replaces_sig() {
        let source = indoc!(
            "module M exposing (..)\n\
             update msg model =\n\
             \x20   let\n\
             \x20       helper : Int -> Int\n\
             \x20       helper n =\n\
             \x20           n + 1\n\
             \x20   in\n\
             \x20   helper 0\n"
        );
        let summary = make_summary(source);
        let spec = BindingSpec {
            name: "helper".to_string(),
            body: "String.fromInt n".to_string(),
            type_annotation: Some("Int -> String".to_string()),
            params: None,
            no_type: false,
            after: None,
            before: None,
            line: None,
        };
        let result = upsert_let_binding(source, &summary, "update", &spec).unwrap();
        assert!(
            result.contains("helper : Int -> String"),
            "sig should be replaced; got:\n{result}"
        );
        assert!(
            !result.contains("helper : Int -> Int"),
            "old sig should be gone; got:\n{result}"
        );
    }

    // -----------------------------------------------------------------------
    // Scenario (c): --no-type removes the sig
    // -----------------------------------------------------------------------
    #[test]
    fn test_no_type_removes_sig() {
        let source = indoc!(
            "module M exposing (..)\n\
             update msg model =\n\
             \x20   let\n\
             \x20       helper : Int -> Int\n\
             \x20       helper n =\n\
             \x20           n + 1\n\
             \x20   in\n\
             \x20   helper 0\n"
        );
        let summary = make_summary(source);
        let spec = BindingSpec {
            name: "helper".to_string(),
            body: "n + 1".to_string(),
            type_annotation: None,
            params: None,
            no_type: true,
            after: None,
            before: None,
            line: None,
        };
        let result = upsert_let_binding(source, &summary, "update", &spec).unwrap();
        assert!(
            !result.contains("helper : Int -> Int"),
            "sig should be removed; got:\n{result}"
        );
        assert!(
            result.contains("helper n"),
            "binding should still exist; got:\n{result}"
        );
    }

    // -----------------------------------------------------------------------
    // Scenario (d): insert new value binding (appends to outermost let block)
    // -----------------------------------------------------------------------
    #[test]
    fn test_insert_new_value_binding() {
        let source = indoc!(
            "module M exposing (..)\n\
             processItem item =\n\
             \x20   let\n\
             \x20       a =\n\
             \x20           1\n\
             \x20   in\n\
             \x20   a\n"
        );
        let summary = make_summary(source);
        let spec = BindingSpec {
            name: "b".to_string(),
            body: "2".to_string(),
            type_annotation: None,
            params: None,
            no_type: false,
            after: None,
            before: None,
            line: None,
        };
        let result = upsert_let_binding(source, &summary, "processItem", &spec).unwrap();
        assert!(
            result.contains("a ="),
            "a binding should exist; got:\n{result}"
        );
        assert!(
            result.contains("b ="),
            "b binding should be inserted; got:\n{result}"
        );
        // b should appear after a
        let pos_a = result.find("a =").unwrap();
        let pos_b = result.find("b =").unwrap();
        assert!(pos_b > pos_a, "b should appear after a; got:\n{result}");
    }

    // -----------------------------------------------------------------------
    // Scenario (e): insert new function binding with params + type
    // -----------------------------------------------------------------------
    #[test]
    fn test_insert_typed_function_binding() {
        let source = indoc!(
            "module M exposing (..)\n\
             update msg model =\n\
             \x20   let\n\
             \x20       existing =\n\
             \x20           1\n\
             \x20   in\n\
             \x20   existing\n"
        );
        let summary = make_summary(source);
        let spec = BindingSpec {
            name: "helper".to_string(),
            body: "n + 1".to_string(),
            type_annotation: Some("Int -> Int".to_string()),
            params: Some(vec!["n".to_string()]),
            no_type: false,
            after: None,
            before: None,
            line: None,
        };
        let result = upsert_let_binding(source, &summary, "update", &spec).unwrap();
        assert!(
            result.contains("helper : Int -> Int"),
            "type sig should be inserted; got:\n{result}"
        );
        assert!(
            result.contains("helper n ="),
            "function binding with param should exist; got:\n{result}"
        );
        assert!(
            result.contains("n + 1"),
            "body should be present; got:\n{result}"
        );
    }

    // -----------------------------------------------------------------------
    // Scenario (f): --after / --before on insert (positioning correct)
    // -----------------------------------------------------------------------
    #[test]
    fn test_insert_with_after_positioning() {
        let source = indoc!(
            "module M exposing (..)\n\
             update msg model =\n\
             \x20   let\n\
             \x20       initial =\n\
             \x20           1\n\
             \x20       current =\n\
             \x20           2\n\
             \x20       final_ =\n\
             \x20           3\n\
             \x20   in\n\
             \x20   initial\n"
        );
        let summary = make_summary(source);
        let spec = BindingSpec {
            name: "cached".to_string(),
            body: "expensive model".to_string(),
            type_annotation: None,
            params: None,
            no_type: false,
            after: Some("initial".to_string()),
            before: None,
            line: None,
        };
        let result = upsert_let_binding(source, &summary, "update", &spec).unwrap();
        let pos_initial = result.find("initial =").unwrap();
        let pos_cached = result.find("cached =").unwrap();
        let pos_current = result.find("current =").unwrap();
        assert!(
            pos_initial < pos_cached && pos_cached < pos_current,
            "cached should be between initial and current; got:\n{result}"
        );
    }

    #[test]
    fn test_insert_with_before_positioning() {
        let source = indoc!(
            "module M exposing (..)\n\
             update msg model =\n\
             \x20   let\n\
             \x20       a =\n\
             \x20           1\n\
             \x20       b =\n\
             \x20           2\n\
             \x20   in\n\
             \x20   a\n"
        );
        let summary = make_summary(source);
        let spec = BindingSpec {
            name: "mid".to_string(),
            body: "99".to_string(),
            type_annotation: None,
            params: None,
            no_type: false,
            after: None,
            before: Some("b".to_string()),
            line: None,
        };
        let result = upsert_let_binding(source, &summary, "update", &spec).unwrap();
        let pos_a = result.find("a =").unwrap();
        let pos_mid = result.find("mid =").unwrap();
        let pos_b = result.find("b =").unwrap();
        assert!(
            pos_a < pos_mid && pos_mid < pos_b,
            "mid should be between a and b; got:\n{result}"
        );
    }

    // -----------------------------------------------------------------------
    // Scenario (g): upsert with --after on existing binding moves it
    // -----------------------------------------------------------------------
    #[test]
    fn test_upsert_with_after_moves_binding() {
        let source = indoc!(
            "module M exposing (..)\n\
             update msg model =\n\
             \x20   let\n\
             \x20       a =\n\
             \x20           1\n\
             \x20       helper =\n\
             \x20           2\n\
             \x20       b =\n\
             \x20           3\n\
             \x20   in\n\
             \x20   a\n"
        );
        let summary = make_summary(source);
        let spec = BindingSpec {
            name: "helper".to_string(),
            body: "n + 2".to_string(),
            type_annotation: None,
            params: None,
            no_type: false,
            after: Some("b".to_string()),
            before: None,
            line: None,
        };
        let result = upsert_let_binding(source, &summary, "update", &spec).unwrap();
        let pos_a = result.find("a =").unwrap();
        let pos_b = result.find("b =").unwrap();
        let pos_helper = result.find("helper =").unwrap();
        assert!(
            pos_a < pos_b && pos_b < pos_helper,
            "order should be a, b, helper; got:\n{result}"
        );
        assert!(
            result.contains("n + 2"),
            "body should be updated; got:\n{result}"
        );
    }

    // -----------------------------------------------------------------------
    // Scenario (h): ambiguity error lists candidate lines AND scope hint
    // -----------------------------------------------------------------------
    #[test]
    fn test_ambiguity_error_lists_candidates() {
        let source = indoc!(
            "module M exposing (..)\n\
             outer x =\n\
             \x20   let\n\
             \x20       helperA =\n\
             \x20           let\n\
             \x20               h =\n\
             \x20                   1\n\
             \x20           in\n\
             \x20           h\n\
             \x20       helperB =\n\
             \x20           let\n\
             \x20               h =\n\
             \x20                   2\n\
             \x20           in\n\
             \x20           h\n\
             \x20   in\n\
             \x20   helperA + helperB\n"
        );
        let summary = make_summary(source);
        let spec = BindingSpec {
            name: "h".to_string(),
            body: "42".to_string(),
            type_annotation: None,
            params: None,
            no_type: false,
            after: None,
            before: None,
            line: None,
        };
        let err = upsert_let_binding(source, &summary, "outer", &spec)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("ambiguous"),
            "error should mention ambiguity; got: {err}"
        );
        assert!(
            err.contains("line"),
            "error should list candidate lines; got: {err}"
        );
        assert!(
            err.contains("retry with --line"),
            "error should suggest retry; got: {err}"
        );
        // Should include scope hints
        assert!(
            err.contains("inside helperA") || err.contains("inside helperB"),
            "error should include scope hints; got: {err}"
        );
    }

    // -----------------------------------------------------------------------
    // Scenario (i): --line resolves ambiguous target, only edits that site
    // -----------------------------------------------------------------------
    #[test]
    fn test_line_resolves_ambiguous_target() {
        let source = indoc!(
            "module M exposing (..)\n\
             outer x =\n\
             \x20   let\n\
             \x20       helperA =\n\
             \x20           let\n\
             \x20               h =\n\
             \x20                   1\n\
             \x20           in\n\
             \x20           h\n\
             \x20       helperB =\n\
             \x20           let\n\
             \x20               h =\n\
             \x20                   2\n\
             \x20           in\n\
             \x20           h\n\
             \x20   in\n\
             \x20   helperA + helperB\n"
        );
        let summary = make_summary(source);
        // Find the line of the first `h` binding.
        let tree = parse(source).unwrap();
        let root = tree.root_node();
        let decl_node = find_value_decl_node(root, source, "outer").unwrap();
        let sites = analysis::collect_let_sites(decl_node, source);
        let h_sites: Vec<&LetSite> = sites.iter().filter(|s| s.name == "h").collect();
        assert_eq!(h_sites.len(), 2, "should find 2 h bindings");
        let first_h_line = h_sites[0].line;

        let spec = BindingSpec {
            name: "h".to_string(),
            body: "42".to_string(),
            type_annotation: None,
            params: None,
            no_type: false,
            after: None,
            before: None,
            line: Some(first_h_line),
        };
        let result = upsert_let_binding(source, &summary, "outer", &spec).unwrap();
        // One h should be 42, the other should still be 1 or 2.
        assert!(
            result.contains("42"),
            "first h should be updated to 42; got:\n{result}"
        );
        assert!(
            result.contains("2"),
            "second h should be unchanged; got:\n{result}"
        );
    }

    // -----------------------------------------------------------------------
    // Scenario (j): rm let multi-target all-or-nothing: partial failure
    // -----------------------------------------------------------------------
    #[test]
    fn test_rm_batch_all_or_nothing() {
        let source = indoc!(
            "module M exposing (..)\n\
             update msg model =\n\
             \x20   let\n\
             \x20       helper =\n\
             \x20           1\n\
             \x20       cached =\n\
             \x20           2\n\
             \x20   in\n\
             \x20   helper + cached\n"
        );
        let summary = make_summary(source);
        let names = vec![
            "helper".to_string(),
            "nonexistent".to_string(),
            "cached".to_string(),
        ];
        let err = remove_let_bindings_batch(source, &summary, "update", &names)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("nonexistent"),
            "error should mention missing binding; got: {err}"
        );
        // Source should be unchanged (function returns Err, not Ok with partial).
    }

    // -----------------------------------------------------------------------
    // Scenario (k): rename let updates every reference in the enclosing decl
    // -----------------------------------------------------------------------
    #[test]
    fn test_rename_updates_references() {
        let source = indoc!(
            "module M exposing (..)\n\
             update msg model =\n\
             \x20   let\n\
             \x20       h =\n\
             \x20           expensive model\n\
             \x20   in\n\
             \x20   view h model.count h\n"
        );
        let summary = make_summary(source);
        let result = rename_let_binding(source, &summary, "update", "h", "helper", None).unwrap();
        assert!(
            result.contains("helper ="),
            "binding name should be renamed; got:\n{result}"
        );
        // Both references in `view h model.count h` should be renamed.
        let in_pos = result.find("in\n").unwrap();
        let after_in = &result[in_pos..];
        let count = after_in.matches("helper").count();
        assert!(
            count >= 2,
            "both references should be renamed; got {count} occurrences in:\n{after_in}"
        );
        assert!(
            !result.contains(" h ") && !result.contains(" h\n"),
            "old name should not appear as standalone identifier; got:\n{result}"
        );
    }

    // -----------------------------------------------------------------------
    // Scenario (l): rename let collision with in-scope name errors without writing
    // -----------------------------------------------------------------------
    #[test]
    fn test_rename_collision_errors() {
        let source = indoc!(
            "module M exposing (..)\n\
             update msg model =\n\
             \x20   let\n\
             \x20       helper =\n\
             \x20           1\n\
             \x20   in\n\
             \x20   helper + model.count\n"
        );
        let summary = make_summary(source);
        // `model` is a function param — should be in scope.
        let err = rename_let_binding(source, &summary, "update", "helper", "model", None)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("already in scope") || err.contains("in scope"),
            "error should mention scope collision; got: {err}"
        );
    }

    // -----------------------------------------------------------------------
    // Additional: rm single binding removes it and its sig
    // -----------------------------------------------------------------------
    #[test]
    fn test_rm_single_binding() {
        let source = indoc!(
            "module M exposing (..)\n\
             update msg model =\n\
             \x20   let\n\
             \x20       helper : Int\n\
             \x20       helper =\n\
             \x20           1\n\
             \x20       other =\n\
             \x20           2\n\
             \x20   in\n\
             \x20   other\n"
        );
        let summary = make_summary(source);
        let result = remove_let_binding(source, &summary, "update", "helper", None).unwrap();
        assert!(
            !result.contains("helper"),
            "helper should be removed; got:\n{result}"
        );
        assert!(
            result.contains("other ="),
            "other should remain; got:\n{result}"
        );
    }

    // -----------------------------------------------------------------------
    // Additional: rm batch removes multiple bindings atomically
    // -----------------------------------------------------------------------
    #[test]
    fn test_rm_batch_success() {
        // Use a fixture with one extra binding that we DON'T remove, and a body
        // that doesn't reference the removed names, so `contains` checks are clean.
        let source = indoc!(
            "module M exposing (..)\n\
             update msg model =\n\
             \x20   let\n\
             \x20       helper =\n\
             \x20           1\n\
             \x20       cached =\n\
             \x20           2\n\
             \x20       stale =\n\
             \x20           3\n\
             \x20       keeper =\n\
             \x20           4\n\
             \x20   in\n\
             \x20   keeper\n"
        );
        let summary = make_summary(source);
        let names = vec![
            "helper".to_string(),
            "cached".to_string(),
            "stale".to_string(),
        ];
        let result = remove_let_bindings_batch(source, &summary, "update", &names).unwrap();
        assert!(
            !result.contains("helper ="),
            "helper binding should be removed; got:\n{result}"
        );
        assert!(
            !result.contains("cached ="),
            "cached binding should be removed; got:\n{result}"
        );
        assert!(
            !result.contains("stale ="),
            "stale binding should be removed; got:\n{result}"
        );
        assert!(
            result.contains("keeper ="),
            "keeper should remain; got:\n{result}"
        );
    }
}
