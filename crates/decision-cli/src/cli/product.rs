//! `dec product *` — clap adapter that forwards to product_core (FT-136 / ADR-077).
//!
//! The adapter routes through `product_cmd::run`, which loads the
//! KnowledgeGraph via product_core and renders the result. Wiring only
//! per ADR-013 §Rule 3 — clap argument capture + delegation.

use std::path::Path;
use std::process::ExitCode;

use clap::Args;

use decision_cli::product_cmd;

/// Capture trailing args verbatim so the absorbed product-cli clap tree
/// gets the full argv it expects (subcommand + args).
#[derive(Debug, Args)]
pub struct ProductArgs {
    /// Subcommand and arguments forwarded to product-cli verbatim.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

pub fn run(_workdir: &Path, args: ProductArgs) -> ExitCode {
    product_cmd::run(args.args)
}
