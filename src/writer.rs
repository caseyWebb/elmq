use crate::FileSummary;
use anyhow::{Context, Result, bail};
use std::path::Path;

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

/// Remove an import by module name.
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
        bail!("import '{name}' not found");
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
        bail!("'{item}' is not in the exposing list");
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
fn find_module_declaration(source: &str) -> Result<String> {
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

fn extract_exposing_content(module_decl: &str) -> Result<String> {
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
