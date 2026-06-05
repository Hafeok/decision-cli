//! Cluster logic for TaskType execution.

use crate::core::task_type::cell_decl::{CellDecl, PlanError};
use std::collections::{HashMap, HashSet};

/// Represents a TaskType cluster ready for execution.
pub struct Cluster<'a> {
    /// The cells in this cluster.
    pub cells: &'a [CellDecl],
}

impl<'a> Cluster<'a> {
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