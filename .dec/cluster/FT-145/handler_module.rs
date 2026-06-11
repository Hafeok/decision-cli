use crate::cli::product::StatusArgs;
use crate::config::ProductConfig;
use crate::error::CliResult;
use crate::output::format_status_output;

pub fn handle_product_status(args: &StatusArgs) -> CliResult<()> {
    let config = ProductConfig::discover()?;
    
    // Load the product graph
    let graph = config.load_graph()?;
    
    // Format and output based on args
    let output = format_status_output(&graph, args)?;
    println!("{}", output);
    
    Ok(())
}