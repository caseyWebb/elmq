use crate::imports::ImportContext;
use crate::parser;
use crate::project::Project;
use crate::writer;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use tree_sitter::Node;

// -- Result types --

#[derive(Debug, Clone, Serialize)]
pub struct VariantResult {
    pub dry_run: bool,
    pub type_file: String,
    pub type_name: String,
    pub variant_name: String,
    pub edits: Vec<CaseEdit>,
    pub skipped: Vec<CaseSkip>,
    /// Every project-wide reference to the removed constructor that `variant rm`
    /// did NOT rewrite — expression-position uses, refutable patterns in function
    /// or lambda arguments, let-binding patterns, etc. Populated only by
    /// `execute_rm_variant`; empty for `execute_add_variant`. Advisory, not a
    /// gate: `variant rm` still writes its files even when this is non-empty,
    /// and the agent is expected to fix these by hand before running `elm make`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references_not_rewritten: Vec<ConstructorSiteReport>,
}

/// Public JSON-serializable view of a constructor reference site. Used by both
/// `variant refs` output and the `references_not_rewritten` field of
/// `VariantResult`. `kind` is one of `case-branch`, `case-wildcard-covered`,
/// `function-arg-pattern`, `lambda-arg-pattern`, `let-binding-pattern`, or
/// `expression-position` — the same strings documented in
/// `openspec/changes/variant-refs/design.md` §7.
#[derive(Debug, Clone, Serialize)]
pub struct ConstructorSiteReport {
    pub file: String,
    pub module: String,
    pub declaration: String,
    pub line: usize,
    pub column: usize,
    pub kind: String,
    pub snippet: String,
}

/// Read-only result of `elmq variant refs` — every project-wide reference to a
/// given constructor, grouped by file, with a summary of clean vs. blocking
/// counts. Both grouped (`sites_by_file`) and flat (`sites`) views are emitted
/// so compact output can render per-file sections while JSON consumers can
/// iterate `sites` directly.
#[derive(Debug, Clone, Serialize)]
pub struct RefsResult {
    pub type_file: String,
    pub type_name: String,
    pub constructor: String,
    pub total_sites: usize,
    pub total_clean: usize,
    pub total_blocking: usize,
    pub sites: Vec<ConstructorSiteReport>,
    #[serde(skip_serializing)]
    pub sites_by_file: Vec<(String, Vec<ConstructorSiteReport>)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaseEdit {
    pub file: String,
    pub module: String,
    pub function: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaseSkip {
    pub file: String,
    pub module: String,
    pub function: String,
    pub line: usize,
    pub reason: String,
}

/// Result of `elmq variant cases` — a read-only report of every case expression in
/// the project that matches on the target type, with enough context (enclosing
/// function body, stable key) for a caller to compose `variant add --fill`.
#[derive(Debug, Clone, Serialize)]
pub struct CasesResult {
    #[serde(rename = "type")]
    pub type_name: String,
    pub type_file: String,
    pub sites: Vec<CasesSite>,
    pub skipped: Vec<CaseSkip>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CasesSite {
    pub file: String,
    pub module: String,
    pub function: String,
    pub key: String,
    pub line: usize,
    /// Full source text of the enclosing top-level declaration, including any
    /// preceding type annotation. Captured verbatim from the file at walk time.
    pub body: String,
}

// -- Constructor map --

/// Info about a single union variant within a type.
#[derive(Debug, Clone)]
struct VariantInfo {
    /// Constructor name (e.g. "Increment")
    name: String,
}

/// Info about a custom type declaration.
#[derive(Debug, Clone)]
struct TypeInfo {
    /// Module that defines this type (e.g. "Sample")
    module: String,
    /// Type name (e.g. "Msg")
    type_name: String,
    /// All constructors of this type
    variants: Vec<VariantInfo>,
}

/// Build a map from constructor name to type info for a single file.
fn extract_type_infos(tree: &tree_sitter::Tree, source: &str, module_name: &str) -> Vec<TypeInfo> {
    let root = tree.root_node();
    let mut cursor = root.walk();
    let mut result = Vec::new();

    for child in root.named_children(&mut cursor) {
        if child.kind() != "type_declaration" {
            continue;
        }
        let Some(name_node) = child.child_by_field_name("name") else {
            continue;
        };
        let Ok(type_name) = name_node.utf8_text(source.as_bytes()) else {
            continue;
        };

        let mut variants = Vec::new();
        let mut inner = child.walk();
        for desc in child.named_children(&mut inner) {
            if desc.kind() == "union_variant"
                && let Some(first) = desc.named_child(0)
                && let Ok(ctor_name) = first.utf8_text(source.as_bytes())
            {
                variants.push(VariantInfo {
                    name: ctor_name.to_string(),
                });
            }
        }

        result.push(TypeInfo {
            module: module_name.to_string(),
            type_name: type_name.to_string(),
            variants,
        });
    }

    result
}

/// Build a project-wide map: constructor_name -> TypeInfo
fn build_constructor_map(
    project: &Project,
    elm_files: &[PathBuf],
) -> Result<HashMap<String, TypeInfo>> {
    let mut map = HashMap::new();

    for file in elm_files {
        let source = std::fs::read_to_string(file)
            .with_context(|| format!("could not read {}", file.display()))?;
        let tree = match parser::parse(&source) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let module_name = match project.module_name(file) {
            Ok(m) => m,
            Err(_) => continue,
        };

        for type_info in extract_type_infos(&tree, &source, &module_name) {
            for variant in &type_info.variants {
                map.insert(variant.name.clone(), type_info.clone());
            }
        }
    }

    Ok(map)
}

// -- Variant definition parsing --

/// Parse a variant definition string (e.g. "SetName String") into a name and arg count.
/// Uses tree-sitter to handle complex type expressions correctly.
fn parse_variant_definition(definition: &str) -> Result<(String, usize)> {
    let synthetic = format!("module T exposing (..)\n\ntype T\n    = X__\n    | {definition}\n");
    let tree = parser::parse(&synthetic).context("could not parse variant definition")?;
    let root = tree.root_node();

    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() != "type_declaration" {
            continue;
        }
        let mut inner = child.walk();
        let mut found_placeholder = false;
        for desc in child.named_children(&mut inner) {
            if desc.kind() == "union_variant"
                && let Some(first) = desc.named_child(0)
                && let Ok(name) = first.utf8_text(synthetic.as_bytes())
            {
                if name == "X__" {
                    found_placeholder = true;
                    continue;
                }
                if found_placeholder {
                    let arg_count = desc.named_child_count() - 1;
                    return Ok((name.to_string(), arg_count));
                }
            }
        }
    }

    bail!("could not parse variant definition: {definition}")
}

// -- Case expression analysis --

/// Info about a case expression that matches the target type. Used only by
/// `execute_rm_variant` — the add flow now routes through `CaseSite`, which carries
/// a superset of the information.
struct CaseExprInfo {
    /// Byte range of the entire case_of_expr node
    byte_range: std::ops::Range<usize>,
    /// Line number (1-based) of the case_of_expr
    line: usize,
    /// Enclosing function name
    function: String,
}

#[derive(Debug, Clone)]
struct TuplePatternInfo {
    /// Which element in the tuple contains the constructor (0-based)
    variant_index: usize,
    /// Total number of elements in the tuple
    total_elements: usize,
}

/// Information about a branch that matches a specific constructor.
struct MatchingBranch {
    /// Byte range of the entire case_of_branch
    byte_range: std::ops::Range<usize>,
}

/// A case expression matching the target type, captured with all context needed by
/// both `execute_cases` (for read-only reporting) and `execute_add_variant` (for
/// branch insertion). Produced by `collect_case_sites`.
#[derive(Debug, Clone)]
struct CaseSite {
    /// Canonical path to the file containing the case expression.
    file: PathBuf,
    /// Module name (e.g. "Page.Home") of the containing file.
    module: String,
    /// Relative display path (e.g. "src/Page/Home.elm").
    display: String,
    /// Name of the top-level function enclosing the case expression.
    function: String,
    /// 1-based line number of the case_of_expr.
    line: usize,
    /// Byte range of the case_of_expr in the source used for the walk.
    byte_range: std::ops::Range<usize>,
    /// Whether any branch of the case has a wildcard / catch-all pattern.
    has_wildcard: bool,
    /// Leading-space indent that new branches should use.
    branch_indent: usize,
    /// Leading-space indent that new branch bodies should use.
    body_indent: usize,
    /// Tuple-position info when the case matches on a tuple pattern and the target
    /// type appears in one position.
    tuple_position: Option<TuplePatternInfo>,
    /// Byte range of the enclosing top-level declaration, including any preceding
    /// type annotation. Used by `execute_cases` to slice the enclosing function body
    /// as context for the caller.
    declaration_byte_range: std::ops::Range<usize>,
}

/// Shared context for type resolution during case expression analysis.
struct TypeContext<'a> {
    target_module: &'a str,
    target_type: &'a str,
    constructor_map: &'a HashMap<String, TypeInfo>,
    current_module: &'a str,
}

/// Find the enclosing function name for a node by walking up to value_declaration.
fn find_enclosing_function<'a>(node: &Node<'a>, source: &str) -> String {
    let mut current = *node;
    loop {
        if current.kind() == "value_declaration" {
            // The function name is in the functionDeclarationLeft child.
            if let Some(fdl) = current.child_by_field_name("functionDeclarationLeft")
                && let Some(name_node) = fdl.named_child(0)
                && let Ok(name) = name_node.utf8_text(source.as_bytes())
            {
                return name.to_string();
            }
            // Fallback: try pattern field (for let bindings).
            if let Some(pat) = current.child_by_field_name("pattern")
                && let Ok(text) = pat.utf8_text(source.as_bytes())
            {
                return text.to_string();
            }
            return "<unknown>".to_string();
        }
        if let Some(parent) = current.parent() {
            current = parent;
        } else {
            return "<top-level>".to_string();
        }
    }
}

/// Check if a node (pattern) contains a union_pattern with a constructor that resolves
/// to the target module+type. Returns the constructor name if found.
fn find_constructor_in_pattern(
    node: &Node,
    source: &str,
    ctx: &TypeContext,
    import_ctx: &ImportContext,
) -> Option<String> {
    match node.kind() {
        "union_pattern" => {
            // The first child is the constructor reference (upper_case_qid).
            if let Some(ctor_node) = node.named_child(0)
                && let Ok(ctor_text) = ctor_node.utf8_text(source.as_bytes())
            {
                // Resolve the constructor to a (module, type).
                if let Some(resolved) = resolve_constructor(
                    ctor_text,
                    ctx.constructor_map,
                    import_ctx,
                    ctx.current_module,
                ) && resolved.0 == ctx.target_module
                    && resolved.1 == ctx.target_type
                {
                    // Extract bare constructor name (strip any qualifier).
                    let bare = ctor_text.rsplit('.').next().unwrap_or(ctor_text);
                    return Some(bare.to_string());
                }
            }
            // Nested constructor: the outer `union_pattern` did not match the
            // target type, but its argument patterns might contain a nested
            // `union_pattern` that does (e.g. `Just Increment` when removing
            // `Increment`). Recurse into children so the iterative branch
            // removal loop in `execute_rm_variant` sees nested references.
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if let Some(found) = find_constructor_in_pattern(&child, source, ctx, import_ctx) {
                    return Some(found);
                }
            }
            None
        }
        "upper_case_qid" => {
            // Bare constructor reference inside a pattern — e.g. the
            // `Increment` inside `Just Increment`, which tree-sitter-elm
            // wraps in `nullary_constructor_argument_pattern` → `upper_case_qid`
            // (no outer `union_pattern` because the constructor takes no args).
            if let Ok(ctor_text) = node.utf8_text(source.as_bytes())
                && let Some(resolved) = resolve_constructor(
                    ctor_text,
                    ctx.constructor_map,
                    import_ctx,
                    ctx.current_module,
                )
                && resolved.0 == ctx.target_module
                && resolved.1 == ctx.target_type
            {
                let bare = ctor_text.rsplit('.').next().unwrap_or(ctor_text);
                return Some(bare.to_string());
            }
            None
        }
        "pattern"
        | "tuple_pattern"
        | "cons_pattern"
        | "list_pattern"
        | "nullary_constructor_argument_pattern" => {
            // Recurse into sub-patterns. `nullary_constructor_argument_pattern`
            // is how tree-sitter-elm wraps no-arg constructor arguments like
            // the `Increment` in `Just Increment`.
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if let Some(found) = find_constructor_in_pattern(&child, source, ctx, import_ctx) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

/// Resolve a constructor reference (possibly qualified) to (module, type_name).
/// `current_module` is the module name of the file being analyzed (bare constructors
/// from the same module are always accessible).
fn resolve_constructor(
    ctor_text: &str,
    constructor_map: &HashMap<String, TypeInfo>,
    import_ctx: &ImportContext,
    current_module: &str,
) -> Option<(String, String)> {
    if let Some(dot_pos) = ctor_text.rfind('.') {
        // Qualified: e.g. "Msg.Increment" or "Sample.Increment"
        let prefix = &ctor_text[..dot_pos];
        let bare_name = &ctor_text[dot_pos + 1..];
        // Resolve the prefix to a module name.
        if let Some(module) = import_ctx.resolve_prefix(prefix) {
            // Look up the bare constructor in the map.
            if let Some(type_info) = constructor_map.get(bare_name)
                && type_info.module == module
            {
                return Some((module.to_string(), type_info.type_name.clone()));
            }
        }
    } else {
        // Bare constructor: look up in map, then verify it's accessible.
        if let Some(type_info) = constructor_map.get(ctor_text) {
            // Same module — always accessible.
            if type_info.module == current_module {
                return Some((type_info.module.clone(), type_info.type_name.clone()));
            }
            // Check if the type's module imports it with exposed constructors.
            if let Some(imp) = import_ctx.get(&type_info.module) {
                if imp.has_exposing_all {
                    return Some((type_info.module.clone(), type_info.type_name.clone()));
                }
                for item in &imp.exposed {
                    if let crate::imports::ExposedItem::TypeWithConstructors(n) = item
                        && n == &type_info.type_name
                    {
                        return Some((type_info.module.clone(), type_info.type_name.clone()));
                    }
                }
            }
        }
    }
    None
}

/// Check if a case_of_expr has a wildcard/catch-all branch.
fn has_wildcard_branch(case_node: &Node, source: &str) -> bool {
    let mut cursor = case_node.walk();
    for child in case_node.named_children(&mut cursor) {
        if child.kind() != "case_of_branch" {
            continue;
        }
        if let Some(pattern) = child.child_by_field_name("pattern")
            && pattern_is_wildcard(&pattern, source)
        {
            return true;
        }
    }
    false
}

/// Check if a pattern is a wildcard (catches everything: `_` or a bare variable).
fn pattern_is_wildcard(node: &Node, _source: &str) -> bool {
    match node.kind() {
        "anything_pattern" => true,
        "lower_pattern" => true,
        "pattern" => {
            // A pattern node wraps another pattern. Check the inner child.
            if let Some(child_node) = node.child_by_field_name("child") {
                return pattern_is_wildcard(&child_node, _source);
            }
            // If it only has patternAs, it might be `_ as x` — check.
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if child.kind() == "anything_pattern" || child.kind() == "lower_pattern" {
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}

/// Detect if the case expression uses tuple patterns and where the constructor appears.
fn detect_tuple_position(
    case_node: &Node,
    source: &str,
    ctx: &TypeContext,
    import_ctx: &ImportContext,
) -> Option<TuplePatternInfo> {
    let mut cursor = case_node.walk();
    for child in case_node.named_children(&mut cursor) {
        if child.kind() != "case_of_branch" {
            continue;
        }
        let Some(pattern) = child.child_by_field_name("pattern") else {
            continue;
        };

        // Look for a tuple_pattern.
        if let Some(tuple) = find_tuple_pattern(&pattern) {
            let total = tuple.named_child_count();
            let mut inner_cursor = tuple.walk();
            for (i, element) in tuple.named_children(&mut inner_cursor).enumerate() {
                if find_constructor_in_pattern(&element, source, ctx, import_ctx).is_some() {
                    return Some(TuplePatternInfo {
                        variant_index: i,
                        total_elements: total,
                    });
                }
            }
        }
    }
    None
}

/// Find a tuple_pattern within a pattern node (may be wrapped in pattern node).
fn find_tuple_pattern<'a>(node: &Node<'a>) -> Option<Node<'a>> {
    if node.kind() == "tuple_pattern" {
        return Some(*node);
    }
    if node.kind() == "pattern"
        && let Some(child) = node.child_by_field_name("child")
        && child.kind() == "tuple_pattern"
    {
        return Some(child);
    }
    None
}

/// Get the branch indentation from existing branches in a case expression.
fn get_branch_indentation(case_node: &Node, source: &str) -> (usize, usize) {
    let mut cursor = case_node.walk();
    for child in case_node.named_children(&mut cursor) {
        if child.kind() != "case_of_branch" {
            continue;
        }
        let line = child.start_position().row;
        let line_text = source.lines().nth(line).unwrap_or("");
        let branch_indent = line_text.len() - line_text.trim_start().len();

        // Body indent: look at the expression child.
        if let Some(expr) = child.child_by_field_name("expr") {
            let expr_line = expr.start_position().row;
            let expr_text = source.lines().nth(expr_line).unwrap_or("");
            let body_indent = expr_text.len() - expr_text.trim_start().len();
            return (branch_indent, body_indent);
        }

        return (branch_indent, branch_indent + 4);
    }
    (8, 12)
}

/// Find all case expressions in a file that match the target type.
fn find_matching_case_exprs(
    root: &Node,
    source: &str,
    ctx: &TypeContext,
    import_ctx: &ImportContext,
) -> Vec<CaseExprInfo> {
    let mut results = Vec::new();
    collect_case_exprs(root, source, ctx, import_ctx, &mut results);
    results
}

/// Walk from `node` up to the nearest **top-level** declaration (a direct child of
/// the root file node). Returns `None` if the node is not inside a top-level
/// `value_declaration`. Elm type annotations live as siblings of their paired
/// value_declaration at the root level, so "top-level" is what lets us also locate
/// the preceding type annotation.
fn find_top_level_declaration<'a>(node: &Node<'a>, root: &Node<'a>) -> Option<Node<'a>> {
    let mut current = *node;
    while let Some(parent) = current.parent() {
        if parent.id() == root.id() {
            if current.kind() == "value_declaration" {
                return Some(current);
            }
            return None;
        }
        current = parent;
    }
    None
}

/// Return the byte range covering the top-level declaration **plus** its preceding
/// type annotation (if any), so a slice of the source over this range yields the
/// full `name : type` + `name args = body` block that a user would expect to read.
/// Also returns the function name as declared in the value_declaration.
fn declaration_range_with_annotation<'a>(
    decl: &Node<'a>,
    root: &Node<'a>,
    source: &str,
) -> (String, std::ops::Range<usize>) {
    // Extract the function name from the declaration's functionDeclarationLeft.
    let name = decl
        .child_by_field_name("functionDeclarationLeft")
        .and_then(|fdl| fdl.named_child(0))
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "<unknown>".to_string());

    let mut start_byte = decl.start_byte();
    let end_byte = decl.end_byte();

    // Scan root's named children for the sibling immediately preceding `decl`. If it
    // is a type_annotation naming the same function, extend the start to cover it.
    let mut cursor = root.walk();
    let mut prev: Option<Node> = None;
    for child in root.named_children(&mut cursor) {
        if child.id() == decl.id() {
            if let Some(p) = prev
                && p.kind() == "type_annotation"
                && let Some(name_node) = p.child_by_field_name("name")
                && let Ok(ann_name) = name_node.utf8_text(source.as_bytes())
                && ann_name == name
            {
                start_byte = p.start_byte();
            }
            break;
        }
        prev = Some(child);
    }

    (name, start_byte..end_byte)
}

/// Walk the project and collect every case expression matching the target type.
///
/// Shared by `execute_cases` (read-only) and `execute_add_variant` (which then runs
/// its own insertion loop over the returned sites). Accepts optional `source_overrides`
/// so `execute_add_variant` can swap in the type file's post-append source without
/// writing to disk first.
fn collect_case_sites(
    project: &Project,
    elm_files: &[PathBuf],
    target_module: &str,
    target_type: &str,
    constructor_map: &HashMap<String, TypeInfo>,
    source_overrides: &HashMap<PathBuf, String>,
) -> Result<Vec<CaseSite>> {
    let mut sites = Vec::new();

    for elm_file in elm_files {
        let file_source = if let Some(over) = source_overrides.get(elm_file) {
            over.clone()
        } else {
            match std::fs::read_to_string(elm_file) {
                Ok(s) => s,
                Err(_) => continue,
            }
        };

        let file_tree = match parser::parse(&file_source) {
            Ok(t) => t,
            Err(_) => continue,
        };

        let root = file_tree.root_node();
        let import_ctx = ImportContext::from_tree(&root, &file_source);
        let module_name = match project.module_name(elm_file) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let display = relative_display(elm_file, &project.root);
        let type_ctx = TypeContext {
            target_module,
            target_type,
            constructor_map,
            current_module: &module_name,
        };

        // Walk the tree and collect sites inline — we have each Node available, so
        // we can resolve the enclosing top-level declaration without a second scan.
        collect_case_sites_in_tree(
            &root,
            &file_source,
            &type_ctx,
            &import_ctx,
            elm_file,
            &module_name,
            &display,
            &mut sites,
        );
    }

    Ok(sites)
}

/// Recursive walker used by `collect_case_sites`: for each matching case_of_expr,
/// compute its enclosing top-level declaration (name + byte range with annotation)
/// and push a `CaseSite` onto `out`. Operates in a single tree pass per file.
#[allow(clippy::too_many_arguments)]
fn collect_case_sites_in_tree(
    node: &Node,
    source: &str,
    ctx: &TypeContext,
    import_ctx: &ImportContext,
    file: &Path,
    module: &str,
    display: &str,
    out: &mut Vec<CaseSite>,
) {
    if node.kind() == "case_of_expr" {
        // Re-check that this case matches on the target type (same predicate as
        // `collect_case_exprs` uses). Avoid pulling in CaseExprInfo just to reuse it.
        let mut cursor = node.walk();
        let mut matches = false;
        for child in node.named_children(&mut cursor) {
            if child.kind() != "case_of_branch" {
                continue;
            }
            if let Some(pattern) = child.child_by_field_name("pattern")
                && find_constructor_in_pattern(&pattern, source, ctx, import_ctx).is_some()
            {
                matches = true;
                break;
            }
        }

        if matches {
            let has_wildcard = has_wildcard_branch(node, source);
            let (branch_indent, body_indent) = get_branch_indentation(node, source);
            let tuple_position = detect_tuple_position(node, source, ctx, import_ctx);

            // Walk up to the top-level declaration for a stable function name and
            // a body range that covers `sig + impl`. If the case expression isn't
            // inside a top-level declaration (rare: module-level weirdness), fall
            // back to the innermost-declaration name and an empty body range.
            let root = top_most_ancestor(node);
            let (function_name, declaration_byte_range) =
                match find_top_level_declaration(node, &root) {
                    Some(decl) => declaration_range_with_annotation(&decl, &root, source),
                    None => (find_enclosing_function(node, source), 0..0),
                };

            out.push(CaseSite {
                file: file.to_path_buf(),
                module: module.to_string(),
                display: display.to_string(),
                function: function_name,
                line: node.start_position().row + 1,
                byte_range: node.byte_range(),
                has_wildcard,
                branch_indent,
                body_indent,
                tuple_position,
                declaration_byte_range,
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_case_sites_in_tree(&child, source, ctx, import_ctx, file, module, display, out);
    }
}

// -- Constructor-site walker (variant refs + rm advisory) --

/// Internal classification of a single `upper_case_qid` that resolves to the
/// target constructor. Unit variants — today's rm flow runs its own byte-range
/// discovery via `find_matching_branch`, so the classifier only needs to tell
/// the caller which category the site falls into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SiteKind {
    CaseBranch,
    CaseWildcardCovered,
    FunctionArgPattern,
    LambdaArgPattern,
    LetBindingPattern,
    ExpressionPosition,
}

impl SiteKind {
    fn kind_str(&self) -> &'static str {
        match self {
            SiteKind::CaseBranch => "case-branch",
            SiteKind::CaseWildcardCovered => "case-wildcard-covered",
            SiteKind::FunctionArgPattern => "function-arg-pattern",
            SiteKind::LambdaArgPattern => "lambda-arg-pattern",
            SiteKind::LetBindingPattern => "let-binding-pattern",
            SiteKind::ExpressionPosition => "expression-position",
        }
    }

    fn is_clean_removal(&self) -> bool {
        matches!(self, SiteKind::CaseBranch | SiteKind::CaseWildcardCovered)
    }
}

/// Full internal record of a constructor reference site. Rendered down to
/// `ConstructorSiteReport` for public output.
#[derive(Debug, Clone)]
struct ConstructorSite {
    display: String,
    module: String,
    declaration: String,
    line: usize,
    column: usize,
    snippet: String,
    classification: SiteKind,
}

impl ConstructorSite {
    fn into_report(self) -> ConstructorSiteReport {
        ConstructorSiteReport {
            file: self.display,
            module: self.module,
            declaration: self.declaration,
            line: self.line,
            column: self.column,
            kind: self.classification.kind_str().to_string(),
            snippet: self.snippet,
        }
    }
}

/// Classifier context carried down the tree as the walker descends. Transitions
/// at field boundaries (pattern vs. expression body) reset or establish the
/// enclosing pattern kind. `Unknown` collapses to `ExpressionPosition` at the
/// leaf; `TypeDeclaration` causes qids to be skipped entirely (so the target
/// constructor's own definition is not reported as a reference to itself).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClassifyCtx {
    Unknown,
    CaseBranchPattern,
    FunctionArgPattern,
    LambdaArgPattern,
    LetBindingPattern,
    TypeDeclaration,
}

/// Compute the classifier context for a child node given its parent kind and
/// the field name it occupies. Pattern-hosting fields (`case_of_branch.pattern`,
/// `function_declaration_left.pattern`, `anonymous_function_expr.param`,
/// `value_declaration.pattern`) set the corresponding pattern ctx; expression
/// body fields reset to `Unknown`; other transitions propagate the parent ctx
/// unchanged so we stay inside whatever pattern context was previously set.
fn descend_ctx(parent_kind: &str, field: Option<&str>, parent_ctx: ClassifyCtx) -> ClassifyCtx {
    if parent_ctx == ClassifyCtx::TypeDeclaration {
        return ClassifyCtx::TypeDeclaration;
    }
    match (parent_kind, field) {
        ("case_of_branch", Some("pattern")) => ClassifyCtx::CaseBranchPattern,
        ("case_of_branch", Some("expr")) => ClassifyCtx::Unknown,
        ("case_of_expr", Some("expr")) => ClassifyCtx::Unknown,
        ("function_declaration_left", Some("pattern")) => ClassifyCtx::FunctionArgPattern,
        ("anonymous_function_expr", Some("param")) => ClassifyCtx::LambdaArgPattern,
        ("anonymous_function_expr", Some("expr")) => ClassifyCtx::Unknown,
        ("value_declaration", Some("pattern")) => ClassifyCtx::LetBindingPattern,
        ("value_declaration", Some("body")) => ClassifyCtx::Unknown,
        ("let_in_expr", Some("body")) => ClassifyCtx::Unknown,
        _ => parent_ctx,
    }
}

/// Return the top-level `value_declaration` enclosing `node`, or `None` if the
/// node is not inside any top-level declaration.
fn enclosing_top_level_decl<'a>(node: &Node<'a>) -> Option<Node<'a>> {
    let mut cur = *node;
    while let Some(p) = cur.parent() {
        if p.parent().is_none() && cur.kind() == "value_declaration" {
            return Some(cur);
        }
        cur = p;
    }
    None
}

/// Extract the 1-based line containing `byte_offset` as a trimmed snippet,
/// capped at 200 characters so advisory output stays compact.
fn line_snippet(source: &str, byte_offset: usize) -> String {
    let start = source[..byte_offset]
        .rfind('\n')
        .map(|p| p + 1)
        .unwrap_or(0);
    let end = source[byte_offset..]
        .find('\n')
        .map(|p| byte_offset + p)
        .unwrap_or(source.len());
    let raw = source[start..end].trim();
    if raw.len() > 200 {
        format!("{}…", &raw[..200])
    } else {
        raw.to_string()
    }
}

/// Walk every `.elm` file in the project and classify every `upper_case_qid`
/// that resolves to the target constructor. Also emits `CaseWildcardCovered`
/// sites via a second targeted pass over `case_of_expr` nodes on the target
/// type. The returned list is ordered by (file, byte offset); rendering groups
/// by file downstream.
fn collect_constructor_sites(
    project: &Project,
    elm_files: &[PathBuf],
    target_module: &str,
    target_type: &str,
    target_constructor: &str,
    constructor_map: &HashMap<String, TypeInfo>,
) -> Result<Vec<ConstructorSite>> {
    let mut sites: Vec<ConstructorSite> = Vec::new();

    for elm_file in elm_files {
        let file_source = match std::fs::read_to_string(elm_file) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let tree = match parser::parse(&file_source) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let root = tree.root_node();
        let import_ctx = ImportContext::from_tree(&root, &file_source);
        let module_name = match project.module_name(elm_file) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let display = relative_display(elm_file, &project.root);
        let type_ctx = TypeContext {
            target_module,
            target_type,
            constructor_map,
            current_module: &module_name,
        };

        // Pass 1: walk every upper_case_qid, classify by ancestor context.
        classify_qids(
            &root,
            &file_source,
            ClassifyCtx::Unknown,
            &type_ctx,
            &import_ctx,
            target_constructor,
            &display,
            &module_name,
            &mut sites,
        );

        // Pass 2: for each case_of_expr that matches the target type but has no
        // explicit branch for the target constructor AND has a wildcard branch,
        // emit a `CaseWildcardCovered` site so `variant rm` can surface it in
        // the skip list and `variant refs` can show it under exploration.
        collect_wildcard_covered(
            &root,
            &file_source,
            &type_ctx,
            &import_ctx,
            target_constructor,
            &display,
            &module_name,
            &mut sites,
        );
    }

    // Stable ordering: group by file, then by byte offset. `display` is the
    // relative path, which makes the grouping intuitive in compact output.
    sites.sort_by(|a, b| {
        a.display
            .cmp(&b.display)
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.column.cmp(&b.column))
    });
    Ok(sites)
}

/// Pass 1 of the constructor-site walker: descend the tree, tracking the
/// classifier context at each field boundary. Whenever we hit an
/// `upper_case_qid` that resolves to the target constructor (and we are not
/// inside the target type's own declaration), emit a site classified by the
/// current context.
#[allow(clippy::too_many_arguments)]
fn classify_qids(
    node: &Node,
    source: &str,
    ctx: ClassifyCtx,
    type_ctx: &TypeContext,
    import_ctx: &ImportContext,
    target_constructor: &str,
    display: &str,
    module: &str,
    out: &mut Vec<ConstructorSite>,
) {
    // Skip the constructor's own type declaration (and any other type
    // declarations in the file — constructor names in a `type` definition are
    // definitions, not references).
    if node.kind() == "type_declaration" {
        return;
    }

    if node.kind() == "upper_case_qid" && ctx != ClassifyCtx::TypeDeclaration {
        if let Ok(ctor_text) = node.utf8_text(source.as_bytes()) {
            let bare = ctor_text.rsplit('.').next().unwrap_or(ctor_text);
            if bare == target_constructor
                && let Some(resolved) = resolve_constructor(
                    ctor_text,
                    type_ctx.constructor_map,
                    import_ctx,
                    type_ctx.current_module,
                )
                && resolved.0 == type_ctx.target_module
                && resolved.1 == type_ctx.target_type
            {
                let classification = classify_from_ctx(node, ctx);
                let start = node.start_position();
                let declaration = enclosing_top_level_decl(node)
                    .map(|d| {
                        d.child_by_field_name("functionDeclarationLeft")
                            .and_then(|fdl| fdl.named_child(0))
                            .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "<unknown>".to_string())
                    })
                    .unwrap_or_else(|| "<top-level>".to_string());
                out.push(ConstructorSite {
                    display: display.to_string(),
                    module: module.to_string(),
                    declaration,
                    line: start.row + 1,
                    column: start.column + 1,
                    snippet: line_snippet(source, node.start_byte()),
                    classification,
                });
            }
        }
        // qids have no named children we care about — don't recurse into them
        return;
    }

    // Walk children using a cursor so we can observe each child's field name
    // and propagate the context accordingly.
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.is_named() {
                let child_ctx = descend_ctx(node.kind(), cursor.field_name(), ctx);
                classify_qids(
                    &child,
                    source,
                    child_ctx,
                    type_ctx,
                    import_ctx,
                    target_constructor,
                    display,
                    module,
                    out,
                );
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Resolve a classifier context at a leaf qid into a concrete `SiteKind`.
fn classify_from_ctx(_qid: &Node, ctx: ClassifyCtx) -> SiteKind {
    match ctx {
        ClassifyCtx::CaseBranchPattern => SiteKind::CaseBranch,
        ClassifyCtx::FunctionArgPattern => SiteKind::FunctionArgPattern,
        ClassifyCtx::LambdaArgPattern => SiteKind::LambdaArgPattern,
        ClassifyCtx::LetBindingPattern => SiteKind::LetBindingPattern,
        ClassifyCtx::Unknown => SiteKind::ExpressionPosition,
        ClassifyCtx::TypeDeclaration => SiteKind::ExpressionPosition, // unreachable
    }
}

/// Pass 2 of the constructor-site walker: walk every `case_of_expr` whose
/// scrutinee type resolves to the target type, and if the target constructor
/// has no explicit branch but the case has a wildcard, emit a
/// `CaseWildcardCovered` site. This is the equivalent of today's "wildcard
/// branch handled {constructor_name}" skip message, lifted into the structured
/// site model.
#[allow(clippy::too_many_arguments)]
fn collect_wildcard_covered(
    node: &Node,
    source: &str,
    type_ctx: &TypeContext,
    import_ctx: &ImportContext,
    target_constructor: &str,
    display: &str,
    module: &str,
    out: &mut Vec<ConstructorSite>,
) {
    if node.kind() == "type_declaration" {
        return;
    }

    if node.kind() == "case_of_expr" {
        let mut branches_on_type = false;
        let mut has_target_branch = false;
        let mut has_wildcard = false;
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() != "case_of_branch" {
                continue;
            }
            let Some(pattern) = child.child_by_field_name("pattern") else {
                continue;
            };
            if pattern_is_wildcard(&pattern, source) {
                has_wildcard = true;
                continue;
            }
            if let Some(found) = find_constructor_in_pattern(&pattern, source, type_ctx, import_ctx)
            {
                branches_on_type = true;
                if found == target_constructor {
                    has_target_branch = true;
                }
            }
        }

        if branches_on_type && has_wildcard && !has_target_branch {
            let start = node.start_position();
            let declaration = enclosing_top_level_decl(node)
                .map(|d| {
                    d.child_by_field_name("functionDeclarationLeft")
                        .and_then(|fdl| fdl.named_child(0))
                        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "<unknown>".to_string())
                })
                .unwrap_or_else(|| "<top-level>".to_string());
            out.push(ConstructorSite {
                display: display.to_string(),
                module: module.to_string(),
                declaration,
                line: start.row + 1,
                column: start.column + 1,
                snippet: line_snippet(source, node.start_byte()),
                classification: SiteKind::CaseWildcardCovered,
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_wildcard_covered(
            &child,
            source,
            type_ctx,
            import_ctx,
            target_constructor,
            display,
            module,
            out,
        );
    }
}

/// Walk to the outermost ancestor of a node (its tree's root). Needed because the
/// recursive walker in `collect_case_sites_in_tree` receives nodes without a direct
/// root reference and must locate root on demand for top-level declaration lookup.
fn top_most_ancestor<'a>(node: &Node<'a>) -> Node<'a> {
    let mut current = *node;
    while let Some(p) = current.parent() {
        current = p;
    }
    current
}

/// Compute the shortest unambiguous key for every site in `sites`, per the progressive
/// qualification scheme in `openspec/changes/variant-fill/design.md` §3/§8:
///
/// 1. `<function>` when there is exactly one site for this function name project-wide.
/// 2. `<function>#<N>` when multiple sites share the same function name, all in one file.
/// 3. `<file>:<function>` when the same function name appears in different files.
/// 4. `<file>:<function>#<N>` when (3) and one of those files has multiple cases in that function.
///
/// Ordinals are 1-based and source-ordered by `byte_range.start`.
fn compute_site_keys(sites: &[CaseSite]) -> Vec<String> {
    let mut keys = vec![String::new(); sites.len()];

    // Group indices by bare function name.
    let mut by_function: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, site) in sites.iter().enumerate() {
        by_function
            .entry(site.function.clone())
            .or_default()
            .push(i);
    }

    for (fname, indices) in by_function {
        if indices.len() == 1 {
            keys[indices[0]] = fname;
            continue;
        }

        // Subgroup by file display path.
        let mut by_file: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for &i in &indices {
            by_file.entry(sites[i].display.clone()).or_default().push(i);
        }

        if by_file.len() == 1 {
            // All sites in one file: disambiguate with #N.
            let mut sorted = indices;
            sorted.sort_by_key(|&i| sites[i].byte_range.start);
            for (ord, i) in sorted.into_iter().enumerate() {
                keys[i] = format!("{}#{}", fname, ord + 1);
            }
        } else {
            // Sites span multiple files: prefix with file:
            for (file, file_indices) in by_file {
                if file_indices.len() == 1 {
                    keys[file_indices[0]] = format!("{}:{}", file, fname);
                } else {
                    let mut sorted = file_indices;
                    sorted.sort_by_key(|&i| sites[i].byte_range.start);
                    for (ord, i) in sorted.into_iter().enumerate() {
                        keys[i] = format!("{}:{}#{}", file, fname, ord + 1);
                    }
                }
            }
        }
    }

    keys
}

fn collect_case_exprs(
    node: &Node,
    source: &str,
    ctx: &TypeContext,
    import_ctx: &ImportContext,
    results: &mut Vec<CaseExprInfo>,
) {
    if node.kind() == "case_of_expr" {
        // Check if any branch pattern contains a constructor of the target type.
        let mut cursor = node.walk();
        let mut matches = false;
        for child in node.named_children(&mut cursor) {
            if child.kind() != "case_of_branch" {
                continue;
            }
            if let Some(pattern) = child.child_by_field_name("pattern")
                && find_constructor_in_pattern(&pattern, source, ctx, import_ctx).is_some()
            {
                matches = true;
                break;
            }
        }

        if matches {
            let function = find_enclosing_function(node, source);

            results.push(CaseExprInfo {
                byte_range: node.byte_range(),
                line: node.start_position().row + 1,
                function,
            });
        }
    }

    // Recurse into children.
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_case_exprs(&child, source, ctx, import_ctx, results);
    }
}

/// Find the branch matching a specific constructor in a case expression.
fn find_matching_branch(
    case_node: &Node,
    source: &str,
    constructor_name: &str,
    ctx: &TypeContext,
    import_ctx: &ImportContext,
) -> Option<MatchingBranch> {
    let mut cursor = case_node.walk();
    for child in case_node.named_children(&mut cursor) {
        if child.kind() != "case_of_branch" {
            continue;
        }
        if let Some(pattern) = child.child_by_field_name("pattern")
            && let Some(found) = find_constructor_in_pattern(&pattern, source, ctx, import_ctx)
            && found == constructor_name
        {
            return Some(MatchingBranch {
                byte_range: child.byte_range(),
            });
        }
    }
    None
}

// -- Branch generation --

/// Generate a new case branch for the added variant.
fn generate_branch(
    constructor_name: &str,
    arg_count: usize,
    branch_indent: usize,
    body_indent: usize,
    tuple_info: Option<&TuplePatternInfo>,
) -> String {
    let branch_pad = " ".repeat(branch_indent);
    let body_pad = " ".repeat(body_indent);

    let args = if arg_count > 0 {
        format!(" {}", vec!["_"; arg_count].join(" "))
    } else {
        String::new()
    };

    let pattern = if let Some(tuple) = tuple_info {
        let mut parts: Vec<String> = (0..tuple.total_elements).map(|_| "_".to_string()).collect();
        parts[tuple.variant_index] = format!("{constructor_name}{args}");
        format!("( {} )", parts.join(", "))
    } else {
        format!("{constructor_name}{args}")
    };

    format!("{branch_pad}{pattern} ->\n{body_pad}Debug.todo \"{constructor_name}\"")
}

// -- Text manipulation --

/// Append a variant to the type declaration in the source text.
fn append_variant_to_type(
    source: &str,
    tree: &tree_sitter::Tree,
    type_name: &str,
    definition: &str,
) -> Result<String> {
    let root = tree.root_node();
    let mut cursor = root.walk();

    for child in root.named_children(&mut cursor) {
        if child.kind() != "type_declaration" {
            continue;
        }
        if let Some(name_node) = child.child_by_field_name("name")
            && let Ok(name) = name_node.utf8_text(source.as_bytes())
            && name == type_name
        {
            // Find the last union_variant to determine indentation.
            let mut last_variant: Option<Node> = None;
            let mut inner = child.walk();
            for desc in child.named_children(&mut inner) {
                if desc.kind() == "union_variant" {
                    last_variant = Some(desc);
                }
            }

            let Some(last) = last_variant else {
                bail!("type {type_name} has no variants");
            };

            // Get indentation of existing variants.
            let variant_line = last.start_position().row;
            let variant_line_text = source.lines().nth(variant_line).unwrap_or("");
            let indent = variant_line_text.len() - variant_line_text.trim_start().len();
            let indent_str = " ".repeat(indent);

            // Insert after the last variant's line.
            let insert_after_byte = last.end_byte();
            let new_variant_text = format!("\n{indent_str}| {definition}");

            let mut result = String::with_capacity(source.len() + new_variant_text.len());
            result.push_str(&source[..insert_after_byte]);
            result.push_str(&new_variant_text);
            result.push_str(&source[insert_after_byte..]);

            return Ok(result);
        }
    }

    bail!("type {type_name} not found")
}

/// Remove a variant from the type declaration in the source text.
fn remove_variant_from_type(
    source: &str,
    tree: &tree_sitter::Tree,
    type_name: &str,
    constructor_name: &str,
) -> Result<String> {
    let root = tree.root_node();
    let mut cursor = root.walk();

    for child in root.named_children(&mut cursor) {
        if child.kind() != "type_declaration" {
            continue;
        }
        if let Some(name_node) = child.child_by_field_name("name")
            && let Ok(name) = name_node.utf8_text(source.as_bytes())
            && name == type_name
        {
            let mut variants: Vec<Node> = Vec::new();
            let mut target_idx = None;
            let mut inner = child.walk();
            for desc in child.named_children(&mut inner) {
                if desc.kind() == "union_variant" {
                    if let Some(first) = desc.named_child(0)
                        && let Ok(ctor) = first.utf8_text(source.as_bytes())
                        && ctor == constructor_name
                    {
                        target_idx = Some(variants.len());
                    }
                    variants.push(desc);
                }
            }

            let Some(idx) = target_idx else {
                bail!("constructor {constructor_name} not found in type {type_name}");
            };

            if variants.len() == 1 {
                bail!(
                    "cannot remove the last variant from type {type_name}; use `elmq rm` to remove the entire type"
                );
            }

            let target = variants[idx];

            // Determine the byte range to remove, including the `|` separator and surrounding whitespace.
            let (remove_start, remove_end) = if idx == 0 {
                // First variant: remove from after `=` to after the `|` before the next variant.
                // This keeps `= <next_variant>` intact.
                let next = variants[idx + 1];
                let eq_pos = source[child.start_byte()..target.start_byte()]
                    .find('=')
                    .map(|p| p + child.start_byte());

                if let Some(eq) = eq_pos {
                    let pipe_before_next = source[target.end_byte()..next.start_byte()]
                        .find('|')
                        .map(|p| p + target.end_byte());
                    if let Some(pipe) = pipe_before_next {
                        (eq + 1, pipe + 1)
                    } else {
                        (target.start_byte(), target.end_byte())
                    }
                } else {
                    (target.start_byte(), target.end_byte())
                }
            } else {
                // Non-first variant: remove the `|` before it and the variant itself.
                let prev = variants[idx - 1];
                // Find the `|` between prev and target.
                let between_start = prev.end_byte();
                let pipe_pos = source[between_start..target.start_byte()]
                    .find('|')
                    .map(|p| p + between_start);

                if let Some(pipe) = pipe_pos {
                    // Remove from before the `|` (including the newline before it) to end of variant.
                    let remove_from = source[..pipe].rfind('\n').unwrap_or(pipe);
                    (remove_from, target.end_byte())
                } else {
                    (target.start_byte(), target.end_byte())
                }
            };

            let mut result = String::with_capacity(source.len());
            result.push_str(&source[..remove_start]);
            result.push_str(&source[remove_end..]);
            return Ok(result);
        }
    }

    bail!("type {type_name} not found")
}

/// Insert a new branch into a case expression at the end (before wildcard if present).
fn insert_case_branch(source: &str, case_node: &Node, new_branch: &str) -> String {
    // Find the last non-wildcard branch, or the last branch overall.
    let mut last_branch: Option<Node> = None;
    let mut last_non_wildcard: Option<Node> = None;
    let mut cursor = case_node.walk();
    for child in case_node.named_children(&mut cursor) {
        if child.kind() == "case_of_branch" {
            last_branch = Some(child);
            if let Some(pattern) = child.child_by_field_name("pattern")
                && !pattern_is_wildcard(&pattern, source)
            {
                last_non_wildcard = Some(child);
            }
        }
    }

    // Insert after the last non-wildcard branch if there is one, otherwise after last branch.
    let insert_after = last_non_wildcard.or(last_branch);

    if let Some(branch) = insert_after {
        let insert_pos = branch.end_byte();
        let mut result = String::with_capacity(source.len() + new_branch.len() + 2);
        result.push_str(&source[..insert_pos]);
        result.push_str("\n\n");
        result.push_str(new_branch);
        result.push_str(&source[insert_pos..]);
        result
    } else {
        source.to_string()
    }
}

/// Remove a branch from a case expression.
fn remove_case_branch(source: &str, branch: &MatchingBranch) -> String {
    // Find the start of the line containing the branch.
    let line_start = source[..branch.byte_range.start]
        .rfind('\n')
        .unwrap_or(branch.byte_range.start);

    // Find the end: include trailing whitespace/blank lines up to next branch.
    let after = &source[branch.byte_range.end..];
    let trailing = after
        .find(|c: char| !c.is_whitespace())
        .unwrap_or(after.len());

    // Check if there's a blank line between branches — keep one newline.
    let remove_end = branch.byte_range.end + trailing;

    let mut result = String::with_capacity(source.len());
    result.push_str(&source[..line_start]);
    result.push_str(&source[remove_end..]);
    result
}

// -- Display helpers --

fn display_path(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn relative_display(file: &Path, root: &PathBuf) -> String {
    let relative = file.strip_prefix(root).unwrap_or(file);
    display_path(relative)
}

// -- Public API --

/// Add a variant to a custom type and insert branches in all case expressions project-wide.
/// `fills` maps site keys (as produced by `compute_site_keys`) to branch text that replaces
/// the default `Debug.todo "<VariantName>"` stub. Keys not matched by any site cause a
/// pre-write error; sites not matched by any fill receive the default stub (graceful
/// degradation).
pub fn execute_add_variant(
    file: &Path,
    type_name: &str,
    definition: &str,
    dry_run: bool,
    fills: HashMap<String, String>,
) -> Result<VariantResult> {
    let (constructor_name, arg_count) = parse_variant_definition(definition)?;

    let project = Project::discover(file)?;
    let target_module = project.module_name(file)?;
    let elm_files = project.elm_files()?;
    let type_file_display = relative_display(file, &project.root);

    // Build the constructor map BEFORE adding the new variant (so we can find existing cases).
    let constructor_map = build_constructor_map(&project, &elm_files)?;

    // Verify the type exists and the constructor doesn't already exist.
    let source = std::fs::read_to_string(file)
        .with_context(|| format!("could not read {}", file.display()))?;
    let tree = parser::parse(&source)?;

    // Check type exists.
    {
        let root = tree.root_node();
        let mut cursor = root.walk();
        let mut found = false;
        for child in root.named_children(&mut cursor) {
            if child.kind() == "type_declaration"
                && let Some(name_node) = child.child_by_field_name("name")
                && let Ok(name) = name_node.utf8_text(source.as_bytes())
                && name == type_name
            {
                found = true;
                break;
            }
        }
        if !found {
            bail!("type {type_name} not found in {}", file.display());
        }
    }

    if constructor_map.contains_key(&constructor_name) {
        let existing = &constructor_map[&constructor_name];
        if existing.module == target_module && existing.type_name == type_name {
            bail!("constructor {constructor_name} already exists in type {type_name}");
        }
    }

    // Step 1: Modify the type declaration file.
    let new_source = append_variant_to_type(&source, &tree, type_name, definition)?;

    // Step 2: Collect every matching case site across the project, using the modified
    // type-file source so the walk sees the post-append state for the type's own file.
    let mut source_overrides = HashMap::new();
    source_overrides.insert(file.to_path_buf(), new_source.clone());
    let sites = collect_case_sites(
        &project,
        &elm_files,
        &target_module,
        type_name,
        &constructor_map,
        &source_overrides,
    )?;
    let keys = compute_site_keys(&sites);

    // Step 2b: validate fills against the computed keys before touching any file.
    validate_fills(&fills, &keys)?;

    // Step 3: Apply insertions file-by-file using the collected sites.
    let mut edits = Vec::new();
    let mut skipped = Vec::new();
    let mut pending_writes: Vec<(PathBuf, String)> = Vec::new();
    // The type file is always pending (it gets the new variant even if no case expressions matched).
    pending_writes.push((file.to_path_buf(), new_source.clone()));

    // Group site indices by file so we can run the per-file insertion loop against a
    // single mutable `modified_source` and re-parse after each insertion to keep byte
    // positions consistent.
    let mut sites_by_file: BTreeMap<PathBuf, Vec<usize>> = BTreeMap::new();
    for (i, site) in sites.iter().enumerate() {
        sites_by_file.entry(site.file.clone()).or_default().push(i);
    }

    for (file_path, mut indices) in sites_by_file {
        // Process sites within a file in reverse byte-order so earlier edits don't
        // invalidate later byte ranges. The inner loop still re-parses after each
        // insertion via `find_case_node_at` — byte ranges are approximate anchors.
        indices.sort_by(|a, b| sites[*b].byte_range.start.cmp(&sites[*a].byte_range.start));

        let starting_source = if file_path == file {
            new_source.clone()
        } else {
            match std::fs::read_to_string(&file_path) {
                Ok(s) => s,
                Err(_) => continue,
            }
        };
        let mut modified_source = starting_source.clone();

        // Type context has to be rebuilt per-file because `current_module` varies.
        let module_name = match project.module_name(&file_path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        for idx in &indices {
            let site = &sites[*idx];
            if site.has_wildcard {
                skipped.push(CaseSkip {
                    file: site.display.clone(),
                    module: site.module.clone(),
                    function: site.function.clone(),
                    line: site.line,
                    reason: "wildcard branch covers new variant".to_string(),
                });
                continue;
            }

            let branch_text = if let Some(fill_body) = fills.get(&keys[*idx]) {
                generate_filled_branch(site.branch_indent, fill_body)
            } else {
                generate_branch(
                    &constructor_name,
                    arg_count,
                    site.branch_indent,
                    site.body_indent,
                    site.tuple_position.as_ref(),
                )
            };

            let fresh_tree = parser::parse(&modified_source)?;
            let fresh_root = fresh_tree.root_node();
            let fresh_import_ctx = ImportContext::from_tree(&fresh_root, &modified_source);
            let type_ctx = TypeContext {
                target_module: &target_module,
                target_type: type_name,
                constructor_map: &constructor_map,
                current_module: &module_name,
            };

            if let Some(case_node) = find_case_node_at(
                &fresh_root,
                site.byte_range.start,
                &modified_source,
                &type_ctx,
                &fresh_import_ctx,
            ) {
                modified_source = insert_case_branch(&modified_source, &case_node, &branch_text);
                edits.push(CaseEdit {
                    file: site.display.clone(),
                    module: site.module.clone(),
                    function: site.function.clone(),
                    line: site.line,
                });
            }
        }

        if modified_source != starting_source {
            if file_path == file {
                pending_writes[0].1 = modified_source;
            } else {
                pending_writes.push((file_path, modified_source));
            }
        }
    }

    // Step 4: Write all files atomically.
    if !dry_run {
        for (path, content) in &pending_writes {
            writer::atomic_write(path, content)?;
        }
    }

    Ok(VariantResult {
        dry_run,
        type_file: type_file_display,
        type_name: type_name.to_string(),
        variant_name: constructor_name,
        edits,
        skipped,
        references_not_rewritten: Vec::new(),
    })
}

/// Validate that every fill key corresponds to some site key. Bare-name fill keys
/// that hit an ambiguous function name produce a targeted error listing the valid
/// disambiguated keys (`function#1`, `function#2`, etc.). Unknown keys produce a
/// generic error with the full key list to orient the caller on their next try.
fn validate_fills(fills: &HashMap<String, String>, keys: &[String]) -> Result<()> {
    if fills.is_empty() {
        return Ok(());
    }

    let keyset: std::collections::HashSet<&str> = keys.iter().map(|s| s.as_str()).collect();
    let mut errors: Vec<String> = Vec::new();

    for fill_key in fills.keys() {
        if keyset.contains(fill_key.as_str()) {
            continue;
        }

        // Check for ambiguous bare name: does any site key start with "<fill_key>#"
        // (same file, disambiguated by ordinal) or look like "<file>:<fill_key>[#N]"
        // (different files disambiguated by path)?
        let suffixed = format!("{}#", fill_key);
        let mid_colon = format!(":{}", fill_key);
        let disambiguations: Vec<&str> = keys
            .iter()
            .filter(|k| {
                k.starts_with(&suffixed)
                    || k.ends_with(&mid_colon)
                    || k.contains(&format!("{}#", mid_colon))
            })
            .map(|s| s.as_str())
            .collect();

        if !disambiguations.is_empty() {
            errors.push(format!(
                "fill key '{}' is ambiguous; use one of: {}",
                fill_key,
                disambiguations.join(", ")
            ));
        } else if keys.is_empty() {
            errors.push(format!(
                "no case site matched fill key: {} (project has no case expressions on this type)\n  \
                 hint: --fill only targets case expressions; use `elmq patch` for list-based dispatch (e.g. parser combinators)",
                fill_key
            ));
        } else {
            errors.push(format!(
                "no case site matched fill key: {}\n  valid keys: {}\n  \
                 hint: --fill only targets case expressions; use `elmq patch` for list-based dispatch (e.g. parser combinators)",
                fill_key,
                keys.join(", ")
            ));
        }
    }

    if !errors.is_empty() {
        bail!("{}", errors.join("\n"));
    }
    Ok(())
}

/// Indent a user-supplied branch text so the first non-empty line lands at `branch_indent`
/// and subsequent lines preserve their relative indentation. Auto-detects the user's
/// baseline indent (minimum leading whitespace across non-empty lines) and rebases from
/// there so both "zero-indented" inputs (`Reset -> text "reset"`) and already-indented
/// inputs work identically.
fn generate_filled_branch(branch_indent: usize, fill_text: &str) -> String {
    let base_indent: usize = fill_text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);

    let pad = " ".repeat(branch_indent);
    fill_text
        .lines()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                let on_line = line.len() - line.trim_start().len();
                let strip = base_indent.min(on_line);
                format!("{}{}", pad, &line[strip..])
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Read-only: collect every case expression in the project that matches on `type_name`
/// and return each site with the full text of its enclosing top-level declaration.
///
/// This is the planning counterpart to `execute_add_variant` — the caller feeds the
/// returned bodies and keys into `variant add --fill`, so both commands agree on
/// which sites exist and what their stable identifiers are.
pub fn execute_cases(file: &Path, type_name: &str) -> Result<CasesResult> {
    let project = Project::discover(file)?;
    let target_module = project.module_name(file)?;
    let elm_files = project.elm_files()?;
    let type_file_display = relative_display(file, &project.root);

    // Validate the type exists in the supplied file before walking the project, so
    // typos surface as a precise error instead of an empty result set.
    let source = std::fs::read_to_string(file)
        .with_context(|| format!("could not read {}", file.display()))?;
    let tree = parser::parse(&source)?;
    {
        let root = tree.root_node();
        let mut cursor = root.walk();
        let mut found = false;
        for child in root.named_children(&mut cursor) {
            if child.kind() == "type_declaration"
                && let Some(name_node) = child.child_by_field_name("name")
                && let Ok(name) = name_node.utf8_text(source.as_bytes())
                && name == type_name
            {
                found = true;
                break;
            }
        }
        if !found {
            bail!("type {type_name} not found in {}", file.display());
        }
    }

    let constructor_map = build_constructor_map(&project, &elm_files)?;
    let collected = collect_case_sites(
        &project,
        &elm_files,
        &target_module,
        type_name,
        &constructor_map,
        &HashMap::new(),
    )?;

    // Split wildcard-covered sites off into `skipped`; only non-wildcard sites get
    // keys because they are the ones that `variant add --fill` would write to.
    let (active_sites, skipped_sites): (Vec<CaseSite>, Vec<CaseSite>) =
        collected.into_iter().partition(|s| !s.has_wildcard);

    let keys = compute_site_keys(&active_sites);

    // Cache per-file sources so we only read each file once during body slicing.
    let mut source_cache: HashMap<PathBuf, String> = HashMap::new();
    let mut sites = Vec::with_capacity(active_sites.len());
    for (i, site) in active_sites.iter().enumerate() {
        let body = if site.declaration_byte_range.start == site.declaration_byte_range.end {
            String::new()
        } else {
            let src = match source_cache.get(&site.file) {
                Some(s) => s.clone(),
                None => {
                    let s = std::fs::read_to_string(&site.file)
                        .with_context(|| format!("could not read {}", site.file.display()))?;
                    source_cache.insert(site.file.clone(), s.clone());
                    s
                }
            };
            src[site.declaration_byte_range.clone()].to_string()
        };

        sites.push(CasesSite {
            file: site.display.clone(),
            module: site.module.clone(),
            function: site.function.clone(),
            key: keys[i].clone(),
            line: site.line,
            body,
        });
    }

    let skipped: Vec<CaseSkip> = skipped_sites
        .into_iter()
        .map(|s| CaseSkip {
            file: s.display,
            module: s.module,
            function: s.function,
            line: s.line,
            reason: "wildcard branch covers type".to_string(),
        })
        .collect();

    Ok(CasesResult {
        type_name: format!("{}.{}", target_module, type_name),
        type_file: type_file_display,
        sites,
        skipped,
    })
}

/// Remove a variant from a custom type and remove branches from all case expressions project-wide.
pub fn execute_rm_variant(
    file: &Path,
    type_name: &str,
    constructor_name: &str,
    dry_run: bool,
) -> Result<VariantResult> {
    let project = Project::discover(file)?;
    let target_module = project.module_name(file)?;
    let elm_files = project.elm_files()?;
    let type_file_display = relative_display(file, &project.root);

    // Build the constructor map.
    let constructor_map = build_constructor_map(&project, &elm_files)?;

    // Verify the constructor exists in the target type.
    if let Some(type_info) = constructor_map.get(constructor_name) {
        if type_info.module != target_module || type_info.type_name != type_name {
            bail!(
                "constructor {constructor_name} belongs to {}.{}, not {target_module}.{type_name}",
                type_info.module,
                type_info.type_name
            );
        }
    } else {
        bail!("constructor {constructor_name} not found");
    }

    // Step 1: Run the classifier over the pristine project state. This gives
    // us (a) the full set of reference sites for the advisory list and (b) a
    // `CaseWildcardCovered` report for cases whose wildcard already catches
    // the removed constructor. The iterative branch-removal loop below uses
    // its own per-file walk (with `find_matching_branch`) rather than trying
    // to reuse byte ranges from here, because removals shift positions.
    let pristine_sites = collect_constructor_sites(
        &project,
        &elm_files,
        &target_module,
        type_name,
        constructor_name,
        &constructor_map,
    )?;

    // Step 2: Remove the variant from the type declaration.
    let source = std::fs::read_to_string(file)
        .with_context(|| format!("could not read {}", file.display()))?;
    let tree = parser::parse(&source)?;

    let new_source = remove_variant_from_type(&source, &tree, type_name, constructor_name)?;

    // Step 3: Walk the project and remove branches, collecting all writes.
    let mut edits = Vec::new();
    let mut skipped = Vec::new();
    let mut pending_writes: Vec<(PathBuf, String)> = Vec::new();

    // The type file itself is always written (with the variant removed).
    pending_writes.push((file.to_path_buf(), new_source));

    for elm_file in &elm_files {
        let file_source = match std::fs::read_to_string(elm_file) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let file_tree = match parser::parse(&file_source) {
            Ok(t) => t,
            Err(_) => continue,
        };

        let root = file_tree.root_node();
        let import_ctx = ImportContext::from_tree(&root, &file_source);
        let module_name = match project.module_name(elm_file) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let display = relative_display(elm_file, &project.root);
        let type_ctx = TypeContext {
            target_module: &target_module,
            target_type: type_name,
            constructor_map: &constructor_map,
            current_module: &module_name,
        };

        let case_exprs = find_matching_case_exprs(&root, &file_source, &type_ctx, &import_ctx);

        if case_exprs.is_empty() {
            continue;
        }

        let mut modified_source = file_source.clone();
        let mut file_edits = Vec::new();

        // Remove one branch at a time, re-parsing between each to keep positions valid.
        loop {
            let fresh_tree = match parser::parse(&modified_source) {
                Ok(t) => t,
                Err(_) => break,
            };
            let fresh_root = fresh_tree.root_node();
            let fresh_import_ctx = ImportContext::from_tree(&fresh_root, &modified_source);

            let fresh_cases = find_matching_case_exprs(
                &fresh_root,
                &modified_source,
                &type_ctx,
                &fresh_import_ctx,
            );

            let mut removed_one = false;
            for fresh_case in &fresh_cases {
                if let Some(case_node) = find_case_node_at(
                    &fresh_root,
                    fresh_case.byte_range.start,
                    &modified_source,
                    &type_ctx,
                    &fresh_import_ctx,
                ) && let Some(branch) = find_matching_branch(
                    &case_node,
                    &modified_source,
                    constructor_name,
                    &type_ctx,
                    &fresh_import_ctx,
                ) {
                    modified_source = remove_case_branch(&modified_source, &branch);
                    file_edits.push(CaseEdit {
                        file: display.clone(),
                        module: module_name.clone(),
                        function: fresh_case.function.clone(),
                        line: fresh_case.line,
                    });
                    removed_one = true;
                    break; // Re-parse after each removal.
                }
            }
            if !removed_one {
                break;
            }
        }

        edits.extend(file_edits);

        if modified_source != file_source {
            pending_writes.push((elm_file.clone(), modified_source));
        }
    }

    // Step 4: Build the skip list from the pristine walker's
    // `CaseWildcardCovered` sites, and the advisory list from every
    // non-clean-removal site. The advisory is the elmq-structured hint a
    // caller would otherwise have to reconstruct from `elm make` diagnostics.
    let mut references_not_rewritten: Vec<ConstructorSiteReport> = Vec::new();
    for site in pristine_sites {
        match &site.classification {
            SiteKind::CaseBranch => {
                // Reported via the branch-removal loop above as a `CaseEdit`.
            }
            SiteKind::CaseWildcardCovered => {
                skipped.push(CaseSkip {
                    file: site.display.clone(),
                    module: site.module.clone(),
                    function: site.declaration.clone(),
                    line: site.line,
                    reason: "wildcard branch covers removed variant".to_string(),
                });
            }
            _ => {
                references_not_rewritten.push(site.into_report());
            }
        }
    }

    // Step 5: Write all files atomically.
    if !dry_run {
        for (path, content) in &pending_writes {
            writer::atomic_write(path, content)?;
        }
    }

    Ok(VariantResult {
        dry_run,
        type_file: type_file_display,
        type_name: type_name.to_string(),
        variant_name: constructor_name.to_string(),
        edits,
        skipped,
        references_not_rewritten,
    })
}

/// Read-only: collect every project-wide reference to a given constructor,
/// grouped by file, with classification (case branch, expression position,
/// refutable-pattern position, etc.). The discovery/audit counterpart to
/// `execute_rm_variant`'s advisory list — useful on its own but not part of
/// the rm loop.
pub fn execute_variant_refs(
    file: &Path,
    type_name: &str,
    constructor_name: &str,
) -> Result<RefsResult> {
    let project = Project::discover(file)?;
    let target_module = project.module_name(file)?;
    let elm_files = project.elm_files()?;
    let type_file_display = relative_display(file, &project.root);

    // Validate: the target type must exist in the given file.
    let source = std::fs::read_to_string(file)
        .with_context(|| format!("could not read {}", file.display()))?;
    let tree = parser::parse(&source)?;
    {
        let root = tree.root_node();
        let mut cursor = root.walk();
        let mut found = false;
        for child in root.named_children(&mut cursor) {
            if child.kind() == "type_declaration"
                && let Some(name_node) = child.child_by_field_name("name")
                && let Ok(name) = name_node.utf8_text(source.as_bytes())
                && name == type_name
            {
                found = true;
                break;
            }
        }
        if !found {
            bail!("type {type_name} not found in {}", file.display());
        }
    }

    let constructor_map = build_constructor_map(&project, &elm_files)?;

    // Validate: the constructor must exist and belong to the given type.
    match constructor_map.get(constructor_name) {
        Some(type_info) => {
            if type_info.module != target_module || type_info.type_name != type_name {
                bail!(
                    "constructor {constructor_name} belongs to {}.{}, not {target_module}.{type_name}",
                    type_info.module,
                    type_info.type_name
                );
            }
        }
        None => bail!("constructor {constructor_name} not found"),
    }

    let sites = collect_constructor_sites(
        &project,
        &elm_files,
        &target_module,
        type_name,
        constructor_name,
        &constructor_map,
    )?;

    let total_sites = sites.len();
    let total_clean = sites
        .iter()
        .filter(|s| s.classification.is_clean_removal())
        .count();
    let total_blocking = total_sites - total_clean;

    let reports: Vec<ConstructorSiteReport> = sites.into_iter().map(|s| s.into_report()).collect();

    // Group by file, preserving walker order (already sorted by (file, line, col)).
    let mut grouped: Vec<(String, Vec<ConstructorSiteReport>)> = Vec::new();
    for r in &reports {
        match grouped.last_mut() {
            Some((last_file, group)) if last_file == &r.file => group.push(r.clone()),
            _ => grouped.push((r.file.clone(), vec![r.clone()])),
        }
    }

    Ok(RefsResult {
        type_file: type_file_display,
        type_name: type_name.to_string(),
        constructor: constructor_name.to_string(),
        total_sites,
        total_clean,
        total_blocking,
        sites: reports,
        sites_by_file: grouped,
    })
}

/// Find a case_of_expr node near a given byte offset.
fn find_case_node_at<'a>(
    root: &Node<'a>,
    _near_byte: usize,
    source: &str,
    ctx: &TypeContext,
    import_ctx: &ImportContext,
) -> Option<Node<'a>> {
    // Find all matching case expressions and return the one closest to near_byte.
    let mut all_cases = Vec::new();
    collect_all_case_nodes(root, source, ctx, import_ctx, &mut all_cases);

    // Find the closest one.
    all_cases
        .into_iter()
        .min_by_key(|n| (n.start_byte() as isize - _near_byte as isize).unsigned_abs())
}

fn collect_all_case_nodes<'a>(
    node: &Node<'a>,
    source: &str,
    ctx: &TypeContext,
    import_ctx: &ImportContext,
    results: &mut Vec<Node<'a>>,
) {
    if node.kind() == "case_of_expr" {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() != "case_of_branch" {
                continue;
            }
            if let Some(pattern) = child.child_by_field_name("pattern")
                && find_constructor_in_pattern(&pattern, source, ctx, import_ctx).is_some()
            {
                results.push(*node);
                break;
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_all_case_nodes(&child, source, ctx, import_ctx, results);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_variant_definition_simple() {
        let (name, args) = parse_variant_definition("SetName").unwrap();
        assert_eq!(name, "SetName");
        assert_eq!(args, 0);
    }

    #[test]
    fn test_parse_variant_definition_one_arg() {
        let (name, args) = parse_variant_definition("SetName String").unwrap();
        assert_eq!(name, "SetName");
        assert_eq!(args, 1);
    }

    #[test]
    fn test_parse_variant_definition_multi_arg() {
        let (name, args) = parse_variant_definition("Node (List String) Int").unwrap();
        assert_eq!(name, "Node");
        assert_eq!(args, 2);
    }

    #[test]
    fn test_generate_branch_simple() {
        let branch = generate_branch("SetName", 1, 8, 12, None);
        assert_eq!(
            branch,
            "        SetName _ ->\n            Debug.todo \"SetName\""
        );
    }

    #[test]
    fn test_generate_branch_no_args() {
        let branch = generate_branch("Reset", 0, 8, 12, None);
        assert_eq!(branch, "        Reset ->\n            Debug.todo \"Reset\"");
    }

    #[test]
    fn test_generate_branch_tuple() {
        let tuple_info = TuplePatternInfo {
            variant_index: 0,
            total_elements: 2,
        };
        let branch = generate_branch("SetName", 1, 8, 12, Some(&tuple_info));
        assert_eq!(
            branch,
            "        ( SetName _, _ ) ->\n            Debug.todo \"SetName\""
        );
    }

    // -- compute_site_keys --
    //
    // These tests build synthetic CaseSite values (all tree-sitter metadata nulled out
    // except the fields the key algorithm reads) to exercise the four qualification
    // levels from design.md §3/§8 without needing real Elm source on disk.
    fn stub_site(display: &str, function: &str, start: usize) -> CaseSite {
        CaseSite {
            file: PathBuf::from(display),
            module: "M".to_string(),
            display: display.to_string(),
            function: function.to_string(),
            line: 1,
            byte_range: start..(start + 10),
            has_wildcard: false,
            branch_indent: 8,
            body_indent: 12,
            tuple_position: None,
            declaration_byte_range: 0..0,
        }
    }

    #[test]
    fn site_key_single_site_is_bare_function() {
        let sites = vec![stub_site("src/Main.elm", "update", 100)];
        let keys = compute_site_keys(&sites);
        assert_eq!(keys, vec!["update"]);
    }

    #[test]
    fn site_key_multiple_sites_distinct_functions_stay_bare() {
        let sites = vec![
            stub_site("src/Main.elm", "update", 100),
            stub_site("src/Main.elm", "view", 200),
            stub_site("src/Main.elm", "subscriptions", 300),
        ];
        let keys = compute_site_keys(&sites);
        assert_eq!(keys, vec!["update", "view", "subscriptions"]);
    }

    #[test]
    fn site_key_two_cases_in_same_function_get_ordinals() {
        let sites = vec![
            stub_site("src/Main.elm", "update", 500),
            stub_site("src/Main.elm", "update", 100),
        ];
        let keys = compute_site_keys(&sites);
        // Sites at byte 100 should get #1 (earlier in source), byte 500 gets #2.
        assert_eq!(keys, vec!["update#2", "update#1"]);
    }

    #[test]
    fn site_key_same_function_in_different_files_gets_file_prefix() {
        let sites = vec![
            stub_site("src/Main.elm", "update", 100),
            stub_site("src/Page.elm", "update", 200),
        ];
        let keys = compute_site_keys(&sites);
        assert_eq!(keys, vec!["src/Main.elm:update", "src/Page.elm:update"]);
    }

    #[test]
    fn site_key_full_qualification_file_and_ordinal() {
        // Two files both define `update`. One file has two case expressions inside
        // `update`. Expected shape:
        //   src/Main.elm:update#1, src/Main.elm:update#2, src/Page.elm:update
        let sites = vec![
            stub_site("src/Main.elm", "update", 500),
            stub_site("src/Main.elm", "update", 100),
            stub_site("src/Page.elm", "update", 200),
        ];
        let keys = compute_site_keys(&sites);
        // Main's byte-100 site sorts before byte-500 → #1 / #2.
        assert_eq!(
            keys,
            vec![
                "src/Main.elm:update#2",
                "src/Main.elm:update#1",
                "src/Page.elm:update",
            ]
        );
    }

    // -- validate_fills --

    #[test]
    fn validate_fills_accepts_matching_bare_key() {
        let mut fills = HashMap::new();
        fills.insert("update".to_string(), "Reset -> model".to_string());
        let keys = vec!["update".to_string(), "view".to_string()];
        assert!(validate_fills(&fills, &keys).is_ok());
    }

    #[test]
    fn validate_fills_rejects_unknown_key() {
        let mut fills = HashMap::new();
        fills.insert("nosuch".to_string(), "body".to_string());
        let keys = vec!["update".to_string(), "view".to_string()];
        let err = validate_fills(&fills, &keys).unwrap_err().to_string();
        assert!(err.contains("no case site matched fill key: nosuch"));
        assert!(err.contains("update"));
        assert!(err.contains("view"));
    }

    #[test]
    fn validate_fills_rejects_ambiguous_bare_key() {
        let mut fills = HashMap::new();
        fills.insert("update".to_string(), "body".to_string());
        let keys = vec!["update#1".to_string(), "update#2".to_string()];
        let err = validate_fills(&fills, &keys).unwrap_err().to_string();
        assert!(err.contains("'update' is ambiguous"));
        assert!(err.contains("update#1"));
        assert!(err.contains("update#2"));
    }

    #[test]
    fn validate_fills_rejects_ambiguous_across_files() {
        let mut fills = HashMap::new();
        fills.insert("update".to_string(), "body".to_string());
        let keys = vec![
            "src/Main.elm:update".to_string(),
            "src/Page.elm:update".to_string(),
        ];
        let err = validate_fills(&fills, &keys).unwrap_err().to_string();
        assert!(err.contains("'update' is ambiguous"));
        assert!(err.contains("src/Main.elm:update"));
        assert!(err.contains("src/Page.elm:update"));
    }

    // -- generate_filled_branch --

    #[test]
    fn filled_branch_zero_indented_input() {
        let out = generate_filled_branch(8, "Reset -> text \"reset\"");
        assert_eq!(out, "        Reset -> text \"reset\"");
    }

    #[test]
    fn filled_branch_multiline_input() {
        let input = "Reset ->\n    text \"reset\"";
        let out = generate_filled_branch(8, input);
        assert_eq!(out, "        Reset ->\n            text \"reset\"");
    }

    #[test]
    fn filled_branch_rebases_already_indented_input() {
        // User wrote at base indent 4 (e.g. from pasting code); we rebase to 8.
        let input = "    Reset ->\n        text \"reset\"";
        let out = generate_filled_branch(8, input);
        assert_eq!(out, "        Reset ->\n            text \"reset\"");
    }

    #[test]
    fn test_last_variant_removal_error() {
        let source = "module T exposing (..)\n\ntype Msg\n    = Only\n";
        let tree = parser::parse(source).unwrap();
        let result = remove_variant_from_type(source, &tree, "Msg", "Only");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("cannot remove the last variant")
        );
    }
}
