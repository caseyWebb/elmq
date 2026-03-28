mod cli;
mod mcp;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Command, Format, ImportCommand};
use elmq::parser;
use elmq::project;
use elmq::refs;
use elmq::writer;
use elmq::{Declaration, DeclarationKind, FileSummary};
use std::io::Read;
use std::path::Path;

fn load_and_parse(file: &Path) -> Result<(String, FileSummary)> {
    let source = std::fs::read_to_string(file)
        .with_context(|| format!("could not read file: {}", file.display()))?;

    let tree = parser::parse(&source)?;

    if tree.root_node().has_error() {
        eprintln!("warning: parse errors detected in {}", file.display());
    }

    let summary = parser::extract_summary(&tree, &source);
    Ok((source, summary))
}

fn read_stdin() -> Result<String> {
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .context("failed to read from stdin")?;
    Ok(buf)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::List { file, format, docs } => {
            let (_source, summary) = load_and_parse(&file)?;

            match format {
                Format::Compact => print_compact(&summary, docs),
                Format::Json => print_json(&summary)?,
            }
        }
        Command::Get { file, name, format } => {
            let (source, summary) = load_and_parse(&file)?;

            let decl = summary.find_declaration(&name).with_context(|| {
                format!("declaration '{}' not found in {}", name, file.display())
            })?;

            let source_lines: Vec<&str> = source.lines().collect();
            let start = decl.start_line - 1;
            let end = decl.end_line.min(source_lines.len());
            let decl_source = source_lines[start..end].join("\n");

            match format {
                Format::Compact => println!("{decl_source}"),
                Format::Json => print_get_json(decl, &decl_source)?,
            }
        }
        Command::Set { file, name } => {
            let (source, summary) = load_and_parse(&file)?;
            let new_source = read_stdin()?;

            let decl_name = if let Some(name) = name {
                name
            } else {
                parser::extract_declaration_name(&new_source).context(
                    "could not parse declaration name from stdin (use --name to specify)",
                )?
            };

            let result = writer::upsert_declaration(&source, &summary, &decl_name, &new_source);
            writer::atomic_write(&file, &result)?;
        }
        Command::Patch {
            file,
            name,
            old,
            new,
        } => {
            let (source, summary) = load_and_parse(&file)?;
            let result = writer::patch_declaration(&source, &summary, &name, &old, &new)?;
            writer::atomic_write(&file, &result)?;
        }
        Command::Rm { file, name } => {
            let (source, summary) = load_and_parse(&file)?;
            let result = writer::remove_declaration(&source, &summary, &name)?;
            writer::atomic_write(&file, &result)?;
        }
        Command::Import { command } => match command {
            ImportCommand::Add { file, import } => {
                let (source, summary) = load_and_parse(&file)?;
                let result = writer::add_import(&source, &summary, &import);
                writer::atomic_write(&file, &result)?;
            }
            ImportCommand::Remove { file, module_name } => {
                let (source, _summary) = load_and_parse(&file)?;
                let result = writer::remove_import(&source, &module_name)?;
                writer::atomic_write(&file, &result)?;
            }
        },
        Command::Expose { file, item } => {
            let (source, summary) = load_and_parse(&file)?;
            let result = writer::expose(&source, &summary, &item)?;
            writer::atomic_write(&file, &result)?;
        }
        Command::Unexpose { file, item } => {
            let (source, summary) = load_and_parse(&file)?;
            let result = writer::unexpose(&source, &summary, &item)?;
            writer::atomic_write(&file, &result)?;
        }
        Command::Mv {
            file,
            new_path,
            format,
            dry_run,
        } => {
            let old_path = file
                .canonicalize()
                .with_context(|| format!("file not found: {}", file.display()))?;

            // For CLI, create parent dirs before resolving (so canonicalize works).
            if !dry_run
                && let Some(parent) = new_path.parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("could not create directory: {}", parent.display()))?;
            }

            let resolved_new = project::resolve_new_path(&new_path)?;
            let result = project::execute_mv(&old_path, &resolved_new, dry_run)?;

            match format {
                Format::Compact => {
                    let prefix = if dry_run { "(dry run) " } else { "" };
                    println!(
                        "{prefix}renamed {} -> {}",
                        result.old_display, result.new_display
                    );
                    for f in &result.updated_files {
                        println!("{prefix}updated {f}");
                    }
                }
                Format::Json => {
                    let json = serde_json::json!({
                        "dry_run": dry_run,
                        "renamed": {
                            "from": result.old_display,
                            "to": result.new_display,
                        },
                        "updated": result.updated_files,
                    });
                    println!("{}", serde_json::to_string_pretty(&json)?);
                }
            }
        }
        Command::Refs { file, name, format } => {
            let canonical = file
                .canonicalize()
                .with_context(|| format!("file not found: {}", file.display()))?;

            let project = project::Project::discover(&canonical)?;
            let target_module = project.module_name(&canonical)?;
            let matches = refs::find_refs(&project, &target_module, name.as_deref())?;

            match format {
                Format::Compact => {
                    for r in &matches {
                        if let Some(text) = &r.text {
                            println!("{}:{}: {}", r.file, r.line, text);
                        } else {
                            println!("{}:{}", r.file, r.line);
                        }
                    }
                }
                Format::Json => {
                    let json = serde_json::to_string_pretty(&matches)?;
                    println!("{json}");
                }
            }
        }
        Command::Mcp => {
            tokio::runtime::Runtime::new()
                .context("failed to create tokio runtime")?
                .block_on(mcp::run_mcp_server())?;
        }
    }

    Ok(())
}

fn print_compact(summary: &FileSummary, show_docs: bool) {
    println!("{}", summary.module_line);

    if !summary.imports.is_empty() {
        println!();
        println!("imports:");
        for imp in &summary.imports {
            println!("  {imp}");
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
            println!();
            println!("{label}");
            for d in decls {
                println!("  {:<name_w$}  L{}-{}", d.name, d.start_line, d.end_line);
                if show_docs && let Some(doc) = &d.doc_comment {
                    print_doc_comment(doc);
                }
            }
        }
    }

    if !functions.is_empty() {
        let name_w = functions.iter().map(|d| d.name.len()).max().unwrap_or(0);
        println!();
        println!("functions:");
        for d in &functions {
            let type_str = d.type_annotation.as_deref().unwrap_or("");
            if type_str.is_empty() {
                println!("  {:<name_w$}  L{}-{}", d.name, d.start_line, d.end_line,);
            } else {
                println!(
                    "  {:<name_w$}  {}  L{}-{}",
                    d.name, type_str, d.start_line, d.end_line,
                );
            }
            if show_docs && let Some(doc) = &d.doc_comment {
                print_doc_comment(doc);
            }
        }
    }

    if !ports.is_empty() {
        let name_w = ports.iter().map(|d| d.name.len()).max().unwrap_or(0);
        println!();
        println!("ports:");
        for d in &ports {
            let type_str = d.type_annotation.as_deref().unwrap_or("");
            if type_str.is_empty() {
                println!("  {:<name_w$}  L{}-{}", d.name, d.start_line, d.end_line,);
            } else {
                println!(
                    "  {:<name_w$}  {}  L{}-{}",
                    d.name, type_str, d.start_line, d.end_line,
                );
            }
            if show_docs && let Some(doc) = &d.doc_comment {
                print_doc_comment(doc);
            }
        }
    }
}

fn print_doc_comment(doc: &str) {
    let stripped = doc
        .strip_prefix("{-|")
        .unwrap_or(doc)
        .strip_suffix("-}")
        .unwrap_or(doc)
        .trim();
    for line in stripped.lines() {
        println!("    {}", line.trim());
    }
}

fn print_get_json(decl: &Declaration, source: &str) -> Result<()> {
    let json = serde_json::json!({
        "name": decl.name,
        "kind": decl.kind,
        "source": source,
        "start_line": decl.start_line,
        "end_line": decl.end_line,
    });
    println!("{}", serde_json::to_string_pretty(&json)?);
    Ok(())
}

fn print_json(summary: &FileSummary) -> Result<()> {
    let json = serde_json::to_string_pretty(summary)?;
    println!("{json}");
    Ok(())
}
