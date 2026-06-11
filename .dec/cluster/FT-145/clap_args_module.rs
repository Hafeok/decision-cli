use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct ProductArgs {
    #[clap(subcommand)]
    pub command: ProductCommand,
}

#[derive(Subcommand, Debug)]
pub enum ProductCommand {
    /// Show project status information
    Status(StatusArgs),
}

#[derive(Parser, Debug)]
pub struct StatusArgs {
    /// Output format
    #[clap(long = "format", default_value = "text")]
    pub format: String,

    /// Filter by phase number
    #[clap(long = "phase")]
    pub phase: Option<u32>,
}