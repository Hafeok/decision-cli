//! product-cli — knowledge graph CLI absorbed into the decision-cli workspace per FT-105.
//!
//! This crate is the absorption point named in [ADR-067]. The full
//! product-cli sources land here via `git subtree add --prefix=crates/product-cli`
//! at the absorption commit. Until the subtree merge ships, this lib
//! exposes the **minimal Rust API surface** that downstream crates
//! (`crates/decision-cli/`, `crates/product-shim/`) depend on:
//!
//! * [`build_command`] — the clap `Command` tree for the `product`
//!   binary. `dec product *` re-exports this same tree so the two
//!   surfaces stay byte-identical (TC-176).
//! * [`dispatch`] — execute a parsed `ArgMatches` against the
//!   in-process handlers. Hot-path reads (the kind ADR-009 used to
//!   route through subprocess) call this directly.
//! * [`tool_descriptors`] — the MCP tool name / description set the
//!   combined `dec mcp` server folds into its registry alongside the
//!   `dec_*` tools (TC-177).
//!
//! FT-105 §SDP invariant: nothing in this crate imports from
//! `decision-cli` or `oxi-events`. The TC-179 fitness check audits
//! both `Cargo.toml` and the source tree for that property.

#![deny(clippy::unwrap_used)]

use std::process::ExitCode;

use clap::{ArgMatches, Command};
use serde::{Deserialize, Serialize};

/// Crate version pinned to the workspace `Cargo.toml`. Surfaced by
/// `product --version` and by `dec product --version` so the two
/// CLIs report the same string.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Single MCP tool entry the combined `dec mcp` server reads from
/// this crate.  Field names mirror the JSON shape the MCP wire
/// protocol publishes on `tools/list`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpToolEntry {
    /// Tool name. For product-cli tools the prefix is `product_`
    /// (e.g. `product_feature_show`), satisfying TC-177's "no name
    /// collision with `dec_*` tools" invariant by construction.
    pub name: String,
    /// One-line description rendered on `tools/list`.
    pub description: String,
}

/// Build the clap [`Command`] tree exposed by the `product` binary.
///
/// `crates/product-shim/src/main.rs` (the deprecation shim) and the
/// `dec product` subcommand both construct subcommands by reading from
/// this tree, so a verb added here surfaces on both surfaces without
/// further wiring.
#[must_use]
pub fn build_command() -> Command {
    Command::new("product")
        .about("Knowledge graph CLI for managing features, ADRs, and test criteria")
        .version(VERSION)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("feature")
                .about("Feature navigation and management")
                .subcommand_required(true)
                .subcommand(
                    Command::new("show")
                        .about("Show a feature")
                        .arg(clap::Arg::new("id").required(true)),
                )
                .subcommand(Command::new("list").about("List features"))
                .subcommand(Command::new("next").about("Next implementable feature")),
        )
        .subcommand(
            Command::new("adr")
                .about("ADR navigation and management")
                .subcommand_required(true)
                .subcommand(
                    Command::new("show")
                        .about("Show an ADR")
                        .arg(clap::Arg::new("id").required(true)),
                )
                .subcommand(Command::new("list").about("List ADRs")),
        )
        .subcommand(
            Command::new("context")
                .about("Assemble context bundle for an artifact")
                .arg(clap::Arg::new("id").required(true)),
        )
        .subcommand(
            Command::new("preflight")
                .about("Preflight analysis for a feature")
                .arg(clap::Arg::new("id").required(true)),
        )
        .subcommand(
            Command::new("graph")
                .about("Graph operations")
                .subcommand_required(true)
                .subcommand(Command::new("check").about("Check graph integrity"))
                .subcommand(Command::new("stats").about("Graph statistics")),
        )
}

/// Execute a parsed [`ArgMatches`] against the in-process handlers.
///
/// The absorbed product-cli sources will replace this stub with calls
/// into the real subcommand handlers. Until that lands, the stub emits
/// a one-line deterministic identifier for each verb so callers (the
/// `dec product` adapter and the deprecation shim) can route through
/// the same code path and assert byte-identical stdout.
#[must_use]
pub fn dispatch(matches: &ArgMatches) -> ExitCode {
    let Some((verb, sub)) = matches.subcommand() else {
        eprintln!("product: no subcommand supplied");
        return ExitCode::from(2);
    };
    match verb {
        "feature" => dispatch_feature(sub),
        "adr" => dispatch_adr(sub),
        "context" => dispatch_simple_id("context", sub),
        "preflight" => dispatch_simple_id("preflight", sub),
        "graph" => dispatch_graph(sub),
        other => {
            eprintln!("product: unknown subcommand '{other}'");
            ExitCode::from(2)
        }
    }
}

fn dispatch_feature(matches: &ArgMatches) -> ExitCode {
    let Some((verb, sub)) = matches.subcommand() else {
        eprintln!("product feature: no subcommand supplied");
        return ExitCode::from(2);
    };
    match verb {
        "show" => dispatch_simple_id("feature show", sub),
        "list" => {
            println!("feature list (stub)");
            ExitCode::SUCCESS
        }
        "next" => {
            println!("feature next (stub)");
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("product feature: unknown subcommand '{other}'");
            ExitCode::from(2)
        }
    }
}

fn dispatch_adr(matches: &ArgMatches) -> ExitCode {
    let Some((verb, sub)) = matches.subcommand() else {
        eprintln!("product adr: no subcommand supplied");
        return ExitCode::from(2);
    };
    match verb {
        "show" => dispatch_simple_id("adr show", sub),
        "list" => {
            println!("adr list (stub)");
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("product adr: unknown subcommand '{other}'");
            ExitCode::from(2)
        }
    }
}

fn dispatch_graph(matches: &ArgMatches) -> ExitCode {
    let Some((verb, _sub)) = matches.subcommand() else {
        eprintln!("product graph: no subcommand supplied");
        return ExitCode::from(2);
    };
    match verb {
        "check" => {
            println!("graph check (stub)");
            ExitCode::SUCCESS
        }
        "stats" => {
            println!("graph stats (stub)");
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("product graph: unknown subcommand '{other}'");
            ExitCode::from(2)
        }
    }
}

fn dispatch_simple_id(verb: &str, sub: &ArgMatches) -> ExitCode {
    if let Some(id) = sub.get_one::<String>("id") {
        println!("{verb} {id} (stub)");
        ExitCode::SUCCESS
    } else {
        eprintln!("{verb}: id required");
        ExitCode::from(2)
    }
}

/// Catalog of MCP tools surfaced by product-cli into the combined
/// `dec mcp` server. The list is the source of truth referenced by
/// TC-177 ("combined MCP server exposes both sets without collision").
///
/// Every entry's name uses the `product_*` prefix; the combined
/// registry asserts that no `dec_*` name collides with any of these.
#[must_use]
pub fn tool_descriptors() -> Vec<McpToolEntry> {
    vec![
        McpToolEntry {
            name: "product_feature_show".into(),
            description: "Show a feature spec by id.".into(),
        },
        McpToolEntry {
            name: "product_feature_list".into(),
            description: "List features in the catalog.".into(),
        },
        McpToolEntry {
            name: "product_feature_next".into(),
            description: "Return the next implementable feature.".into(),
        },
        McpToolEntry {
            name: "product_adr_show".into(),
            description: "Show an ADR by id.".into(),
        },
        McpToolEntry {
            name: "product_adr_list".into(),
            description: "List ADRs in the catalog.".into(),
        },
        McpToolEntry {
            name: "product_context".into(),
            description: "Assemble a context bundle for an artifact.".into(),
        },
        McpToolEntry {
            name: "product_preflight".into(),
            description: "Preflight coverage report for a feature.".into(),
        },
        McpToolEntry {
            name: "product_graph_check".into(),
            description: "Check the graph integrity.".into(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_tree_lists_expected_verbs() {
        let cmd = build_command();
        let subs: Vec<&str> = cmd.get_subcommands().map(clap::Command::get_name).collect();
        assert!(subs.contains(&"feature"));
        assert!(subs.contains(&"adr"));
        assert!(subs.contains(&"context"));
        assert!(subs.contains(&"preflight"));
        assert!(subs.contains(&"graph"));
    }

    #[test]
    fn tool_descriptors_use_product_prefix() {
        for entry in tool_descriptors() {
            assert!(
                entry.name.starts_with("product_"),
                "{} did not start with product_",
                entry.name
            );
        }
    }

    #[test]
    fn tool_descriptor_names_are_unique() {
        let mut names: Vec<String> = tool_descriptors().into_iter().map(|t| t.name).collect();
        names.sort();
        let n = names.len();
        names.dedup();
        assert_eq!(n, names.len(), "duplicate tool names in product-cli");
    }
}
