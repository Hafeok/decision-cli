//! Unit tests for FT-139 task_type substrate.

use super::types::{CellDecl, CoherenceAuditSpec, TaskTypeDecl};
use super::{lookup, registered_names, topo_order, TopoError};
use std::path::PathBuf;

fn dummy_audit() -> CoherenceAuditSpec {
    CoherenceAuditSpec {
        script_path: PathBuf::from("scripts/checks/dummy.py"),
        timeout_seconds: 30,
    }
}

fn cell(name: &str, derived_from: &[&str]) -> CellDecl {
    CellDecl {
        name: name.to_string(),
        artifact_type: "test".to_string(),
        prompt_template_path: PathBuf::from(format!("templates/{name}.tmpl")),
        model_binding_capability_id: String::new(),
        derived_from: derived_from.iter().map(|s| (*s).to_string()).collect(),
    }
}

// ----------------------------------------------------------------------
// TC-370 (exit-criteria) — add-judge-worker cluster topological order is
// acyclic, dependency-respecting, deterministic.
// ----------------------------------------------------------------------

#[test]
fn add_judge_worker_topo_order() {
    let tt = lookup("add-judge-worker").expect("add-judge-worker registered");
    let order = topo_order(&tt.cells).expect("topo order is acyclic");

    // Every cell appears exactly once.
    assert_eq!(order.len(), tt.cells.len(), "every cell appears once");
    let unique: std::collections::HashSet<_> = order.iter().collect();
    assert_eq!(unique.len(), order.len(), "no duplicates");

    // Every dep precedes its dependent.
    for (i, cell_name) in order.iter().enumerate() {
        let cell = tt
            .cells
            .iter()
            .find(|c| &c.name == cell_name)
            .expect("cell exists");
        for dep in &cell.derived_from {
            let dep_pos = order
                .iter()
                .position(|n| n == dep)
                .expect("dep in order");
            assert!(
                dep_pos < i,
                "{dep} must precede {cell_name} in order, got dep_pos={dep_pos} pos={i}"
            );
        }
    }

    // Deterministic: byte-identical when re-run.
    let order2 = topo_order(&tt.cells).expect("second run also acyclic");
    assert_eq!(order, order2, "topo order is deterministic");

    // Registry stability: name is present.
    assert!(registered_names().contains(&"add-judge-worker"));
}

#[test]
fn topo_detects_cycle() {
    // a -> b -> a
    let cells = vec![cell("a", &["b"]), cell("b", &["a"])];
    let err = topo_order(&cells).expect_err("cycle must be detected");
    matches!(err, TopoError::Cycle { .. });
}

#[test]
fn topo_detects_missing_dep() {
    let cells = vec![cell("a", &["nonexistent"])];
    let err = topo_order(&cells).expect_err("missing dep must error");
    matches!(err, TopoError::MissingCell { .. });
}

#[test]
fn topo_detects_duplicate_name() {
    let cells = vec![cell("a", &[]), cell("a", &[])];
    let err = topo_order(&cells).expect_err("duplicate must error");
    matches!(err, TopoError::DuplicateName { .. });
}

#[test]
fn topo_handles_diamond() {
    // top -> {l, r} -> bot
    let cells = vec![
        cell("top", &[]),
        cell("l", &["top"]),
        cell("r", &["top"]),
        cell("bot", &["l", "r"]),
    ];
    let order = topo_order(&cells).unwrap();
    assert_eq!(order[0], "top");
    assert_eq!(order[3], "bot");
    // l before bot, r before bot.
    let l_pos = order.iter().position(|s| s == "l").unwrap();
    let r_pos = order.iter().position(|s| s == "r").unwrap();
    let bot_pos = order.iter().position(|s| s == "bot").unwrap();
    assert!(l_pos < bot_pos);
    assert!(r_pos < bot_pos);
}

#[test]
fn add_judge_worker_audit_path_matches_spec() {
    let tt = lookup("add-judge-worker").unwrap();
    assert!(tt
        .coherence_audit
        .script_path
        .ends_with("cluster-audit-add-judge-worker.py"));
}
