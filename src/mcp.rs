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
        ServerInfo {
            instructions: Some(
                "Query and edit Elm files — like jq for Elm. \
                 Use elm_summary to see file structure, elm_get to read declarations, \
                 elm_edit to modify declarations, and elm_module to manage imports and exposing."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: rmcp::model::Implementation {
                name: "elmq".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

// -- Parameter types --

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SummaryParams {
    /// Path to the Elm file
    pub file: String,
    /// Output format: "compact" (default) or "json"
    pub format: Option<String>,
    /// Include doc comments in output
    pub docs: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetParams {
    /// Path to the Elm file
    pub file: String,
    /// Name of the declaration to extract
    pub name: String,
    /// Output format: "compact" (default) or "json"
    pub format: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EditAction {
    /// Upsert a declaration from source text
    Set,
    /// Find-and-replace within a declaration
    Patch,
    /// Remove a declaration
    Rm,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditParams {
    /// Path to the Elm file
    pub file: String,
    /// Action to perform
    pub action: EditAction,
    /// Full source text of the declaration (required for "set")
    pub source: Option<String>,
    /// Declaration name (optional for "set" where it's parsed from source; required for "patch" and "rm")
    pub name: Option<String>,
    /// Text to find within the declaration (required for "patch")
    pub old: Option<String>,
    /// Replacement text (required for "patch")
    pub new: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModuleAction {
    /// Add or replace an import clause
    AddImport,
    /// Remove an import by module name
    RemoveImport,
    /// Add an item to the module's exposing list
    Expose,
    /// Remove an item from the module's exposing list
    Unexpose,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ModuleParams {
    /// Path to the Elm file
    pub file: String,
    /// Action to perform
    pub action: ModuleAction,
    /// Import clause, e.g. "Html exposing (Html, div)" (required for "add_import")
    pub import: Option<String>,
    /// Module name to remove, e.g. "Html" (required for "remove_import")
    pub module_name: Option<String>,
    /// Item to expose or unexpose, e.g. "update" or "Msg(..)" (required for "expose"/"unexpose")
    pub item: Option<String>,
}

// -- Helpers --

/// Validate that a file path resolves to within the server's working directory.
fn validate_path(file: &str) -> Result<PathBuf, String> {
    let path = Path::new(file);
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("invalid path \"{file}\": {e}"))?;

    let cwd = std::env::current_dir().map_err(|e| format!("could not determine cwd: {e}"))?;
    let canonical_cwd = cwd
        .canonicalize()
        .map_err(|e| format!("could not canonicalize cwd: {e}"))?;

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
        description = "Modify a declaration in an Elm file. Actions: \"set\" (upsert declaration from source text), \"patch\" (find-and-replace within a declaration), \"rm\" (remove a declaration). File is written atomically."
    )]
    fn elm_edit(&self, Parameters(params): Parameters<EditParams>) -> Result<String, String> {
        let (source, summary) = load_and_parse(&params.file)?;
        let path = validate_path(&params.file)?;

        match params.action {
            EditAction::Set => {
                let new_source = params
                    .source
                    .as_deref()
                    .ok_or("\"source\" is required for action \"set\"")?;

                let decl_name = if let Some(name) = &params.name {
                    name.clone()
                } else {
                    parser::extract_declaration_name(new_source)
                        .ok_or("could not parse declaration name from source (provide \"name\")")?
                };

                let result = writer::upsert_declaration(&source, &summary, &decl_name, new_source);
                writer::atomic_write(&path, &result).map_err(|e| format!("write error: {e}"))?;
                Ok(format!("set {decl_name} in {}", params.file))
            }
            EditAction::Patch => {
                let name = params
                    .name
                    .as_deref()
                    .ok_or("\"name\" is required for action \"patch\"")?;
                let old = params
                    .old
                    .as_deref()
                    .ok_or("\"old\" is required for action \"patch\"")?;
                let new = params
                    .new
                    .as_deref()
                    .ok_or("\"new\" is required for action \"patch\"")?;

                let result = writer::patch_declaration(&source, &summary, name, old, new)
                    .map_err(|e| e.to_string())?;
                writer::atomic_write(&path, &result).map_err(|e| format!("write error: {e}"))?;
                Ok(format!("patched {name} in {}", params.file))
            }
            EditAction::Rm => {
                let name = params
                    .name
                    .as_deref()
                    .ok_or("\"name\" is required for action \"rm\"")?;

                let result = writer::remove_declaration(&source, &summary, name)
                    .map_err(|e| e.to_string())?;
                writer::atomic_write(&path, &result).map_err(|e| format!("write error: {e}"))?;
                Ok(format!("removed {name} from {}", params.file))
            }
        }
    }

    #[tool(
        name = "elm_module",
        description = "Manage an Elm file's imports and exposing list. Actions: \"add_import\" (add or replace an import clause), \"remove_import\" (remove an import by module name), \"expose\" (add item to module exposing list), \"unexpose\" (remove item from module exposing list). File is written atomically."
    )]
    fn elm_module(&self, Parameters(params): Parameters<ModuleParams>) -> Result<String, String> {
        let (source, summary) = load_and_parse(&params.file)?;
        let path = validate_path(&params.file)?;

        match params.action {
            ModuleAction::AddImport => {
                let import = params
                    .import
                    .as_deref()
                    .ok_or("\"import\" is required for action \"add_import\"")?;

                let result = writer::add_import(&source, &summary, import);
                writer::atomic_write(&path, &result).map_err(|e| format!("write error: {e}"))?;
                Ok(format!("added import in {}", params.file))
            }
            ModuleAction::RemoveImport => {
                let module_name = params
                    .module_name
                    .as_deref()
                    .ok_or("\"module_name\" is required for action \"remove_import\"")?;

                let result =
                    writer::remove_import(&source, module_name).map_err(|e| e.to_string())?;
                writer::atomic_write(&path, &result).map_err(|e| format!("write error: {e}"))?;
                Ok(format!("removed import {module_name} from {}", params.file))
            }
            ModuleAction::Expose => {
                let item = params
                    .item
                    .as_deref()
                    .ok_or("\"item\" is required for action \"expose\"")?;

                let result = writer::expose(&source, &summary, item).map_err(|e| e.to_string())?;
                writer::atomic_write(&path, &result).map_err(|e| format!("write error: {e}"))?;
                Ok(format!("exposed {item} in {}", params.file))
            }
            ModuleAction::Unexpose => {
                let item = params
                    .item
                    .as_deref()
                    .ok_or("\"item\" is required for action \"unexpose\"")?;

                let result =
                    writer::unexpose(&source, &summary, item).map_err(|e| e.to_string())?;
                writer::atomic_write(&path, &result).map_err(|e| format!("write error: {e}"))?;
                Ok(format!("unexposed {item} in {}", params.file))
            }
        }
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
