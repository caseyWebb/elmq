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
}

#[derive(Clone, ValueEnum)]
pub enum Format {
    Compact,
    Json,
}
