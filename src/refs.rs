use crate::imports::{ExposedItem, ImportContext};
use crate::parser;
use crate::project::Project;
use anyhow::{Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tree_sitter::Node;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RefMatch {
    pub file: String,
    pub line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Find all references to a module (and optionally a specific declaration) across a project.
pub fn find_refs(
    project: &Project,
    target_module: &str,
    declaration: Option<&str>,
) -> Result<Vec<RefMatch>> {
    let elm_files = project.elm_files()?;
    let mut matches = Vec::new();

    for elm_file in &elm_files {
        // Skip the target module's own file.
        if let Ok(module_name) = project.module_name(elm_file)
            && module_name == target_module
        {
            continue;
        }

        let source = std::fs::read_to_string(elm_file)
            .with_context(|| format!("could not read {}", elm_file.display()))?;

        let tree = match parser::parse(&source) {
            Ok(t) => t,
            Err(_) => continue,
        };

        let root = tree.root_node();
        let import_info = parse_imports(&root, &source, target_module);

        if import_info.is_none() {
            continue;
        }
        let import_info = import_info.unwrap();

        let display_path = relative_display(elm_file, &project.root);

        match declaration {
            None => {
                // Module-level: report the import line.
                matches.push(RefMatch {
                    file: display_path,
                    line: import_info.import_line,
                    text: None,
                });
            }
            Some(decl_name) => {
                collect_declaration_refs(
                    &root,
                    &source,
                    &import_info,
                    decl_name,
                    &display_path,
                    &mut matches,
                );
            }
        }
    }

    matches.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    Ok(matches)
}

/// Information about how a target module is imported in a file.
pub(crate) struct ImportInfo {
    /// Line number of the import statement (1-based).
    pub(crate) import_line: usize,
    /// The full module name (e.g., "Lib.MyModule").
    pub(crate) module_name: String,
    /// Alias if present (e.g., "LM" from `import Lib.MyModule as LM`).
    pub(crate) alias: Option<String>,
    /// Explicitly exposed names (e.g., ["someFunc", "Model"] from `exposing (someFunc, Model)`).
    pub(crate) exposed_names: Vec<String>,
    /// Types whose constructors are exposed (e.g., ["Msg"] from `exposing (Msg(..))`).
    pub(crate) exposed_constructors_of: Vec<String>,
}

/// Parse import clauses to find if/how the target module is imported.
/// Delegates to `ImportContext` for the heavy lifting.
pub(crate) fn parse_imports(root: &Node, source: &str, target_module: &str) -> Option<ImportInfo> {
    let ctx = ImportContext::from_tree(root, source);
    let module_import = ctx.get(target_module)?;

    // Find the import line number (ImportContext doesn't track this).
    let import_line = find_import_line(root, source, target_module)?;

    let mut exposed_names = Vec::new();
    let mut exposed_constructors_of = Vec::new();
    for item in &module_import.exposed {
        match item {
            ExposedItem::Value(n) | ExposedItem::Type(n) => {
                exposed_names.push(n.clone());
            }
            ExposedItem::TypeWithConstructors(n) => {
                exposed_names.push(n.clone());
                exposed_constructors_of.push(n.clone());
            }
        }
    }

    Some(ImportInfo {
        import_line,
        module_name: module_import.module_name.clone(),
        alias: module_import.alias.clone(),
        exposed_names,
        exposed_constructors_of,
    })
}

/// Find the line number of the import clause for a target module.
fn find_import_line(root: &Node, source: &str, target_module: &str) -> Option<usize> {
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() != "import_clause" {
            continue;
        }
        if let Some(module_name_node) = child.child_by_field_name("moduleName")
            && let Ok(module_name) = module_name_node.utf8_text(source.as_bytes())
            && module_name == target_module
        {
            return Some(child.start_position().row + 1);
        }
    }
    None
}

/// Collect all reference sites for a specific declaration in a file.
fn collect_declaration_refs(
    root: &Node,
    source: &str,
    import_info: &ImportInfo,
    decl_name: &str,
    display_path: &str,
    matches: &mut Vec<RefMatch>,
) {
    // If the declaration is explicitly exposed, report the import line.
    if import_info.exposed_names.iter().any(|n| n == decl_name) {
        let import_line_text = source
            .lines()
            .nth(import_info.import_line - 1)
            .unwrap_or("")
            .trim();
        matches.push(RefMatch {
            file: display_path.to_string(),
            line: import_info.import_line,
            text: Some(import_line_text.to_string()),
        });
    }

    // Walk the tree for qualified references.
    collect_qualified_refs(root, source, import_info, decl_name, display_path, matches);
}

fn collect_qualified_refs(
    node: &Node,
    source: &str,
    import_info: &ImportInfo,
    decl_name: &str,
    display_path: &str,
    matches: &mut Vec<RefMatch>,
) {
    let is_exposed = import_info.exposed_names.iter().any(|n| n == decl_name);

    match node.kind() {
        "value_qid" | "upper_case_qid" => {
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                let full_prefix = format!("{}.", import_info.module_name);
                let alias_prefix = import_info.alias.as_ref().map(|a| format!("{a}."));

                let is_match = if let Some(suffix) = text.strip_prefix(&full_prefix) {
                    suffix == decl_name
                } else if let Some(ref alias_pfx) = alias_prefix
                    && let Some(suffix) = text.strip_prefix(alias_pfx.as_str())
                {
                    suffix == decl_name
                } else {
                    // Bare reference (no dots) — match if explicitly exposed.
                    is_exposed && text == decl_name
                };

                if is_match {
                    let line = node.start_position().row + 1;
                    let line_text = source.lines().nth(line - 1).unwrap_or("").trim();
                    matches.push(RefMatch {
                        file: display_path.to_string(),
                        line,
                        text: Some(line_text.to_string()),
                    });
                }
            }
            return;
        }
        "module_declaration" | "import_clause" => return,
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_qualified_refs(
            &child,
            source,
            import_info,
            decl_name,
            display_path,
            matches,
        );
    }
}

fn relative_display(file: &Path, root: &PathBuf) -> String {
    let relative = file.strip_prefix(root).unwrap_or(file);
    relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_test_project(dir: &Path, source_dirs: &[&str]) {
        let elm_json = format!(
            r#"{{
  "type": "application",
  "source-directories": [{}]
}}"#,
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

    fn write_elm(dir: &Path, relative: &str, content: &str) {
        let path = dir.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[test]
    fn test_module_level_refs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        create_test_project(root, &["src"]);

        write_elm(
            root,
            "src/Lib/Utils.elm",
            "module Lib.Utils exposing (helper)\n\nhelper = 1\n",
        );
        write_elm(
            root,
            "src/Main.elm",
            "module Main exposing (..)\n\nimport Lib.Utils exposing (helper)\n\nmain = helper\n",
        );
        write_elm(
            root,
            "src/Other.elm",
            "module Other exposing (..)\n\nimport Lib.Utils\n\nfoo = Lib.Utils.helper\n",
        );

        let project = Project::discover(root).unwrap();
        let refs = find_refs(&project, "Lib.Utils", None).unwrap();

        assert_eq!(refs.len(), 2);
        assert!(refs.iter().any(|r| r.file == "src/Main.elm"));
        assert!(refs.iter().any(|r| r.file == "src/Other.elm"));
        assert!(refs.iter().all(|r| r.text.is_none()));
    }

    #[test]
    fn test_qualified_ref() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        create_test_project(root, &["src"]);

        write_elm(
            root,
            "src/Lib/Utils.elm",
            "module Lib.Utils exposing (helper)\n\nhelper = 1\n",
        );
        write_elm(
            root,
            "src/Main.elm",
            "module Main exposing (..)\n\nimport Lib.Utils\n\nmain = Lib.Utils.helper\n",
        );

        let project = Project::discover(root).unwrap();
        let refs = find_refs(&project, "Lib.Utils", Some("helper")).unwrap();

        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].file, "src/Main.elm");
        assert_eq!(refs[0].line, 5);
        assert_eq!(refs[0].text.as_deref(), Some("main = Lib.Utils.helper"));
    }

    #[test]
    fn test_alias_ref() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        create_test_project(root, &["src"]);

        write_elm(
            root,
            "src/Lib/Utils.elm",
            "module Lib.Utils exposing (helper)\n\nhelper = 1\n",
        );
        write_elm(
            root,
            "src/Main.elm",
            "module Main exposing (..)\n\nimport Lib.Utils as LU\n\nmain = LU.helper\n",
        );

        let project = Project::discover(root).unwrap();
        let refs = find_refs(&project, "Lib.Utils", Some("helper")).unwrap();

        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].file, "src/Main.elm");
        assert_eq!(refs[0].line, 5);
        assert_eq!(refs[0].text.as_deref(), Some("main = LU.helper"));
    }

    #[test]
    fn test_exposed_name_ref() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        create_test_project(root, &["src"]);

        write_elm(
            root,
            "src/Lib/Utils.elm",
            "module Lib.Utils exposing (helper)\n\nhelper = 1\n",
        );
        write_elm(
            root,
            "src/Main.elm",
            "module Main exposing (..)\n\nimport Lib.Utils exposing (helper)\n\nmain = helper\n",
        );

        let project = Project::discover(root).unwrap();
        let refs = find_refs(&project, "Lib.Utils", Some("helper")).unwrap();

        // Should report both the import line and the bare usage site.
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].file, "src/Main.elm");
        assert_eq!(refs[0].line, 3);
        assert_eq!(
            refs[0].text.as_deref(),
            Some("import Lib.Utils exposing (helper)")
        );
        assert_eq!(refs[1].file, "src/Main.elm");
        assert_eq!(refs[1].line, 5);
        assert_eq!(refs[1].text.as_deref(), Some("main = helper"));
    }

    #[test]
    fn test_exposed_type_ref() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        create_test_project(root, &["src"]);

        write_elm(
            root,
            "src/Lib/Types.elm",
            "module Lib.Types exposing (Model)\n\ntype alias Model = { name : String }\n",
        );
        write_elm(
            root,
            "src/Main.elm",
            "module Main exposing (..)\n\nimport Lib.Types exposing (Model)\n\ntype alias Page = Model\n",
        );

        let project = Project::discover(root).unwrap();
        let refs = find_refs(&project, "Lib.Types", Some("Model")).unwrap();

        // Should report both the import line and the bare type usage.
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].line, 3);
        assert_eq!(refs[1].line, 5);
        assert_eq!(refs[1].text.as_deref(), Some("type alias Page = Model"));
    }

    #[test]
    fn test_no_refs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        create_test_project(root, &["src"]);

        write_elm(
            root,
            "src/Lib/Utils.elm",
            "module Lib.Utils exposing (helper)\n\nhelper = 1\n",
        );
        write_elm(
            root,
            "src/Main.elm",
            "module Main exposing (..)\n\nmain = 1\n",
        );

        let project = Project::discover(root).unwrap();
        let refs = find_refs(&project, "Lib.Utils", None).unwrap();

        assert!(refs.is_empty());
    }

    #[test]
    fn test_exposing_all_not_traced() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        create_test_project(root, &["src"]);

        write_elm(
            root,
            "src/Lib/Utils.elm",
            "module Lib.Utils exposing (helper)\n\nhelper = 1\n",
        );
        write_elm(
            root,
            "src/Main.elm",
            "module Main exposing (..)\n\nimport Lib.Utils exposing (..)\n\nmain = helper\n",
        );

        let project = Project::discover(root).unwrap();
        // Module-level: should still report the import.
        let module_refs = find_refs(&project, "Lib.Utils", None).unwrap();
        assert_eq!(module_refs.len(), 1);

        // Declaration-level: should NOT report bare `helper` usage.
        let decl_refs = find_refs(&project, "Lib.Utils", Some("helper")).unwrap();
        assert!(decl_refs.is_empty());
    }

    #[test]
    fn test_type_ref() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        create_test_project(root, &["src"]);

        write_elm(
            root,
            "src/Lib/Types.elm",
            "module Lib.Types exposing (Model)\n\ntype alias Model = { name : String }\n",
        );
        write_elm(
            root,
            "src/Main.elm",
            "module Main exposing (..)\n\nimport Lib.Types\n\ntype alias Page = Lib.Types.Model\n",
        );

        let project = Project::discover(root).unwrap();
        let refs = find_refs(&project, "Lib.Types", Some("Model")).unwrap();

        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].line, 5);
    }
}
