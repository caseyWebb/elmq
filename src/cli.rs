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
