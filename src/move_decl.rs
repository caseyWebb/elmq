use crate::DeclarationKind;
use crate::imports::{ExposedItem, ImportContext, ModuleImport, is_auto_imported};
use crate::parser;
use crate::project::Project;
use crate::writer;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tree_sitter::Node;

/// Result of a move-declaration operation.
#[derive(Debug, Serialize)]
pub struct MoveResult {
    /// Declarations that were moved.
    pub moved: Vec<String>,
    /// Helpers that were auto-included in the move.
    pub auto_included: Vec<String>,
    /// Helpers that were copied (not moved) due to --copy-shared-helpers.
    pub copied: Vec<String>,
    /// Files that were updated (relative paths).
    pub updated_files: Vec<String>,
    /// Whether this was a dry run.
    pub dry_run: bool,
}

/// Execute a move-declaration operation.
pub fn execute_move_declaration(
    source_file: &Path,
    names: &[String],
    target_file: &Path,
    copy_shared_helpers: bool,
    dry_run: bool,
) -> Result<MoveResult> {
    if names.is_empty() {
        bail!("no declarations specified to move");
    }

    // Parse source file.
    let source_text = std::fs::read_to_string(source_file)
        .with_context(|| format!("could not read {}", source_file.display()))?;
    let source_tree = parser::parse(&source_text)
        .with_context(|| format!("parse error in {}", source_file.display()))?;
    let source_summary = parser::extract_summary(&source_tree, &source_text);
    let source_root = source_tree.root_node();
    let source_ctx = ImportContext::from_tree(&source_root, &source_text);

    // Validate requested names exist and aren't constructors.
    for name in names {
        if source_summary.find_declaration(name).is_none() {
            // Check if it's a constructor.
            if let Some(parent_type) = find_variant_parent_type(&source_root, &source_text, name) {
                bail!("{name} is a constructor of {parent_type}; move {parent_type} instead");
            }
            bail!(
                "declaration '{}' not found in {}",
                name,
                source_file.display()
            );
        }
    }

    // Compute the move set (requested + auto-included helpers).
    let requested: HashSet<String> = names.iter().cloned().collect();
    let (move_set, auto_included, copied) = compute_move_set(
        &source_text,
        &source_summary,
        &requested,
        copy_shared_helpers,
    )?;

    // Discover project.
    let project = Project::discover(source_file)?;
    let source_module = project.module_name(source_file)?;

    // Determine which moved declarations were exposed from the source module.
    let exposed_names: HashSet<String> = move_set
        .iter()
        .filter(|name| is_declaration_exposed(&source_tree, &source_text, name))
        .cloned()
        .collect();

    // Parse or create target file.
    let target_exists = target_file.is_file();
    let (target_text, target_tree, target_module) = if target_exists {
        let text = std::fs::read_to_string(target_file)
            .with_context(|| format!("could not read {}", target_file.display()))?;
        let tree = parser::parse(&text)
            .with_context(|| format!("parse error in {}", target_file.display()))?;
        let module = project.module_name(target_file)?;
        (text, Some(tree), module)
    } else {
        // Create parent dirs.
        if !dry_run && let Some(parent) = target_file.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("could not create directory for {}", target_file.display())
            })?;
        }
        let module = project.module_name(target_file)?;
        (String::new(), None, module)
    };

    let target_ctx = if let Some(ref tree) = target_tree {
        ImportContext::from_tree(&tree.root_node(), &target_text)
    } else {
        ImportContext::empty()
    };

    // Check if any moved declarations are ports and whether target needs port module upgrade.
    let has_ports = move_set.iter().any(|name| {
        source_summary
            .find_declaration(name)
            .is_some_and(|d| d.kind == DeclarationKind::Port)
    });

    // Analyze dependencies of each declaration in move set.
    let mut required_imports: HashMap<String, ModuleImport> = HashMap::new();
    let mut needs_source_import: HashSet<String> = HashSet::new();
    let all_source_decl_names: HashSet<String> = source_summary
        .declarations
        .iter()
        .map(|d| d.name.clone())
        .collect();

    for name in &move_set {
        let decl = source_summary.find_declaration(name).unwrap();
        let source_lines: Vec<&str> = source_text.lines().collect();
        let start = decl.start_line - 1;
        let end = decl.end_line.min(source_lines.len());
        let decl_text = source_lines[start..end].join("\n");

        let decl_tree = match parser::parse(&decl_text) {
            Ok(t) => t,
            Err(_) => continue,
        };

        collect_dependencies(
            &decl_tree.root_node(),
            &decl_text,
            &source_ctx,
            &all_source_decl_names,
            &move_set,
            &mut required_imports,
            &mut needs_source_import,
        );
    }

    // Build the new target ImportContext with merged imports.
    let mut new_target_ctx = target_ctx.clone();
    for (module, import) in &required_imports {
        new_target_ctx.ensure_import(module, import);
    }

    // If any declarations staying in source are needed, add import for source module.
    if !needs_source_import.is_empty() {
        let exposed_items: Vec<ExposedItem> = needs_source_import
            .iter()
            .map(|n| {
                let decl = source_summary.find_declaration(n);
                match decl.map(|d| d.kind) {
                    Some(DeclarationKind::Type) => ExposedItem::TypeWithConstructors(n.clone()),
                    Some(DeclarationKind::TypeAlias) => ExposedItem::Type(n.clone()),
                    _ => ExposedItem::Value(n.clone()),
                }
            })
            .collect();
        let exposing_str = exposed_items
            .iter()
            .map(|item| match item {
                ExposedItem::Value(n) => n.clone(),
                ExposedItem::Type(n) => n.clone(),
                ExposedItem::TypeWithConstructors(n) => format!("{n}(..)"),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let raw_line = format!("import {source_module} exposing ({exposing_str})");
        let import = ModuleImport {
            module_name: source_module.clone(),
            alias: None,
            exposed: exposed_items,
            has_exposing_all: false,
            raw_line,
        };
        new_target_ctx.ensure_import(&source_module, &import);
    }

    // Rewrite declaration bodies for target import context.
    let mut rewritten_decls: Vec<(String, String)> = Vec::new(); // (name, rewritten_source)
    for name in &move_set {
        let decl = source_summary.find_declaration(name).unwrap();
        let source_lines: Vec<&str> = source_text.lines().collect();
        let start = decl.start_line - 1;
        let end = decl.end_line.min(source_lines.len());
        let decl_text = source_lines[start..end].join("\n");

        let rewritten = rewrite_declaration_body(
            &decl_text,
            &source_ctx,
            &new_target_ctx,
            &all_source_decl_names,
            &move_set,
            &source_module,
        );
        rewritten_decls.push((name.clone(), rewritten));
    }

    // Also rewrite copied helpers.
    let mut copied_decls: Vec<(String, String)> = Vec::new();
    for name in &copied {
        let decl = source_summary.find_declaration(name).unwrap();
        let source_lines: Vec<&str> = source_text.lines().collect();
        let start = decl.start_line - 1;
        let end = decl.end_line.min(source_lines.len());
        let decl_text = source_lines[start..end].join("\n");

        let rewritten = rewrite_declaration_body(
            &decl_text,
            &source_ctx,
            &new_target_ctx,
            &all_source_decl_names,
            &move_set,
            &source_module,
        );
        copied_decls.push((name.clone(), rewritten));
    }

    // Build the new target file content.
    let new_target_text = if target_exists {
        build_updated_target(
            &target_text,
            &rewritten_decls,
            &copied_decls,
            &exposed_names,
            &new_target_ctx,
            has_ports,
        )?
    } else {
        build_new_target(
            &target_module,
            &rewritten_decls,
            &copied_decls,
            &exposed_names,
            &new_target_ctx,
            has_ports,
        )
    };

    // Build new source file content: remove moved declarations, update exposing list.
    let decls_to_remove: HashSet<&str> = move_set.iter().map(|s| s.as_str()).collect();
    let new_source_text = build_updated_source(&source_text, &source_summary, &decls_to_remove)?;

    // Scan project for files that reference moved declarations and rewrite them.
    let elm_files = project.elm_files()?;
    let source_canonical = source_file
        .canonicalize()
        .with_context(|| format!("could not canonicalize {}", source_file.display()))?;
    let target_canonical = if target_exists {
        target_file.canonicalize().ok()
    } else {
        None
    };

    let mut file_updates: Vec<(PathBuf, String)> = Vec::new();
    let mut updated_files: Vec<String> = Vec::new();

    for elm_file in &elm_files {
        let Ok(elm_canonical) = elm_file.canonicalize() else {
            continue;
        };
        if elm_canonical == source_canonical {
            continue;
        }
        if target_canonical
            .as_ref()
            .is_some_and(|tc| &elm_canonical == tc)
        {
            continue;
        }

        let file_source = std::fs::read_to_string(elm_file)
            .with_context(|| format!("could not read {}", elm_file.display()))?;
        let file_tree = match parser::parse(&file_source) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let file_root = file_tree.root_node();
        let file_ctx = ImportContext::from_tree(&file_root, &file_source);

        // Check if this file imports the source module.
        let Some(source_import) = file_ctx.get(&source_module) else {
            continue;
        };

        // Skip files using exposing (..) for the source module.
        if source_import.has_exposing_all {
            continue;
        }

        let updated =
            rewrite_importing_file(&file_source, &source_module, &target_module, &move_set);

        if updated != file_source {
            let display = display_path(elm_file.strip_prefix(&project.root).unwrap_or(elm_file));
            updated_files.push(display);
            file_updates.push((elm_file.clone(), updated));
        }
    }

    // Write everything atomically.
    if !dry_run {
        writer::atomic_write(source_file, &new_source_text)?;
        writer::atomic_write(target_file, &new_target_text)?;
        for (path, content) in &file_updates {
            writer::atomic_write(path, content)?;
        }
    }

    // Add source and target to updated files.
    let source_display = display_path(
        source_file
            .strip_prefix(&project.root)
            .unwrap_or(source_file),
    );
    let target_display = display_path(
        target_file
            .strip_prefix(&project.root)
            .unwrap_or(target_file),
    );
    updated_files.insert(0, target_display);
    updated_files.insert(0, source_display);

    Ok(MoveResult {
        moved: move_set.iter().cloned().collect(),
        auto_included,
        copied,
        updated_files,
        dry_run,
    })
}

/// Compute the full move set: requested declarations + auto-included unexposed helpers.
/// Returns (move_set, auto_included_names, copied_names).
fn compute_move_set(
    source: &str,
    summary: &crate::FileSummary,
    requested: &HashSet<String>,
    copy_shared_helpers: bool,
) -> Result<(HashSet<String>, Vec<String>, Vec<String>)> {
    let tree = parser::parse(source)?;

    // Determine which declarations are exposed from the module.
    let exposed: HashSet<String> = summary
        .declarations
        .iter()
        .filter(|d| is_declaration_exposed(&tree, source, &d.name))
        .map(|d| d.name.clone())
        .collect();

    // For each declaration, find which other local declarations it references.
    let mut local_refs: HashMap<String, HashSet<String>> = HashMap::new();
    for decl in &summary.declarations {
        let lines: Vec<&str> = source.lines().collect();
        let start = decl.start_line - 1;
        let end = decl.end_line.min(lines.len());
        let decl_text = lines[start..end].join("\n");

        let decl_tree = match parser::parse(&decl_text) {
            Ok(t) => t,
            Err(_) => continue,
        };

        let mut refs = HashSet::new();
        collect_local_refs(&decl_tree.root_node(), &decl_text, &mut refs);

        // Filter to only names that are actually declarations in this file.
        let all_decl_names: HashSet<&str> = summary
            .declarations
            .iter()
            .map(|d| d.name.as_str())
            .collect();
        refs.retain(|r| all_decl_names.contains(r.as_str()) && r != &decl.name);

        local_refs.insert(decl.name.clone(), refs);
    }

    let mut move_set = requested.clone();
    let mut auto_included = Vec::new();
    let mut copied = Vec::new();

    // Iterate to find unexposed helpers used only by move set.
    loop {
        let mut added = false;
        for decl in &summary.declarations {
            if move_set.contains(&decl.name) || exposed.contains(&decl.name) {
                continue;
            }
            // This is an unexposed declaration. Check if it's referenced by any move set member.
            let referenced_by_move_set = move_set.iter().any(|m| {
                local_refs
                    .get(m)
                    .is_some_and(|refs| refs.contains(&decl.name))
            });

            if !referenced_by_move_set {
                continue;
            }

            // Check if also referenced by non-move-set declarations.
            let referenced_by_others = summary
                .declarations
                .iter()
                .filter(|d| !move_set.contains(&d.name) && d.name != decl.name)
                .any(|d| {
                    local_refs
                        .get(&d.name)
                        .is_some_and(|refs| refs.contains(&decl.name))
                });

            if referenced_by_others {
                if copy_shared_helpers {
                    copied.push(decl.name.clone());
                } else {
                    bail!(
                        "'{}' is used by both moved and non-moved declarations; use --copy-shared-helpers to duplicate it",
                        decl.name
                    );
                }
            } else {
                move_set.insert(decl.name.clone());
                auto_included.push(decl.name.clone());
                added = true;
            }
        }
        if !added {
            break;
        }
    }

    Ok((move_set, auto_included, copied))
}

/// Collect bare identifiers referenced in a declaration body (for local reference detection).
fn collect_local_refs(node: &Node, source: &str, refs: &mut HashSet<String>) {
    match node.kind() {
        "value_qid" => {
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                // Only bare names (no dots) are local refs.
                if !text.contains('.') {
                    refs.insert(text.to_string());
                }
            }
            return;
        }
        "upper_case_qid" => {
            if let Ok(text) = node.utf8_text(source.as_bytes())
                && !text.contains('.')
            {
                refs.insert(text.to_string());
            }
            return;
        }
        "module_declaration" | "import_clause" | "type_annotation" => return,
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_local_refs(&child, source, refs);
    }
}

/// Collect import dependencies from a declaration body.
fn collect_dependencies(
    root: &Node,
    source: &str,
    source_ctx: &ImportContext,
    all_source_decl_names: &HashSet<String>,
    move_set: &HashSet<String>,
    required_imports: &mut HashMap<String, ModuleImport>,
    needs_source_import: &mut HashSet<String>,
) {
    collect_deps_recursive(
        root,
        source,
        source_ctx,
        all_source_decl_names,
        move_set,
        required_imports,
        needs_source_import,
    );
}

#[allow(clippy::too_many_arguments)]
fn collect_deps_recursive(
    node: &Node,
    source: &str,
    source_ctx: &ImportContext,
    all_source_decl_names: &HashSet<String>,
    move_set: &HashSet<String>,
    required_imports: &mut HashMap<String, ModuleImport>,
    needs_source_import: &mut HashSet<String>,
) {
    match node.kind() {
        "value_qid" | "upper_case_qid" => {
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                if let Some(dot_pos) = text.rfind('.') {
                    // Qualified reference.
                    let prefix = &text[..dot_pos];
                    if let Some(canonical) = source_ctx.resolve_prefix(prefix)
                        && !is_auto_imported(canonical)
                        && let Some(imp) = source_ctx.get(canonical)
                    {
                        required_imports
                            .entry(canonical.to_string())
                            .or_insert_with(|| imp.clone());
                    }
                } else {
                    // Bare reference — could be local or from an exposed import.
                    if all_source_decl_names.contains(text) {
                        // Local reference. If it's not in the move set, we need an import.
                        if !move_set.contains(text) {
                            needs_source_import.insert(text.to_string());
                        }
                    } else if let Some(canonical) = source_ctx.resolve_bare(text)
                        && !is_auto_imported(canonical)
                        && let Some(imp) = source_ctx.get(canonical)
                    {
                        required_imports
                            .entry(canonical.to_string())
                            .or_insert_with(|| imp.clone());
                    }
                }
            }
            return;
        }
        "module_declaration" | "import_clause" => return,
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_deps_recursive(
            &child,
            source,
            source_ctx,
            all_source_decl_names,
            move_set,
            required_imports,
            needs_source_import,
        );
    }
}

/// Rewrite a declaration body from source import context to target import context.
fn rewrite_declaration_body(
    decl_text: &str,
    source_ctx: &ImportContext,
    target_ctx: &ImportContext,
    all_source_decl_names: &HashSet<String>,
    move_set: &HashSet<String>,
    source_module: &str,
) -> String {
    let tree = match parser::parse(decl_text) {
        Ok(t) => t,
        Err(_) => return decl_text.to_string(),
    };

    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    collect_body_replacements(
        &tree.root_node(),
        decl_text,
        source_ctx,
        target_ctx,
        all_source_decl_names,
        move_set,
        source_module,
        &mut replacements,
    );

    if replacements.is_empty() {
        return decl_text.to_string();
    }

    // Sort by start byte descending so we can apply back-to-front.
    replacements.sort_by(|a, b| b.0.cmp(&a.0));

    let mut result = decl_text.to_string();
    for (start, end, replacement) in &replacements {
        result.replace_range(*start..*end, replacement);
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn collect_body_replacements(
    node: &Node,
    source: &str,
    source_ctx: &ImportContext,
    target_ctx: &ImportContext,
    all_source_decl_names: &HashSet<String>,
    move_set: &HashSet<String>,
    source_module: &str,
    replacements: &mut Vec<(usize, usize, String)>,
) {
    match node.kind() {
        "value_qid" | "upper_case_qid" => {
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                if let Some(dot_pos) = text.rfind('.') {
                    // Qualified reference.
                    let prefix = &text[..dot_pos];
                    let name = &text[dot_pos + 1..];
                    if let Some(canonical) = source_ctx.resolve_prefix(prefix) {
                        let new_ref = target_ctx.emit_ref(canonical, name);
                        if new_ref != text {
                            replacements.push((node.start_byte(), node.end_byte(), new_ref));
                        }
                    }
                } else {
                    // Bare reference.
                    if all_source_decl_names.contains(text) && !move_set.contains(text) {
                        // Reference to a declaration staying in source.
                        let new_ref = target_ctx.emit_ref(source_module, text);
                        if new_ref != text {
                            replacements.push((node.start_byte(), node.end_byte(), new_ref));
                        }
                    } else if !all_source_decl_names.contains(text) {
                        // Bare reference from an exposed import.
                        if let Some(canonical) = source_ctx.resolve_bare(text) {
                            let new_ref = target_ctx.emit_ref(canonical, text);
                            if new_ref != text {
                                replacements.push((node.start_byte(), node.end_byte(), new_ref));
                            }
                        }
                    }
                }
            }
            return;
        }
        "module_declaration" | "import_clause" => return,
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_body_replacements(
            &child,
            source,
            source_ctx,
            target_ctx,
            all_source_decl_names,
            move_set,
            source_module,
            replacements,
        );
    }
}

/// Build the updated source file content after removing moved declarations.
fn build_updated_source(
    source: &str,
    summary: &crate::FileSummary,
    decls_to_remove: &HashSet<&str>,
) -> Result<String> {
    let mut result = source.to_string();

    // Remove declarations in reverse order (by line number) to preserve positions.
    let mut to_remove: Vec<_> = summary
        .declarations
        .iter()
        .filter(|d| decls_to_remove.contains(d.name.as_str()))
        .collect();
    to_remove.sort_by(|a, b| b.start_line.cmp(&a.start_line));

    for decl in to_remove {
        let temp_summary = {
            let tree = parser::parse(&result)?;
            parser::extract_summary(&tree, &result)
        };
        result = writer::remove_declaration(&result, &temp_summary, &decl.name)?;
    }

    // Update exposing list: remove moved names.
    for name in decls_to_remove {
        let tree = parser::parse(&result)?;
        let summary = parser::extract_summary(&tree, &result);
        // Only try to unexpose if it's currently exposed.
        if is_declaration_exposed(&tree, &result, name) {
            result = writer::unexpose(&result, &summary, name).unwrap_or(result);
        }
    }

    Ok(result)
}

/// Build a new target file from scratch.
fn build_new_target(
    module_name: &str,
    moved_decls: &[(String, String)],
    copied_decls: &[(String, String)],
    exposed_names: &HashSet<String>,
    target_ctx: &ImportContext,
    has_ports: bool,
) -> String {
    let prefix = if has_ports { "port module" } else { "module" };

    // Exposing list: only expose declarations that were exposed in source.
    let expose_list: Vec<&str> = moved_decls
        .iter()
        .filter(|(name, _)| exposed_names.contains(name))
        .map(|(name, _)| name.as_str())
        .collect();

    let exposing = if expose_list.is_empty() {
        "(..)".to_string()
    } else {
        format!("({})", expose_list.join(", "))
    };

    let mut result = format!("{prefix} {module_name} exposing {exposing}\n");

    let imports = target_ctx.render_imports();
    if !imports.is_empty() {
        result.push('\n');
        result.push_str(&imports);
        result.push('\n');
    }

    for (_, decl_source) in moved_decls {
        result.push_str("\n\n");
        result.push_str(decl_source);
        result.push('\n');
    }
    for (_, decl_source) in copied_decls {
        result.push_str("\n\n");
        result.push_str(decl_source);
        result.push('\n');
    }

    result
}

/// Build updated target file with moved declarations inserted.
fn build_updated_target(
    target_text: &str,
    moved_decls: &[(String, String)],
    copied_decls: &[(String, String)],
    exposed_names: &HashSet<String>,
    target_ctx: &ImportContext,
    has_ports: bool,
) -> Result<String> {
    let mut result = target_text.to_string();

    // Replace the import block with the merged version.
    let new_imports = target_ctx.render_imports();

    // Find the import range in the current file.
    let lines: Vec<&str> = result.lines().collect();
    let mut import_start = None;
    let mut import_end = 0;
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("import ") {
            if import_start.is_none() {
                import_start = Some(i);
            }
            import_end = i + 1;
        }
    }

    if let Some(start) = import_start {
        // Replace existing import block.
        let before = lines[..start].join("\n");
        let after = lines[import_end..].join("\n");
        result = format!("{before}\n{new_imports}\n{after}");
    } else if !new_imports.is_empty() {
        // No imports yet — insert after module line.
        let tree = parser::parse(&result)?;
        let summary = parser::extract_summary(&tree, &result);
        // Use add_import for the first import to position correctly.
        result = writer::add_import(&result, &summary, new_imports.lines().next().unwrap_or(""));
        for line in new_imports.lines().skip(1) {
            let tree = parser::parse(&result)?;
            let summary = parser::extract_summary(&tree, &result);
            result = writer::add_import(&result, &summary, line);
        }
    }

    // Append declarations using their actual names (upsert will append since they don't exist yet).
    for (name, decl_source) in moved_decls {
        let tree = parser::parse(&result)?;
        let summary = parser::extract_summary(&tree, &result);
        result = writer::upsert_declaration(&result, &summary, name, decl_source);
    }
    for (name, decl_source) in copied_decls {
        let tree = parser::parse(&result)?;
        let summary = parser::extract_summary(&tree, &result);
        result = writer::upsert_declaration(&result, &summary, name, decl_source);
    }

    // Update exposing list.
    for (name, _) in moved_decls {
        if exposed_names.contains(name) {
            let tree = parser::parse(&result)?;
            let summary = parser::extract_summary(&tree, &result);
            result = writer::expose(&result, &summary, name)?;
        }
    }

    // Upgrade to port module if needed.
    if has_ports && !result.starts_with("port module") {
        result = result.replacen("module ", "port module ", 1);
    }

    Ok(result)
}

/// Rewrite a file that imports the source module to reference the target module for moved declarations.
fn rewrite_importing_file(
    file_source: &str,
    source_module: &str,
    target_module: &str,
    move_set: &HashSet<String>,
) -> String {
    let tree = match parser::parse(file_source) {
        Ok(t) => t,
        Err(_) => return file_source.to_string(),
    };
    let root = tree.root_node();
    let ctx = ImportContext::from_tree(&root, file_source);

    let Some(source_import) = ctx.get(source_module) else {
        return file_source.to_string();
    };

    // Determine which moved names are referenced in this file.
    let mut moved_names_used: HashSet<String> = HashSet::new();
    let mut has_qualified_source_refs = false;

    // Check exposed names from import.
    for item in &source_import.exposed {
        if move_set.contains(item.name()) {
            moved_names_used.insert(item.name().to_string());
        }
    }

    // Check for qualified references in the body.
    collect_qualified_moved_refs(
        &root,
        file_source,
        source_module,
        source_import,
        move_set,
        &mut moved_names_used,
        &mut has_qualified_source_refs,
    );

    if moved_names_used.is_empty() {
        return file_source.to_string();
    }

    let mut result = file_source.to_string();

    // Rewrite qualified references: Source.funcA → Target.funcA.
    let alias = source_import.alias.as_deref();
    let prefix_full = format!("{source_module}.");
    let prefix_alias = alias.map(|a| format!("{a}."));

    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    collect_module_ref_replacements(
        &root,
        file_source,
        &prefix_full,
        prefix_alias.as_deref(),
        move_set,
        target_module,
        &mut replacements,
    );

    if !replacements.is_empty() {
        replacements.sort_by(|a, b| b.0.cmp(&a.0));
        for (start, end, replacement) in &replacements {
            result.replace_range(*start..*end, replacement);
        }
    }

    // Update import lines.
    // Remove moved names from source import's exposing list.
    for name in &moved_names_used {
        if source_import.exposed.iter().any(|item| item.name() == name) {
            result = remove_name_from_import_exposing(&result, source_module, name);
        }
    }

    // Add import for target module with moved names.
    let exposing_items: Vec<String> = moved_names_used
        .iter()
        .filter(|n| {
            source_import
                .exposed
                .iter()
                .any(|item| item.name() == n.as_str())
        })
        .cloned()
        .collect();

    if !exposing_items.is_empty() {
        let import_line = format!(
            "import {target_module} exposing ({})",
            exposing_items.join(", ")
        );
        let tree = parser::parse(&result).unwrap_or_else(|_| parser::parse(file_source).unwrap());
        let summary = parser::extract_summary(&tree, &result);
        result = writer::add_import(&result, &summary, &import_line);
    } else {
        // Only qualified references — just add a plain import if not already present.
        let tree = parser::parse(&result).unwrap_or_else(|_| parser::parse(file_source).unwrap());
        let ctx = ImportContext::from_tree(&tree.root_node(), &result);
        if ctx.get(target_module).is_none() {
            let summary = parser::extract_summary(&tree, &result);
            result = writer::add_import(&result, &summary, &format!("import {target_module}"));
        }
    }

    // Clean up: if source import has no more exposed names and no remaining qualified refs,
    // remove the import entirely.
    let tree = match parser::parse(&result) {
        Ok(t) => t,
        Err(_) => return result,
    };
    let new_ctx = ImportContext::from_tree(&tree.root_node(), &result);
    if let Some(remaining_import) = new_ctx.get(source_module) {
        // Check if any remaining content uses this import.
        let has_remaining_refs =
            has_any_refs_to_module(&tree.root_node(), &result, source_module, remaining_import);
        if !has_remaining_refs {
            result = writer::remove_import(&result, source_module).unwrap_or(result);
        }
    }

    result
}

fn collect_qualified_moved_refs(
    node: &Node,
    source: &str,
    source_module: &str,
    source_import: &ModuleImport,
    move_set: &HashSet<String>,
    moved_names_used: &mut HashSet<String>,
    has_qualified: &mut bool,
) {
    match node.kind() {
        "value_qid" | "upper_case_qid" => {
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                let full_prefix = format!("{source_module}.");
                let alias_prefix = source_import.alias.as_ref().map(|a| format!("{a}."));

                let name = if let Some(suffix) = text.strip_prefix(&full_prefix) {
                    Some(suffix)
                } else if let Some(ref alias_pfx) = alias_prefix {
                    text.strip_prefix(alias_pfx.as_str())
                } else {
                    None
                };

                if let Some(name) = name {
                    if move_set.contains(name) {
                        moved_names_used.insert(name.to_string());
                    }
                    *has_qualified = true;
                }
            }
            return;
        }
        "module_declaration" | "import_clause" => return,
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_qualified_moved_refs(
            &child,
            source,
            source_module,
            source_import,
            move_set,
            moved_names_used,
            has_qualified,
        );
    }
}

fn collect_module_ref_replacements(
    node: &Node,
    source: &str,
    prefix_full: &str,
    prefix_alias: Option<&str>,
    move_set: &HashSet<String>,
    target_module: &str,
    replacements: &mut Vec<(usize, usize, String)>,
) {
    match node.kind() {
        "value_qid" | "upper_case_qid" => {
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                let name = if let Some(suffix) = text.strip_prefix(prefix_full) {
                    Some(suffix)
                } else if let Some(alias_pfx) = prefix_alias {
                    text.strip_prefix(alias_pfx)
                } else {
                    None
                };

                if let Some(name) = name
                    && move_set.contains(name)
                {
                    let new_ref = format!("{target_module}.{name}");
                    replacements.push((node.start_byte(), node.end_byte(), new_ref));
                }
            }
            return;
        }
        "module_declaration" | "import_clause" => return,
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_module_ref_replacements(
            &child,
            source,
            prefix_full,
            prefix_alias,
            move_set,
            target_module,
            replacements,
        );
    }
}

/// Remove a specific name from an import's exposing list via text manipulation.
fn remove_name_from_import_exposing(source: &str, module_name: &str, name: &str) -> String {
    let import_prefix = format!("import {module_name} ");
    let import_exact = format!("import {module_name}\n");
    let mut result = String::new();
    for line in source.lines() {
        let is_target_import = line.starts_with(&import_prefix)
            || line == format!("import {module_name}")
            || format!("{line}\n") == import_exact;
        if is_target_import
            && line.contains("exposing")
            && let Some(exp_start) = line.find("exposing")
        {
            let after = &line[exp_start + "exposing".len()..];
            if let (Some(open), Some(close)) = (after.find('('), after.rfind(')')) {
                let content = &after[open + 1..close];
                let items: Vec<&str> = content
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| {
                        let base = s.split('(').next().unwrap_or(s).trim();
                        base != name
                    })
                    .collect();
                if items.is_empty() {
                    // No more exposed names — simplify to just the import.
                    let import_part = &line[..exp_start].trim_end();
                    result.push_str(import_part);
                } else {
                    let prefix = &line[..exp_start];
                    result.push_str(&format!("{}exposing ({})", prefix, items.join(", ")));
                }
                result.push('\n');
                continue;
            }
        }
        result.push_str(line);
        result.push('\n');
    }
    result
}

/// Check if any references to a module remain in the file body (outside import/module declarations).
fn has_any_refs_to_module(
    root: &Node,
    source: &str,
    module_name: &str,
    import: &ModuleImport,
) -> bool {
    has_refs_recursive(root, source, module_name, import)
}

fn has_refs_recursive(node: &Node, source: &str, module_name: &str, import: &ModuleImport) -> bool {
    match node.kind() {
        "value_qid" | "upper_case_qid" => {
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                let full_prefix = format!("{module_name}.");
                if text.starts_with(&full_prefix) {
                    return true;
                }
                if let Some(ref alias) = import.alias {
                    let alias_prefix = format!("{alias}.");
                    if text.starts_with(&alias_prefix) {
                        return true;
                    }
                }
                // Check bare exposed names.
                if !text.contains('.') {
                    for item in &import.exposed {
                        if item.name() == text {
                            return true;
                        }
                    }
                }
            }
            return false;
        }
        "module_declaration" | "import_clause" => return false,
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if has_refs_recursive(&child, source, module_name, import) {
            return true;
        }
    }
    false
}

/// Check if a declaration name is exposed from the module.
fn is_declaration_exposed(tree: &tree_sitter::Tree, source: &str, name: &str) -> bool {
    let root = tree.root_node();
    let Some(module_decl) = root.child_by_field_name("moduleDeclaration") else {
        return false;
    };
    let Some(exposing_list) = module_decl.child_by_field_name("exposing") else {
        return false;
    };
    let mut cursor = exposing_list.walk();
    for child in exposing_list.named_children(&mut cursor) {
        match child.kind() {
            "double_dot" => return true,
            "exposed_value" => {
                if let Ok(text) = child.utf8_text(source.as_bytes())
                    && text == name
                {
                    return true;
                }
            }
            "exposed_type" => {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    let base = text.split('(').next().unwrap_or(text).trim();
                    if base == name {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

/// Find the parent type name of a variant.
fn find_variant_parent_type(root: &Node, source: &str, variant_name: &str) -> Option<String> {
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() != "type_declaration" {
            continue;
        }
        let mut inner = child.walk();
        for descendant in child.named_children(&mut inner) {
            if descendant.kind() == "union_variant"
                && let Some(first) = descendant.named_child(0)
                && let Ok(text) = first.utf8_text(source.as_bytes())
                && text == variant_name
                && let Some(name_node) = child.child_by_field_name("name")
                && let Ok(type_name) = name_node.utf8_text(source.as_bytes())
            {
                return Some(type_name.to_string());
            }
        }
    }
    None
}

fn display_path(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}
