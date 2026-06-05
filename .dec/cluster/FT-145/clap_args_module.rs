use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
pub struct ProductArgs {
    #[clap(subcommand)]
    pub command: ProductSubcommands,
}

#[derive(Debug, Subcommand)]
pub enum ProductSubcommands {
    /// Show the status of the project
    Status(StatusArgs),
}

#[derive(Debug, Parser)]
pub struct StatusArgs {
    /// Output format
    #[clap(long = "format", default_value = "text")]
    pub format: String,

    /// Filter by phase
    #[clap(long = "phase")]
    pub phase: Option<u32>,
}