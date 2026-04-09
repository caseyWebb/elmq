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
    /// Show a summary of an Elm file
    List {
        /// Path to the Elm file
        file: PathBuf,

        /// Output format
        #[arg(long, value_enum, default_value_t = Format::Compact)]
        format: Format,

        /// Include doc comments in output
        #[arg(long)]
        docs: bool,
    },
    /// Extract the full source of a declaration by name
    Get {
        /// Path to the Elm file
        file: PathBuf,

        /// Name of the declaration to extract
        name: String,

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
    /// Remove a declaration by name
    Rm {
        /// Path to the Elm file
        file: PathBuf,

        /// Name of the declaration to remove
        name: String,
    },
    /// Manage imports
    Import {
        #[command(subcommand)]
        command: ImportCommand,
    },
    /// Add an item to the module's exposing list
    Expose {
        /// Path to the Elm file
        file: PathBuf,

        /// Item to expose (e.g. "update" or "Msg(..)")
        item: String,
    },
    /// Remove an item from the module's exposing list
    Unexpose {
        /// Path to the Elm file
        file: PathBuf,

        /// Item to unexpose (e.g. "helper")
        item: String,
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
    /// Find all references to a module or declaration across the project
    Refs {
        /// Path to the Elm file whose module to search for
        file: PathBuf,

        /// Declaration name to search for (if omitted, reports module-level imports)
        name: Option<String>,

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

        /// Names of declarations to move (can be repeated)
        #[arg(long = "name")]
        names: Vec<String>,

        /// Path to the target Elm file
        #[arg(long = "to")]
        target: PathBuf,

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
    /// Add or replace an import
    Add {
        /// Path to the Elm file
        file: PathBuf,

        /// Import clause (e.g. "Html exposing (Html, div, text)")
        import: String,
    },
    /// Remove an import by module name
    Remove {
        /// Path to the Elm file
        file: PathBuf,

        /// Module name to remove (e.g. "Html")
        module_name: String,
    },
}

#[derive(Clone, ValueEnum)]
pub enum Format {
    Compact,
    Json,
}
