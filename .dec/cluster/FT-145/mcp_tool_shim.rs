use clap::{Parser, Subcommand};
use std::process;

#[derive(Parser)]
#[clap(name = "dec")]
#[clap(bin_name = "dec")]
pub struct DecCli {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Manage product information
    Product(ProductSubcommand),
}

#[derive(Subcommand)]
pub enum ProductSubcommand {
    /// Show product status
    Status(StatusArgs),
}

#[derive(Parser)]
pub struct StatusArgs {
    /// Output format
    #[clap(long, default_value = "text")]
    pub format: String,
    
    /// Filter by phase
    #[clap(long)]
    pub phase: Option<u32>,
}

fn main() {
    let cli = DecCli::parse();
    
    match &cli.command {
        Commands::Product(product_cmd) => {
            match product_cmd {
                ProductSubcommand::Status(args) => {
                    // Import and call the handler function
                    if let Err(e) = crate::features::product_cmd::handle_status(args) {
                        eprintln!("Error: {}", e);
                        process::exit(1);
                    }
                }
            }
        }
    }
}