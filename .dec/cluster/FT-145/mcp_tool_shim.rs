use clap::Parser;

/// The main CLI application for dec commands
#[derive(Parser)]
#[command(name = "dec")]
#[command(about = "Decision tool for managing product features")]
pub struct DecCli {
    #[clap(subcommand)]
    pub command: Commands,
}

/// Available commands
#[derive(clap::Subcommand)]
pub enum Commands {
    /// Manage product features
    Product(ProductCommands),
}

/// Product-related commands
#[derive(clap::Subcommand)]
pub enum ProductCommands {
    /// Show product status
    Status(StatusArgs),
}

/// Arguments for the status command
#[derive(Parser)]
pub struct StatusArgs {
    /// Output format
    #[arg(long, value_enum, default_value = "text")]
    pub format: OutputFormat,
    
    /// Filter by phase
    #[arg(long)]
    pub phase: Option<u32>,
}

/// Output format options
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    Text,
    Json,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_args_parse() {
        let args = StatusArgs::try_parse_from(["dec", "status"]).unwrap();
        assert_eq!(args.format, OutputFormat::Text);
        assert_eq!(args.phase, None);
    }

    #[test]
    fn test_status_args_with_format() {
        let args = StatusArgs::try_parse_from(["dec", "status", "--format", "json"]).unwrap();
        assert_eq!(args.format, OutputFormat::Json);
    }

    #[test]
    fn test_status_args_with_phase() {
        let args = StatusArgs::try_parse_from(["dec", "status", "--phase", "2"]).unwrap();
        assert_eq!(args.phase, Some(2));
    }
}