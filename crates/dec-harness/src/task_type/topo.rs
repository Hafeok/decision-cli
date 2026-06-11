//! Kahn-style topological sort over `CellDecl::derived_from`.
//!
//! Deterministic by construction: ties broken by lexicographic cell
//! name. Re-running over the same input produces a byte-identical
//! `Vec<String>` — locked by TC-370.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::types::CellDecl;

/// Errors `topo_order` surfaces.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TopoError {
    /// `derived_from` referenced a cell not present in the cluster.
    #[error("cell {citing:?} derives from unknown cell {missing:?}")]
    MissingCell {
        /// Cell whose `derived_from` referenced the missing name.
        citing: String,
        /// The referenced name with no matching CellDecl.
        missing: String,
    },
    /// At least one cycle exists in the `derived_from` graph.
    #[error("cycle in derived_from graph; cells remaining: {remaining:?}")]
    Cycle {
        /// Cells that could not be ordered (cycle members + downstream).
        remaining: Vec<String>,
    },
    /// Two cells share the same name.
    #[error("duplicate cell name {name:?}")]
    DuplicateName {
        /// The conflicting name.
        name: String,
    },
}

/// Return a topological order over `cells` respecting `derived_from`.
/// Deterministic: ties broken lexicographically.
///
/// Per FT-139 / TC-370 acceptance criteria:
/// - Every cell name appears exactly once in the output.
/// - For every cell `c` and `dep ∈ c.derived_from`, `dep` precedes `c`.
/// - Re-running over identical input produces a byte-identical result.
pub fn topo_order(cells: &[CellDecl]) -> Result<Vec<String>, TopoError> {
    // Detect duplicates.
    let mut seen_names: BTreeSet<&str> = BTreeSet::new();
    for c in cells {
        if !seen_names.insert(c.name.as_str()) {
            return Err(TopoError::DuplicateName {
                name: c.name.clone(),
            });
        }
    }

    // Build in-degree and adjacency. BTreeMap keys give deterministic
    // iteration order; BTreeSet edges break ties lexicographically.
    let mut in_degree: BTreeMap<String, usize> = BTreeMap::new();
    let mut adjacency: HashMap<String, BTreeSet<String>> = HashMap::new();
    for c in cells {
        in_degree.entry(c.name.clone()).or_insert(0);
        adjacency.entry(c.name.clone()).or_default();
    }
    for c in cells {
        for dep in &c.derived_from {
            if !in_degree.contains_key(dep) {
                return Err(TopoError::MissingCell {
                    citing: c.name.clone(),
                    missing: dep.clone(),
                });
            }
            adjacency
                .entry(dep.clone())
                .or_default()
                .insert(c.name.clone());
            *in_degree.entry(c.name.clone()).or_insert(0) += 1;
        }
    }

    // Seed the queue with every zero-in-degree cell.
    let mut ready: BTreeSet<String> = in_degree
        .iter()
        .filter_map(|(name, &deg)| if deg == 0 { Some(name.clone()) } else { None })
        .collect();

    let mut order: Vec<String> = Vec::with_capacity(cells.len());
    while let Some(next) = ready.iter().next().cloned() {
        ready.remove(&next);
        order.push(next.clone());
        if let Some(downstream) = adjacency.get(&next).cloned() {
            for child in downstream {
                let entry = in_degree.entry(child.clone()).or_insert(0);
                *entry = entry.saturating_sub(1);
                if *entry == 0 {
                    ready.insert(child);
                }
            }
        }
    }

    if order.len() != cells.len() {
        let placed: BTreeSet<&String> = order.iter().collect();
        let remaining: Vec<String> = in_degree
            .keys()
            .filter(|k| !placed.contains(k))
            .cloned()
            .collect();
        return Err(TopoError::Cycle { remaining });
    }

    Ok(order)
}
