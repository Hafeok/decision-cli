//! Declaration of a Cell - a single artifact-producing unit within a TaskType cluster.

use crate::core::task_type::cluster::Cluster;
use oxigraph::model::NamedNode;
use std::path::PathBuf;

/// A Cell declares a single artifact-producing unit within a TaskType cluster.
#[derive(Debug, Clone)]
pub struct CellDecl {
    /// The unique name of this Cell within its TaskType.
    pub name: String,

    /// The type of artifact this Cell produces (e.g., "pydantic_io_models", "system_prompt").
    pub artifact_type: String,

    /// The path to the Jinja template used to render the prompt for this Cell.
    pub prompt_template_path: PathBuf,

    /// The capability IRI used to resolve the model binding for this Cell.
    pub model_binding_capability_iri: NamedNode,

    /// The names of Cells this Cell depends on (must be emitted before this Cell).
    pub derived_from: Vec<String>,
}

impl CellDecl {
    /// Check if this Cell has dependencies.
    pub fn has_dependencies(&self) -> bool {
        !self.derived_from.is_empty()
    }
}

impl Cluster {
    /// Validates that the dependency graph is acyclic and returns a topological order.
    ///
    /// This is a Kahn-style topological sort that ensures all dependencies are satisfied.
    pub fn topo_order(cells: &[CellDecl]) -> Result<Vec<String>, PlanError> {
        use std::collections::{HashMap, HashSet, VecDeque};

        // Build adjacency list and in-degree count
        let mut adj_list: HashMap<String, Vec<String>> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();

        // Initialize all nodes
        for cell in cells {
            in_degree.insert(cell.name.clone(), 0);
            adj_list.insert(cell.name.clone(), Vec::new());
        }

        // Build graph
        for cell in cells {
            // For each dependency, increment in-degree of dependent node
            for dep in &cell.derived_from {
                if !in_degree.contains_key(dep) {
                    return Err(PlanError::MissingDependency {
                        cell: cell.name.clone(),
                        dependency: dep.clone(),
                    });
                }
                in_degree.insert(dep.clone(), in_degree[dep] + 1);
                adj_list.get_mut(&cell.name).unwrap().push(dep.clone());
            }
        }

        // Kahn's algorithm for topological sorting
        let mut queue = VecDeque::new();
        let mut result = Vec::new();

        // Find all nodes with zero in-degree
        for (node, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(node.clone());
            }
        }

        // Process nodes
        while let Some(node) = queue.pop_front() {
            result.push(node.clone());

            // For each neighbor (dependent node), reduce in-degree
            for neighbor in adj_list.get(&node).unwrap_or(&vec![]) {
                let new_degree = in_degree.get_mut(neighbor).unwrap() - 1;
                *in_degree.get_mut(neighbor).unwrap() = new_degree;
                
                if new_degree == 0 {
                    queue.push_back(neighbor.clone());
                }
            }
        }

        // Check for cycles
        if result.len() != cells.len() {
            // Find the cycle by finding nodes with non-zero in-degree
            let mut cycle_nodes: Vec<String> = in_degree
                .into_iter()
                .filter(|(_, &degree)| degree > 0)
                .map(|(node, _)| node)
                .collect();
            
            // Sort to make the error deterministic
            cycle_nodes.sort();
            
            return Err(PlanError::ClusterCycle {
                cycle_path: cycle_nodes,
            });
        }

        Ok(result)
    }
}

/// Errors that can occur during cluster planning.
#[derive(Debug, Clone, PartialEq)]
pub enum PlanError {
    /// A cell refers to a dependency that doesn't exist.
    MissingDependency {
        cell: String,
        dependency: String,
    },
    /// A cycle was detected in the cluster's derived_from graph.
    ClusterCycle {
        cycle_path: Vec<String>,
    },
}