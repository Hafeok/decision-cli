use crate::features::product_cmd::StatusArgs;
use crate::product::ProductConfig;
use anyhow::Result;
use std::collections::HashMap;

pub fn handle_status(args: &StatusArgs) -> Result<()> {
    let product_config = ProductConfig::discover()?;
    
    // Simulate fetching project status data
    let mut status_data = HashMap::new();
    
    // Add per-phase counts (mock data)
    status_data.insert("phase_1", vec!["ready", "blocked", "in_progress"]);
    status_data.insert("phase_2", vec!["ready", "in_progress"]);
    status_data.insert("phase_3", vec!["blocked"]);
    
    // Add gate states (mock data)
    status_data.insert("gates", vec!["ready", "blocked"]);
    
    // Add exit criteria coverage (mock data)
    status_data.insert("exit_criteria", vec!["covered", "partial", "not_covered"]);
    
    // Add features by status (mock data)
    status_data.insert("features_by_status", vec!["ready", "blocked", "in_progress"]);
    
    // Format output based on --format flag
    match args.format.as_str() {
        "json" => {
            let json_output = serde_json::to_string_pretty(&status_data)?;
            println!("{}", json_output);
        }
        _ => {
            // Default to text format
            println!("Project Status Summary:");
            println!("=======================");
            
            // Print per-phase counts
            println!("\nPhase Status:");
            for (phase, statuses) in &status_data {
                if phase.starts_with("phase_") {
                    println!("  {}: {:?}", phase, statuses);
                }
            }
            
            // Print gate states
            if let Some(gates) = status_data.get("gates") {
                println!("\nGates:");
                for gate in gates {
                    println!("  {}", gate);
                }
            }
            
            // Print exit criteria coverage
            if let Some(criteria) = status_data.get("exit_criteria") {
                println!("\nExit Criteria Coverage:");
                for criterion in criteria {
                    println!("  {}", criterion);
                }
            }
            
            // Print features by status
            if let Some(features) = status_data.get("features_by_status") {
                println!("\nFeatures by Status:");
                for feature in features {
                    println!("  {}", feature);
                }
            }
        }
    }
    
    Ok(())
}