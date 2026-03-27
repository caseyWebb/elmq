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
            let new_source = writer::rename_module_references(&source, &old_module, &new_module);
            if new_source != source {
                if !dry_run {
                    writer::atomic_write(elm_file, &new_source)?;
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

        writer::atomic_write(resolved_new, &new_source)?;
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
