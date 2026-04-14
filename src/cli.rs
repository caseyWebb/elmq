use clap::{Args, Parser, Subcommand, ValueEnum};
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
        #[arg(num_args = 1.., required = true)]
        files: Vec<PathBuf>,

        #[arg(long, value_enum, default_value_t = Format::Compact)]
        format: Format,

        #[arg(long)]
        docs: bool,
    },
    /// Extract the full source of one or more declarations by name
    ///
    /// Two forms:
    ///   elmq get <FILE> <NAME>...           bare positional (single file)
    ///   elmq get -f <FILE> <NAME>... [-f …] grouped multi-file
    Get {
        file: Option<PathBuf>,

        #[arg(num_args = 0..)]
        names: Vec<String>,

        /// File group: -f <FILE> <NAME>... (repeatable for multi-file reads)
        #[arg(short = 'f', long = "file", num_args = 2.., action = clap::ArgAction::Append)]
        from: Vec<String>,

        #[arg(long, value_enum, default_value_t = Format::Compact)]
        format: Format,
    },
    /// Upsert a declaration, let binding, or case branch
    Set {
        #[command(subcommand)]
        command: SetCommand,
    },
    /// Surgical find-and-replace within a declaration
    Patch {
        file: PathBuf,
        name: String,
        #[arg(long)]
        old: String,
        #[arg(long)]
        new: String,
    },
    /// Remove declarations, let bindings, case branches, args, variants, or imports
    Rm {
        #[command(subcommand)]
        command: RmCommand,
    },
    /// Rename a declaration, let binding, or function argument
    Rename {
        #[command(subcommand)]
        command: RenameCommand,
    },
    /// Add a function argument, variant constructor, or import
    Add {
        #[command(subcommand)]
        command: AddCommand,
    },
    /// Add one or more items to the module's exposing list
    Expose {
        file: PathBuf,
        #[arg(num_args = 1.., required = true)]
        items: Vec<String>,
    },
    /// Remove one or more items from the module's exposing list
    Unexpose {
        file: PathBuf,
        #[arg(num_args = 1.., required = true)]
        items: Vec<String>,
    },
    /// Rename a module: move file and update all imports and qualified references
    Mv {
        file: PathBuf,
        new_path: PathBuf,
        #[arg(long, value_enum, default_value_t = Format::Compact)]
        format: Format,
        #[arg(long)]
        dry_run: bool,
    },
    /// Find all references to a module, declarations, or type constructors.
    Refs {
        file: PathBuf,
        #[arg(num_args = 0..)]
        names: Vec<String>,
        #[arg(long, value_enum, default_value_t = Format::Compact)]
        format: Format,
    },
    /// Move declarations from one module to another, updating all references
    MoveDecl {
        file: PathBuf,
        #[arg(long = "to")]
        target: PathBuf,
        #[arg(num_args = 1.., required = true)]
        names: Vec<String>,
        #[arg(long)]
        copy_shared_helpers: bool,
        #[arg(long, value_enum, default_value_t = Format::Compact)]
        format: Format,
        #[arg(long)]
        dry_run: bool,
    },
    /// Read-only companion commands for variant custom types
    Variant {
        #[command(subcommand)]
        command: VariantReadCommand,
    },
    /// Print the agent integration guide
    Guide,
    /// Search for a regex in Elm sources, annotated with enclosing top-level declaration.
    Grep {
        pattern: String,
        path: Option<PathBuf>,
        #[arg(short = 'F', long)]
        fixed: bool,
        #[arg(short = 'i', long)]
        ignore_case: bool,
        #[arg(long)]
        include_comments: bool,
        #[arg(long)]
        include_strings: bool,
        #[arg(long)]
        definitions: bool,
        #[arg(long)]
        source: bool,
        #[arg(long, value_enum, default_value_t = GrepFormat::Compact)]
        format: GrepFormat,
    },
}

#[derive(Clone, ValueEnum)]
pub enum GrepFormat {
    Compact,
    Json,
}

// --- set ---

#[derive(Subcommand)]
pub enum SetCommand {
    /// Upsert a top-level declaration (content via --content or stdin)
    Decl(SetDecl),
    /// Upsert a let binding inside a top-level declaration
    Let(SetLet),
    /// Upsert a branch in a case expression inside a top-level declaration
    Case(SetCase),
}

#[derive(Args)]
pub struct SetDecl {
    pub file: PathBuf,

    /// Declaration name. Must match parsed name in content if content has one.
    #[arg(long)]
    pub name: Option<String>,

    /// Inline content (exactly-one-of with stdin).
    #[arg(long)]
    pub content: Option<String>,
}

#[derive(Args)]
pub struct SetLet {
    pub file: PathBuf,

    /// Enclosing top-level declaration name
    pub decl: String,

    /// Let binding name
    #[arg(long)]
    pub name: String,

    /// RHS expression (exactly-one-of with stdin)
    #[arg(long)]
    pub body: Option<String>,

    /// Type annotation (absent = preserve existing sig on update)
    #[arg(long = "type", conflicts_with = "no_type")]
    pub type_annotation: Option<String>,

    /// Space-separated parameter names (function binding)
    #[arg(long)]
    pub params: Option<String>,

    /// Remove existing signature on update
    #[arg(long)]
    pub no_type: bool,

    #[arg(long, conflicts_with = "before")]
    pub after: Option<String>,

    #[arg(long)]
    pub before: Option<String>,

    /// Disambiguate ambiguous --name by absolute file line
    #[arg(long)]
    pub line: Option<usize>,
}

#[derive(Args)]
pub struct SetCase {
    pub file: PathBuf,

    /// Enclosing top-level declaration name
    pub decl: String,

    /// Pattern to upsert (byte-exact after whitespace trim)
    #[arg(long)]
    pub pattern: String,

    /// Branch body expression (exactly-one-of with stdin)
    #[arg(long)]
    pub body: Option<String>,

    /// Scrutinee selector when the enclosing decl has more than one case expression
    #[arg(long)]
    pub on: Option<String>,

    /// Disambiguate by absolute file line
    #[arg(long)]
    pub line: Option<usize>,
}

// --- rm ---

#[derive(Subcommand)]
pub enum RmCommand {
    Decl(RmDecl),
    Let(RmLet),
    Case(RmCase),
    Arg(RmArg),
    Variant(RmVariant),
    Import(RmImport),
}

#[derive(Args)]
pub struct RmDecl {
    pub file: PathBuf,

    #[arg(num_args = 1.., required = true)]
    pub names: Vec<String>,
}

#[derive(Args)]
pub struct RmLet {
    pub file: PathBuf,

    pub decl: String,

    #[arg(num_args = 1.., required = true)]
    pub names: Vec<String>,

    #[arg(long)]
    pub line: Option<usize>,
}

#[derive(Args)]
pub struct RmCase {
    pub file: PathBuf,

    pub decl: String,

    #[arg(long, num_args = 1, required = true, action = clap::ArgAction::Append)]
    pub pattern: Vec<String>,

    #[arg(long)]
    pub on: Option<String>,

    #[arg(long)]
    pub line: Option<usize>,
}

#[derive(Args)]
pub struct RmArg {
    pub file: PathBuf,

    pub decl: String,

    /// 1-indexed parameter position(s) (repeatable, mutually exclusive with --name)
    #[arg(long, num_args = 1, action = clap::ArgAction::Append, conflicts_with = "name")]
    pub at: Vec<usize>,

    /// Parameter name(s) (repeatable, mutually exclusive with --at)
    #[arg(long, num_args = 1, action = clap::ArgAction::Append)]
    pub name: Vec<String>,
}

#[derive(Args)]
pub struct RmVariant {
    pub file: PathBuf,

    #[arg(long = "type")]
    pub type_name: String,

    pub constructor: String,

    #[arg(long, value_enum, default_value_t = Format::Compact)]
    pub format: Format,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct RmImport {
    pub file: PathBuf,

    #[arg(num_args = 1.., required = true)]
    pub module_names: Vec<String>,
}

// --- rename ---

#[derive(Subcommand)]
pub enum RenameCommand {
    Decl(RenameDecl),
    Let(RenameLet),
    Arg(RenameArg),
}

#[derive(Args)]
pub struct RenameDecl {
    pub file: PathBuf,
    pub old_name: String,
    pub new_name: String,

    #[arg(long, value_enum, default_value_t = Format::Compact)]
    pub format: Format,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct RenameLet {
    pub file: PathBuf,
    pub decl: String,

    #[arg(long)]
    pub from: String,

    #[arg(long)]
    pub to: String,

    #[arg(long)]
    pub line: Option<usize>,
}

#[derive(Args)]
pub struct RenameArg {
    pub file: PathBuf,
    pub decl: String,

    #[arg(long)]
    pub from: String,

    #[arg(long)]
    pub to: String,
}

// --- add ---

#[derive(Subcommand)]
pub enum AddCommand {
    Arg(AddArg),
    Variant(AddVariant),
    Import(AddImport),
}

#[derive(Args)]
pub struct AddArg {
    pub file: PathBuf,
    pub decl: String,

    #[arg(long)]
    pub at: usize,

    #[arg(long)]
    pub name: String,

    /// Required when the target function has a type signature
    #[arg(long = "type")]
    pub type_annotation: Option<String>,
}

#[derive(Args)]
pub struct AddVariant {
    pub file: PathBuf,

    #[arg(long = "type")]
    pub type_name: String,

    pub definition: String,

    #[arg(long, value_enum, default_value_t = Format::Compact)]
    pub format: Format,

    #[arg(long)]
    pub dry_run: bool,

    /// Fill a case-site branch body instead of the default `Debug.todo "<Variant>"` stub.
    /// Syntax: `--fill <key>=<branch_text>` (repeatable). Split on the first `=`.
    #[arg(long, value_name = "KEY=BRANCH")]
    pub fill: Vec<String>,
}

#[derive(Args)]
pub struct AddImport {
    pub file: PathBuf,

    #[arg(num_args = 1.., required = true)]
    pub imports: Vec<String>,
}

// --- variant (read-only) ---

#[derive(Subcommand)]
pub enum VariantReadCommand {
    /// List every case expression on a type, with its enclosing function body and a
    /// stable site key.
    Cases {
        file: PathBuf,

        #[arg(long = "type")]
        type_name: String,

        #[arg(long, value_enum, default_value_t = Format::Compact)]
        format: Format,
    },
}

#[derive(Clone, ValueEnum)]
pub enum Format {
    Compact,
    Json,
}
