use crate::writer;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Represents an Elm project discovered via elm.json.
#[derive(Debug)]
pub struct Project {
    /// Directory containing elm.json.
    pub root: PathBuf,
    /// Absolute paths to source directories.
    pub source_dirs: Vec<PathBuf>,
}

#[derive(serde::Deserialize)]
struct ElmJson {
    #[serde(rename = "source-directories")]
    source_directories: Vec<String>,
}

impl Project {
    /// Discover a project by walking up from `start` looking for elm.json.
    /// Returns `Ok(Some(project))` if an ancestor `elm.json` is found,
    /// `Ok(None)` if none is found anywhere up the tree, or `Err` on I/O
    /// errors or a malformed `elm.json`.
    ///
    /// Unlike [`Project::discover`], this does not treat a missing `elm.json`
    /// as an error — callers can fall back to CWD-rooted walking when this
    /// returns `None`.
    pub fn try_discover(start: &Path) -> Result<Option<Self>> {
        let start = start
            .canonicalize()
            .with_context(|| format!("could not resolve path: {}", start.display()))?;

        let mut dir = if start.is_file() {
            start.parent().unwrap_or(&start).to_path_buf()
        } else {
            start.clone()
        };

        loop {
            let candidate = dir.join("elm.json");
            if candidate.is_file() {
                return Self::from_elm_json(&candidate).map(Some);
            }
            if !dir.pop() {
                return Ok(None);
            }
        }
    }

    /// Discover a project by walking up from `start` looking for elm.json.
    pub fn discover(start: &Path) -> Result<Self> {
        let start = start
            .canonicalize()
            .with_context(|| format!("could not resolve path: {}", start.display()))?;

        let mut dir = if start.is_file() {
            start.parent().unwrap_or(&start).to_path_buf()
        } else {
            start.clone()
        };

        loop {
            let candidate = dir.join("elm.json");
            if candidate.is_file() {
                return Self::from_elm_json(&candidate);
            }
            if !dir.pop() {
                bail!(
                    "no elm.json found in {} or any parent directory",
                    start.display()
                );
            }
        }
    }

    fn from_elm_json(elm_json_path: &Path) -> Result<Self> {
        let root = elm_json_path
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();

        let content = std::fs::read_to_string(elm_json_path)
            .with_context(|| format!("could not read {}", elm_json_path.display()))?;

        let elm_json: ElmJson = serde_json::from_str(&content)
            .with_context(|| format!("could not parse {}", elm_json_path.display()))?;

        let mut source_dirs = Vec::new();
        for dir in &elm_json.source_directories {
            let abs = root.join(dir);
            let canonical = abs.canonicalize().with_context(|| {
                format!(
                    "source-directory \"{}\" does not exist (resolved to {})",
                    dir,
                    abs.display()
                )
            })?;
            source_dirs.push(canonical);
        }

        // Sort by path length descending for longest-prefix matching.
        source_dirs.sort_by_key(|b| std::cmp::Reverse(b.as_os_str().len()));

        Ok(Project { root, source_dirs })
    }

    /// Find which source directory contains the given file path.
    /// Uses longest-prefix match (source_dirs are pre-sorted by length descending).
    pub fn source_dir_for(&self, file: &Path) -> Result<&Path> {
        for sd in &self.source_dirs {
            if file.starts_with(sd) {
                return Ok(sd);
            }
        }
        bail!("file {} is not under any source-directory", file.display())
    }

    /// Derive the Elm module name from a file path.
    /// e.g., `src/Foo/Bar.elm` with source-dir `src` -> `Foo.Bar`
    ///
    /// Works on paths that don't exist yet (needed for mv target).
    pub fn module_name(&self, file: &Path) -> Result<String> {
        // Canonicalize the parent directory (which should exist) and append the filename.
        let resolved = if let Ok(canonical) = file.canonicalize() {
            canonical
        } else {
            // File doesn't exist yet — canonicalize parent and append filename.
            let parent = file
                .parent()
                .with_context(|| format!("invalid path: {}", file.display()))?;
            let canonical_parent = parent.canonicalize().with_context(|| {
                format!("parent directory does not exist: {}", parent.display())
            })?;
            canonical_parent.join(
                file.file_name()
                    .with_context(|| format!("path has no filename: {}", file.display()))?,
            )
        };

        let source_dir = self.source_dir_for(&resolved)?;
        let relative = resolved.strip_prefix(source_dir).with_context(|| {
            format!(
                "could not strip source-dir prefix from {}",
                resolved.display()
            )
        })?;

        let stem = relative.with_extension("");
        let module_name: String = stem
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(".");

        if module_name.is_empty() {
            bail!("could not derive module name from {}", file.display());
        }

        Ok(module_name)
    }

    /// Resolve a file path to its Elm module name, returning `None` when no
    /// `elm.json` is discoverable. Convenience wrapper used by multi-file `get`
    /// and `grep --source` for output framing.
    pub fn resolve_module_for_file(file: &Path) -> Option<String> {
        let project = Self::try_discover(file).ok()??;
        project.module_name(file).ok()
    }

    /// List all .elm files across all source directories.
    pub fn elm_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        for sd in &self.source_dirs {
            for entry in WalkDir::new(sd)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.is_file() && path.extension().is_some_and(|ext| ext == "elm") {
                    files.push(path.to_path_buf());
                }
            }
        }
        Ok(files)
    }
}

/// Result of an `mv` operation.
#[derive(Debug)]
pub struct MvResult {
    /// Display path of the old file (relative to project root).
    pub old_display: String,
    /// Display path of the new file (relative to project root).
    pub new_display: String,
    /// Display paths of files that were updated (relative to project root).
    pub updated_files: Vec<String>,
}

/// Execute a module rename: move file, update module declaration, rewrite all
/// imports and qualified references across the project.
///
/// Both `old_path` and `resolved_new` must be absolute, canonicalized paths.
/// The caller is responsible for validating that these paths are in allowed locations.
/// Parent directories for `resolved_new` will be created if `dry_run` is false.
pub fn execute_mv(old_path: &Path, resolved_new: &Path, dry_run: bool) -> Result<MvResult> {
    if !old_path.is_file() {
        bail!("not a file: {}", old_path.display());
    }
    if resolved_new.exists() {
        bail!("target already exists: {}", resolved_new.display());
    }

    let project = Project::discover(old_path)?;
    let old_module = project.module_name(old_path)?;
    let new_module = project.module_name(resolved_new)?;

    let elm_files = project.elm_files()?;
    let mut updated_files: Vec<String> = Vec::new();

    // Rewrite references in all other files (if module name changed).
    if old_module != new_module {
        for elm_file in &elm_files {
            if elm_file == old_path {
                continue;
            }
            let source = std::fs::read_to_string(elm_file)
                .with_context(|| format!("could not read {}", elm_file.display()))?;
            crate::parser::ensure_clean_parse(&source, elm_file)?;
            let new_source = writer::rename_module_references(&source, &old_module, &new_module);
            if new_source != source {
                if !dry_run {
                    writer::validated_write(elm_file, &new_source, "mv")?;
                }
                updated_files.push(display_path(
                    elm_file.strip_prefix(&project.root).unwrap_or(elm_file),
                ));
            }
        }
    }

    // Read old file, update module declaration, write to new path.
    let old_source = std::fs::read_to_string(old_path)
        .with_context(|| format!("could not read {}", old_path.display()))?;
    crate::parser::ensure_clean_parse(&old_source, old_path)?;
    let mut new_source = if old_module != new_module {
        writer::rename_module_declaration(&old_source, &new_module)?
    } else {
        old_source.clone()
    };
    if old_module != new_module {
        new_source = writer::rename_module_references(&new_source, &old_module, &new_module);
    }

    if !dry_run {
        // Create parent directories for the new path.
        if let Some(parent) = resolved_new.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("could not create directory: {}", parent.display()))?;
        }

        writer::validated_write(resolved_new, &new_source, "mv")?;
        std::fs::remove_file(old_path)
            .with_context(|| format!("could not remove old file: {}", old_path.display()))?;

        // Remove empty ancestor directories up to source-dir.
        if let Ok(source_dir) = project.source_dir_for(old_path) {
            let source_dir_canonical = source_dir
                .canonicalize()
                .unwrap_or_else(|_| source_dir.to_path_buf());
            let mut dir = old_path.parent().unwrap_or(Path::new(".")).to_path_buf();
            while dir != source_dir_canonical {
                if std::fs::read_dir(&dir)
                    .map(|mut d| d.next().is_none())
                    .unwrap_or(false)
                {
                    let _ = std::fs::remove_dir(&dir);
                } else {
                    break;
                }
                if !dir.pop() {
                    break;
                }
            }
        }
    }

    let old_display = display_path(old_path.strip_prefix(&project.root).unwrap_or(old_path));
    let new_display = display_path(
        resolved_new
            .strip_prefix(&project.root)
            .unwrap_or(resolved_new),
    );

    Ok(MvResult {
        old_display,
        new_display,
        updated_files,
    })
}

/// Result of a `rename` operation.
#[derive(Debug)]
pub struct RenameResult {
    /// The old declaration name.
    pub old_name: String,
    /// The new declaration name.
    pub new_name: String,
    /// Display paths of files that were updated (relative to project root).
    pub updated_files: Vec<String>,
}

/// Execute a project-wide declaration rename.
///
/// Renames the declaration in the defining file, and if the declaration is exposed,
/// updates all references across the project.
pub fn execute_rename(
    file: &Path,
    old_name: &str,
    new_name: &str,
    dry_run: bool,
) -> Result<RenameResult> {
    if !file.is_file() {
        bail!("not a file: {}", file.display());
    }

    let source = std::fs::read_to_string(file)
        .with_context(|| format!("could not read {}", file.display()))?;

    let tree = crate::parser::ensure_clean_parse(&source, file)?;
    let summary = crate::parser::extract_summary(&tree, &source);

    // Check that old_name exists as a declaration or variant.
    let is_declaration = summary.find_declaration(old_name).is_some();
    let is_variant = !is_declaration && find_variant_in_source(&tree, &source, old_name);

    if !is_declaration && !is_variant {
        bail!(
            "declaration or variant '{}' not found in {}",
            old_name,
            file.display()
        );
    }

    // Check for name conflicts.
    if summary.find_declaration(new_name).is_some() {
        bail!(
            "'{}' already exists as a declaration in {}",
            new_name,
            file.display()
        );
    }
    if find_variant_in_source(&tree, &source, new_name) {
        bail!(
            "'{}' already exists as a variant in {}",
            new_name,
            file.display()
        );
    }

    // Rename in the defining file.
    let new_source = writer::rename_declaration_in_file(&source, old_name, new_name)?;
    let mut updated_files: Vec<String> = Vec::new();

    // Determine if the declaration is exposed from the module.
    // For variants, check if the parent type is exposed with (..).
    let variant_parent_type = if is_variant {
        find_variant_parent_type(&tree, &source, old_name)
    } else {
        None
    };
    let is_exposed = if is_variant {
        // A variant is exposed if its parent type is exposed with (..).
        variant_parent_type
            .as_deref()
            .is_some_and(|parent| is_type_exposed_with_constructors(&tree, &source, parent))
    } else {
        is_declaration_exposed(&tree, &source, old_name)
    };

    // If exposed, scan all project files for references to update.
    if is_exposed {
        let project = Project::discover(file)?;
        let target_module = project.module_name(file)?;
        let elm_files = project.elm_files()?;
        let file_canonical = file
            .canonicalize()
            .with_context(|| format!("could not canonicalize {}", file.display()))?;

        for elm_file in &elm_files {
            let Ok(elm_canonical) = elm_file.canonicalize() else {
                continue;
            };
            if elm_canonical == file_canonical {
                continue;
            }

            let file_source = std::fs::read_to_string(elm_file)
                .with_context(|| format!("could not read {}", elm_file.display()))?;

            let file_tree = crate::parser::ensure_clean_parse(&file_source, elm_file)?;

            let root = file_tree.root_node();
            let import_info = crate::refs::parse_imports(&root, &file_source, &target_module);

            let Some(import_info) = import_info else {
                continue;
            };

            // Skip files using exposing(..)
            if has_exposing_all(&file_tree, &file_source, &target_module) {
                continue;
            }

            let updated = writer::rename_references_in_file(
                &file_source,
                old_name,
                new_name,
                &import_info,
                variant_parent_type.as_deref(),
            );

            if updated != file_source {
                if !dry_run {
                    writer::validated_write(elm_file, &updated, "rename")?;
                }
                updated_files.push(display_path(
                    elm_file.strip_prefix(&project.root).unwrap_or(elm_file),
                ));
            }
        }
    }

    // Write the defining file last.
    if new_source != source && !dry_run {
        writer::validated_write(file, &new_source, "rename")?;
    }

    Ok(RenameResult {
        old_name: old_name.to_string(),
        new_name: new_name.to_string(),
        updated_files,
    })
}

/// Check if a variant name exists in any type declaration.
fn find_variant_in_source(tree: &tree_sitter::Tree, source: &str, name: &str) -> bool {
    find_variant_parent_type(tree, source, name).is_some()
}

/// Find the parent type name of a variant. Returns None if the variant doesn't exist.
fn find_variant_parent_type(
    tree: &tree_sitter::Tree,
    source: &str,
    variant_name: &str,
) -> Option<String> {
    let root = tree.root_node();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() != "type_declaration" {
            continue;
        }
        // Check if any union_variant in this type has the given name.
        let mut inner = child.walk();
        for descendant in child.named_children(&mut inner) {
            if descendant.kind() == "union_variant"
                && let Some(first) = descendant.named_child(0)
                && let Ok(text) = first.utf8_text(source.as_bytes())
                && text == variant_name
            {
                // Return the type's name.
                if let Some(name_node) = child.child_by_field_name("name")
                    && let Ok(type_name) = name_node.utf8_text(source.as_bytes())
                {
                    return Some(type_name.to_string());
                }
            }
        }
    }
    None
}

/// Check if a declaration name appears in the module's exposing list.
/// Uses tree-sitter AST to handle multiline module declarations correctly.
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

/// Check if a type is exposed with (..) in the module's exposing list.
/// e.g., `module Foo exposing (Msg(..))` → `is_type_exposed_with_constructors(tree, source, "Msg")` is true.
fn is_type_exposed_with_constructors(
    tree: &tree_sitter::Tree,
    source: &str,
    type_name: &str,
) -> bool {
    let root = tree.root_node();
    let Some(module_decl) = root.child_by_field_name("moduleDeclaration") else {
        return false;
    };
    let Some(exposing_list) = module_decl.child_by_field_name("exposing") else {
        return false;
    };
    let mut cursor = exposing_list.walk();
    for child in exposing_list.named_children(&mut cursor) {
        if child.kind() == "double_dot" {
            // exposing(..) exposes everything including all constructors.
            return true;
        }
        if child.kind() == "exposed_type"
            && let Ok(text) = child.utf8_text(source.as_bytes())
        {
            let base = text.split('(').next().unwrap_or(text).trim();
            if base == type_name && text.contains("(..)") {
                return true;
            }
        }
    }
    false
}

/// Check if a file uses `exposing (..)` for a specific module import.
fn has_exposing_all(tree: &tree_sitter::Tree, source: &str, target_module: &str) -> bool {
    let root = tree.root_node();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() != "import_clause" {
            continue;
        }
        if let Some(module_name_node) = child.child_by_field_name("moduleName")
            && let Ok(module_name) = module_name_node.utf8_text(source.as_bytes())
            && module_name == target_module
            && let Some(exposing) = child.child_by_field_name("exposing")
        {
            let mut ecursor = exposing.walk();
            for ec in exposing.named_children(&mut ecursor) {
                if ec.kind() == "double_dot" {
                    return true;
                }
            }
        }
    }
    false
}

/// Format a path for display, always using forward slashes for cross-platform consistency.
fn display_path(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Resolve a new file path for mv: canonicalize the parent directory, append filename.
/// Returns the resolved absolute path. Does NOT create directories.
pub fn resolve_new_path(new_path: &Path) -> Result<PathBuf> {
    let new_parent = new_path.parent().unwrap_or(Path::new("."));
    let new_parent_abs = if new_parent.as_os_str().is_empty() {
        std::env::current_dir().context("could not determine current directory")?
    } else {
        new_parent
            .canonicalize()
            .with_context(|| format!("parent directory does not exist: {}", new_parent.display()))?
    };
    let new_filename = new_path
        .file_name()
        .with_context(|| format!("invalid path: {}", new_path.display()))?;
    Ok(new_parent_abs.join(new_filename))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_test_project(dir: &Path, source_dirs: &[&str], elm_json_type: &str) {
        let elm_json = format!(
            r#"{{
  "type": "{}",
  "source-directories": [{}]
}}"#,
            elm_json_type,
            source_dirs
                .iter()
                .map(|s| format!("\"{s}\""))
                .collect::<Vec<_>>()
                .join(", ")
        );
        fs::write(dir.join("elm.json"), elm_json).unwrap();
        for sd in source_dirs {
            fs::create_dir_all(dir.join(sd)).unwrap();
        }
    }

    #[test]
    fn test_discover_from_subdirectory() {
        let tmp = tempfile::tempdir().unwrap();
        create_test_project(tmp.path(), &["src"], "application");
        fs::create_dir_all(tmp.path().join("src/Foo")).unwrap();
        fs::write(
            tmp.path().join("src/Foo/Bar.elm"),
            "module Foo.Bar exposing (..)",
        )
        .unwrap();

        let project = Project::discover(&tmp.path().join("src/Foo")).unwrap();
        assert_eq!(
            project.root.canonicalize().unwrap(),
            tmp.path().canonicalize().unwrap()
        );
        assert_eq!(project.source_dirs.len(), 1);
    }

    #[test]
    fn test_discover_no_elm_json() {
        let tmp = tempfile::tempdir().unwrap();
        let result = Project::discover(tmp.path());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("no elm.json found")
        );
    }

    #[test]
    fn test_multiple_source_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        create_test_project(tmp.path(), &["src", "lib"], "application");

        let project = Project::discover(tmp.path()).unwrap();
        assert_eq!(project.source_dirs.len(), 2);
    }

    #[test]
    fn test_module_name_simple() {
        let tmp = tempfile::tempdir().unwrap();
        create_test_project(tmp.path(), &["src"], "application");
        fs::create_dir_all(tmp.path().join("src/Foo")).unwrap();
        fs::write(tmp.path().join("src/Foo/Bar.elm"), "").unwrap();

        let project = Project::discover(tmp.path()).unwrap();
        let name = project
            .module_name(&tmp.path().join("src/Foo/Bar.elm"))
            .unwrap();
        assert_eq!(name, "Foo.Bar");
    }

    #[test]
    fn test_module_name_root_level() {
        let tmp = tempfile::tempdir().unwrap();
        create_test_project(tmp.path(), &["src"], "application");
        fs::write(tmp.path().join("src/Main.elm"), "").unwrap();

        let project = Project::discover(tmp.path()).unwrap();
        let name = project
            .module_name(&tmp.path().join("src/Main.elm"))
            .unwrap();
        assert_eq!(name, "Main");
    }

    #[test]
    fn test_module_name_nonexistent_file() {
        let tmp = tempfile::tempdir().unwrap();
        create_test_project(tmp.path(), &["src"], "application");
        fs::create_dir_all(tmp.path().join("src/Foo")).unwrap();

        let project = Project::discover(tmp.path()).unwrap();
        // File doesn't exist but parent dir does — should still work
        let name = project
            .module_name(&tmp.path().join("src/Foo/Baz.elm"))
            .unwrap();
        assert_eq!(name, "Foo.Baz");
    }

    #[test]
    fn test_module_name_not_under_source_dir() {
        let tmp = tempfile::tempdir().unwrap();
        create_test_project(tmp.path(), &["src"], "application");

        let project = Project::discover(tmp.path()).unwrap();
        let result = project.module_name(&tmp.path().join("other/Foo.elm"));
        assert!(result.is_err());
    }

    #[test]
    fn test_elm_files() {
        let tmp = tempfile::tempdir().unwrap();
        create_test_project(tmp.path(), &["src"], "application");
        fs::create_dir_all(tmp.path().join("src/Foo")).unwrap();
        fs::write(tmp.path().join("src/Main.elm"), "").unwrap();
        fs::write(tmp.path().join("src/Foo/Bar.elm"), "").unwrap();
        fs::write(tmp.path().join("src/Foo/readme.txt"), "").unwrap();

        let project = Project::discover(tmp.path()).unwrap();
        let files = project.elm_files().unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| f.extension().unwrap() == "elm"));
    }

    #[test]
    fn test_package_elm_json() {
        let tmp = tempfile::tempdir().unwrap();
        create_test_project(tmp.path(), &["src"], "package");

        let project = Project::discover(tmp.path()).unwrap();
        assert_eq!(project.source_dirs.len(), 1);
    }

    #[test]
    fn test_nested_source_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        create_test_project(tmp.path(), &["src", "src/generated"], "application");
        fs::write(tmp.path().join("src/generated/Api.elm"), "").unwrap();

        let project = Project::discover(tmp.path()).unwrap();
        // src/generated is longer, should match first for files under it
        let name = project
            .module_name(&tmp.path().join("src/generated/Api.elm"))
            .unwrap();
        assert_eq!(name, "Api");
    }

    #[test]
    fn test_source_dir_doesnt_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let elm_json = r#"{"type": "application", "source-directories": ["nonexistent"]}"#;
        fs::write(tmp.path().join("elm.json"), elm_json).unwrap();

        let result = Project::discover(tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }
}
