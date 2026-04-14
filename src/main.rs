mod cli;
mod framing;

use anyhow::{Context, Result, bail};
use clap::{CommandFactory, FromArgMatches};
use cli::{
    AddCommand, Cli, Command, Format, GrepFormat, RenameCommand, RmCommand, SetCommand,
    VariantReadCommand,
};
use elmq::parser;
use elmq::project;
use elmq::refs;
use elmq::writer;
use elmq::{Declaration, DeclarationKind, FileSummary};
use framing::{ItemResult, format_results};
use std::fmt::Write as _;
use std::io::Read;
use std::path::{Path, PathBuf};

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

/// Write-path read helper. Unlike `load_and_parse`, this returns an error
/// (not a warning) when tree-sitter produces any ERROR or MISSING nodes,
/// so write commands refuse to splice edits into an already-damaged CST.
/// Read-only commands keep using `load_and_parse`.
fn load_and_parse_for_write(file: &Path) -> Result<(String, FileSummary)> {
    let source = std::fs::read_to_string(file)
        .with_context(|| format!("could not read file: {}", file.display()))?;

    let tree = parser::ensure_clean_parse(&source, file)?;
    let summary = parser::extract_summary(&tree, &source);
    Ok((source, summary))
}

/// Read-and-validate helper for write commands that re-parse the source
/// inside a per-iteration loop (rm, import, expose, unexpose). Performs
/// the same hard `has_error()` check up front, then returns just the
/// source string — the loop bodies already construct their own summaries
/// after each mutation.
fn load_source_for_write(file: &Path) -> Result<String> {
    let source = std::fs::read_to_string(file)
        .with_context(|| format!("could not read file: {}", file.display()))?;
    parser::ensure_clean_parse(&source, file)?;
    Ok(source)
}

fn read_stdin() -> Result<String> {
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .context("failed to read from stdin")?;
    Ok(buf)
}

/// Render an anyhow-style error chain into a short, user-facing single line
/// suitable for a framed per-argument error body. Uses the root cause's
/// message to avoid dumping multi-line debug output inside a `## <arg>`
/// block.
fn err_to_line(e: &anyhow::Error) -> String {
    // Walk to the root cause for a terse message.
    let root = e.chain().last().map(|c| c.to_string()).unwrap_or_default();
    if root.is_empty() { e.to_string() } else { root }
}

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    // Parse via ArgMatches so we can recover per-occurrence grouping for -f
    // (see cli.rs design note on task 1.2). We still derive the Cli struct
    // from the same matches for all other commands.
    let matches = match Cli::command().try_get_matches() {
        Ok(m) => m,
        Err(e) => {
            let _ = e.print();
            return 1;
        }
    };

    let cli = match Cli::from_arg_matches(&matches) {
        Ok(cli) => cli,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };

    // Extract -f occurrence groups for the Get subcommand. Each occurrence
    // of -f with num_args=2.. produces one group of values; we split each
    // into (file, names).
    let file_groups = matches
        .subcommand_matches("get")
        .and_then(|sub| {
            sub.get_occurrences::<String>("from").map(|occ| {
                occ.map(|vals| {
                    let vals: Vec<&String> = vals.collect();
                    let file = PathBuf::from(&vals[0]);
                    let names: Vec<String> = vals[1..].iter().map(|s| (*s).clone()).collect();
                    (file, names)
                })
                .collect::<Vec<_>>()
            })
        })
        .unwrap_or_default();

    match run_command(cli, file_groups) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            1
        }
    }
}

fn run_command(cli: Cli, file_groups: Vec<(PathBuf, Vec<String>)>) -> Result<i32> {
    match cli.command {
        Command::Guide => {
            print!("{}", include_str!("guide.md"));
            Ok(0)
        }

        Command::List {
            files,
            format,
            docs,
        } => run_list(files, format, docs),

        Command::Get {
            file,
            names,
            from: _,
            format,
        } => {
            // Mutual exclusion (task 1.3): bare positionals vs -f groups.
            let has_bare = file.is_some() || !names.is_empty();
            let has_grouped = !file_groups.is_empty();

            if has_bare && has_grouped {
                bail!("cannot mix bare positional arguments with -f/--file groups");
            }

            if has_grouped {
                run_get_multi(file_groups, format)
            } else if let Some(file) = file {
                if names.is_empty() {
                    bail!("at least one declaration name is required");
                }
                run_get(file, names, format)
            } else {
                bail!("either provide <FILE> <NAME>... or use -f <FILE> <NAME>...");
            }
        }

        Command::Set { command } => match command {
            SetCommand::Decl(args) => {
                let (source, summary) = load_and_parse_for_write(&args.file)?;
                let new_source = match args.content {
                    Some(c) => c,
                    None => read_stdin()?,
                };
                let parsed_name: Option<String> = parser::extract_declaration_name(&new_source);
                let decl_name: String = match (args.name.as_deref(), parsed_name.as_deref()) {
                    (Some(flag), Some(parsed)) if flag != parsed => {
                        bail!(
                            "--name '{flag}' does not match parsed name '{parsed}' in content; use `rename decl` to rename"
                        );
                    }
                    (Some(name), _) => name.to_string(),
                    (None, Some(p)) => p.to_string(),
                    (None, None) => bail!(
                        "could not parse declaration name from content (pass --name or ensure content begins with a declaration)"
                    ),
                };
                let result = writer::upsert_declaration(&source, &summary, &decl_name, &new_source);
                writer::validated_write(&args.file, &result, "set decl")?;
                println!("ok");
                Ok(0)
            }
            SetCommand::Let(args) => {
                use writer::let_binding::{self, BindingSpec};
                let (source, summary) = load_and_parse_for_write(&args.file)?;
                let body = match args.body {
                    Some(b) => b,
                    None => read_stdin()?,
                };
                let spec = BindingSpec {
                    name: args.name,
                    body,
                    type_annotation: args.type_annotation,
                    params: args
                        .params
                        .map(|s| s.split_whitespace().map(|p| p.to_string()).collect()),
                    no_type: args.no_type,
                    after: args.after,
                    before: args.before,
                    line: args.line,
                };
                let result = let_binding::upsert_let_binding(&source, &summary, &args.decl, &spec)?;
                writer::validated_write(&args.file, &result, "set let")?;
                println!("ok");
                Ok(0)
            }
            SetCommand::Case(args) => {
                use writer::case_branch::{self, CaseSpec};
                let (source, summary) = load_and_parse_for_write(&args.file)?;
                let body = match args.body {
                    Some(b) => b,
                    None => read_stdin()?,
                };
                let spec = CaseSpec {
                    on: args.on,
                    pattern: args.pattern,
                    body,
                    line: args.line,
                };
                let result = case_branch::upsert_case_branch(&source, &summary, &args.decl, &spec)?;
                writer::validated_write(&args.file, &result, "set case")?;
                println!("ok");
                Ok(0)
            }
        },

        Command::Patch {
            file,
            name,
            old,
            new,
        } => {
            let (source, summary) = load_and_parse_for_write(&file)?;
            let result = writer::patch_declaration(&source, &summary, &name, &old, &new)?;
            writer::validated_write(&file, &result, "patch")?;
            println!("ok");
            Ok(0)
        }

        Command::Rm { command } => match command {
            RmCommand::Decl(args) => run_rm(args.file, args.names),
            RmCommand::Let(args) => {
                use writer::let_binding;
                let (source, summary) = load_and_parse_for_write(&args.file)?;
                let result = if args.names.len() == 1 {
                    let_binding::remove_let_binding(
                        &source,
                        &summary,
                        &args.decl,
                        &args.names[0],
                        args.line,
                    )?
                } else {
                    let_binding::remove_let_bindings_batch(
                        &source,
                        &summary,
                        &args.decl,
                        &args.names,
                    )?
                };
                writer::validated_write(&args.file, &result, "rm let")?;
                println!("ok");
                Ok(0)
            }
            RmCommand::Case(args) => {
                use writer::case_branch;
                let (source, summary) = load_and_parse_for_write(&args.file)?;
                let result = if args.pattern.len() == 1 {
                    case_branch::remove_case_branch(
                        &source,
                        &summary,
                        &args.decl,
                        args.on.as_deref(),
                        &args.pattern[0],
                        args.line,
                    )?
                } else {
                    case_branch::remove_case_branches_batch(
                        &source,
                        &summary,
                        &args.decl,
                        args.on.as_deref(),
                        &args.pattern,
                        args.line,
                    )?
                };
                writer::validated_write(&args.file, &result, "rm case")?;
                println!("ok");
                Ok(0)
            }
            RmCommand::Arg(args) => {
                use writer::function_arg::{self, ArgTarget};
                let (source, summary) = load_and_parse_for_write(&args.file)?;
                let targets: Vec<ArgTarget> = if !args.at.is_empty() {
                    args.at.into_iter().map(ArgTarget::Position).collect()
                } else if !args.name.is_empty() {
                    args.name.into_iter().map(ArgTarget::Name).collect()
                } else {
                    bail!("rm arg requires at least one --at or --name");
                };
                let result = function_arg::remove_function_args_batch(
                    &source, &summary, &args.decl, &targets,
                )?;
                writer::validated_write(&args.file, &result, "rm arg")?;
                println!("ok");
                Ok(0)
            }
            RmCommand::Variant(args) => run_variant_rm(
                args.file,
                args.type_name,
                args.constructor,
                args.format,
                args.dry_run,
            ),
            RmCommand::Import(args) => run_import_remove(args.file, args.module_names),
        },

        Command::Add { command } => match command {
            AddCommand::Arg(args) => {
                use writer::function_arg;
                let (source, summary) = load_and_parse_for_write(&args.file)?;
                let result = function_arg::add_function_arg(
                    &source,
                    &summary,
                    &args.decl,
                    args.at,
                    &args.name,
                    args.type_annotation.as_deref(),
                )?;
                writer::validated_write(&args.file, &result, "add arg")?;
                println!("ok");
                Ok(0)
            }
            AddCommand::Variant(args) => run_variant_add(
                args.file,
                args.type_name,
                args.definition,
                args.format,
                args.dry_run,
                args.fill,
            ),
            AddCommand::Import(args) => run_import_add(args.file, args.imports),
        },

        Command::Expose { file, items } => run_expose(file, items),
        Command::Unexpose { file, items } => run_unexpose(file, items),

        Command::Mv {
            file,
            new_path,
            format,
            dry_run,
        } => {
            let old_path = file
                .canonicalize()
                .with_context(|| format!("file not found: {}", file.display()))?;

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
            Ok(0)
        }

        Command::Refs {
            file,
            names,
            format,
        } => run_refs(file, names, format),

        Command::Rename { command } => match command {
            RenameCommand::Decl(args) => {
                let canonical = args
                    .file
                    .canonicalize()
                    .with_context(|| format!("file not found: {}", args.file.display()))?;

                let result = project::execute_rename(
                    &canonical,
                    &args.old_name,
                    &args.new_name,
                    args.dry_run,
                )?;

                match args.format {
                    Format::Compact => {
                        let prefix = if args.dry_run { "(dry run) " } else { "" };
                        println!("{prefix}renamed {} -> {}", result.old_name, result.new_name);
                        for f in &result.updated_files {
                            println!("{prefix}updated {f}");
                        }
                    }
                    Format::Json => {
                        let json = serde_json::json!({
                            "dry_run": args.dry_run,
                            "renamed": {
                                "from": result.old_name,
                                "to": result.new_name,
                            },
                            "updated": result.updated_files,
                        });
                        println!("{}", serde_json::to_string_pretty(&json)?);
                    }
                }
                Ok(0)
            }
            RenameCommand::Let(args) => {
                use writer::let_binding;
                let (source, summary) = load_and_parse_for_write(&args.file)?;
                let result = let_binding::rename_let_binding(
                    &source, &summary, &args.decl, &args.from, &args.to, args.line,
                )?;
                writer::validated_write(&args.file, &result, "rename let")?;
                println!("ok");
                Ok(0)
            }
            RenameCommand::Arg(args) => {
                use writer::function_arg;
                let (source, summary) = load_and_parse_for_write(&args.file)?;
                let result = function_arg::rename_function_arg(
                    &source, &summary, &args.decl, &args.from, &args.to,
                )?;
                writer::validated_write(&args.file, &result, "rename arg")?;
                println!("ok");
                Ok(0)
            }
        },

        Command::MoveDecl {
            file,
            names,
            target,
            copy_shared_helpers,
            format,
            dry_run,
        } => {
            let canonical = file
                .canonicalize()
                .with_context(|| format!("file not found: {}", file.display()))?;

            let result = elmq::move_decl::execute_move_declaration(
                &canonical,
                &names,
                &target,
                copy_shared_helpers,
                dry_run,
            )?;

            match format {
                Format::Compact => {
                    let prefix = if dry_run { "(dry run) " } else { "" };
                    for name in &result.moved {
                        println!("{prefix}moved {name}");
                    }
                    for name in &result.auto_included {
                        println!("{prefix}auto-included {name}");
                    }
                    for name in &result.copied {
                        println!("{prefix}copied {name}");
                    }
                    for f in &result.updated_files {
                        println!("{prefix}updated {f}");
                    }
                }
                Format::Json => {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
            }
            Ok(0)
        }

        Command::Grep {
            pattern,
            path,
            fixed,
            ignore_case,
            include_comments,
            include_strings,
            definitions,
            source,
            format,
        } => {
            let args = elmq::grep::GrepArgs {
                pattern,
                path,
                fixed,
                ignore_case,
                include_comments,
                include_strings,
                definitions,
                source,
                format: match format {
                    GrepFormat::Compact => elmq::grep::GrepFormat::Compact,
                    GrepFormat::Json => elmq::grep::GrepFormat::Json,
                },
            };
            Ok(elmq::grep::execute(args))
        }

        Command::Variant { command } => match command {
            VariantReadCommand::Cases {
                file,
                type_name,
                format,
            } => {
                let canonical = file
                    .canonicalize()
                    .with_context(|| format!("file not found: {}", file.display()))?;

                let result = elmq::variant::execute_cases(&canonical, &type_name)?;

                match format {
                    Format::Compact => {
                        render_cases_compact(&result);
                    }
                    Format::Json => {
                        println!("{}", serde_json::to_string_pretty(&result)?);
                    }
                }
                Ok(0)
            }
        },
    }
}

// ---------------- add variant / rm variant helpers ----------------

fn run_variant_add(
    file: PathBuf,
    type_name: String,
    definition: String,
    format: Format,
    dry_run: bool,
    fill: Vec<String>,
) -> Result<i32> {
    let canonical = file
        .canonicalize()
        .with_context(|| format!("file not found: {}", file.display()))?;

    let mut fills: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for f in &fill {
        let Some(eq) = f.find('=') else {
            anyhow::bail!(
                "invalid --fill value '{}': expected KEY=BRANCH (missing '=')",
                f
            );
        };
        let (key, rest) = f.split_at(eq);
        let body = &rest[1..];
        fills.insert(key.to_string(), body.to_string());
    }

    let result =
        elmq::variant::execute_add_variant(&canonical, &type_name, &definition, dry_run, fills)?;

    match format {
        Format::Compact => {
            let prefix = if dry_run { "(dry run) " } else { "" };
            println!(
                "{prefix}added {} to {} in {}",
                result.variant_name, result.type_name, result.type_file
            );
            for edit in &result.edits {
                println!(
                    "  {prefix}{}:{}  {}  — inserted branch",
                    edit.file, edit.line, edit.function
                );
            }
            for skip in &result.skipped {
                println!(
                    "  {}:{}  {}  — skipped ({})",
                    skip.file, skip.line, skip.function, skip.reason
                );
            }
        }
        Format::Json => {
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }
    Ok(0)
}

fn run_variant_rm(
    file: PathBuf,
    type_name: String,
    constructor: String,
    format: Format,
    dry_run: bool,
) -> Result<i32> {
    let canonical = file
        .canonicalize()
        .with_context(|| format!("file not found: {}", file.display()))?;

    let result = elmq::variant::execute_rm_variant(&canonical, &type_name, &constructor, dry_run)?;

    match format {
        Format::Compact => {
            let prefix = if dry_run { "(dry run) " } else { "" };
            println!(
                "{prefix}removed {} from {} in {}",
                result.variant_name, result.type_name, result.type_file
            );
            for edit in &result.edits {
                println!(
                    "  {prefix}{}:{}  {}  — removed branch",
                    edit.file, edit.line, edit.function
                );
            }
            for skip in &result.skipped {
                println!(
                    "  {}:{}  {}  — skipped ({})",
                    skip.file, skip.line, skip.function, skip.reason
                );
            }
            // `rm variant`'s `references_not_rewritten` advisory block is the sole
            // documented exception to the confirmation-only write-output rule.
            render_rm_advisory(&result.references_not_rewritten);
        }
        Format::Json => {
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }
    Ok(0)
}

/// Render the `references not rewritten` advisory block under a `variant rm`
/// compact output. Omitted entirely when the list is empty so happy-path
/// output is unchanged. Every blocking site gets file, line, enclosing
/// declaration, classification, and a one-line snippet; the block closes
/// with an `elm make` hint so the agent knows what to do next.
fn render_rm_advisory(refs: &[elmq::variant::ConstructorSiteReport]) {
    if refs.is_empty() {
        return;
    }
    println!();
    println!("references not rewritten ({}):", refs.len());
    for site in refs {
        println!(
            "  {}:{}  {}  {}",
            site.file, site.line, site.declaration, site.kind
        );
        if !site.snippet.is_empty() {
            println!("      {}", site.snippet);
        }
    }
    println!("  run `elm make` to confirm and fix these before continuing");
}

// ---------------- variant cases (compact renderer) ----------------

/// Render a `CasesResult` as human-readable Markdown-ish output. The format is the one
/// specified in `openspec/changes/variant-fill/design.md` §7: headline, then one block
/// per site with a sub-heading carrying the stable key, then the enclosing function
/// body verbatim, then a skipped-sites footer if any wildcard branches were found.
fn render_cases_compact(result: &elmq::variant::CasesResult) {
    // Group sites by file for the "N files, M functions" headline and section headers.
    let mut by_file: std::collections::BTreeMap<&str, Vec<&elmq::variant::CasesSite>> =
        std::collections::BTreeMap::new();
    for site in &result.sites {
        by_file.entry(site.file.as_str()).or_default().push(site);
    }

    if result.sites.is_empty() {
        println!("no case sites found for type {}", result.type_name);
    } else {
        println!(
            "## case sites for type {} ({} file{}, {} function{})",
            result.type_name,
            by_file.len(),
            if by_file.len() == 1 { "" } else { "s" },
            result.sites.len(),
            if result.sites.len() == 1 { "" } else { "s" },
        );
        println!();
    }

    for (file, sites) in &by_file {
        println!("### {file}");
        println!();
        for site in sites {
            println!(
                "#### {} (key: {}, line {})",
                site.function, site.key, site.line
            );
            println!("{}", site.body);
            println!();
        }
    }

    if !result.skipped.is_empty() {
        println!("### skipped");
        for skip in &result.skipped {
            println!(
                "- {}:{} {} — {}",
                skip.file, skip.line, skip.function, skip.reason
            );
        }
    }
}

// ---------------- list ----------------

fn run_list(files: Vec<PathBuf>, format: Format, docs: bool) -> Result<i32> {
    let entries: Vec<(String, ItemResult)> = files
        .iter()
        .map(|f| {
            let arg = f.display().to_string();
            let result: ItemResult = match load_and_parse(f) {
                Ok((source, summary)) => match format {
                    Format::Compact => {
                        let line_count = source.lines().count();
                        Ok(format_compact(&summary, docs, line_count))
                    }
                    Format::Json => format_json(&summary).map_err(|e| err_to_line(&e)),
                },
                Err(e) => Err(err_to_line(&e)),
            };
            (arg, result)
        })
        .collect();

    let (out, code) = format_results(&entries);
    print!("{out}");
    Ok(code)
}

// ---------------- get ----------------

fn run_get(file: PathBuf, names: Vec<String>, format: Format) -> Result<i32> {
    let (source, summary) = load_and_parse(&file)?;
    let source_lines: Vec<&str> = source.lines().collect();

    let entries: Vec<(String, ItemResult)> = names
        .iter()
        .map(|name| {
            let arg = name.clone();
            let result: ItemResult = match summary.find_declaration(name) {
                Some(decl) => {
                    let start = decl.start_line - 1;
                    let end = decl.end_line.min(source_lines.len());
                    let decl_source = source_lines[start..end].join("\n");
                    match format {
                        Format::Compact => Ok(format!("{decl_source}\n")),
                        Format::Json => {
                            format_get_json(decl, &decl_source).map_err(|e| err_to_line(&e))
                        }
                    }
                }
                None => Err(format!("declaration '{name}' not found")),
            };
            (arg, result)
        })
        .collect();

    let (out, code) = format_results(&entries);
    print!("{out}");
    Ok(code)
}

// ---------------- get (multi-file) ----------------

/// A parsed and cached file, keyed by canonical path.
struct ParsedFile {
    summary: FileSummary,
    source_lines: Vec<String>,
}

fn run_get_multi(groups: Vec<(PathBuf, Vec<String>)>, format: Format) -> Result<i32> {
    use std::collections::HashMap;

    // Cache: canonical path → parsed file. Each file is read and parsed at
    // most once regardless of how many -f groups reference it (task 3.2).
    let mut cache: HashMap<PathBuf, Result<ParsedFile, String>> = HashMap::new();

    // Project discovery happens at most once (task 2.3). Try from the first
    // file; None means no elm.json was found.
    let project = groups
        .first()
        .and_then(|(file, _)| project::Project::try_discover(file).ok().flatten());

    // Build module-name map for all files. Detect collisions (task 2.2):
    // two files resolving to the same module name is an error for both.
    let mut module_for_file: HashMap<PathBuf, Result<Option<String>, String>> = HashMap::new();
    if let Some(ref proj) = project {
        let mut module_to_file: HashMap<String, PathBuf> = HashMap::new();
        for (file, _) in &groups {
            let canonical = file.canonicalize().unwrap_or_else(|_| file.clone());
            if module_for_file.contains_key(&canonical) {
                continue;
            }
            match proj.module_name(&canonical) {
                Ok(module) => {
                    if let Some(prev_file) = module_to_file.get(&module) {
                        // Collision: mark both as errors.
                        let msg =
                            format!("ambiguous module resolution for {}", canonical.display());
                        module_for_file.insert(canonical, Err(msg.clone()));
                        let prev_msg =
                            format!("ambiguous module resolution for {}", prev_file.display());
                        module_for_file.insert(prev_file.clone(), Err(prev_msg));
                    } else {
                        module_to_file.insert(module.clone(), canonical.clone());
                        module_for_file.insert(canonical, Ok(Some(module)));
                    }
                }
                Err(_) => {
                    module_for_file.insert(canonical, Ok(None));
                }
            }
        }
    }

    // Flatten groups into (header, result) entries in input order (task 3.3).
    let mut entries: Vec<(String, ItemResult)> = Vec::new();

    for (file, names) in &groups {
        let canonical = file.canonicalize().unwrap_or_else(|_| file.clone());

        // Resolve the header prefix: Module name or file path.
        let header_prefix: Result<String, String> =
            if let Some(res) = module_for_file.get(&canonical) {
                match res {
                    Ok(Some(module)) => Ok(module.clone()),
                    Ok(None) => Ok(file.display().to_string()),
                    Err(msg) => Err(msg.clone()),
                }
            } else {
                // No project discovered — fallback to file path.
                Ok(file.display().to_string())
            };

        // Build the header for a given name.
        let make_header = |name: &str| -> String {
            match &header_prefix {
                Ok(prefix) => {
                    if project.is_some()
                        && module_for_file
                            .get(&canonical)
                            .is_some_and(|r| matches!(r, Ok(Some(_))))
                    {
                        // Module.decl form
                        format!("{prefix}.{name}")
                    } else {
                        // file:decl fallback
                        format!("{prefix}:{name}")
                    }
                }
                Err(_) => {
                    // Collision — still need a header, use file:name.
                    format!("{}:{name}", file.display())
                }
            }
        };

        // If the header prefix is an error (module collision), propagate it
        // to all names in the group (task 3.4).
        if let Err(ref msg) = header_prefix {
            for name in names {
                entries.push((make_header(name), Err(msg.clone())));
            }
            continue;
        }

        // Ensure file is in the cache.
        if !cache.contains_key(&canonical) {
            let parsed = match load_and_parse(&canonical) {
                Ok((source, summary)) => {
                    let source_lines = source.lines().map(String::from).collect();
                    Ok(ParsedFile {
                        summary,
                        source_lines,
                    })
                }
                Err(e) => Err(err_to_line(&e)),
            };
            cache.insert(canonical.clone(), parsed);
        }

        let parsed = cache.get(&canonical).unwrap();
        match parsed {
            Ok(pf) => {
                for name in names {
                    let header = make_header(name);
                    let result: ItemResult = match pf.summary.find_declaration(name) {
                        Some(decl) => {
                            let start = decl.start_line - 1;
                            let end = decl.end_line.min(pf.source_lines.len());
                            let decl_source = pf.source_lines[start..end].join("\n");
                            match format {
                                Format::Compact => Ok(format!("{decl_source}\n")),
                                Format::Json => format_get_json_multi(
                                    decl,
                                    &decl_source,
                                    &file.display().to_string(),
                                    module_for_file
                                        .get(&canonical)
                                        .and_then(|r| r.as_ref().ok())
                                        .and_then(|o| o.as_deref()),
                                )
                                .map_err(|e| err_to_line(&e)),
                            }
                        }
                        None => Err(format!("declaration '{name}' not found")),
                    };
                    entries.push((header, result));
                }
            }
            Err(msg) => {
                // File-level error: propagate to every name in the group.
                for name in names {
                    entries.push((make_header(name), Err(msg.clone())));
                }
            }
        }
    }

    match format {
        Format::Compact => {
            let (out, code) = format_results(&entries);
            print!("{out}");
            Ok(code)
        }
        Format::Json => {
            // Task 4.4: multi-result → JSON array; single-result → scalar object.
            let any_failed = entries.iter().any(|(_, r)| r.is_err());
            let code = if any_failed { 2 } else { 0 };

            // Collect JSON values. Errors become objects with an "error" field.
            let json_values: Vec<serde_json::Value> = entries
                .into_iter()
                .map(|(header, result)| match result {
                    Ok(body) => {
                        // body is a JSON string from format_get_json_multi
                        serde_json::from_str(&body)
                            .unwrap_or(serde_json::json!({"error": "invalid json"}))
                    }
                    Err(msg) => serde_json::json!({"header": header, "error": msg}),
                })
                .collect();

            if json_values.len() == 1 {
                println!("{}", serde_json::to_string_pretty(&json_values[0])?);
            } else {
                println!("{}", serde_json::to_string_pretty(&json_values)?);
            }
            Ok(code)
        }
    }
}

fn format_get_json_multi(
    decl: &Declaration,
    source: &str,
    file: &str,
    module: Option<&str>,
) -> Result<String> {
    let json = serde_json::json!({
        "name": decl.name,
        "kind": decl.kind,
        "source": source,
        "start_line": decl.start_line,
        "end_line": decl.end_line,
        "file": file,
        "module": module,
    });
    Ok(serde_json::to_string_pretty(&json)?)
}

// ---------------- rm ----------------

fn run_rm(file: PathBuf, names: Vec<String>) -> Result<i32> {
    let original = load_source_for_write(&file)?;

    // Apply each removal against an accumulating source, reparsing between
    // iterations because line ranges shift after each removal.
    let mut accumulator = original.clone();
    let mut entries: Vec<(String, ItemResult)> = Vec::with_capacity(names.len());
    let mut any_change = false;

    for name in &names {
        let arg = name.clone();
        let result: ItemResult = (|| -> ItemResult {
            let tree = parser::parse(&accumulator).map_err(|e| format!("parse error: {e}"))?;
            let summary = parser::extract_summary(&tree, &accumulator);
            match writer::remove_declaration(&accumulator, &summary, name) {
                Ok(new_source) => {
                    accumulator = new_source;
                    any_change = true;
                    Ok(String::new())
                }
                Err(e) => Err(format!("declaration '{name}' not found: {e}")),
            }
        })();
        entries.push((arg, result));
    }

    if any_change {
        writer::validated_write(&file, &accumulator, "rm")?;
    }

    let (out, code) = format_results(&entries);
    print!("{out}");
    Ok(code)
}

// ---------------- import add / remove ----------------

fn run_import_add(file: PathBuf, imports: Vec<String>) -> Result<i32> {
    let original = load_source_for_write(&file)?;

    let mut accumulator = original.clone();
    let mut entries: Vec<(String, ItemResult)> = Vec::with_capacity(imports.len());
    let mut any_change = false;

    for clause in &imports {
        let arg = clause.clone();
        let result: ItemResult = match parser::parse(&accumulator) {
            Ok(tree) => {
                let summary = parser::extract_summary(&tree, &accumulator);
                let new_source = writer::add_import(&accumulator, &summary, clause);
                if new_source != accumulator {
                    accumulator = new_source;
                    any_change = true;
                }
                Ok(String::new())
            }
            Err(e) => Err(format!("parse error: {e}")),
        };
        entries.push((arg, result));
    }

    if any_change {
        writer::validated_write(&file, &accumulator, "import add")?;
    }

    let (out, code) = format_results(&entries);
    print!("{out}");
    Ok(code)
}

fn run_import_remove(file: PathBuf, module_names: Vec<String>) -> Result<i32> {
    let original = load_source_for_write(&file)?;

    let mut accumulator = original.clone();
    let mut entries: Vec<(String, ItemResult)> = Vec::with_capacity(module_names.len());
    let mut any_change = false;

    for module_name in &module_names {
        let arg = module_name.clone();
        let result: ItemResult = match writer::remove_import(&accumulator, module_name) {
            Ok(new_source) => {
                if new_source != accumulator {
                    accumulator = new_source;
                    any_change = true;
                }
                Ok(String::new())
            }
            Err(e) => Err(err_to_line(&e)),
        };
        entries.push((arg, result));
    }

    if any_change {
        writer::validated_write(&file, &accumulator, "import remove")?;
    }

    let (out, code) = format_results(&entries);
    print!("{out}");
    Ok(code)
}

// ---------------- expose / unexpose ----------------

fn run_expose(file: PathBuf, items: Vec<String>) -> Result<i32> {
    let original = load_source_for_write(&file)?;

    let mut accumulator = original.clone();
    let mut entries: Vec<(String, ItemResult)> = Vec::with_capacity(items.len());
    let mut any_change = false;

    for item in &items {
        let arg = item.clone();
        let result: ItemResult = (|| -> ItemResult {
            let tree = parser::parse(&accumulator).map_err(|e| format!("parse error: {e}"))?;
            let summary = parser::extract_summary(&tree, &accumulator);
            match writer::expose(&accumulator, &summary, item) {
                Ok(new_source) => {
                    if new_source != accumulator {
                        accumulator = new_source;
                        any_change = true;
                    }
                    Ok(String::new())
                }
                Err(e) => Err(err_to_line(&e)),
            }
        })();
        entries.push((arg, result));
    }

    if any_change {
        writer::validated_write(&file, &accumulator, "expose")?;
    }

    let (out, code) = format_results(&entries);
    print!("{out}");
    Ok(code)
}

fn run_unexpose(file: PathBuf, items: Vec<String>) -> Result<i32> {
    let original = load_source_for_write(&file)?;

    let mut accumulator = original.clone();
    let mut entries: Vec<(String, ItemResult)> = Vec::with_capacity(items.len());
    let mut any_change = false;

    for item in &items {
        let arg = item.clone();
        let result: ItemResult = (|| -> ItemResult {
            let tree = parser::parse(&accumulator).map_err(|e| format!("parse error: {e}"))?;
            let summary = parser::extract_summary(&tree, &accumulator);
            match writer::unexpose(&accumulator, &summary, item) {
                Ok(new_source) => {
                    if new_source != accumulator {
                        accumulator = new_source;
                        any_change = true;
                    }
                    Ok(String::new())
                }
                Err(e) => Err(err_to_line(&e)),
            }
        })();
        entries.push((arg, result));
    }

    if any_change {
        writer::validated_write(&file, &accumulator, "unexpose")?;
    }

    let (out, code) = format_results(&entries);
    print!("{out}");
    Ok(code)
}

// ---------------- refs ----------------

fn run_refs(file: PathBuf, names: Vec<String>, format: Format) -> Result<i32> {
    let canonical = file
        .canonicalize()
        .with_context(|| format!("file not found: {}", file.display()))?;

    let project = project::Project::discover(&canonical)?;
    let target_module = project.module_name(&canonical)?;

    // Zero-name: module-level report, bare output, no framing.
    if names.is_empty() {
        let matches = refs::find_refs(&project, &target_module, None)?;
        print_refs_compact(&matches, format);
        return Ok(0);
    }

    // Each name dispatches independently: constructors of a custom type
    // declared in this file go through the classifier
    // (`execute_refs_for_constructor`), top-level declarations go through
    // the batch decl-ref path, unknown names are reported as per-arg errors.
    // We classify every name up front so the decl subset can be batched in
    // a single project walk (preserving `find_refs_batch`'s parse-once
    // performance property).
    let (_, summary) = load_and_parse(&canonical)?;

    enum Kind {
        Decl(usize),                            // index into `decl_names`
        Constructor(elmq::variant::RefsResult), // resolved eagerly — single-name walks are cheap enough
        Missing,
    }
    let mut decl_names: Vec<&str> = Vec::new();
    let mut kinds: Vec<Kind> = Vec::with_capacity(names.len());
    for name in &names {
        if summary.find_declaration(name).is_some() {
            kinds.push(Kind::Decl(decl_names.len()));
            decl_names.push(name.as_str());
        } else if let Some((_, _)) = elmq::variant::resolve_constructor_in_file(&canonical, name)? {
            let result = elmq::variant::execute_refs_for_constructor(&canonical, name)?;
            kinds.push(Kind::Constructor(result));
        } else {
            kinds.push(Kind::Missing);
        }
    }

    let batch_matches = if decl_names.is_empty() {
        Vec::new()
    } else {
        refs::find_refs_batch(&project, &target_module, &decl_names)?
    };

    let entries: Vec<(String, ItemResult)> = names
        .iter()
        .zip(kinds)
        .map(|(name, kind)| {
            let arg = name.clone();
            let result: ItemResult = match kind {
                Kind::Decl(i) => {
                    let matches = &batch_matches[i];
                    Ok(format_refs_body(matches, &format))
                }
                Kind::Constructor(r) => Ok(format_constructor_refs_body(&r, &format)),
                Kind::Missing => Err(format!("declaration '{name}' not found")),
            };
            (arg, result)
        })
        .collect();

    let (out, code) = format_results(&entries);
    print!("{out}");
    Ok(code)
}

/// Render a classified constructor `RefsResult` into the same framed string
/// shape that `format_refs_body` produces for decl refs, so multi-arg `refs`
/// output can mix both kinds inside the standard `## <arg>` header frames.
fn format_constructor_refs_body(result: &elmq::variant::RefsResult, format: &Format) -> String {
    match format {
        Format::Compact => {
            let mut out = String::new();
            let _ = writeln!(
                out,
                "{}.{} — {} reference{} ({} clean, {} blocking)",
                result.type_name,
                result.constructor,
                result.total_sites,
                if result.total_sites == 1 { "" } else { "s" },
                result.total_clean,
                result.total_blocking,
            );
            for (file, sites) in &result.sites_by_file {
                let _ = writeln!(out, "{file}");
                for site in sites {
                    let _ = writeln!(
                        out,
                        "  {:>4}  {:<14}  {}",
                        site.line, site.declaration, site.kind,
                    );
                    if !site.snippet.is_empty() {
                        let _ = writeln!(out, "        {}", site.snippet);
                    }
                }
            }
            out
        }
        Format::Json => match serde_json::to_string_pretty(result) {
            Ok(s) => format!("{s}\n"),
            Err(_) => String::new(),
        },
    }
}

fn print_refs_compact(matches: &[refs::RefMatch], format: Format) {
    match format {
        Format::Compact => {
            for r in matches {
                if let Some(text) = &r.text {
                    println!("{}:{}: {}", r.file, r.line, text);
                } else {
                    println!("{}:{}", r.file, r.line);
                }
            }
        }
        Format::Json => {
            // best-effort: serialization on Vec<RefMatch> should not fail.
            match serde_json::to_string_pretty(matches) {
                Ok(s) => println!("{s}"),
                Err(e) => eprintln!("error: {e}"),
            }
        }
    }
}

fn format_refs_body(matches: &[refs::RefMatch], format: &Format) -> String {
    let mut out = String::new();
    match format {
        Format::Compact => {
            for r in matches {
                if let Some(text) = &r.text {
                    let _ = writeln!(out, "{}:{}: {}", r.file, r.line, text);
                } else {
                    let _ = writeln!(out, "{}:{}", r.file, r.line);
                }
            }
        }
        Format::Json => {
            if let Ok(s) = serde_json::to_string_pretty(matches) {
                out.push_str(&s);
                out.push('\n');
            }
        }
    }
    out
}

// ---------------- formatters ----------------

fn format_compact(summary: &FileSummary, show_docs: bool, line_count: usize) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{}  ({} lines)", summary.module_line, line_count);

    if !summary.imports.is_empty() {
        out.push('\n');
        out.push_str("imports:\n");
        for imp in &summary.imports {
            let _ = writeln!(out, "  {imp}");
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
            out.push('\n');
            let _ = writeln!(out, "{label}");
            for d in decls {
                let _ = writeln!(
                    out,
                    "  {:<name_w$}  L{}-{}",
                    d.name, d.start_line, d.end_line
                );
                if show_docs && let Some(doc) = &d.doc_comment {
                    format_doc_comment(&mut out, doc);
                }
            }
        }
    }

    if !functions.is_empty() {
        let name_w = functions.iter().map(|d| d.name.len()).max().unwrap_or(0);
        out.push('\n');
        out.push_str("functions:\n");
        for d in &functions {
            let type_str = d.type_annotation.as_deref().unwrap_or("");
            if type_str.is_empty() {
                let _ = writeln!(
                    out,
                    "  {:<name_w$}  L{}-{}",
                    d.name, d.start_line, d.end_line
                );
            } else {
                let _ = writeln!(
                    out,
                    "  {:<name_w$}  {}  L{}-{}",
                    d.name, type_str, d.start_line, d.end_line
                );
            }
            if show_docs && let Some(doc) = &d.doc_comment {
                format_doc_comment(&mut out, doc);
            }
        }
    }

    if !ports.is_empty() {
        let name_w = ports.iter().map(|d| d.name.len()).max().unwrap_or(0);
        out.push('\n');
        out.push_str("ports:\n");
        for d in &ports {
            let type_str = d.type_annotation.as_deref().unwrap_or("");
            if type_str.is_empty() {
                let _ = writeln!(
                    out,
                    "  {:<name_w$}  L{}-{}",
                    d.name, d.start_line, d.end_line
                );
            } else {
                let _ = writeln!(
                    out,
                    "  {:<name_w$}  {}  L{}-{}",
                    d.name, type_str, d.start_line, d.end_line
                );
            }
            if show_docs && let Some(doc) = &d.doc_comment {
                format_doc_comment(&mut out, doc);
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
        let _ = writeln!(out, "    {}", line.trim());
    }
}

fn format_get_json(decl: &Declaration, source: &str) -> Result<String> {
    let json = serde_json::json!({
        "name": decl.name,
        "kind": decl.kind,
        "source": source,
        "start_line": decl.start_line,
        "end_line": decl.end_line,
    });
    Ok(format!("{}\n", serde_json::to_string_pretty(&json)?))
}

fn format_json(summary: &FileSummary) -> Result<String> {
    Ok(format!("{}\n", serde_json::to_string_pretty(summary)?))
}
