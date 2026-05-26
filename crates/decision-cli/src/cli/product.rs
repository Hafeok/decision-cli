//! `dec product *` — clap adapter that re-exports the absorbed product-cli surface.
//!
//! FT-105 §Phase 3: every product-cli verb gets a `dec product *` form
//! by routing through the same `product_cli::dispatch` function the
//! standalone binary uses. The CLI adapter here is wiring only per
//! ADR-013 §Rule 3 — clap argument capture + delegation.

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
