//! `dec _*-binding` — hidden helpers for manual role-binding lifecycle
//! management (FT-118).
//!
//! Three commands:
//! - `_activate-binding` — atomically activate one binding and deactivate
//!   all others for the same role
//! - `_deactivate-binding` — flip a binding's `dec:active` to false
//! - `_list-bindings` — enumerate all bindings (active + inactive) for
//!   a role
//!
//! These are the manual recovery path when bootstrap leaves duplicate-
//! active state or when operators need to roll back a binding version.

use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use oxigraph::model::{GraphName, Literal, NamedNode, Quad};

use decision_cli::core::ontology::role_binding::all_for_role;
use decision_cli::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use decision_cli::core::vocab::role_binding_graph;

#[derive(Debug, clap::Args)]
pub struct ActivateBindingArgs {
    /// Role ID (e.g., "verify-graph-author").
    #[arg(long)]
    pub role: String,
    /// Version to activate (≥ 1).
    #[arg(long)]
    pub version: u32,
}

#[derive(Debug, clap::Args)]
pub struct DeactivateBindingArgs {
    /// Role ID.
    #[arg(long)]
    pub role: String,
    /// Version to deactivate.
    #[arg(long)]
    pub version: u32,
}

#[derive(Debug, clap::Args)]
pub struct ListBindingsArgs {
    /// Optional role filter (lists all roles if omitted).
    #[arg(long)]
    pub role: Option<String>,
}

pub fn activate(workdir: &Path, args: ActivateBindingArgs) -> ExitCode {
    match run_activate(workdir, &args.role, args.version) {
        Ok(()) => {
            println!(
                "activated {}@v{} (deactivated all prior versions)",
                args.role, args.version
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("dec _activate-binding: {e}");
            ExitCode::from(1)
        }
    }
}

pub fn deactivate(workdir: &Path, args: DeactivateBindingArgs) -> ExitCode {
    match run_deactivate(workdir, &args.role, args.version) {
        Ok(changed) => {
            if changed {
                println!("deactivated {}@v{}", args.role, args.version);
            } else {
                println!("{}@v{} already inactive (no-op)", args.role, args.version);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("dec _deactivate-binding: {e}");
            ExitCode::from(1)
        }
    }
}

pub fn list(workdir: &Path, args: ListBindingsArgs) -> ExitCode {
    match run_list(workdir, args.role.as_deref()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("dec _list-bindings: {e}");
            ExitCode::from(1)
        }
    }
}

fn run_activate(workdir: &Path, role_id: &str, version: u32) -> Result<(), String> {
    let dump = orchestration_dump_path(workdir);
    let store = load_store_from_dump(&dump).map_err(|e| e.to_string())?;

    let bindings = all_for_role(&store, role_id).map_err(|e| e.to_string())?;
    let target = bindings
        .iter()
        .find(|b| b.version == version)
        .ok_or_else(|| format!("no such binding {}@v{}", role_id, version))?;

    let bind_graph = role_binding_graph();
    let mut quads_to_remove = Vec::new();
    let mut quads_to_insert = Vec::new();

    let active_pred = NamedNode::new_unchecked("https://decision-cli.dev/ns#active");
    let xsd_bool = NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#boolean");
    let lit_true = Literal::new_typed_literal("true", xsd_bool.clone());
    let lit_false = Literal::new_typed_literal("false", xsd_bool);

    // Deactivate all active bindings for this role
    for binding in &bindings {
        if binding.active {
            quads_to_remove.push(Quad::new(
                binding.iri(),
                active_pred.clone(),
                lit_true.clone(),
                GraphName::from(bind_graph),
            ));
            quads_to_insert.push(Quad::new(
                binding.iri(),
                active_pred.clone(),
                lit_false.clone(),
                GraphName::from(bind_graph),
            ));
        }
    }

    // Activate the target
    quads_to_remove.push(Quad::new(
        target.iri(),
        active_pred.clone(),
        lit_false.clone(),
        GraphName::from(bind_graph),
    ));
    quads_to_insert.push(Quad::new(
        target.iri(),
        active_pred,
        lit_true,
        GraphName::from(bind_graph),
    ));

    store
        .transaction(|mut tx| {
            for q in &quads_to_remove {
                tx.remove(q)?;
            }
            for q in &quads_to_insert {
                tx.insert(q.as_ref())?;
            }
            Ok::<_, oxigraph::store::StorageError>(())
        })
        .map_err(|e| e.to_string())?;

    let store_arc = Arc::new(store);
    persist_store(&store_arc, &dump).map_err(|e| e.to_string())?;

    Ok(())
}

fn run_deactivate(workdir: &Path, role_id: &str, version: u32) -> Result<bool, String> {
    let dump = orchestration_dump_path(workdir);
    let store = load_store_from_dump(&dump).map_err(|e| e.to_string())?;

    let bindings = all_for_role(&store, role_id).map_err(|e| e.to_string())?;
    let target = bindings
        .iter()
        .find(|b| b.version == version)
        .ok_or_else(|| format!("no such binding {}@v{}", role_id, version))?;

    if !target.active {
        return Ok(false);
    }

    let bind_graph = role_binding_graph();
    let active_pred = NamedNode::new_unchecked("https://decision-cli.dev/ns#active");
    let xsd_bool = NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#boolean");
    let lit_true = Literal::new_typed_literal("true", xsd_bool.clone());
    let lit_false = Literal::new_typed_literal("false", xsd_bool);

    let remove_quad = Quad::new(
        target.iri(),
        active_pred.clone(),
        lit_true,
        GraphName::from(bind_graph),
    );
    let insert_quad = Quad::new(target.iri(), active_pred, lit_false, GraphName::from(bind_graph));

    store
        .transaction(|mut tx| {
            tx.remove(&remove_quad)?;
            tx.insert(insert_quad.as_ref())?;
            Ok::<_, oxigraph::store::StorageError>(())
        })
        .map_err(|e| e.to_string())?;

    let store_arc = Arc::new(store);
    persist_store(&store_arc, &dump).map_err(|e| e.to_string())?;

    Ok(true)
}

fn run_list(workdir: &Path, role_filter: Option<&str>) -> Result<(), String> {
    let dump = orchestration_dump_path(workdir);
    let store = load_store_from_dump(&dump).map_err(|e| e.to_string())?;

    let roles_to_list: Vec<String> = if let Some(role) = role_filter {
        vec![role.to_string()]
    } else {
        // List all unique role_ids (requires a SPARQL query)
        use oxigraph::sparql::QueryResults;
        let q = "PREFIX dec: <https://decision-cli.dev/ns#> \
                 SELECT DISTINCT ?role WHERE { \
                   { ?b a dec:RoleBinding ; dec:role_id ?role } \
                   UNION \
                   { GRAPH ?g { ?b a dec:RoleBinding ; dec:role_id ?role } } \
                 }";
        let QueryResults::Solutions(mut sols) = store.query(q).map_err(|e| e.to_string())? else {
            return Ok(());
        };
        let mut roles = Vec::new();
        while let Some(Ok(sol)) = sols.next() {
            if let Some(oxigraph::model::Term::Literal(lit)) = sol.get("role") {
                roles.push(lit.value().to_string());
            }
        }
        roles.sort();
        roles
    };

    for role in roles_to_list {
        let bindings = all_for_role(&store, &role).map_err(|e| e.to_string())?;
        for binding in bindings {
            let status = if binding.active { "active" } else { "inactive" };
            println!(
                "{:<30} v{:<3} {:8} default={}",
                role,
                binding.version,
                status,
                binding.default_capability.as_str()
            );
        }
    }

    Ok(())
}
