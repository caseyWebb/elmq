use crate::FileSummary;
use crate::parser;
use anyhow::{Context, Result, bail};
use std::path::Path;
use tree_sitter::Node;

/// Write content to a file atomically: write to a temp file in the same
/// directory, then rename over the original.
pub fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let temp = dir.join(format!(".elmq-tmp-{}", std::process::id()));
    std::fs::write(&temp, content)
        .with_context(|| format!("failed to write temp file: {}", temp.display()))?;
    std::fs::rename(&temp, path)
        .with_context(|| format!("failed to rename temp file to {}", path.display()))?;
    Ok(())
}

/// Re-parse `content` with tree-sitter-elm and refuse to write if the
/// result has any ERROR or MISSING nodes, then delegate to `atomic_write`.
///
/// Every elmq command that mutates Elm source MUST route its final
/// buffer through this helper (not `atomic_write` directly) so we never
/// commit a syntactically broken file to disk. `op` is a short label
/// for the operation (e.g. `"set"`, `"variant add"`) used in the error
/// message so multi-file commands name the failing step clearly.
pub fn validated_write(path: &Path, content: &str, op: &str) -> Result<()> {
    let tree = parser::parse(content)
        .with_context(|| format!("failed to re-parse output buffer for {}", path.display()))?;
    if tree.root_node().has_error() {
        let where_ = match parser::first_error_location(&tree, content) {
            Some((line, col)) => format!(" at {line}:{col}"),
            None => String::new(),
        };
        bail!(
            "rejected '{op}' write to {}: output would not parse{where_}",
            path.display()
        );
    }
    atomic_write(path, content)
}

/// Upsert a declaration. If a declaration with the given name exists, replace it;
/// otherwise append after the last declaration.
pub fn upsert_declaration(
    source: &str,
    summary: &FileSummary,
    name: &str,
    new_source: &str,
) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let new_source = new_source.trim_end();

    if let Some(decl) = summary.find_declaration(name) {
        // Replace existing declaration
        let before = &lines[..decl.start_line - 1];
        let after = &lines[decl.end_line..];
        let mut result = String::new();
        for line in before {
            result.push_str(line);
            result.push('\n');
        }
        result.push_str(new_source);
        result.push('\n');
        for line in after {
            result.push_str(line);
            result.push('\n');
        }
        result
    } else {
        // Append after last declaration
        let mut result = source.to_string();
        if !result.ends_with('\n') {
            result.push('\n');
        }
        // Ensure two blank lines before the new declaration
        let trailing_newlines = result.len() - result.trim_end_matches('\n').len();
        for _ in trailing_newlines..3 {
            result.push('\n');
        }
        result.push_str(new_source);
        result.push('\n');
        result
    }
}

/// Patch a declaration: find-and-replace `old` with `new` within the declaration's range.
pub fn patch_declaration(
    source: &str,
    summary: &FileSummary,
    name: &str,
    old: &str,
    new: &str,
) -> Result<String> {
    let decl = summary
        .find_declaration(name)
        .with_context(|| format!("declaration '{name}' not found"))?;

    let lines: Vec<&str> = source.lines().collect();
    let decl_text = lines[decl.start_line - 1..decl.end_line].join("\n");

    let count = decl_text.matches(old).count();
    if count == 0 {
        bail!("old string not found in declaration '{name}'");
    }
    if count > 1 {
        bail!("old string matches {count} times in declaration '{name}' (must be unique)");
    }

    let patched = decl_text.replacen(old, new, 1);

    let before = &lines[..decl.start_line - 1];
    let after = &lines[decl.end_line..];
    let mut result = String::new();
    for line in before {
        result.push_str(line);
        result.push('\n');
    }
    result.push_str(&patched);
    result.push('\n');
    for line in after {
        result.push_str(line);
        result.push('\n');
    }
    Ok(result)
}

/// Remove a declaration by name, collapsing excess blank lines.
pub fn remove_declaration(source: &str, summary: &FileSummary, name: &str) -> Result<String> {
    let decl = summary
        .find_declaration(name)
        .with_context(|| format!("declaration '{name}' not found"))?;

    let lines: Vec<&str> = source.lines().collect();
    let before = &lines[..decl.start_line - 1];
    let after = &lines[decl.end_line..];

    let mut result = String::new();
    for line in before {
        result.push_str(line);
        result.push('\n');
    }
    for line in after {
        result.push_str(line);
        result.push('\n');
    }

    // Collapse runs of >2 blank lines to 2
    collapse_blank_lines(&result)
}

/// Add or replace an import. If an import with the same module name exists, replace it.
/// Otherwise insert in alphabetical order.
pub fn add_import(source: &str, summary: &FileSummary, import_clause: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let full_import = if import_clause.starts_with("import ") {
        import_clause.to_string()
    } else {
        format!("import {import_clause}")
    };

    // Extract the module name from the new import for comparison
    let new_module = extract_import_module(&full_import);

    if summary.imports.is_empty() {
        // No imports yet — insert after module line with blank line separation
        let module_end = find_module_end_line(&lines);
        let mut result = String::new();
        for line in &lines[..module_end] {
            result.push_str(line);
            result.push('\n');
        }
        result.push('\n');
        result.push_str(&full_import);
        result.push('\n');
        for line in &lines[module_end..] {
            result.push_str(line);
            result.push('\n');
        }
        return result;
    }

    // Find the import block range
    let (import_start, import_end) = find_import_range(&lines);

    // Collect existing imports, replacing if same module
    let mut imports: Vec<String> = Vec::new();
    let mut replaced = false;
    for line in &lines[import_start..import_end] {
        if line.starts_with("import ") {
            let existing_module = extract_import_module(line);
            if existing_module == new_module {
                imports.push(full_import.clone());
                replaced = true;
            } else {
                imports.push(line.to_string());
            }
        }
    }

    if !replaced {
        imports.push(full_import);
        imports.sort_by(|a, b| {
            let ma = extract_import_module(a);
            let mb = extract_import_module(b);
            ma.cmp(mb)
        });
    }

    let mut result = String::new();
    for line in &lines[..import_start] {
        result.push_str(line);
        result.push('\n');
    }
    for imp in &imports {
        result.push_str(imp);
        result.push('\n');
    }
    for line in &lines[import_end..] {
        result.push_str(line);
        result.push('\n');
    }
    result
}

/// Remove an import by module name. Idempotent: if no import with that name
/// exists, the source is returned unchanged.
pub fn remove_import(source: &str, name: &str) -> Result<String> {
    let lines: Vec<&str> = source.lines().collect();
    let mut found = false;
    let mut result = String::new();

    for line in &lines {
        if line.starts_with("import ") && extract_import_module(line) == name {
            found = true;
            continue;
        }
        result.push_str(line);
        result.push('\n');
    }

    if !found {
        return Ok(source.to_string());
    }

    collapse_blank_lines(&result)
}

/// Add an item to the module's exposing list. No-op if already exposed or `(..)`.
pub fn expose(source: &str, _summary: &FileSummary, item: &str) -> Result<String> {
    let module_decl = find_module_declaration(source)?;

    let exposing_content = extract_exposing_content(&module_decl)?;

    // If exposing (..), everything is already exposed — no-op
    if exposing_content.trim() == ".." {
        return Ok(source.to_string());
    }

    let items = parse_exposing_items(&exposing_content);

    // Check if already exposed (compare base name without (..))
    let item_base = exposing_base_name(item);
    for existing in &items {
        if exposing_base_name(existing) == item_base {
            return Ok(source.to_string());
        }
    }

    let mut new_items = items;
    new_items.push(item.to_string());
    let new_exposing = format!("({})", new_items.join(", "));
    let new_line = replace_exposing(&module_decl, &new_exposing)?;

    Ok(source.replacen(&module_decl, &new_line, 1))
}

/// Remove an item from the module's exposing list. Auto-expands `(..)`.
/// Idempotent: if the item is not in the exposing list (and the list is not
/// `(..)`), the source is returned unchanged.
pub fn unexpose(source: &str, summary: &FileSummary, item: &str) -> Result<String> {
    let module_decl = find_module_declaration(source)?;

    let exposing_content = extract_exposing_content(&module_decl)?;

    let is_wildcard = exposing_content.trim() == "..";
    let items = if is_wildcard {
        // Auto-expand (..) to explicit list from declarations
        expand_expose_all(summary)
    } else {
        parse_exposing_items(&exposing_content)
    };

    let item_base = exposing_base_name(item);
    let original_len = items.len();
    let new_items: Vec<String> = items
        .into_iter()
        .filter(|existing| exposing_base_name(existing) != item_base)
        .collect();

    if new_items.len() == original_len && !is_wildcard {
        return Ok(source.to_string());
    }

    let new_exposing = format!("({})", new_items.join(", "));
    let new_line = replace_exposing(&module_decl, &new_exposing)?;

    Ok(source.replacen(&module_decl, &new_line, 1))
}

/// Extract the base name from an exposing item, stripping any `(..)` suffix.
fn exposing_base_name(item: &str) -> &str {
    item.split('(').next().unwrap_or(item).trim()
}

/// Find the full module declaration, which may span multiple lines.
/// Returns the complete text from `module`/`port module` through the closing `)` of `exposing`.
pub(crate) fn find_module_declaration(source: &str) -> Result<String> {
    let mut start = None;
    for (i, line) in source.lines().enumerate() {
        if start.is_none()
            && (line.starts_with("module ")
                || line.starts_with("port module ")
                || line.starts_with("effect module "))
        {
            start = Some(i);
        }
        if let Some(s) = start
            && line.contains(')')
        {
            let lines: Vec<&str> = source.lines().collect();
            return Ok(lines[s..=i].join("\n"));
        }
    }
    if start.is_some() {
        bail!("module declaration has no closing parenthesis");
    }
    bail!("no module declaration found")
}

pub(crate) fn extract_exposing_content(module_decl: &str) -> Result<String> {
    let exposing_idx = module_decl
        .find("exposing")
        .with_context(|| "module declaration has no exposing clause")?;
    let after_exposing = &module_decl[exposing_idx + "exposing".len()..];
    let open = after_exposing
        .find('(')
        .with_context(|| "malformed exposing clause")?;
    let close = after_exposing
        .rfind(')')
        .with_context(|| "malformed exposing clause")?;
    Ok(after_exposing[open + 1..close].to_string())
}

fn parse_exposing_items(content: &str) -> Vec<String> {
    if content.trim().is_empty() {
        return Vec::new();
    }
    // Split on commas, but respect parentheses (e.g. "Msg(..)")
    let mut items = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    for ch in content.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    items.push(trimmed);
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        items.push(trimmed);
    }
    items
}

fn replace_exposing(module_decl: &str, new_exposing: &str) -> Result<String> {
    let exposing_idx = module_decl
        .find("exposing")
        .with_context(|| "module declaration has no exposing clause")?;
    Ok(format!(
        "{}exposing {new_exposing}",
        &module_decl[..exposing_idx]
    ))
}

/// Crate-visible wrapper around `replace_exposing` for callers outside
/// this module (notably `move_decl` when it needs to fall back to
/// `exposing (..)` instead of producing an empty list).
pub(crate) fn replace_exposing_public(module_decl: &str, new_exposing: &str) -> Result<String> {
    replace_exposing(module_decl, new_exposing)
}

fn expand_expose_all(summary: &FileSummary) -> Vec<String> {
    use crate::DeclarationKind;
    summary
        .declarations
        .iter()
        .map(|d| match d.kind {
            DeclarationKind::Type => format!("{}(..)", d.name),
            _ => d.name.clone(),
        })
        .collect()
}

fn extract_import_module(import_line: &str) -> &str {
    let rest = import_line.strip_prefix("import ").unwrap_or(import_line);
    // Module name is the first word
    rest.split_whitespace().next().unwrap_or(rest)
}

fn find_module_end_line(lines: &[&str]) -> usize {
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("module ")
            || line.starts_with("port module ")
            || line.starts_with("effect module ")
        {
            // Module declaration may span multiple lines — find the closing paren
            for (j, l) in lines.iter().enumerate().skip(i) {
                if l.contains(')') {
                    return j + 1;
                }
            }
            return i + 1;
        }
    }
    1 // Default to after first line if no module found
}

fn find_import_range(lines: &[&str]) -> (usize, usize) {
    let mut start = None;
    let mut end = 0;
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("import ") {
            if start.is_none() {
                start = Some(i);
            }
            end = i + 1;
        }
    }
    (start.unwrap_or(0), end)
}

/// Rewrite all references to `old_module` as `new_module` in the given source.
/// Handles import clauses, qualified value references, and qualified type references.
/// Uses tree-sitter to avoid modifying strings or comments.
pub fn rename_module_references(source: &str, old_module: &str, new_module: &str) -> String {
    let tree = match parser::parse(source) {
        Ok(t) => t,
        Err(_) => return source.to_string(),
    };

    let root = tree.root_node();
    let mut replacements: Vec<(usize, usize, &str)> = Vec::new();

    collect_module_replacements(&root, source, old_module, new_module, &mut replacements);

    if replacements.is_empty() {
        return source.to_string();
    }

    // Sort by start byte descending so we can apply back-to-front.
    replacements.sort_by(|a, b| b.0.cmp(&a.0));

    let mut result = source.to_string();
    for (start, end, replacement) in replacements {
        result.replace_range(start..end, replacement);
    }
    result
}

fn collect_module_replacements<'a>(
    node: &Node,
    source: &str,
    old_module: &str,
    new_module: &'a str,
    replacements: &mut Vec<(usize, usize, &'a str)>,
) {
    match node.kind() {
        "import_clause" => {
            if let Some(module_name_node) = node.child_by_field_name("moduleName")
                && let Ok(text) = module_name_node.utf8_text(source.as_bytes())
                && text == old_module
            {
                replacements.push((
                    module_name_node.start_byte(),
                    module_name_node.end_byte(),
                    new_module,
                ));
            }
            return;
        }
        "module_declaration" => {
            // Skip — module declaration is handled by rename_module_declaration.
            return;
        }
        "value_qid" => {
            // Qualified value like Foo.Bar.baz — check if module prefix matches.
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                let prefix = format!("{old_module}.");
                if text.starts_with(&prefix) {
                    replacements.push((
                        node.start_byte(),
                        node.start_byte() + old_module.len(),
                        new_module,
                    ));
                }
            }
            return;
        }
        "upper_case_qid" => {
            // Qualified type like Foo.Bar.Model — check if module prefix matches.
            // Skip single identifiers (no dots = no module qualification).
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                let prefix = format!("{old_module}.");
                if text.starts_with(&prefix) {
                    replacements.push((
                        node.start_byte(),
                        node.start_byte() + old_module.len(),
                        new_module,
                    ));
                }
            }
            return;
        }
        _ => {}
    }

    // Recurse into children.
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_module_replacements(&child, source, old_module, new_module, replacements);
    }
}

/// Update the module declaration to use a new module name.
/// Preserves the exposing list exactly as-is.
pub fn rename_module_declaration(source: &str, new_name: &str) -> Result<String> {
    let tree = parser::parse(source)?;
    let root = tree.root_node();

    let module_decl = root
        .child_by_field_name("moduleDeclaration")
        .with_context(|| "no module declaration found")?;

    let name_node = module_decl
        .child_by_field_name("name")
        .with_context(|| "module declaration has no name")?;

    let start = name_node.start_byte();
    let end = name_node.end_byte();

    let mut result = source.to_string();
    result.replace_range(start..end, new_name);
    Ok(result)
}

/// Rename a declaration in its defining file.
/// Updates the definition name, type annotation name, all local references,
/// and the module exposing list.
pub fn rename_declaration_in_file(source: &str, old_name: &str, new_name: &str) -> Result<String> {
    let tree = parser::parse(source)?;
    let root = tree.root_node();
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();

    // Collect all identifier nodes matching old_name (skipping strings/comments via AST).
    collect_local_rename_replacements(&root, source, old_name, new_name, &mut replacements);

    // Also update the module exposing list.
    collect_module_exposing_replacements(&root, source, old_name, new_name, &mut replacements);

    if replacements.is_empty() {
        return Ok(source.to_string());
    }

    // Sort by start byte descending so we can apply back-to-front.
    replacements.sort_by(|a, b| b.0.cmp(&a.0));

    let mut result = source.to_string();
    for (start, end, replacement) in &replacements {
        result.replace_range(*start..*end, replacement);
    }
    Ok(result)
}

/// Rename references to a declaration in an importing file.
/// Uses ImportInfo to determine which references to rename (qualified, aliased, bare exposed).
/// Also updates the import exposing list if the name is explicitly exposed.
/// `variant_of_type` is the parent type name when renaming a variant — used to detect
/// bare variant references from `import Foo exposing (Type(..))`.
pub(crate) fn rename_references_in_file(
    source: &str,
    old_name: &str,
    new_name: &str,
    import_info: &crate::refs::ImportInfo,
    variant_of_type: Option<&str>,
) -> String {
    let tree = match parser::parse(source) {
        Ok(t) => t,
        Err(_) => return source.to_string(),
    };

    let root = tree.root_node();
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();

    collect_import_rename_replacements(
        &root,
        source,
        old_name,
        new_name,
        import_info,
        variant_of_type,
        &mut replacements,
    );

    if replacements.is_empty() {
        return source.to_string();
    }

    replacements.sort_by(|a, b| b.0.cmp(&a.0));

    let mut result = source.to_string();
    for (start, end, replacement) in &replacements {
        result.replace_range(*start..*end, replacement);
    }
    result
}

fn collect_local_rename_replacements(
    node: &Node,
    source: &str,
    old_name: &str,
    new_name: &str,
    replacements: &mut Vec<(usize, usize, String)>,
) {
    match node.kind() {
        // Function/value names in definitions and annotations.
        "type_annotation" => {
            // Rename the name field.
            if let Some(name_node) = node.child_by_field_name("name")
                && let Ok(text) = name_node.utf8_text(source.as_bytes())
                && text == old_name
            {
                replacements.push((
                    name_node.start_byte(),
                    name_node.end_byte(),
                    new_name.to_string(),
                ));
            }
            // Also walk the type expression for type references.
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if child.kind() != "lower_case_identifier" {
                    collect_local_rename_replacements(
                        &child,
                        source,
                        old_name,
                        new_name,
                        replacements,
                    );
                }
            }
            return;
        }
        "value_declaration" => {
            // Rename the function name in function_declaration_left.
            if let Some(fdl) = node.child_by_field_name("functionDeclarationLeft") {
                let mut cursor = fdl.walk();
                for child in fdl.named_children(&mut cursor) {
                    if child.kind() == "lower_case_identifier" {
                        if let Ok(text) = child.utf8_text(source.as_bytes())
                            && text == old_name
                        {
                            replacements.push((
                                child.start_byte(),
                                child.end_byte(),
                                new_name.to_string(),
                            ));
                        }
                        break; // Only the first identifier is the function name.
                    }
                }
            }
            // Walk the body for references.
            if let Some(body) = node.child_by_field_name("body") {
                collect_local_rename_replacements(&body, source, old_name, new_name, replacements);
            }
            return;
        }
        "type_declaration" | "type_alias_declaration" => {
            // Rename the type/alias name.
            let name_node = node.child_by_field_name("name");
            if let Some(ref nn) = name_node
                && let Ok(text) = nn.utf8_text(source.as_bytes())
                && text == old_name
            {
                replacements.push((nn.start_byte(), nn.end_byte(), new_name.to_string()));
            }
            // Walk children for variant constructors and type references,
            // but skip the name node (already handled above).
            let name_id = name_node.map(|n| n.id());
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if Some(child.id()) == name_id {
                    continue;
                }
                collect_local_rename_replacements(&child, source, old_name, new_name, replacements);
            }
            return;
        }
        "port_annotation" => {
            // Rename the port name.
            if let Some(name_node) = node.child_by_field_name("name")
                && let Ok(text) = name_node.utf8_text(source.as_bytes())
                && text == old_name
            {
                replacements.push((
                    name_node.start_byte(),
                    name_node.end_byte(),
                    new_name.to_string(),
                ));
            }
            return;
        }
        // Bare value references.
        "value_qid" => {
            if let Ok(text) = node.utf8_text(source.as_bytes())
                && text == old_name
            {
                replacements.push((node.start_byte(), node.end_byte(), new_name.to_string()));
            }
            return;
        }
        // Bare type/variant references.
        "upper_case_qid" => {
            if let Ok(text) = node.utf8_text(source.as_bytes())
                && text == old_name
            {
                replacements.push((node.start_byte(), node.end_byte(), new_name.to_string()));
            }
            return;
        }
        // Bare upper-case identifier (variant constructors in type definitions).
        "upper_case_identifier" => {
            if let Ok(text) = node.utf8_text(source.as_bytes())
                && text == old_name
            {
                replacements.push((node.start_byte(), node.end_byte(), new_name.to_string()));
            }
            return;
        }
        // Constructor pattern in case expressions.
        "union_pattern" => {
            // First child is the constructor name.
            if let Some(first) = node.named_child(0)
                && let Ok(text) = first.utf8_text(source.as_bytes())
                && text == old_name
            {
                replacements.push((first.start_byte(), first.end_byte(), new_name.to_string()));
            }
            // Walk remaining children for nested patterns.
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor).skip(1) {
                collect_local_rename_replacements(&child, source, old_name, new_name, replacements);
            }
            return;
        }
        // Skip module declaration and imports — handled separately.
        "module_declaration" | "import_clause" => return,
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_local_rename_replacements(&child, source, old_name, new_name, replacements);
    }
}

fn collect_module_exposing_replacements(
    root: &Node,
    source: &str,
    old_name: &str,
    new_name: &str,
    replacements: &mut Vec<(usize, usize, String)>,
) {
    let Some(module_decl) = root.child_by_field_name("moduleDeclaration") else {
        return;
    };
    let Some(exposing_list) = module_decl.child_by_field_name("exposing") else {
        return;
    };
    let mut cursor = exposing_list.walk();
    for child in exposing_list.named_children(&mut cursor) {
        match child.kind() {
            "exposed_value" => {
                if let Ok(text) = child.utf8_text(source.as_bytes())
                    && text == old_name
                {
                    replacements.push((child.start_byte(), child.end_byte(), new_name.to_string()));
                }
            }
            "exposed_type" => {
                // Could be "Model" or "Msg(..)".
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    let base = text.split('(').next().unwrap_or(text).trim();
                    if base == old_name {
                        // Replace just the name part, keeping the (..) suffix.
                        let suffix = &text[base.len()..];
                        replacements.push((
                            child.start_byte(),
                            child.end_byte(),
                            format!("{new_name}{suffix}"),
                        ));
                    }
                }
            }
            _ => {}
        }
    }
}

fn collect_import_rename_replacements(
    node: &Node,
    source: &str,
    old_name: &str,
    new_name: &str,
    import_info: &crate::refs::ImportInfo,
    variant_of_type: Option<&str>,
    replacements: &mut Vec<(usize, usize, String)>,
) {
    // A name is "exposed" if it's directly in the exposing list,
    // OR if it's a variant whose parent type is imported with (..).
    let is_exposed = import_info.exposed_names.iter().any(|n| n == old_name)
        || variant_of_type.is_some_and(|parent| {
            import_info
                .exposed_constructors_of
                .iter()
                .any(|t| t == parent)
        });

    match node.kind() {
        "import_clause" => {
            // Check if this is the import for our target module.
            if let Some(module_name_node) = node.child_by_field_name("moduleName")
                && let Ok(module_name) = module_name_node.utf8_text(source.as_bytes())
                && module_name == import_info.module_name
            {
                // Update the exposing list if the name is exposed.
                if let Some(exposing_list) = node.child_by_field_name("exposing") {
                    let mut cursor = exposing_list.walk();
                    for child in exposing_list.named_children(&mut cursor) {
                        match child.kind() {
                            "exposed_value" => {
                                if let Ok(text) = child.utf8_text(source.as_bytes())
                                    && text == old_name
                                {
                                    replacements.push((
                                        child.start_byte(),
                                        child.end_byte(),
                                        new_name.to_string(),
                                    ));
                                }
                            }
                            "exposed_type" => {
                                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                                    let base = text.split('(').next().unwrap_or(text).trim();
                                    if base == old_name {
                                        let suffix = &text[base.len()..];
                                        replacements.push((
                                            child.start_byte(),
                                            child.end_byte(),
                                            format!("{new_name}{suffix}"),
                                        ));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            return;
        }
        "value_qid" | "upper_case_qid" => {
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                let full_prefix = format!("{}.", import_info.module_name);
                let alias_prefix = import_info.alias.as_ref().map(|a| format!("{a}."));

                if let Some(suffix) = text.strip_prefix(&full_prefix)
                    && suffix == old_name
                {
                    // Qualified reference: Module.Name.old -> Module.Name.new
                    let new_text = format!("{}{new_name}", full_prefix);
                    replacements.push((node.start_byte(), node.end_byte(), new_text));
                } else if let Some(ref alias_pfx) = alias_prefix
                    && let Some(suffix) = text.strip_prefix(alias_pfx.as_str())
                    && suffix == old_name
                {
                    // Aliased reference: Alias.old -> Alias.new
                    let new_text = format!("{}{new_name}", alias_pfx);
                    replacements.push((node.start_byte(), node.end_byte(), new_text));
                } else if is_exposed && text == old_name {
                    // Bare exposed reference.
                    replacements.push((node.start_byte(), node.end_byte(), new_name.to_string()));
                }
            }
            return;
        }
        "module_declaration" => return,
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_import_rename_replacements(
            &child,
            source,
            old_name,
            new_name,
            import_info,
            variant_of_type,
            replacements,
        );
    }
}

fn collapse_blank_lines(source: &str) -> Result<String> {
    let mut result = String::new();
    let mut blank_count = 0;
    for line in source.lines() {
        if line.trim().is_empty() {
            blank_count += 1;
            if blank_count <= 2 {
                result.push('\n');
            }
        } else {
            blank_count = 0;
            result.push_str(line);
            result.push('\n');
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validated_write_accepts_valid_source() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Foo.elm");
        let source = "module Foo exposing (bar)\n\nbar : Int\nbar = 1\n";
        validated_write(&path, source, "set").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    }

    #[test]
    fn test_validated_write_rejects_broken_output_and_leaves_file_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Foo.elm");
        let original = "module Foo exposing (bar)\n\nbar = 1\n";
        std::fs::write(&path, original).unwrap();

        let broken = "module Foo exposing (bar)\n\nbar =\n";
        let err = validated_write(&path, broken, "set").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("rejected 'set' write"), "msg: {msg}");
        assert!(msg.contains("Foo.elm"), "msg: {msg}");
        assert!(
            msg.contains(" at "),
            "expected a line:col suffix, got: {msg}"
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            original,
            "target file must be unchanged when validation fails"
        );
    }

    #[test]
    fn test_validated_write_rejects_without_creating_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Nope.elm");
        let broken = "module Nope exposing (..)\n\nfoo =\n    let\n        x = 1\n";
        assert!(validated_write(&path, broken, "patch").is_err());
        assert!(!path.exists(), "path must not be created on rejection");
    }

    #[test]
    fn test_rename_import() {
        let source = "module Main exposing (..)\n\nimport Foo.Bar exposing (baz)\nimport Other\n\nmain = baz\n";
        let result = rename_module_references(source, "Foo.Bar", "Foo.Baz");
        assert!(result.contains("import Foo.Baz exposing (baz)"));
        assert!(result.contains("import Other"));
    }

    #[test]
    fn test_rename_import_with_alias() {
        let source =
            "module Main exposing (..)\n\nimport Foo.Bar as FB exposing (baz)\n\nmain = baz\n";
        let result = rename_module_references(source, "Foo.Bar", "Foo.Baz");
        assert!(result.contains("import Foo.Baz as FB exposing (baz)"));
    }

    #[test]
    fn test_rename_qualified_value() {
        let source =
            "module Main exposing (..)\n\nimport Foo.Bar\n\nmain = Foo.Bar.baz + Foo.Bar.qux\n";
        let result = rename_module_references(source, "Foo.Bar", "Foo.Baz");
        assert!(result.contains("Foo.Baz.baz"));
        assert!(result.contains("Foo.Baz.qux"));
        assert!(!result.contains("Foo.Bar"));
    }

    #[test]
    fn test_rename_qualified_type() {
        let source =
            "module Main exposing (..)\n\nimport Foo.Bar\n\ntype alias Model = Foo.Bar.Model\n";
        let result = rename_module_references(source, "Foo.Bar", "Foo.Baz");
        assert!(result.contains("Foo.Baz.Model"));
        assert!(result.contains("import Foo.Baz"));
    }

    #[test]
    fn test_rename_preserves_unrelated() {
        let source =
            "module Main exposing (..)\n\nimport Other.Module\n\nmain = Other.Module.foo\n";
        let result = rename_module_references(source, "Foo.Bar", "Foo.Baz");
        assert_eq!(result, source);
    }

    #[test]
    fn test_rename_does_not_touch_strings() {
        let source = "module Main exposing (..)\n\nmain = \"Foo.Bar.baz\"\n";
        let result = rename_module_references(source, "Foo.Bar", "Foo.Baz");
        assert!(result.contains("\"Foo.Bar.baz\""));
    }

    #[test]
    fn test_rename_does_not_touch_comments() {
        let source = "module Main exposing (..)\n\n-- See Foo.Bar for details\nmain = 1\n";
        let result = rename_module_references(source, "Foo.Bar", "Foo.Baz");
        assert!(result.contains("-- See Foo.Bar for details"));
    }

    #[test]
    fn test_rename_module_declaration_simple() {
        let source = "module Foo.Bar exposing (baz, Model)\n\nbaz = 1\n";
        let result = rename_module_declaration(source, "Foo.Baz").unwrap();
        assert!(result.starts_with("module Foo.Baz exposing (baz, Model)"));
    }

    #[test]
    fn test_rename_port_module_declaration() {
        let source = "port module Foo.Bar exposing (..)\n\nport send : String -> Cmd msg\n";
        let result = rename_module_declaration(source, "Foo.Baz").unwrap();
        assert!(result.starts_with("port module Foo.Baz exposing (..)"));
    }

    #[test]
    fn test_rename_no_match_returns_unchanged() {
        let source = "module Main exposing (..)\n\nimport Other\n\nmain = 1\n";
        let result = rename_module_references(source, "Foo.Bar", "Foo.Baz");
        assert_eq!(result, source);
    }

    #[test]
    fn test_rename_partial_module_name_no_match() {
        // "Foo.Barista" should NOT match when renaming "Foo.Bar"
        let source = "module Main exposing (..)\n\nimport Foo.Barista\n\nmain = Foo.Barista.make\n";
        let result = rename_module_references(source, "Foo.Bar", "Foo.Baz");
        assert!(result.contains("Foo.Barista"));
        assert!(!result.contains("Foo.Baz"));
    }

    // -- rename_declaration_in_file tests --

    #[test]
    fn test_rename_function_with_annotation() {
        let source = "module Foo exposing (helper)\n\nhelper : String -> String\nhelper name =\n    name\n\nview = helper \"x\"\n";
        let result = rename_declaration_in_file(source, "helper", "formatName").unwrap();
        assert!(result.contains("formatName : String -> String"));
        assert!(result.contains("formatName name ="));
        assert!(result.contains("view = formatName \"x\""));
        assert!(result.contains("exposing (formatName)"));
        assert!(!result.contains("helper"));
    }

    #[test]
    fn test_rename_function_without_annotation() {
        let source = "module Foo exposing (helper)\n\nhelper name = name\n\nview = helper\n";
        let result = rename_declaration_in_file(source, "helper", "formatName").unwrap();
        assert!(result.contains("formatName name = name"));
        assert!(result.contains("view = formatName"));
        assert!(result.contains("exposing (formatName)"));
    }

    #[test]
    fn test_rename_type() {
        let source = "module Foo exposing (Msg(..))\n\ntype Msg = Click | Hover\n\nupdate : Msg -> Msg\nupdate msg = msg\n";
        let result = rename_declaration_in_file(source, "Msg", "Action").unwrap();
        assert!(result.contains("type Action = Click | Hover"));
        assert!(result.contains("update : Action -> Action"));
        assert!(result.contains("exposing (Action(..))"));
        assert!(!result.contains("Msg"));
    }

    #[test]
    fn test_rename_type_alias() {
        let source = "module Foo exposing (Model)\n\ntype alias Model = { name : String }\n\ninit : Model\ninit = { name = \"\" }\n";
        let result = rename_declaration_in_file(source, "Model", "AppModel").unwrap();
        assert!(result.contains("type alias AppModel"));
        assert!(result.contains("init : AppModel"));
        assert!(result.contains("exposing (AppModel)"));
    }

    #[test]
    fn test_rename_port() {
        let source =
            "port module Foo exposing (sendMessage)\n\nport sendMessage : String -> Cmd msg\n";
        let result = rename_declaration_in_file(source, "sendMessage", "sendMsg").unwrap();
        assert!(result.contains("port sendMsg : String -> Cmd msg"));
        assert!(result.contains("exposing (sendMsg)"));
    }

    #[test]
    fn test_rename_variant() {
        let source = "module Foo exposing (Msg(..))\n\ntype Msg = GotResponse String | Other\n\nupdate msg =\n    case msg of\n        GotResponse s -> s\n        Other -> \"\"\n";
        let result = rename_declaration_in_file(source, "GotResponse", "ReceivedResponse").unwrap();
        assert!(result.contains("type Msg = ReceivedResponse String | Other"));
        assert!(result.contains("ReceivedResponse s -> s"));
        assert!(!result.contains("GotResponse"));
    }

    #[test]
    fn test_rename_does_not_touch_strings_or_comments() {
        let source = "module Foo exposing (..)\n\n-- helper is useful\nhelper = \"helper\"\n";
        let result = rename_declaration_in_file(source, "helper", "formatName").unwrap();
        assert!(result.contains("-- helper is useful"));
        assert!(result.contains("\"helper\""));
        assert!(result.contains("formatName ="));
    }

    #[test]
    fn test_rename_unexposed_function() {
        let source = "module Foo exposing (view)\n\nhelper = 1\n\nview = helper\n";
        let result = rename_declaration_in_file(source, "helper", "formatName").unwrap();
        assert!(result.contains("formatName = 1"));
        assert!(result.contains("view = formatName"));
        // exposing list should NOT change.
        assert!(result.contains("exposing (view)"));
    }

    // -- rename_references_in_file tests --

    #[test]
    fn test_rename_ref_qualified() {
        let source = "module Main exposing (..)\n\nimport Lib.Utils\n\nmain = Lib.Utils.helper\n";
        let info = crate::refs::ImportInfo {
            import_line: 3,
            module_name: "Lib.Utils".to_string(),
            alias: None,
            exposed_names: vec![],
            exposed_constructors_of: vec![],
        };
        let result = rename_references_in_file(source, "helper", "formatName", &info, None);
        assert!(result.contains("Lib.Utils.formatName"));
        assert!(!result.contains("Lib.Utils.helper"));
    }

    #[test]
    fn test_rename_ref_aliased() {
        let source = "module Main exposing (..)\n\nimport Lib.Utils as LU\n\nmain = LU.helper\n";
        let info = crate::refs::ImportInfo {
            import_line: 3,
            module_name: "Lib.Utils".to_string(),
            alias: Some("LU".to_string()),
            exposed_names: vec![],
            exposed_constructors_of: vec![],
        };
        let result = rename_references_in_file(source, "helper", "formatName", &info, None);
        assert!(result.contains("LU.formatName"));
        assert!(!result.contains("LU.helper"));
    }

    #[test]
    fn test_rename_ref_exposed_bare() {
        let source = "module Main exposing (..)\n\nimport Lib.Utils exposing (helper)\n\nmain = helper\n\nview = helper\n";
        let info = crate::refs::ImportInfo {
            import_line: 3,
            module_name: "Lib.Utils".to_string(),
            alias: None,
            exposed_names: vec!["helper".to_string()],
            exposed_constructors_of: vec![],
        };
        let result = rename_references_in_file(source, "helper", "formatName", &info, None);
        assert!(result.contains("exposing (formatName)"));
        assert!(result.contains("main = formatName"));
        assert!(result.contains("view = formatName"));
        assert!(!result.contains("helper"));
    }

    #[test]
    fn test_rename_ref_exposed_type() {
        let source = "module Main exposing (..)\n\nimport Lib.Types exposing (Model)\n\ntype alias Page = Model\n";
        let info = crate::refs::ImportInfo {
            import_line: 3,
            module_name: "Lib.Types".to_string(),
            alias: None,
            exposed_names: vec!["Model".to_string()],
            exposed_constructors_of: vec![],
        };
        let result = rename_references_in_file(source, "Model", "AppModel", &info, None);
        assert!(result.contains("exposing (AppModel)"));
        assert!(result.contains("type alias Page = AppModel"));
    }

    #[test]
    fn test_rename_ref_variant_via_type_constructors() {
        let source = "module Main exposing (..)\n\nimport Lib.Types exposing (Msg(..))\n\nupdate msg =\n    case msg of\n        GotResponse s -> s\n        Other -> \"\"\n";
        let info = crate::refs::ImportInfo {
            import_line: 3,
            module_name: "Lib.Types".to_string(),
            alias: None,
            exposed_names: vec!["Msg".to_string()],
            exposed_constructors_of: vec!["Msg".to_string()],
        };
        let result = rename_references_in_file(
            source,
            "GotResponse",
            "ReceivedResponse",
            &info,
            Some("Msg"),
        );
        assert!(result.contains("ReceivedResponse s -> s"));
        assert!(!result.contains("GotResponse"));
        // Msg(..) should remain unchanged — we're renaming the variant, not the type.
        assert!(result.contains("exposing (Msg(..))"));
    }
}
