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
                 READ:\n\
                 - elm_summary: file structure (module, imports, declarations with types/line numbers) in ~10% of the tokens\n\
                 - elm_get: extract one declaration's full source by name\n\
                 \n\
                 WRITE (all atomic):\n\
                 - elm_set: upsert a declaration\n\
                 - elm_patch: find-replace within a declaration\n\
                 - elm_rm: remove a declaration\n\
                 - elm_add_import / elm_rm_import: manage imports\n\
                 - elm_expose / elm_unexpose: manage exposing list\n\
                 \n\
                 PROJECT-WIDE (all atomic):\n\
                 - elm_mv: rename/move module, update all references\n\
                 - elm_rename: rename declaration, update all references\n\
                 - elm_move_decl: move declarations between modules\n\
                 - elm_add_variant / elm_rm_variant: add/remove type constructors, propagate case expressions\n\
                 \n\
                 SEARCH:\n\
                 - elm_refs: find all references to a module or declaration\n\
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

// The Anthropic API rejects type arrays like ["string", "null"] that schemars
// generates for Option<T>. Strip them down to just the non-null type.
fn strip_null_types(schema: &mut schemars::Schema) {
    if let Some(obj) = schema.as_object_mut() {
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
#[schemars(transform = strip_null_types)]
pub struct SummaryParams {
    /// Path to the Elm file
    pub file: String,
    /// Output format: "compact" (default) or "json"
    pub format: Option<String>,
    /// Include doc comments in output
    pub docs: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[schemars(transform = strip_null_types)]
pub struct GetParams {
    /// Path to the Elm file
    pub file: String,
    /// Name of the declaration to extract
    pub name: String,
    /// Output format: "compact" (default) or "json"
    pub format: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[schemars(transform = strip_null_types)]
pub struct SetParams {
    /// Path to the Elm file
    pub file: String,
    /// Full source text of the declaration
    pub source: String,
    /// Override the declaration name (default: parsed from source)
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PatchParams {
    /// Path to the Elm file
    pub file: String,
    /// Name of the declaration to patch
    pub name: String,
    /// Text to find within the declaration
    pub old: String,
    /// Replacement text
    pub new: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RmParams {
    /// Path to the Elm file
    pub file: String,
    /// Name of the declaration to remove
    pub name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddImportParams {
    /// Path to the Elm file
    pub file: String,
    /// Import clause, e.g. "Html exposing (Html, div)"
    pub import: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RmImportParams {
    /// Path to the Elm file
    pub file: String,
    /// Module name to remove, e.g. "Html"
    pub module_name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExposeParams {
    /// Path to the Elm file
    pub file: String,
    /// Item to expose, e.g. "update" or "Msg(..)"
    pub item: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UnexposeParams {
    /// Path to the Elm file
    pub file: String,
    /// Item to unexpose, e.g. "helper"
    pub item: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[schemars(transform = strip_null_types)]
pub struct MvParams {
    /// Path to the Elm file
    pub file: String,
    /// New file path for the module
    pub new_path: String,
    /// If true, preview changes without writing
    pub dry_run: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[schemars(transform = strip_null_types)]
pub struct RenameParams {
    /// Path to the Elm file
    pub file: String,
    /// Current name of the declaration
    pub name: String,
    /// New name for the declaration
    pub new: String,
    /// If true, preview changes without writing
    pub dry_run: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[schemars(transform = strip_null_types)]
pub struct MoveDeclParams {
    /// Path to the Elm file
    pub file: String,
    /// Names of declarations to move
    pub names: Vec<String>,
    /// Path to the target Elm file
    pub target: String,
    /// Copy shared helpers instead of erroring
    pub copy_shared_helpers: Option<bool>,
    /// If true, preview changes without writing
    pub dry_run: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[schemars(transform = strip_null_types)]
pub struct AddVariantParams {
    /// Path to the Elm file
    pub file: String,
    /// Name of the custom type (e.g. "Msg")
    pub type_name: String,
    /// Variant definition (e.g. "SetName String")
    pub definition: String,
    /// If true, preview changes without writing
    pub dry_run: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[schemars(transform = strip_null_types)]
pub struct RmVariantParams {
    /// Path to the Elm file
    pub file: String,
    /// Name of the custom type (e.g. "Msg")
    pub type_name: String,
    /// Constructor name to remove (e.g. "Decrement")
    pub constructor: String,
    /// If true, preview changes without writing
    pub dry_run: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[schemars(transform = strip_null_types)]
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
        name = "elm_set",
        description = "Upsert a declaration (function, type, type alias, or port) in an Elm file. Inserts if new, replaces if it already exists. Atomic write."
    )]
    fn elm_set(&self, Parameters(params): Parameters<SetParams>) -> Result<String, String> {
        let (source, summary) = load_and_parse(&params.file)?;
        let path = validate_path(&params.file)?;

        let decl_name = if let Some(name) = &params.name {
            name.clone()
        } else {
            parser::extract_declaration_name(&params.source)
                .ok_or("could not parse declaration name from source (provide \"name\")")?
        };

        let result = writer::upsert_declaration(&source, &summary, &decl_name, &params.source);
        writer::atomic_write(&path, &result).map_err(|e| format!("write error: {e}"))?;
        Ok(format!("set {decl_name} in {}", params.file))
    }

    #[tool(
        name = "elm_patch",
        description = "Find-and-replace text within a specific declaration in an Elm file. Atomic write."
    )]
    fn elm_patch(&self, Parameters(params): Parameters<PatchParams>) -> Result<String, String> {
        let (source, summary) = load_and_parse(&params.file)?;
        let path = validate_path(&params.file)?;

        let result =
            writer::patch_declaration(&source, &summary, &params.name, &params.old, &params.new)
                .map_err(|e| e.to_string())?;
        writer::atomic_write(&path, &result).map_err(|e| format!("write error: {e}"))?;
        Ok(format!("patched {} in {}", params.name, params.file))
    }

    #[tool(
        name = "elm_rm",
        description = "Remove a declaration (and its type annotation and doc comment) from an Elm file. Atomic write."
    )]
    fn elm_rm(&self, Parameters(params): Parameters<RmParams>) -> Result<String, String> {
        let (source, summary) = load_and_parse(&params.file)?;
        let path = validate_path(&params.file)?;

        let result = writer::remove_declaration(&source, &summary, &params.name)
            .map_err(|e| e.to_string())?;
        writer::atomic_write(&path, &result).map_err(|e| format!("write error: {e}"))?;
        Ok(format!("removed {} from {}", params.name, params.file))
    }

    #[tool(
        name = "elm_add_import",
        description = "Add or replace an import clause in an Elm file. Atomic write."
    )]
    fn elm_add_import(
        &self,
        Parameters(params): Parameters<AddImportParams>,
    ) -> Result<String, String> {
        let (source, summary) = load_and_parse(&params.file)?;
        let path = validate_path(&params.file)?;

        let result = writer::add_import(&source, &summary, &params.import);
        writer::atomic_write(&path, &result).map_err(|e| format!("write error: {e}"))?;
        Ok(format!("added import in {}", params.file))
    }

    #[tool(
        name = "elm_rm_import",
        description = "Remove an import by module name from an Elm file. Atomic write."
    )]
    fn elm_rm_import(
        &self,
        Parameters(params): Parameters<RmImportParams>,
    ) -> Result<String, String> {
        let (source, _summary) = load_and_parse(&params.file)?;
        let path = validate_path(&params.file)?;

        let result =
            writer::remove_import(&source, &params.module_name).map_err(|e| e.to_string())?;
        writer::atomic_write(&path, &result).map_err(|e| format!("write error: {e}"))?;
        Ok(format!(
            "removed import {} from {}",
            params.module_name, params.file
        ))
    }

    #[tool(
        name = "elm_expose",
        description = "Add an item to the module's exposing list in an Elm file. Atomic write."
    )]
    fn elm_expose(&self, Parameters(params): Parameters<ExposeParams>) -> Result<String, String> {
        let (source, summary) = load_and_parse(&params.file)?;
        let path = validate_path(&params.file)?;

        let result = writer::expose(&source, &summary, &params.item).map_err(|e| e.to_string())?;
        writer::atomic_write(&path, &result).map_err(|e| format!("write error: {e}"))?;
        Ok(format!("exposed {} in {}", params.item, params.file))
    }

    #[tool(
        name = "elm_unexpose",
        description = "Remove an item from the module's exposing list in an Elm file. Atomic write."
    )]
    fn elm_unexpose(
        &self,
        Parameters(params): Parameters<UnexposeParams>,
    ) -> Result<String, String> {
        let (source, summary) = load_and_parse(&params.file)?;
        let path = validate_path(&params.file)?;

        let result =
            writer::unexpose(&source, &summary, &params.item).map_err(|e| e.to_string())?;
        writer::atomic_write(&path, &result).map_err(|e| format!("write error: {e}"))?;
        Ok(format!("unexposed {} in {}", params.item, params.file))
    }

    #[tool(
        name = "elm_mv",
        description = "Rename/move an Elm module file and update all imports and qualified references across the project. Atomic writes."
    )]
    fn elm_mv(&self, Parameters(params): Parameters<MvParams>) -> Result<String, String> {
        self.handle_mv(
            &params.file,
            &params.new_path,
            params.dry_run.unwrap_or(false),
        )
    }

    #[tool(
        name = "elm_rename",
        description = "Rename a declaration and update all references across the Elm project. Atomic writes."
    )]
    fn elm_rename(&self, Parameters(params): Parameters<RenameParams>) -> Result<String, String> {
        self.handle_rename(
            &params.file,
            &params.name,
            &params.new,
            params.dry_run.unwrap_or(false),
        )
    }

    #[tool(
        name = "elm_move_decl",
        description = "Move declarations from one Elm module to another with import-aware body rewriting and project-wide reference updates. Atomic writes."
    )]
    fn elm_move_decl(
        &self,
        Parameters(params): Parameters<MoveDeclParams>,
    ) -> Result<String, String> {
        self.handle_move_decl(
            &params.file,
            &params.names,
            &params.target,
            params.copy_shared_helpers.unwrap_or(false),
            params.dry_run.unwrap_or(false),
        )
    }

    #[tool(
        name = "elm_add_variant",
        description = "Add a constructor to a custom type and insert branches in all case expressions project-wide. Atomic writes."
    )]
    fn elm_add_variant(
        &self,
        Parameters(params): Parameters<AddVariantParams>,
    ) -> Result<String, String> {
        let path = validate_path(&params.file)?;
        let result = elmq::variant::execute_add_variant(
            &path,
            &params.type_name,
            &params.definition,
            params.dry_run.unwrap_or(false),
        )
        .map_err(|e: anyhow::Error| e.to_string())?;
        serde_json::to_string_pretty(&result).map_err(|e| format!("JSON error: {e}"))
    }

    #[tool(
        name = "elm_rm_variant",
        description = "Remove a constructor from a custom type and remove branches from all case expressions project-wide. Atomic writes."
    )]
    fn elm_rm_variant(
        &self,
        Parameters(params): Parameters<RmVariantParams>,
    ) -> Result<String, String> {
        let path = validate_path(&params.file)?;
        let result = elmq::variant::execute_rm_variant(
            &path,
            &params.type_name,
            &params.constructor,
            params.dry_run.unwrap_or(false),
        )
        .map_err(|e: anyhow::Error| e.to_string())?;
        serde_json::to_string_pretty(&result).map_err(|e| format!("JSON error: {e}"))
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
