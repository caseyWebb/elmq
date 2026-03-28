use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
    transport::io::stdio,
};
use serde::Deserialize;

use crate::parser;
use crate::refs;
use crate::writer;
use crate::{Declaration, DeclarationKind, FileSummary};

#[derive(Debug, Clone)]
pub struct ElmqServer {
    tool_router: ToolRouter<Self>,
}

impl ElmqServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_handler]
impl ServerHandler for ElmqServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(
                "Structured Elm file operations — prefer these over built-in tools for .elm files:\n\
                 \n\
                 - elm_summary instead of Read: returns file structure (module, imports, declarations \
                 with types/line numbers) in ~10% of the tokens. Use first to understand a file.\n\
                 - elm_get instead of Read: extracts one declaration's full source by name. \
                 Use when you need a specific function, type, or port.\n\
                 - elm_edit instead of Write/Edit: atomic modifications — set/patch/rm declarations, \
                 add/remove imports, expose/unexpose, plus project-wide: mv (rename module), \
                 rename (rename declaration), move_decl (move between modules), \
                 add_variant/rm_variant (propagate through case expressions). \
                 One call replaces multi-step Read+Edit cycles.\n\
                 - elm_refs instead of Grep: finds all references to a module or declaration, \
                 resolving qualified, aliased, and exposed names through import context.\n\
                 \n\
                 Use Read/Write only for raw content outside declarations or creating new files from scratch.",
            )
            .with_server_info(rmcp::model::Implementation::new(
                "elmq",
                env!("CARGO_PKG_VERSION"),
            ))
    }
}

// -- Parameter types --

// Fix schemars-generated schemas for Anthropic API compatibility:
// 1. Strip nullable type arrays ["string", "null"] → "string"
// 2. Flatten oneOf (from serde tagged enums with flatten) into a single object
//    schema with all variant fields as optional properties
fn fix_schema(schema: &mut schemars::Schema) {
    if let Some(obj) = schema.as_object_mut() {
        // Flatten oneOf: merge all variant schemas into a single properties map
        if let Some(one_of) = obj.remove("oneOf") {
            if let serde_json::Value::Array(variants) = one_of {
                let mut all_props = serde_json::Map::new();
                let mut action_values = Vec::new();

                for variant in &variants {
                    if let Some(variant_obj) = variant.as_object() {
                        // Collect action enum values from const fields
                        if let Some(action_prop) = variant_obj
                            .get("properties")
                            .and_then(|p| p.get("action"))
                            .and_then(|a| a.get("const"))
                        {
                            action_values.push(action_prop.clone());
                        }
                        // Merge all properties (skip "action" — we'll rebuild it)
                        if let Some(props) = variant_obj.get("properties").and_then(|p| p.as_object())
                        {
                            for (k, v) in props {
                                if k != "action" {
                                    all_props.entry(k.clone()).or_insert_with(|| v.clone());
                                }
                            }
                        }
                    }
                }

                // Build the action enum property
                all_props.insert(
                    "action".to_owned(),
                    serde_json::json!({
                        "type": "string",
                        "enum": action_values,
                        "description": "The edit action to perform"
                    }),
                );

                // Merge into existing properties
                if let Some(existing_props) = obj
                    .get_mut("properties")
                    .and_then(|p| p.as_object_mut())
                {
                    for (k, v) in all_props {
                        existing_props.entry(k).or_insert(v);
                    }
                } else {
                    obj.insert(
                        "properties".to_owned(),
                        serde_json::Value::Object(all_props),
                    );
                }

                // Set required and type
                obj.insert(
                    "required".to_owned(),
                    serde_json::json!(["file", "action"]),
                );
                obj.insert(
                    "type".to_owned(),
                    serde_json::json!("object"),
                );
            }
        }

        // Strip nullable type arrays in all properties
        if let Some(props) = obj.get_mut("properties") {
            if let Some(props_obj) = props.as_object_mut() {
                for (_key, prop) in props_obj.iter_mut() {
                    if let Some(prop_obj) = prop.as_object_mut() {
                        if let Some(serde_json::Value::Array(types)) = prop_obj.get("type") {
                            let non_null: Vec<_> = types
                                .iter()
                                .filter(|t| t.as_str() != Some("null"))
                                .cloned()
                                .collect();
                            if non_null.len() == 1 {
                                prop_obj.insert("type".to_owned(), non_null[0].clone());
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[schemars(transform = fix_schema)]
pub struct SummaryParams {
    /// Path to the Elm file
    pub file: String,
    /// Output format: "compact" (default) or "json"
    pub format: Option<String>,
    /// Include doc comments in output
    pub docs: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[schemars(transform = fix_schema)]
pub struct GetParams {
    /// Path to the Elm file
    pub file: String,
    /// Name of the declaration to extract
    pub name: String,
    /// Output format: "compact" (default) or "json"
    pub format: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[schemars(transform = fix_schema)]
pub struct EditParams {
    /// Path to the Elm file
    pub file: String,
    /// Action and its parameters
    #[serde(flatten)]
    pub action: EditAction,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum EditAction {
    /// Upsert a declaration from source text
    Set {
        /// Full source text of the declaration
        source: String,
        /// Override the declaration name (default: parsed from source)
        name: Option<String>,
    },
    /// Find-and-replace within a declaration
    Patch {
        /// Name of the declaration to patch
        name: String,
        /// Text to find within the declaration
        old: String,
        /// Replacement text
        new: String,
    },
    /// Remove a declaration
    Rm {
        /// Name of the declaration to remove
        name: String,
    },
    /// Rename/move a module file and update all imports and qualified references across the project
    Mv {
        /// New file path for the module
        new_path: String,
        /// If true, preview changes without writing
        dry_run: Option<bool>,
    },
    /// Rename a declaration and update all references across the project
    Rename {
        /// Current name of the declaration
        name: String,
        /// New name for the declaration
        new: String,
        /// If true, preview changes without writing
        dry_run: Option<bool>,
    },
    /// Move declarations from one module to another, updating all references across the project
    MoveDecl {
        /// Names of declarations to move
        names: Vec<String>,
        /// Path to the target Elm file
        target: String,
        /// Copy shared helpers instead of erroring
        copy_shared_helpers: Option<bool>,
        /// If true, preview changes without writing
        dry_run: Option<bool>,
    },
    /// Add or replace an import clause
    AddImport {
        /// Import clause, e.g. "Html exposing (Html, div)"
        import: String,
    },
    /// Remove an import by module name
    RemoveImport {
        /// Module name to remove, e.g. "Html"
        module_name: String,
    },
    /// Add an item to the module's exposing list
    Expose {
        /// Item to expose, e.g. "update" or "Msg(..)"
        item: String,
    },
    /// Remove an item from the module's exposing list
    Unexpose {
        /// Item to unexpose, e.g. "helper"
        item: String,
    },
    /// Add a constructor to a custom type and insert branches in all case expressions project-wide
    AddVariant {
        /// Name of the custom type (e.g. "Msg")
        type_name: String,
        /// Variant definition (e.g. "SetName String")
        definition: String,
        /// If true, preview changes without writing
        dry_run: Option<bool>,
    },
    /// Remove a constructor from a custom type and remove branches from all case expressions project-wide
    RmVariant {
        /// Name of the custom type (e.g. "Msg")
        type_name: String,
        /// Constructor name to remove (e.g. "Decrement")
        constructor: String,
        /// If true, preview changes without writing
        dry_run: Option<bool>,
    },
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[schemars(transform = fix_schema)]
pub struct RefsParams {
    /// Path to the Elm file whose module to search for
    pub file: String,
    /// Declaration name to search for (if omitted, returns module-level references)
    pub name: Option<String>,
    /// Output format: "compact" (default) or "json"
    pub format: Option<String>,
}

// -- Helpers --

/// Validate that a file path resolves to within the server's working directory.
fn validate_path(file: &str) -> Result<PathBuf, String> {
    let path = Path::new(file);

    let cwd = std::env::current_dir().map_err(|e| format!("could not determine cwd: {e}"))?;
    let canonical_cwd = cwd
        .canonicalize()
        .map_err(|e| format!("could not canonicalize cwd: {e}"))?;

    // For absolute paths, check containment without requiring the file to exist.
    // This avoids platform-dependent behavior where canonicalize() fails on
    // non-existent files (e.g. "/etc/hosts" on Windows).
    if path.is_absolute() {
        // Normalize what we can without requiring existence
        let normalized = if let Ok(canonical) = path.canonicalize() {
            canonical
        } else {
            path.to_path_buf()
        };
        if !normalized.starts_with(&canonical_cwd) {
            return Err(format!(
                "path \"{file}\" resolves outside the working directory"
            ));
        }
    }

    let canonical = path
        .canonicalize()
        .map_err(|e| format!("invalid path \"{file}\": {e}"))?;

    if !canonical.starts_with(&canonical_cwd) {
        return Err(format!(
            "path \"{file}\" resolves outside the working directory"
        ));
    }

    Ok(canonical)
}

fn load_and_parse(file: &str) -> Result<(String, FileSummary), String> {
    let path = validate_path(file)?;
    let source =
        std::fs::read_to_string(&path).map_err(|e| format!("could not read file {file}: {e}"))?;

    let tree = parser::parse(&source).map_err(|e| format!("parse error: {e}"))?;

    if tree.root_node().has_error() {
        // Continue with warning — tree-sitter produces partial trees
    }

    let summary = parser::extract_summary(&tree, &source);
    Ok((source, summary))
}

fn format_compact(summary: &FileSummary, show_docs: bool) -> String {
    let mut out = String::new();
    out.push_str(&summary.module_line);

    if !summary.imports.is_empty() {
        out.push_str("\n\nimports:");
        for imp in &summary.imports {
            out.push_str(&format!("\n  {imp}"));
        }
    }

    let type_aliases: Vec<_> = summary
        .declarations
        .iter()
        .filter(|d| d.kind == DeclarationKind::TypeAlias)
        .collect();
    let types: Vec<_> = summary
        .declarations
        .iter()
        .filter(|d| d.kind == DeclarationKind::Type)
        .collect();
    let functions: Vec<_> = summary
        .declarations
        .iter()
        .filter(|d| d.kind == DeclarationKind::Function)
        .collect();
    let ports: Vec<_> = summary
        .declarations
        .iter()
        .filter(|d| d.kind == DeclarationKind::Port)
        .collect();

    for (label, decls) in [("type aliases:", &type_aliases), ("types:", &types)] {
        if !decls.is_empty() {
            let name_w = decls.iter().map(|d| d.name.len()).max().unwrap_or(0);
            out.push_str(&format!("\n\n{label}"));
            for d in decls {
                out.push_str(&format!(
                    "\n  {:<name_w$}  L{}-{}",
                    d.name, d.start_line, d.end_line
                ));
                if show_docs && let Some(doc) = &d.doc_comment {
                    format_doc_comment(&mut out, doc);
                }
            }
        }
    }

    for (label, decls) in [("functions:", &functions), ("ports:", &ports)] {
        if !decls.is_empty() {
            let name_w = decls.iter().map(|d| d.name.len()).max().unwrap_or(0);
            out.push_str(&format!("\n\n{label}"));
            for d in decls {
                let type_str = d.type_annotation.as_deref().unwrap_or("");
                if type_str.is_empty() {
                    out.push_str(&format!(
                        "\n  {:<name_w$}  L{}-{}",
                        d.name, d.start_line, d.end_line
                    ));
                } else {
                    out.push_str(&format!(
                        "\n  {:<name_w$}  {}  L{}-{}",
                        d.name, type_str, d.start_line, d.end_line
                    ));
                }
                if show_docs && let Some(doc) = &d.doc_comment {
                    format_doc_comment(&mut out, doc);
                }
            }
        }
    }

    out
}

fn format_doc_comment(out: &mut String, doc: &str) {
    let stripped = doc
        .strip_prefix("{-|")
        .unwrap_or(doc)
        .strip_suffix("-}")
        .unwrap_or(doc)
        .trim();
    for line in stripped.lines() {
        out.push_str(&format!("\n    {}", line.trim()));
    }
}

fn format_get_json(decl: &Declaration, source: &str) -> Result<String, String> {
    let json = serde_json::json!({
        "name": decl.name,
        "kind": decl.kind,
        "source": source,
        "start_line": decl.start_line,
        "end_line": decl.end_line,
    });
    serde_json::to_string_pretty(&json).map_err(|e| format!("JSON serialization error: {e}"))
}

// -- Tool implementations --

#[tool_router]
impl ElmqServer {
    #[tool(
        name = "elm_summary",
        description = "Show a summary of an Elm file's structure: module declaration, imports, and all declarations grouped by kind (types, type aliases, functions, ports) with line numbers and optional type annotations."
    )]
    fn elm_summary(&self, Parameters(params): Parameters<SummaryParams>) -> Result<String, String> {
        let (_source, summary) = load_and_parse(&params.file)?;
        let is_json = params.format.as_deref() == Some("json");
        let show_docs = params.docs.unwrap_or(false);

        if is_json {
            serde_json::to_string_pretty(&summary)
                .map_err(|e| format!("JSON serialization error: {e}"))
        } else {
            Ok(format_compact(&summary, show_docs))
        }
    }

    #[tool(
        name = "elm_get",
        description = "Extract the full source text of a declaration (function, type, type alias, or port) by name from an Elm file."
    )]
    fn elm_get(&self, Parameters(params): Parameters<GetParams>) -> Result<String, String> {
        let (source, summary) = load_and_parse(&params.file)?;
        let is_json = params.format.as_deref() == Some("json");

        let decl = summary
            .find_declaration(&params.name)
            .ok_or_else(|| format!("declaration '{}' not found in {}", params.name, params.file))?;

        let source_lines: Vec<&str> = source.lines().collect();
        let start = decl.start_line - 1;
        let end = decl.end_line.min(source_lines.len());
        let decl_source = source_lines[start..end].join("\n");

        if is_json {
            format_get_json(decl, &decl_source)
        } else {
            Ok(decl_source)
        }
    }

    #[tool(
        name = "elm_edit",
        description = "Modify an Elm file. Actions: \"set\" (upsert declaration), \"patch\" (find-replace in declaration), \"rm\" (remove declaration), \"mv\" (rename module across project), \"rename\" (rename declaration across project), \"move_decl\" (move declarations to another module), \"add_import\" (add/replace import), \"remove_import\" (remove import), \"expose\" (add to exposing list), \"unexpose\" (remove from exposing list), \"add_variant\" (add constructor to custom type, insert case branches project-wide), \"rm_variant\" (remove constructor from custom type, remove case branches project-wide). All writes are atomic."
    )]
    fn elm_edit(&self, Parameters(params): Parameters<EditParams>) -> Result<String, String> {
        match params.action {
            EditAction::Set {
                source: new_source,
                name,
            } => {
                let (source, summary) = load_and_parse(&params.file)?;
                let path = validate_path(&params.file)?;

                let decl_name = if let Some(name) = &name {
                    name.clone()
                } else {
                    parser::extract_declaration_name(&new_source)
                        .ok_or("could not parse declaration name from source (provide \"name\")")?
                };

                let result = writer::upsert_declaration(&source, &summary, &decl_name, &new_source);
                writer::atomic_write(&path, &result).map_err(|e| format!("write error: {e}"))?;
                Ok(format!("set {decl_name} in {}", params.file))
            }
            EditAction::Patch { name, old, new } => {
                let (source, summary) = load_and_parse(&params.file)?;
                let path = validate_path(&params.file)?;

                let result = writer::patch_declaration(&source, &summary, &name, &old, &new)
                    .map_err(|e| e.to_string())?;
                writer::atomic_write(&path, &result).map_err(|e| format!("write error: {e}"))?;
                Ok(format!("patched {name} in {}", params.file))
            }
            EditAction::Rm { name } => {
                let (source, summary) = load_and_parse(&params.file)?;
                let path = validate_path(&params.file)?;

                let result = writer::remove_declaration(&source, &summary, &name)
                    .map_err(|e| e.to_string())?;
                writer::atomic_write(&path, &result).map_err(|e| format!("write error: {e}"))?;
                Ok(format!("removed {name} from {}", params.file))
            }
            EditAction::Mv {
                ref new_path,
                dry_run,
            } => self.handle_mv(&params.file, new_path, dry_run.unwrap_or(false)),
            EditAction::Rename {
                ref name,
                ref new,
                dry_run,
            } => self.handle_rename(&params.file, name, new, dry_run.unwrap_or(false)),
            EditAction::MoveDecl {
                ref names,
                ref target,
                copy_shared_helpers,
                dry_run,
            } => self.handle_move_decl(
                &params.file,
                names,
                target,
                copy_shared_helpers.unwrap_or(false),
                dry_run.unwrap_or(false),
            ),
            EditAction::AddImport { import } => {
                let (source, summary) = load_and_parse(&params.file)?;
                let path = validate_path(&params.file)?;

                let result = writer::add_import(&source, &summary, &import);
                writer::atomic_write(&path, &result).map_err(|e| format!("write error: {e}"))?;
                Ok(format!("added import in {}", params.file))
            }
            EditAction::RemoveImport { module_name } => {
                let (source, _summary) = load_and_parse(&params.file)?;
                let path = validate_path(&params.file)?;

                let result =
                    writer::remove_import(&source, &module_name).map_err(|e| e.to_string())?;
                writer::atomic_write(&path, &result).map_err(|e| format!("write error: {e}"))?;
                Ok(format!("removed import {module_name} from {}", params.file))
            }
            EditAction::Expose { item } => {
                let (source, summary) = load_and_parse(&params.file)?;
                let path = validate_path(&params.file)?;

                let result = writer::expose(&source, &summary, &item).map_err(|e| e.to_string())?;
                writer::atomic_write(&path, &result).map_err(|e| format!("write error: {e}"))?;
                Ok(format!("exposed {item} in {}", params.file))
            }
            EditAction::Unexpose { item } => {
                let (source, summary) = load_and_parse(&params.file)?;
                let path = validate_path(&params.file)?;

                let result =
                    writer::unexpose(&source, &summary, &item).map_err(|e| e.to_string())?;
                writer::atomic_write(&path, &result).map_err(|e| format!("write error: {e}"))?;
                Ok(format!("unexposed {item} in {}", params.file))
            }
            EditAction::AddVariant {
                type_name,
                definition,
                dry_run,
            } => {
                let path = validate_path(&params.file)?;
                let result = elmq::variant::execute_add_variant(
                    &path,
                    &type_name,
                    &definition,
                    dry_run.unwrap_or(false),
                )
                .map_err(|e: anyhow::Error| e.to_string())?;
                serde_json::to_string_pretty(&result).map_err(|e| format!("JSON error: {e}"))
            }
            EditAction::RmVariant {
                type_name,
                constructor,
                dry_run,
            } => {
                let path = validate_path(&params.file)?;
                let result = elmq::variant::execute_rm_variant(
                    &path,
                    &type_name,
                    &constructor,
                    dry_run.unwrap_or(false),
                )
                .map_err(|e: anyhow::Error| e.to_string())?;
                serde_json::to_string_pretty(&result).map_err(|e| format!("JSON error: {e}"))
            }
        }
    }

    #[tool(
        name = "elm_refs",
        description = "Find all references to a module or declaration across the Elm project. Without a name, returns files that import the module. With a name, returns all usage sites (qualified, aliased, and explicitly exposed references) with line numbers."
    )]
    fn elm_refs(&self, Parameters(params): Parameters<RefsParams>) -> Result<String, String> {
        let path = validate_path(&params.file)?;
        let is_json = params.format.as_deref() == Some("json");

        let project = elmq::project::Project::discover(&path).map_err(|e| e.to_string())?;
        let target_module = project.module_name(&path).map_err(|e| e.to_string())?;
        let matches = refs::find_refs(&project, &target_module, params.name.as_deref())
            .map_err(|e| e.to_string())?;

        if is_json {
            serde_json::to_string_pretty(&matches)
                .map_err(|e| format!("JSON serialization error: {e}"))
        } else {
            let lines: Vec<String> = matches
                .iter()
                .map(|r| {
                    if let Some(text) = &r.text {
                        format!("{}:{}: {}", r.file, r.line, text)
                    } else {
                        format!("{}:{}", r.file, r.line)
                    }
                })
                .collect();
            Ok(lines.join("\n"))
        }
    }
}

impl ElmqServer {
    fn handle_rename(
        &self,
        file: &str,
        name: &str,
        new_name: &str,
        dry_run: bool,
    ) -> Result<String, String> {
        let path = validate_path(file)?;

        let result = elmq::project::execute_rename(&path, name, new_name, dry_run)
            .map_err(|e| e.to_string())?;

        let json = serde_json::json!({
            "dry_run": dry_run,
            "renamed": {
                "from": result.old_name,
                "to": result.new_name,
            },
            "updated": result.updated_files,
        });
        serde_json::to_string_pretty(&json).map_err(|e| format!("JSON error: {e}"))
    }

    fn handle_mv(&self, file: &str, new_path_str: &str, dry_run: bool) -> Result<String, String> {
        let old_path = validate_path(file)?;

        // Resolve new path relative to CWD, then validate it's within CWD.
        let cwd = std::env::current_dir().map_err(|e| format!("could not determine cwd: {e}"))?;
        let canonical_cwd = cwd.canonicalize().map_err(|e| format!("cwd error: {e}"))?;

        let new_path_raw = Path::new(new_path_str);
        let new_path_abs = if new_path_raw.is_absolute() {
            new_path_raw.to_path_buf()
        } else {
            cwd.join(new_path_raw)
        };

        // Create parent dirs only if not dry_run, then resolve.
        if !dry_run && let Some(parent) = new_path_abs.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("could not create directory: {e}"))?;
        }

        let resolved_new = elmq::project::resolve_new_path(&new_path_abs)
            .map_err(|e| format!("invalid new path: {e}"))?;

        // Validate new path is within CWD BEFORE any operations.
        if !resolved_new.starts_with(&canonical_cwd) {
            return Err(format!(
                "new path \"{new_path_str}\" resolves outside the working directory"
            ));
        }

        let result = elmq::project::execute_mv(&old_path, &resolved_new, dry_run)
            .map_err(|e| e.to_string())?;

        let json = serde_json::json!({
            "dry_run": dry_run,
            "renamed": {
                "from": result.old_display,
                "to": result.new_display,
            },
            "updated": result.updated_files,
        });
        serde_json::to_string_pretty(&json).map_err(|e| format!("JSON error: {e}"))
    }
    fn handle_move_decl(
        &self,
        file: &str,
        names: &[String],
        target: &str,
        copy_shared_helpers: bool,
        dry_run: bool,
    ) -> Result<String, String> {
        let source_path = validate_path(file)?;

        // Resolve target path relative to CWD.
        let cwd = std::env::current_dir().map_err(|e| format!("could not determine cwd: {e}"))?;
        let canonical_cwd = cwd.canonicalize().map_err(|e| format!("cwd error: {e}"))?;

        let target_raw = Path::new(target);
        let target_abs = if target_raw.is_absolute() {
            target_raw.to_path_buf()
        } else {
            cwd.join(target_raw)
        };

        // Validate target is within CWD (check parent since file may not exist yet).
        let target_parent = target_abs
            .parent()
            .ok_or_else(|| format!("invalid target path \"{target}\""))?;
        let canonical_parent = target_parent
            .canonicalize()
            .map_err(|e| format!("invalid target path \"{target}\": {e}"))?;
        if !canonical_parent.starts_with(&canonical_cwd) {
            return Err(format!(
                "target path \"{target}\" resolves outside the working directory"
            ));
        }

        let result = elmq::move_decl::execute_move_declaration(
            &source_path,
            names,
            &target_abs,
            copy_shared_helpers,
            dry_run,
        )
        .map_err(|e| e.to_string())?;

        serde_json::to_string_pretty(&result).map_err(|e| format!("JSON error: {e}"))
    }
}

// -- Server entry point --

pub async fn run_mcp_server() -> Result<()> {
    let server = ElmqServer::new();
    let service = server
        .serve(stdio())
        .await
        .context("failed to start MCP server")?;
    service.waiting().await?;
    Ok(())
}
