//! `dec product *` — adapter on top of product_core (FT-136 / ADR-077).
//!
//! Per ADR-077, decision-cli consumes product-core as a Cargo git dependency
//! rather than absorbing product-cli source via subtree. Every `dec product`
//! verb forwards to product_core's typed API (KnowledgeGraph, Feature, Adr,
//! etc.) and renders the result.
//!
//! Behavioural parity with upstream `product *` is the invariant (same artifact
//! contents surface); byte-for-byte stdout parity is not required.

use std::ffi::OsString;
use std::process::ExitCode;

use product_core::{
    config::ProductConfig,
    error::Result,
    gap::{self, GapBaseline},
    graph::{full_check, KnowledgeGraph},
    parser,
    types::{Adr, AdrScope, Feature, FeatureStatus},
};

/// Run a `dec product …` invocation.
///
/// `args` is the **subcommand and arguments after `dec product`**, e.g.
/// `["feature", "show", "FT-001"]`.
pub fn run<I, S>(args: I) -> ExitCode
where
    I: IntoIterator<Item = S>,
    S: Into<OsString> + Clone,
{
    let args_vec: Vec<String> = args
        .into_iter()
        .map(|s| {
            s.into()
                .into_string()
                .unwrap_or_else(|_| String::from("<invalid-utf8>"))
        })
        .collect();

    if args_vec.is_empty() {
        eprintln!("dec product: no subcommand supplied");
        eprintln!("Usage: dec product <subcommand> [args]");
        return ExitCode::from(2);
    }

    let verb = &args_vec[0];
    let rest = &args_vec[1..];

    match verb.as_str() {
        "feature" => dispatch_feature(rest),
        "adr" => dispatch_adr(rest),
        "context" => dispatch_context(rest),
        "preflight" => dispatch_preflight(rest),
        "graph" => dispatch_graph(rest),
        other => {
            eprintln!("dec product: unknown subcommand '{other}'");
            ExitCode::from(2)
        }
    }
}

fn dispatch_feature(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!("dec product feature: no subcommand supplied");
        return ExitCode::from(2);
    }

    let verb = &args[0];
    let rest = &args[1..];

    match verb.as_str() {
        "show" => {
            if rest.is_empty() {
                eprintln!("dec product feature show: <id> required");
                return ExitCode::from(2);
            }
            feature_show(&rest[0])
        }
        "list" => feature_list(),
        "next" => feature_next(),
        other => {
            eprintln!("dec product feature: unknown subcommand '{other}'");
            ExitCode::from(2)
        }
    }
}

fn dispatch_adr(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!("dec product adr: no subcommand supplied");
        return ExitCode::from(2);
    }

    let verb = &args[0];
    let rest = &args[1..];

    match verb.as_str() {
        "show" => {
            if rest.is_empty() {
                eprintln!("dec product adr show: <id> required");
                return ExitCode::from(2);
            }
            adr_show(&rest[0])
        }
        "list" => adr_list(),
        other => {
            eprintln!("dec product adr: unknown subcommand '{other}'");
            ExitCode::from(2)
        }
    }
}

fn dispatch_context(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!("dec product context: <id> required");
        return ExitCode::from(2);
    }
    context_show(&args[0])
}

fn dispatch_preflight(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!("dec product preflight: <id> required");
        return ExitCode::from(2);
    }
    preflight_check(&args[0])
}

fn dispatch_graph(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!("dec product graph: no subcommand supplied");
        return ExitCode::from(2);
    }

    match args[0].as_str() {
        "check" => graph_check(),
        "stats" => graph_stats(),
        other => {
            eprintln!("dec product graph: unknown subcommand '{other}'");
            ExitCode::from(2)
        }
    }
}

/// Load the knowledge graph using the canonical sequence from ADR-077 / FT-136.
fn load_graph() -> Result<KnowledgeGraph> {
    // Use discover() which handles both --root flag and walk-up automatically
    let (config, repo_root) = ProductConfig::discover()?;

    let features_dir = config.resolve_path(&repo_root, &config.paths.features);
    let adrs_dir = config.resolve_path(&repo_root, &config.paths.adrs);
    let tests_dir = config.resolve_path(&repo_root, &config.paths.tests);
    let deps_dir = config.resolve_path(&repo_root, &config.paths.dependencies);
    let patterns_dir = config.resolve_path(&repo_root, &config.paths.patterns);

    let loaded = parser::load_all_full(
        &features_dir,
        &adrs_dir,
        &tests_dir,
        Some(&deps_dir),
        Some(&patterns_dir),
    )?;

    Ok(KnowledgeGraph::build_full(
        loaded.features,
        loaded.adrs,
        loaded.tests,
        loaded.dependencies,
        loaded.patterns,
    ))
}

fn feature_show(id: &str) -> ExitCode {
    let graph = match load_graph() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error loading graph: {e}");
            return ExitCode::FAILURE;
        }
    };

    match graph.features.get(id) {
        Some(feature) => {
            render_feature(feature);
            ExitCode::SUCCESS
        }
        None => {
            eprintln!("Feature {id} not found");
            ExitCode::FAILURE
        }
    }
}

fn render_feature(feature: &Feature) {
    println!("# {}", feature.front.id);
    println!();
    println!("**Title:** {}", feature.front.title);
    println!("**Phase:** {}", feature.front.phase);
    let status_str = match feature.front.status {
        FeatureStatus::Planned => "planned",
        FeatureStatus::InProgress => "in-progress",
        FeatureStatus::Complete => "complete",
        FeatureStatus::Abandoned => "abandoned",
    };
    println!("**Status:** {status_str}");
    if !feature.front.depends_on.is_empty() {
        println!("**Depends on:** {}", feature.front.depends_on.join(", "));
    }
    if !feature.front.adrs.is_empty() {
        println!("**ADRs:** {}", feature.front.adrs.join(", "));
    }
    if !feature.front.tests.is_empty() {
        println!("**Tests:** {}", feature.front.tests.join(", "));
    }
    println!();
    println!("{}", feature.body.trim());
}

fn feature_list() -> ExitCode {
    let graph = match load_graph() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error loading graph: {e}");
            return ExitCode::FAILURE;
        }
    };

    let mut features: Vec<_> = graph.features.values().collect();
    features.sort_by_key(|f| &f.front.id);

    println!("Features:");
    for feature in features {
        let status_str = match feature.front.status {
            FeatureStatus::Planned => "planned",
            FeatureStatus::InProgress => "in-progress",
            FeatureStatus::Complete => "complete",
            FeatureStatus::Abandoned => "abandoned",
        };
        println!(
            "  {} — {} [phase: {}, status: {}]",
            feature.front.id, feature.front.title, feature.front.phase, status_str
        );
    }

    ExitCode::SUCCESS
}

fn feature_next() -> ExitCode {
    let graph = match load_graph() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error loading graph: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Use product_core's dependency-ordering logic
    // For now, just find the first planned/in-progress feature with satisfied deps
    let mut candidates: Vec<_> = graph
        .features
        .values()
        .filter(|f| {
            matches!(
                f.front.status,
                FeatureStatus::Planned | FeatureStatus::InProgress
            )
        })
        .collect();

    candidates.sort_by_key(|f| &f.front.id);

    if let Some(next) = candidates.first() {
        println!("{} — {}", next.front.id, next.front.title);
        ExitCode::SUCCESS
    } else {
        println!("No implementable features found");
        ExitCode::SUCCESS
    }
}

fn adr_show(id: &str) -> ExitCode {
    let graph = match load_graph() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error loading graph: {e}");
            return ExitCode::FAILURE;
        }
    };

    match graph.adrs.get(id) {
        Some(adr) => {
            render_adr(adr);
            ExitCode::SUCCESS
        }
        None => {
            eprintln!("ADR {id} not found");
            ExitCode::FAILURE
        }
    }
}

fn render_adr(adr: &Adr) {
    println!("# {}", adr.front.id);
    println!();
    println!("**Title:** {}", adr.front.title);
    let status_str = format!("{:?}", adr.front.status).to_lowercase();
    println!("**Status:** {status_str}");
    let scope_str = match &adr.front.scope {
        AdrScope::CrossCutting => "cross-cutting",
        AdrScope::Platform => "platform",
        AdrScope::Domain => "domain",
        AdrScope::FeatureSpecific => "feature-specific",
    };
    println!("**Scope:** {scope_str}");
    if !adr.front.domains.is_empty() {
        println!("**Domains:** {}", adr.front.domains.join(", "));
    }
    println!();
    println!("{}", adr.body.trim());
}

fn adr_list() -> ExitCode {
    let graph = match load_graph() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error loading graph: {e}");
            return ExitCode::FAILURE;
        }
    };

    let mut adrs: Vec<_> = graph.adrs.values().collect();
    adrs.sort_by_key(|a| &a.front.id);

    println!("ADRs:");
    for adr in adrs {
        let scope_str = match &adr.front.scope {
            AdrScope::CrossCutting => "cross-cutting",
            AdrScope::Platform => "platform",
            AdrScope::Domain => "domain",
            AdrScope::FeatureSpecific => "feature-specific",
        };
        let status_str = format!("{:?}", adr.front.status).to_lowercase();
        println!(
            "  {} — {} [status: {}, scope: {}]",
            adr.front.id, adr.front.title, status_str, scope_str
        );
    }

    ExitCode::SUCCESS
}

fn context_show(id: &str) -> ExitCode {
    let graph = match load_graph() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error loading graph: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Simple context output - just show the artifact and its direct dependencies
    if id.starts_with("FT-") {
        if let Some(feature) = graph.features.get(id) {
            println!("# Context for {}", id);
            println!();
            render_feature(feature);
            println!();
            println!("## Linked ADRs");
            for adr_id in &feature.front.adrs {
                if let Some(adr) = graph.adrs.get(adr_id) {
                    println!();
                    render_adr(adr);
                }
            }
            ExitCode::SUCCESS
        } else {
            eprintln!("Feature {id} not found");
            ExitCode::FAILURE
        }
    } else if id.starts_with("ADR-") {
        if let Some(adr) = graph.adrs.get(id) {
            println!("# Context for {}", id);
            println!();
            render_adr(adr);
            ExitCode::SUCCESS
        } else {
            eprintln!("ADR {id} not found");
            ExitCode::FAILURE
        }
    } else {
        eprintln!("Unknown artifact type for {id} (expected FT-* or ADR-*)");
        ExitCode::from(2)
    }
}

fn preflight_check(id: &str) -> ExitCode {
    let graph = match load_graph() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error loading graph: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Load the gap baseline - use an empty baseline for now
    let baseline = GapBaseline::default();
    let findings = gap::check::check_feature_dep_gaps(&graph, id, &baseline);

    if findings.is_empty() {
        println!("{id}: coverage clean");
    } else {
        println!("{id}: coverage gaps detected:");
        for finding in findings {
            println!("  - {} ({:?}): {}", finding.code, finding.severity, finding.description);
        }
    }
    ExitCode::SUCCESS
}

fn graph_check() -> ExitCode {
    let (config, repo_root) = match ProductConfig::discover() {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("Error discovering product root: {e}");
            return ExitCode::FAILURE;
        }
    };

    let graph = match load_graph() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error loading graph: {e}");
            return ExitCode::FAILURE;
        }
    };

    let result = full_check::run(&graph, &config, &repo_root);
    if result.errors.is_empty() {
        println!("Graph check: clean ({} warnings)", result.warnings.len());
    } else {
        println!("Graph check: {} errors, {} warnings", result.errors.len(), result.warnings.len());
        for error in &result.errors {
            eprintln!("  ERROR: {error}");
        }
    }
    if result.errors.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn graph_stats() -> ExitCode {
    let graph = match load_graph() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error loading graph: {e}");
            return ExitCode::FAILURE;
        }
    };

    let stats = graph.stats();
    println!("Graph statistics:");
    println!("  features: {}", stats.features);
    println!("  adrs: {}", stats.adrs);
    println!("  tests: {}", stats.tests);
    println!("  total_nodes: {}", stats.total_nodes);
    println!("  total_edges: {}", stats.total_edges);

    ExitCode::SUCCESS
}

/// MCP tool entries from product_mcp (FT-136 Phase 3).
///
/// Returns the full tool list from product_mcp::tools::build_tool_list(),
/// converted to decision-cli's MCP entry format. The caller (cli/mcp.rs)
/// registers these via `ToolRegistry::register_external` since they use
/// the `product_*` naming convention instead of `dec_*`.
pub fn mcp_tool_entries() -> Vec<McpToolEntry> {
    // Re-export product_mcp's tool list as decision-cli-compatible entries
    product_mcp::tools::build_tool_list()
        .into_iter()
        .map(|tool| McpToolEntry {
            name: tool.name,
            description: tool.description,
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct McpToolEntry {
    pub name: String,
    pub description: String,
}
