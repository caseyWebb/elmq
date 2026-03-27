mod cli;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Command, Format};
use elmq::{DeclarationKind, FileSummary};
use elmq::parser;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::List { file, format, docs } => {
            let source = std::fs::read_to_string(&file)
                .with_context(|| format!("could not read file: {}", file.display()))?;

            let tree = parser::parse(&source)?;

            if tree.root_node().has_error() {
                eprintln!(
                    "warning: parse errors detected in {}",
                    file.display()
                );
            }

            let summary = parser::extract_summary(&tree, &source);

            match format {
                Format::Compact => print_compact(&summary, docs),
                Format::Json => print_json(&summary)?,
            }
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
                if show_docs
                    && let Some(doc) = &d.doc_comment
                {
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
                println!(
                    "  {:<name_w$}  L{}-{}",
                    d.name, d.start_line, d.end_line,
                );
            } else {
                println!(
                    "  {:<name_w$}  {}  L{}-{}",
                    d.name, type_str, d.start_line, d.end_line,
                );
            }
            if show_docs
                && let Some(doc) = &d.doc_comment
            {
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
                println!(
                    "  {:<name_w$}  L{}-{}",
                    d.name, d.start_line, d.end_line,
                );
            } else {
                println!(
                    "  {:<name_w$}  {}  L{}-{}",
                    d.name, type_str, d.start_line, d.end_line,
                );
            }
            if show_docs
                && let Some(doc) = &d.doc_comment
            {
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

fn print_json(summary: &FileSummary) -> Result<()> {
    let json = serde_json::to_string_pretty(summary)?;
    println!("{json}");
    Ok(())
}
