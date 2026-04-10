use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "elmq", about = "Query and edit Elm files — like jq for Elm")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Show a summary of one or more Elm files
    List {
        /// Paths to the Elm files (one or more)
        #[arg(num_args = 1.., required = true)]
        files: Vec<PathBuf>,

        /// Output format
        #[arg(long, value_enum, default_value_t = Format::Compact)]
        format: Format,

        /// Include doc comments in output
        #[arg(long)]
        docs: bool,
    },
    /// Extract the full source of one or more declarations by name
    ///
    /// Two forms:
    ///   elmq get <FILE> <NAME>...           bare positional (single file)
    ///   elmq get -f <FILE> <NAME>... [-f …] grouped multi-file
    ///
    /// The forms are mutually exclusive; mixing positionals with -f is a usage
    /// error. Each -f group takes a file path followed by one or more names.
    Get {
        /// Path to the Elm file (bare positional form; omit when using -f)
        file: Option<PathBuf>,

        /// Names of declarations to extract (bare positional form; omit when using -f)
        #[arg(num_args = 0..)]
        names: Vec<String>,

        // Design note (task 1.2): clap derive cannot represent per-occurrence
        // value groups directly — Vec<Vec<String>> is not supported because
        // Vec<String> doesn't implement FromStr. We store the flat
        // concatenation here and recover occurrence boundaries via
        // ArgMatches::get_occurrences in main.rs. The derive field exists
        // so clap generates correct help text and validates num_args.
        /// File group: -f <FILE> <NAME>... (repeatable for multi-file reads)
        #[arg(short = 'f', long = "file", num_args = 2.., action = clap::ArgAction::Append)]
        from: Vec<String>,

        /// Output format
        #[arg(long, value_enum, default_value_t = Format::Compact)]
        format: Format,
    },
    /// Upsert a declaration (reads source from stdin)
    Set {
        /// Path to the Elm file
        file: PathBuf,

        /// Override the declaration name (default: parsed from stdin)
        #[arg(long)]
        name: Option<String>,
    },
    /// Surgical find-and-replace within a declaration
    Patch {
        /// Path to the Elm file
        file: PathBuf,

        /// Name of the declaration to patch
        name: String,

        /// Text to find
        #[arg(long)]
        old: String,

        /// Text to replace with
        #[arg(long)]
        new: String,
    },
    /// Remove one or more declarations by name
    Rm {
        /// Path to the Elm file
        file: PathBuf,

        /// Names of declarations to remove (one or more)
        #[arg(num_args = 1.., required = true)]
        names: Vec<String>,
    },
    /// Manage imports
    Import {
        #[command(subcommand)]
        command: ImportCommand,
    },
    /// Add one or more items to the module's exposing list
    Expose {
        /// Path to the Elm file
        file: PathBuf,

        /// Items to expose (one or more, e.g. "update", "Msg(..)")
        #[arg(num_args = 1.., required = true)]
        items: Vec<String>,
    },
    /// Remove one or more items from the module's exposing list
    Unexpose {
        /// Path to the Elm file
        file: PathBuf,

        /// Items to unexpose (one or more)
        #[arg(num_args = 1.., required = true)]
        items: Vec<String>,
    },
    /// Rename a module: move file and update all imports and qualified references
    Mv {
        /// Path to the Elm file to rename
        file: PathBuf,

        /// New file path
        new_path: PathBuf,

        /// Output format
        #[arg(long, value_enum, default_value_t = Format::Compact)]
        format: Format,

        /// Preview changes without writing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Find all references to a module or one or more declarations across the project
    Refs {
        /// Path to the Elm file whose module to search for
        file: PathBuf,

        /// Declaration names to search for (zero or more; if omitted, reports module-level imports)
        #[arg(num_args = 0..)]
        names: Vec<String>,

        /// Output format
        #[arg(long, value_enum, default_value_t = Format::Compact)]
        format: Format,
    },
    /// Rename a declaration and update all references across the project
    Rename {
        /// Path to the Elm file containing the declaration
        file: PathBuf,

        /// Current name of the declaration or variant
        old_name: String,

        /// New name for the declaration or variant
        new_name: String,

        /// Output format
        #[arg(long, value_enum, default_value_t = Format::Compact)]
        format: Format,

        /// Preview changes without writing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Move declarations from one module to another, updating all references
    MoveDecl {
        /// Path to the source Elm file
        file: PathBuf,

        /// Path to the target Elm file
        #[arg(long = "to")]
        target: PathBuf,

        /// Names of declarations to move (one or more, positional)
        #[arg(num_args = 1.., required = true)]
        names: Vec<String>,

        /// Copy shared helpers instead of erroring
        #[arg(long)]
        copy_shared_helpers: bool,

        /// Output format
        #[arg(long, value_enum, default_value_t = Format::Compact)]
        format: Format,

        /// Preview changes without writing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Add or remove type variant constructors, updating case expressions project-wide
    Variant {
        #[command(subcommand)]
        command: VariantCommand,
    },
    /// Print the agent integration guide
    Guide,
    /// Search for a regex in Elm sources, annotated with enclosing top-level declaration.
    ///
    /// This is the discovery entry point: use `elmq grep` to locate text in Elm files
    /// and get back the containing declaration name for free, then pipe into `elmq get`.
    ///
    /// By default, matches inside comments (`--` and `{- -}`) and inside string literals
    /// (including `"""`) are filtered out. Pass `--include-comments` or `--include-strings`
    /// to re-enable each class independently.
    ///
    /// Project discovery walks up for `elm.json`; if none is found, falls back to
    /// recursively walking the current directory. Both paths honor `.gitignore`.
    ///
    /// Exit codes: 0 if matches found, 1 if none, 2 on error. Matches ripgrep.
    Grep {
        /// Regex pattern (Rust-regex syntax). Use -F for literal matching.
        pattern: String,

        /// Optional path to restrict search to (file or directory).
        path: Option<PathBuf>,

        /// Treat the pattern as a literal string rather than a regex.
        #[arg(short = 'F', long)]
        fixed: bool,

        /// Case-insensitive matching.
        #[arg(short = 'i', long)]
        ignore_case: bool,

        /// Include matches that fall inside `--` or `{- -}` comments (filtered by default).
        #[arg(long)]
        include_comments: bool,

        /// Include matches that fall inside string literals (filtered by default).
        #[arg(long)]
        include_strings: bool,

        /// Only emit matches at the declaration name site (not call sites).
        #[arg(long)]
        definitions: bool,

        /// Emit full declaration source for each matched decl, deduped by (file, decl).
        #[arg(long)]
        source: bool,

        /// Output format: compact `file:line:decl:text` (default) or NDJSON.
        #[arg(long, value_enum, default_value_t = GrepFormat::Compact)]
        format: GrepFormat,
    },
}

#[derive(Clone, ValueEnum)]
pub enum GrepFormat {
    Compact,
    Json,
}

#[derive(Subcommand)]
pub enum VariantCommand {
    /// Add a constructor to a custom type and insert branches in all case expressions
    Add {
        /// Path to the Elm file containing the type
        file: PathBuf,

        /// Name of the custom type (e.g. "Msg")
        #[arg(long = "type")]
        type_name: String,

        /// Variant definition (e.g. "SetName String")
        definition: String,

        /// Output format
        #[arg(long, value_enum, default_value_t = Format::Compact)]
        format: Format,

        /// Preview changes without writing anything
        #[arg(long)]
        dry_run: bool,

        /// Fill a case-site branch body instead of the default `Debug.todo "<Variant>"`
        /// stub. Syntax: `--fill <key>=<branch_text>` (repeatable). Split on the first
        /// `=`; the key comes from `elmq variant cases` output, the body is the branch
        /// text that replaces the stub.
        #[arg(long, value_name = "KEY=BRANCH")]
        fill: Vec<String>,
    },
    /// List every case expression on a type, with its enclosing function body and a
    /// stable site key. Read-only companion to `variant add --fill` — run this first
    /// to gather the context needed to synthesize fill bodies.
    Cases {
        /// Path to the Elm file that declares the custom type
        file: PathBuf,

        /// Name of the custom type (e.g. "Msg")
        #[arg(long = "type")]
        type_name: String,

        /// Output format
        #[arg(long, value_enum, default_value_t = Format::Compact)]
        format: Format,
    },
    /// Remove a constructor from a custom type and remove branches from all case expressions
    Rm {
        /// Path to the Elm file containing the type
        file: PathBuf,

        /// Name of the custom type (e.g. "Msg")
        #[arg(long = "type")]
        type_name: String,

        /// Constructor name to remove (e.g. "Decrement")
        constructor: String,

        /// Output format
        #[arg(long, value_enum, default_value_t = Format::Compact)]
        format: Format,

        /// Preview changes without writing anything
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
pub enum ImportCommand {
    /// Add one or more imports
    Add {
        /// Path to the Elm file
        file: PathBuf,

        /// Import clauses (one or more, e.g. "Html exposing (Html, div, text)")
        #[arg(num_args = 1.., required = true)]
        imports: Vec<String>,
    },
    /// Remove one or more imports by module name
    Remove {
        /// Path to the Elm file
        file: PathBuf,

        /// Module names to remove (one or more)
        #[arg(num_args = 1.., required = true)]
        module_names: Vec<String>,
    },
}

#[derive(Clone, ValueEnum)]
pub enum Format {
    Compact,
    Json,
}
