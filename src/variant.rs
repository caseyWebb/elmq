use crate::imports::ImportContext;
use crate::parser;
use crate::project::Project;
use crate::writer;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::collections::HashMap;
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

/// Info about a case expression that matches the target type.
struct CaseExprInfo {
    /// Byte range of the entire case_of_expr node
    byte_range: std::ops::Range<usize>,
    /// Line number (1-based) of the case_of_expr
    line: usize,
    /// Enclosing function name
    function: String,
    /// Whether the case has a wildcard/catch-all branch
    has_wildcard: bool,
    /// Indentation (in spaces) used for branches
    branch_indent: usize,
    /// Indentation (in spaces) used for branch bodies
    body_indent: usize,
    /// For nested patterns (tuples): the position of the constructor within the tuple, if any.
    /// None means the constructor is the direct pattern.
    tuple_position: Option<TuplePatternInfo>,
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
            None
        }
        "pattern" | "tuple_pattern" | "cons_pattern" | "list_pattern" => {
            // Recurse into sub-patterns.
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
            let wildcard = has_wildcard_branch(node, source);
            let (branch_indent, body_indent) = get_branch_indentation(node, source);
            let tuple_pos = detect_tuple_position(node, source, ctx, import_ctx);
            let function = find_enclosing_function(node, source);

            results.push(CaseExprInfo {
                byte_range: node.byte_range(),
                line: node.start_position().row + 1,
                function,
                has_wildcard: wildcard,
                branch_indent,
                body_indent,
                tuple_position: tuple_pos,
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
pub fn execute_add_variant(
    file: &Path,
    type_name: &str,
    definition: &str,
    dry_run: bool,
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

    // Step 2: Walk the project and insert branches, collecting all writes.
    let mut edits = Vec::new();
    let mut skipped = Vec::new();
    let mut pending_writes: Vec<(PathBuf, String)> = Vec::new();

    // The type file itself is always written (with the new variant).
    pending_writes.push((file.to_path_buf(), new_source.clone()));

    for elm_file in &elm_files {
        let file_source = if *elm_file == file.to_path_buf() {
            // Use the modified source for the type's own file.
            new_source.clone()
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
            target_module: &target_module,
            target_type: type_name,
            constructor_map: &constructor_map,
            current_module: &module_name,
        };

        let case_exprs = find_matching_case_exprs(&root, &file_source, &type_ctx, &import_ctx);

        if case_exprs.is_empty() {
            continue;
        }

        // Process case expressions in reverse order (by byte offset) to preserve positions.
        let mut sorted_cases = case_exprs;
        sorted_cases.sort_by(|a, b| b.byte_range.start.cmp(&a.byte_range.start));

        let mut modified_source = file_source.clone();

        for case_info in &sorted_cases {
            if case_info.has_wildcard {
                skipped.push(CaseSkip {
                    file: display.clone(),
                    module: module_name.clone(),
                    function: case_info.function.clone(),
                    line: case_info.line,
                    reason: "wildcard branch covers new variant".to_string(),
                });
                continue;
            }

            let branch_text = generate_branch(
                &constructor_name,
                arg_count,
                case_info.branch_indent,
                case_info.body_indent,
                case_info.tuple_position.as_ref(),
            );

            // Re-parse to get fresh node positions after previous edits.
            let fresh_tree = parser::parse(&modified_source)?;
            let fresh_root = fresh_tree.root_node();

            // Find the case_of_expr at the expected position.
            if let Some(case_node) = find_case_node_at(
                &fresh_root,
                case_info.byte_range.start,
                &modified_source,
                &type_ctx,
                &import_ctx,
            ) {
                modified_source = insert_case_branch(&modified_source, &case_node, &branch_text);
                edits.push(CaseEdit {
                    file: display.clone(),
                    module: module_name.clone(),
                    function: case_info.function.clone(),
                    line: case_info.line,
                });
            }
        }

        if modified_source != file_source {
            if *elm_file == file.to_path_buf() {
                // Update the existing entry for the type file.
                pending_writes[0].1 = modified_source;
            } else {
                pending_writes.push((elm_file.clone(), modified_source));
            }
        }
    }

    // Step 3: Write all files atomically.
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

    // Step 1: Remove the variant from the type declaration.
    let source = std::fs::read_to_string(file)
        .with_context(|| format!("could not read {}", file.display()))?;
    let tree = parser::parse(&source)?;

    let new_source = remove_variant_from_type(&source, &tree, type_name, constructor_name)?;

    // Step 2: Walk the project and remove branches, collecting all writes.
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
                ) {
                    if let Some(branch) = find_matching_branch(
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
                    } else if fresh_case.has_wildcard {
                        skipped.push(CaseSkip {
                            file: display.clone(),
                            module: module_name.clone(),
                            function: fresh_case.function.clone(),
                            line: fresh_case.line,
                            reason: format!("wildcard branch handled {constructor_name}"),
                        });
                    }
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

    // Step 3: Write all files atomically.
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
