//! `elmq grep` — regex search over Elm sources that annotates each match with
//! its enclosing top-level declaration.
//!
//! This is the "step 0" locator command. Plain `rg` can tell you file and line;
//! this command additionally tells you the name of the containing declaration so
//! the result can be piped into `elmq get` without reading the whole file.
//!
//! Design highlights (see `openspec/changes/add-grep-command/design.md`):
//!
//! - Project discovery walks up for `elm.json`; falls back to CWD-recursive walk
//!   when none is found. Both paths honor `.gitignore` via the `ignore` crate.
//! - Per file: one tree-sitter parse + one regex scan. Top-level declaration
//!   byte ranges are collected once and searched via `partition_point`.
//! - Comment and string-literal matches are filtered by default; two independent
//!   flags (`--include-comments`, `--include-strings`) opt each back in.
//! - Only top-level declarations are tracked — matches inside nested let-bindings
//!   resolve to the enclosing top-level decl, not the inner name, because
//!   `elmq get` only addresses top-level names.

use crate::parser;
use anyhow::{Context, Result};
use ignore::{WalkBuilder, types::TypesBuilder};
use regex::RegexBuilder;
use std::io::Write;
use std::path::{Path, PathBuf};
use tree_sitter::{Node, Tree};

/// User-facing arguments for `elmq grep`, one-to-one with the clap derive in
/// `src/cli.rs`.
#[derive(Debug, Clone)]
pub struct GrepArgs {
    pub pattern: String,
    pub path: Option<PathBuf>,
    pub fixed: bool,
    pub ignore_case: bool,
    pub include_comments: bool,
    pub include_strings: bool,
    pub format: GrepFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrepFormat {
    Compact,
    Json,
}

/// Top-level declaration byte range used for offset → decl mapping.
#[derive(Debug, Clone)]
struct DeclRange {
    start: usize,
    end: usize,
    name: String,
    kind: &'static str,
}

/// Excluded byte range (comment or string literal) used for filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExcludedKind {
    Comment,
    String,
}

#[derive(Debug, Clone, Copy)]
struct ExcludedRange {
    start: usize,
    end: usize,
    kind: ExcludedKind,
}

/// A single grep hit ready to be formatted and emitted.
struct GrepMatch<'a> {
    file_display: String,
    module: Option<String>,
    line: usize,
    column: usize,
    decl_name: Option<&'a str>,
    decl_kind: Option<&'static str>,
    match_text: &'a str,
    line_text: &'a str,
}

/// Top-level entry point called from `main.rs`. Returns the process exit code
/// (0 = matches found, 1 = no matches, 2 = error). Exit codes match ripgrep.
pub fn execute(args: GrepArgs) -> i32 {
    match run(&args) {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(e) => {
            eprintln!("error: {e:#}");
            2
        }
    }
}

fn run(args: &GrepArgs) -> Result<bool> {
    let regex = build_regex(args)?;
    let discovery = discover_files(args.path.as_deref())?;
    let display_root = discovery.display_root;
    let files = discovery.files;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut any_match = false;

    for file in &files {
        let source = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("warning: could not read {}: {}", file.display(), e);
                continue;
            }
        };

        let file_display = display_path(file, display_root.as_deref());

        // Parse the file. On parse failure, still run the regex but with empty
        // decl/exclusion ranges and a null decl for every match.
        let (decl_ranges, excluded_ranges, module) = match parser::parse(&source) {
            Ok(tree) => {
                let decls = collect_decl_ranges(&tree, &source);
                let excluded = collect_excluded_ranges(&tree, &source);
                let module = extract_module_name(&tree, &source);
                (decls, excluded, module)
            }
            Err(_) => {
                eprintln!(
                    "warning: could not parse {} as Elm; reporting regex matches without declaration context",
                    file.display()
                );
                (Vec::new(), Vec::new(), None)
            }
        };

        for mat in regex.find_iter(&source) {
            let offset = mat.start();

            // Filter comment/string literal matches unless flags opt in.
            if let Some(kind) = offset_excluded_kind(offset, &excluded_ranges)
                && !match kind {
                    ExcludedKind::Comment => args.include_comments,
                    ExcludedKind::String => args.include_strings,
                }
            {
                continue;
            }

            let (line, column) = line_col_at(&source, offset);
            let line_text = source_line(&source, line);
            let enclosing = offset_to_decl(offset, &decl_ranges);

            let hit = GrepMatch {
                file_display: file_display.clone(),
                module: module.clone(),
                line,
                column,
                decl_name: enclosing.map(|d| d.name.as_str()),
                decl_kind: enclosing.map(|d| d.kind),
                match_text: mat.as_str(),
                line_text,
            };

            match emit(&mut out, &hit, args.format) {
                Ok(()) => {}
                Err(e) if is_broken_pipe(&e) => {
                    // Downstream reader closed the pipe (e.g. `| head`). This
                    // is the expected termination path for interactive use;
                    // flush-and-exit silently rather than emitting an error.
                    out.flush().ok();
                    return Ok(any_match);
                }
                Err(e) => return Err(e),
            }
            any_match = true;
        }
    }

    out.flush().ok();
    Ok(any_match)
}

/// Detect broken-pipe errors anywhere in the error chain.
fn is_broken_pipe(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io| io.kind() == std::io::ErrorKind::BrokenPipe)
    })
}

// ---------------- regex ----------------

fn build_regex(args: &GrepArgs) -> Result<regex::Regex> {
    let pat = if args.fixed {
        regex::escape(&args.pattern)
    } else {
        args.pattern.clone()
    };
    RegexBuilder::new(&pat)
        .case_insensitive(args.ignore_case)
        .build()
        .with_context(|| format!("invalid regex: {}", args.pattern))
}

// ---------------- file discovery ----------------

/// Result of file discovery: the set of files to search plus the root against
/// which they should be displayed. Output paths are rendered relative to this
/// root so pipelines like `elmq grep … | elmq get …` receive stable paths
/// regardless of the invocation directory.
struct Discovery {
    files: Vec<PathBuf>,
    /// Directory to show paths relative to. For projects with `elm.json`, this
    /// is the directory containing `elm.json`. For the CWD-fallback path,
    /// this is the CWD. `None` only if a root cannot be determined.
    display_root: Option<PathBuf>,
}

/// Resolve the set of `.elm` files to search.
///
/// Strategy:
/// 1. Walk up from CWD looking for `elm.json`. If found, use its
///    `source-directories` as roots. This works correctly from monorepo
///    subdirectories because the walker resolves from the ancestor `elm.json`.
/// 2. Otherwise fall back to walking CWD recursively.
///
/// In both cases the walker honors `.gitignore` (via the `ignore` crate) and
/// excludes hidden directories. When `path_filter` is provided, only files
/// whose canonical path starts with that filter (also canonicalized) are kept.
fn discover_files(path_filter: Option<&Path>) -> Result<Discovery> {
    use crate::project::Project;

    let cwd = std::env::current_dir().context("could not determine current directory")?;

    let (roots, display_root): (Vec<PathBuf>, Option<PathBuf>) = match Project::try_discover(&cwd)?
    {
        Some(project) => (project.source_dirs.clone(), Some(project.root)),
        None => (vec![cwd.clone()], Some(cwd.clone())),
    };

    let canonical_filter = path_filter
        .map(|p| {
            p.canonicalize()
                .with_context(|| format!("path not found: {}", p.display()))
        })
        .transpose()?;

    let mut types = TypesBuilder::new();
    types
        .add("elm", "*.elm")
        .context("failed to register elm file type")?;
    types.select("elm");
    let matcher = types.build().context("failed to build file type matcher")?;

    let mut files: Vec<PathBuf> = Vec::new();
    let mut seen: std::collections::BTreeSet<PathBuf> = std::collections::BTreeSet::new();

    for root in &roots {
        let mut builder = WalkBuilder::new(root);
        builder
            .hidden(true)
            .git_ignore(true)
            .git_exclude(true)
            .ignore(true)
            .parents(true)
            .types(matcher.clone());

        for entry in builder.build() {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                continue;
            }
            let path = entry.into_path();
            // Canonicalize once for dedup + filter checks.
            let canonical = match path.canonicalize() {
                Ok(c) => c,
                Err(_) => path.clone(),
            };
            if let Some(filter) = &canonical_filter
                && !canonical.starts_with(filter)
            {
                continue;
            }
            if seen.insert(canonical.clone()) {
                files.push(path);
            }
        }
    }

    files.sort();
    Ok(Discovery {
        files,
        display_root,
    })
}

// ---------------- tree walking ----------------

/// Collect top-level declaration byte ranges from the parse tree.
///
/// Pairs type annotations with their following value declaration so both
/// positions map to the same declaration name, matching how `extract_summary`
/// groups them. The resulting vec is sorted by `start` (tree-sitter guarantees
/// child order).
fn collect_decl_ranges(tree: &Tree, source: &str) -> Vec<DeclRange> {
    let root = tree.root_node();
    let children: Vec<Node> = root.named_children(&mut root.walk()).collect();

    let mut ranges: Vec<DeclRange> = Vec::new();
    let mut i = 0;
    while i < children.len() {
        let node = children[i];
        match node.kind() {
            "type_annotation" => {
                let name = node_field_text(&node, "name", source).unwrap_or_default();
                if i + 1 < children.len() && children[i + 1].kind() == "value_declaration" {
                    let val_node = children[i + 1];
                    ranges.push(DeclRange {
                        start: node.start_byte(),
                        end: val_node.end_byte(),
                        name,
                        kind: "function",
                    });
                    i += 2;
                    continue;
                }
                ranges.push(DeclRange {
                    start: node.start_byte(),
                    end: node.end_byte(),
                    name,
                    kind: "function",
                });
            }
            "value_declaration" => {
                let name = value_decl_name(&node, source).unwrap_or_default();
                ranges.push(DeclRange {
                    start: node.start_byte(),
                    end: node.end_byte(),
                    name,
                    kind: "function",
                });
            }
            "type_declaration" => {
                let name = node_field_text(&node, "name", source).unwrap_or_default();
                ranges.push(DeclRange {
                    start: node.start_byte(),
                    end: node.end_byte(),
                    name,
                    kind: "type",
                });
            }
            "type_alias_declaration" => {
                let name = node_field_text(&node, "name", source).unwrap_or_default();
                ranges.push(DeclRange {
                    start: node.start_byte(),
                    end: node.end_byte(),
                    name,
                    kind: "type_alias",
                });
            }
            "port_annotation" => {
                let name = node_field_text(&node, "name", source).unwrap_or_default();
                ranges.push(DeclRange {
                    start: node.start_byte(),
                    end: node.end_byte(),
                    name,
                    kind: "port",
                });
            }
            _ => {}
        }
        i += 1;
    }

    ranges
}

/// Collect comment and string-literal byte ranges by walking the tree.
fn collect_excluded_ranges(tree: &Tree, _source: &str) -> Vec<ExcludedRange> {
    let mut ranges: Vec<ExcludedRange> = Vec::new();
    let root = tree.root_node();
    walk_excluded(&root, &mut ranges);
    ranges.sort_by_key(|r| r.start);
    ranges
}

fn walk_excluded(node: &Node, out: &mut Vec<ExcludedRange>) {
    let kind = node.kind();
    let classification = match kind {
        "line_comment" | "block_comment" => Some(ExcludedKind::Comment),
        "string_constant_expr"
        | "string_literal"
        | "string_escape"
        | "regular_string_part"
        | "open_quote"
        | "close_quote"
        | "open_quote_multiline"
        | "close_quote_multiline" => Some(ExcludedKind::String),
        _ => None,
    };
    if let Some(k) = classification {
        out.push(ExcludedRange {
            start: node.start_byte(),
            end: node.end_byte(),
            kind: k,
        });
        // Strings/comments are leaves for our purposes — don't descend.
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_excluded(&child, out);
    }
}

fn extract_module_name(tree: &Tree, source: &str) -> Option<String> {
    let root = tree.root_node();
    let module_decl = root.child_by_field_name("moduleDeclaration")?;
    let name_node = module_decl.child_by_field_name("name")?;
    name_node
        .utf8_text(source.as_bytes())
        .ok()
        .map(|s| s.to_string())
}

fn node_field_text(node: &Node, field: &str, source: &str) -> Option<String> {
    let child = node.child_by_field_name(field)?;
    Some(child.utf8_text(source.as_bytes()).ok()?.to_string())
}

fn value_decl_name(node: &Node, source: &str) -> Option<String> {
    let fdl = node.child_by_field_name("functionDeclarationLeft")?;
    let mut cursor = fdl.walk();
    for child in fdl.named_children(&mut cursor) {
        if child.kind() == "lower_case_identifier" {
            return Some(child.utf8_text(source.as_bytes()).ok()?.to_string());
        }
    }
    None
}

// ---------------- offset → decl / excluded lookups ----------------

/// Find the declaration whose byte range contains `offset`.
///
/// Uses `partition_point` over the sorted `ranges` vec. Correct only if ranges
/// do not overlap (top-level decls in Elm never overlap each other).
fn offset_to_decl(offset: usize, ranges: &[DeclRange]) -> Option<&DeclRange> {
    let idx = ranges.partition_point(|r| r.start <= offset);
    if idx == 0 {
        return None;
    }
    let candidate = &ranges[idx - 1];
    if offset < candidate.end {
        Some(candidate)
    } else {
        None
    }
}

/// Return the `ExcludedKind` of the range containing `offset`, or `None` if
/// `offset` is not inside any excluded range.
fn offset_excluded_kind(offset: usize, ranges: &[ExcludedRange]) -> Option<ExcludedKind> {
    // Linear scan is fine: excluded ranges per file are small, and we short-
    // circuit on the first containing range.
    for r in ranges {
        if r.start > offset {
            return None;
        }
        if offset < r.end {
            return Some(r.kind);
        }
    }
    None
}

// ---------------- position + line helpers ----------------

fn line_col_at(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut last_nl = 0usize;
    for (i, b) in source.as_bytes().iter().enumerate() {
        if i >= offset {
            break;
        }
        if *b == b'\n' {
            line += 1;
            last_nl = i + 1;
        }
    }
    // +1 for 1-based column.
    let column = offset.saturating_sub(last_nl) + 1;
    (line, column)
}

fn source_line(source: &str, line: usize) -> &str {
    source.lines().nth(line.saturating_sub(1)).unwrap_or("")
}

// ---------------- output ----------------

fn emit(out: &mut impl Write, hit: &GrepMatch, format: GrepFormat) -> Result<()> {
    match format {
        GrepFormat::Compact => {
            let decl = hit.decl_name.unwrap_or("-");
            writeln!(
                out,
                "{}:{}:{}:{}",
                hit.file_display, hit.line, decl, hit.line_text
            )?;
        }
        GrepFormat::Json => {
            let value = serde_json::json!({
                "file": hit.file_display,
                "line": hit.line,
                "column": hit.column,
                "module": hit.module,
                "decl": hit.decl_name,
                "decl_kind": hit.decl_kind,
                "match": hit.match_text,
                "line_text": hit.line_text,
            });
            writeln!(out, "{}", serde_json::to_string(&value)?)?;
        }
    }
    Ok(())
}

/// Render a file path relative to the discovery root using forward slashes.
///
/// Tries `display_root` first (the `elm.json` directory or, in fallback mode,
/// the CWD) so output lines are stable regardless of the caller's invocation
/// directory — this is what makes `elmq grep | elmq get` pipelines work from
/// monorepo subdirectories. Canonicalizes both sides to strip surprises like
/// symlinks or relative prefixes before attempting `strip_prefix`. Falls back
/// to `path.display().to_string()` if no sensible relative path can be found,
/// which preserves absolute paths verbatim instead of corrupting them with
/// stray leading slashes.
fn display_path(path: &Path, display_root: Option<&Path>) -> String {
    if let Some(root) = display_root {
        let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        if let Ok(stripped) = canonical_path.strip_prefix(&canonical_root) {
            let joined = stripped
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            if !joined.is_empty() {
                return joined;
            }
        }
    }
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"module Sample exposing (..)

import Http


fetchUsers : Cmd msg
fetchUsers =
    Http.get { url = "/users" }


type Msg
    = Load
    | Retry
"#;

    #[test]
    fn decl_ranges_include_annotation_and_body() {
        let tree = parser::parse(SAMPLE).unwrap();
        let ranges = collect_decl_ranges(&tree, SAMPLE);

        let fetch = ranges
            .iter()
            .find(|r| r.name == "fetchUsers")
            .expect("fetchUsers range");
        assert_eq!(fetch.kind, "function");

        // The annotation line starts with "fetchUsers :", so the range's start
        // byte should index into "fetchUsers" (not into "import" or the module
        // header), confirming annotation inclusion.
        let at_start = &SAMPLE[fetch.start..];
        assert!(at_start.starts_with("fetchUsers"));

        // The body ends with the closing brace of the record; ensure the range
        // extends past "Http.get".
        let body_needle = SAMPLE.find("Http.get").unwrap();
        assert!(fetch.start < body_needle);
        assert!(fetch.end > body_needle);
    }

    #[test]
    fn offset_to_decl_finds_enclosing_function() {
        let tree = parser::parse(SAMPLE).unwrap();
        let ranges = collect_decl_ranges(&tree, SAMPLE);

        let http_offset = SAMPLE.find("Http.get").unwrap();
        let decl = offset_to_decl(http_offset, &ranges).expect("should find decl");
        assert_eq!(decl.name, "fetchUsers");
    }

    #[test]
    fn offset_to_decl_returns_none_for_import_line() {
        let tree = parser::parse(SAMPLE).unwrap();
        let ranges = collect_decl_ranges(&tree, SAMPLE);

        let import_offset = SAMPLE.find("import Http").unwrap();
        assert!(offset_to_decl(import_offset, &ranges).is_none());
    }

    #[test]
    fn string_literal_match_is_classified_excluded() {
        let tree = parser::parse(SAMPLE).unwrap();
        let excluded = collect_excluded_ranges(&tree, SAMPLE);

        let slash_users = SAMPLE.find("\"/users\"").unwrap();
        let inside_string = slash_users + 2; // inside the string literal body
        let kind = offset_excluded_kind(inside_string, &excluded);
        assert_eq!(kind, Some(ExcludedKind::String));
    }

    #[test]
    fn comment_match_is_classified_excluded() {
        let src = "module M exposing (..)\n\n-- TODO: fix\nfoo = 1\n";
        let tree = parser::parse(src).unwrap();
        let excluded = collect_excluded_ranges(&tree, src);

        let todo = src.find("TODO").unwrap();
        let kind = offset_excluded_kind(todo, &excluded);
        assert_eq!(kind, Some(ExcludedKind::Comment));
    }

    #[test]
    fn line_col_computes_correctly() {
        let src = "abc\ndefg\nhi";
        assert_eq!(line_col_at(src, 0), (1, 1));
        assert_eq!(line_col_at(src, 4), (2, 1));
        assert_eq!(line_col_at(src, 6), (2, 3));
        assert_eq!(line_col_at(src, 9), (3, 1));
    }

    #[test]
    fn build_regex_fixed_mode_escapes_metachars() {
        let args = GrepArgs {
            pattern: "a.b".to_string(),
            path: None,
            fixed: true,
            ignore_case: false,
            include_comments: false,
            include_strings: false,
            format: GrepFormat::Compact,
        };
        let re = build_regex(&args).unwrap();
        assert!(re.is_match("a.b"));
        assert!(!re.is_match("aXb"));
    }

    #[test]
    fn build_regex_case_insensitive() {
        let args = GrepArgs {
            pattern: "http".to_string(),
            path: None,
            fixed: false,
            ignore_case: true,
            include_comments: false,
            include_strings: false,
            format: GrepFormat::Compact,
        };
        let re = build_regex(&args).unwrap();
        assert!(re.is_match("Http"));
        assert!(re.is_match("HTTP"));
    }

    #[test]
    fn invalid_regex_returns_error() {
        let args = GrepArgs {
            pattern: "[unclosed".to_string(),
            path: None,
            fixed: false,
            ignore_case: false,
            include_comments: false,
            include_strings: false,
            format: GrepFormat::Compact,
        };
        assert!(build_regex(&args).is_err());
    }
}
