//! TaskType + Cell substrate for decision-cli's self-implementation pipeline
//!
//! Implements ADR-080's decomposition of the SDLC into typed clusters of
//! cells, each with a specific artifact type and prompt.

use std::collections::{HashMap, HashSet};

use crate::core::capability_resolver::CapabilityResolver;

/// A TaskType declares a cluster of cells and a coherence audit.
#[derive(Debug, Clone)]
pub struct TaskTypeDecl {
    /// Unique name of the task type
    pub name: String,
    /// Ordered list of cells that make up this task cluster
    pub cells: Vec<CellDecl>,
    /// Pointer to the coherence audit script
    pub coherence_audit: CoherenceAuditSpec,
}

/// A Cell declares a specific artifact-producing component of a task cluster.
#[derive(Debug, Clone)]
pub struct CellDecl {
    /// Unique name of the cell
    pub name: String,
    /// Artifact type this cell produces
    pub artifact_type: String,
    /// Path to the Jinja prompt template
    pub prompt_template_path: String,
    /// Capability IRI that provides model binding for this cell
    pub model_binding_capability_iri: String,
    /// Names of cells this cell depends on (for topological ordering)
    pub derived_from: Vec<String>,
}

/// Specification for a coherence audit.
#[derive(Debug, Clone)]
pub struct CoherenceAuditSpec {
    /// Path to the audit script
    pub script_path: String,
    /// Timeout in seconds
    pub timeout_seconds: u32,
}

/// Cluster represents a task type's cell cluster with ordering capabilities.
pub struct Cluster;

impl Cluster {
    /// Returns a topological order of cells based on their `derived_from` dependencies.
    ///
    /// # Errors
    /// Returns `PlanError::ClusterCycle` if a cycle is detected in the dependency graph.
    pub fn topo_order(cells: &[CellDecl]) -> Result<Vec<String>, PlanError> {
        let mut cell_map = HashMap::new();
        for cell in cells {
            cell_map.insert(&cell.name, cell);
        }

        // Build dependency graph
        let mut dependencies = HashMap::new();
        let mut dependents = HashMap::new();

        for cell in cells {
            dependencies.insert(&cell.name, HashSet::new());
            dependents.insert(&cell.name, HashSet::new());
        }

        // Populate dependencies
        for cell in cells {
            for dep in &cell.derived_from {
                if !cell_map.contains_key(dep) {
                    return Err(PlanError::MissingDerivedFromTarget {
                        cell_name: cell.name.clone(),
                        missing_target: dep.clone(),
                    });
                }
                dependencies.get_mut(dep).unwrap().insert(cell.name.clone());
                dependents.get_mut(&cell.name).unwrap().insert(dep.clone());
            }
        }

        // Kahn's algorithm for topological sort
        let mut ready = Vec::new();
        let mut result = Vec::new();

        // Find nodes with no incoming edges
        for cell in cells {
            if dependents[&cell.name].is_empty() {
                ready.push(cell.name.clone());
            }
        }

        while let Some(current) = ready.pop() {
            result.push(current.clone());

            // Remove current node from dependencies
            if let Some(deps) = dependencies.get(&current) {
                for dep in deps {
                    if let Some(dependents_of_dep) = dependents.get_mut(dep) {
                        dependents_of_dep.remove(&current);
                        if dependents_of_dep.is_empty() {
                            ready.push(dep.clone());
                        }
                    }
                }
            }
        }

        // Check for cycles
        if result.len() != cells.len() {
            // Find the cycle by finding nodes that were not processed
            let mut visited = HashSet::new();
            for cell in cells {
                visited.insert(&cell.name);
            }
            for name in &result {
                visited.remove(name);
            }

            // Return one cycle path
            let cycle_start = visited.iter().next().cloned().unwrap();
            let mut cycle = vec![cycle_start.clone()];
            let mut current = cycle_start;
            let mut seen = HashSet::new();
            seen.insert(current.clone());

            // Follow the dependency chain to find a cycle
            loop {
                let next = dependencies.get(&current).and_then(|deps| deps.iter().next()).cloned();
                match next {
                    Some(next_node) => {
                        if seen.contains(&next_node) {
                            // Found cycle
                            cycle.push(next_node);
                            break;
                        }
                        seen.insert(next_node.clone());
                        cycle.push(next_node.clone());
                        current = next_node;
                    }
                    None => break,
                }
            }

            return Err(PlanError::ClusterCycle { cycle_path: cycle });
        }

        Ok(result)
    }
}

/// Errors that can occur during task type planning
#[derive(Debug, Clone)]
pub enum PlanError {
    /// Detected a cycle in the cluster's `derived_from` dependencies
    ClusterCycle { cycle_path: Vec<String> },
    /// A cell references a `derived_from` target that doesn't exist
    MissingDerivedFromTarget {
        cell_name: String,
        missing_target: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topo_order_acyclic() {
        let cells = vec![
            CellDecl {
                name: "capability_binding".to_string(),
                artifact_type: "capability_binding".to_string(),
                prompt_template_path: "templates/capability_binding.j2".to_string(),
                model_binding_capability_iri: "cap:binding".to_string(),
                derived_from: vec![],
            },
            CellDecl {
                name: "pydantic_io_models".to_string(),
                artifact_type: "pydantic_io_models".to_string(),
                prompt_template_path: "templates/pydantic_io_models.j2".to_string(),
                model_binding_capability_iri: "cap:model".to_string(),
                derived_from: vec!["capability_binding".to_string()],
            },
            CellDecl {
                name: "system_prompt".to_string(),
                artifact_type: "system_prompt".to_string(),
                prompt_template_path: "templates/system_prompt.j2".to_string(),
                model_binding_capability_iri: "cap:prompt".to_string(),
                derived_from: vec!["pydantic_io_models".to_string()],
            },
            CellDecl {
                name: "agent_loop".to_string(),
                artifact_type: "agent_loop".to_string(),
                prompt_template_path: "templates/agent_loop.j2".to_string(),
                model_binding_capability_iri: "cap:loop".to_string(),
                derived_from: vec!["pydantic_io_models".to_string(), "system_prompt".to_string()],
            },
            CellDecl {
                name: "unit_tests".to_string(),
                artifact_type: "unit_tests".to_string(),
                prompt_template_path: "templates/unit_tests.j2".to_string(),
                model_binding_capability_iri: "cap:test".to_string(),
                derived_from: vec!["pydantic_io_models".to_string(), "agent_loop".to_string()],
            },
        ];

        let order = Cluster::topo_order(&cells).unwrap();
        assert_eq!(order.len(), 5);

        // Check that dependencies are respected
        let positions: HashMap<String, usize> = order
            .iter()
            .enumerate()
            .map(|(i, s)| (s.clone(), i))
            .collect();

        // Verify each dependency comes before dependent
        assert!(positions[&"capability_binding".to_string()] < positions[&"pydantic_io_models".to_string()]);
        assert!(positions[&"pydantic_io_models".to_string()] < positions[&"system_prompt".to_string()]);
        assert!(positions[&"pydantic_io_models".to_string()] < positions[&"agent_loop".to_string()]);
        assert!(positions[&"system_prompt".to_string()] < positions[&"agent_loop".to_string()]);
        assert!(positions[&"pydantic_io_models".to_string()] < positions[&"unit_tests".to_string()]);
        assert!(positions[&"agent_loop".to_string()] < positions[&"unit_tests".to_string()]);
    }

    #[test]
    fn test_topo_order_cycle_detection() {
        let cells = vec![
            CellDecl {
                name: "cell_a".to_string(),
                artifact_type: "artifact_a".to_string(),
                prompt_template_path: "templates/a.j2".to_string(),
                model_binding_capability_iri: "cap:a".to_string(),
                derived_from: vec!["cell_b".to_string()],
            },
            CellDecl {
                name: "cell_b".to_string(),
                artifact_type: "artifact_b".to_string(),
                prompt_template_path: "templates/b.j2".to_string(),
                model_binding_capability_iri: "cap:b".to_string(),
                derived_from: vec!["cell_a".to_string()],
            },
        ];

        let result = Cluster::topo_order(&cells);
        assert!(matches!(result, Err(PlanError::ClusterCycle { .. })));
    }

    #[test]
    fn test_topo_order_missing_dependency() {
        let cells = vec![
            CellDecl {
                name: "cell_a".to_string(),
                artifact_type: "artifact_a".to_string(),
                prompt_template_path: "templates/a.j2".to_string(),
                model_binding_capability_iri: "cap:a".to_string(),
                derived_from: vec!["nonexistent_cell".to_string()],
            },
        ];

        let result = Cluster::topo_order(&cells);
        assert!(matches!(result, Err(PlanError::MissingDerivedFromTarget { .. })));
    }
}