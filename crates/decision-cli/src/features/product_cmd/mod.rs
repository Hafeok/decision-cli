//! `dec product *` — re-export of the absorbed product-cli CLI surface (FT-105 §Phase 3).
//!
//! Per ADR-067, product-cli is absorbed into the workspace as a sibling
//! crate. Every product-cli verb gets a `dec product *` form by routing
//! through the **same** `product_cli::dispatch` function the standalone
//! `product` binary uses. The two surfaces stay byte-identical because
//! they share the handler implementation — TC-176 asserts this property
//! for a representative parity set.
//!
//! This module is intentionally minimal: it constructs a clap matches
//! tree by feeding caller args into [`product_cli::build_command`] and
//! hands the result to [`product_cli::dispatch`]. No re-implementation,
//! no per-verb adapter — re-export, not copy.

use std::ffi::OsString;
use std::process::ExitCode;

/// Re-export so callers wiring the clap tree can compose the absorbed
/// product-cli surface without naming the dependency directly.
pub use product_cli::build_command as build_product_command;

/// MCP tool entries surfaced by the absorbed product-cli crate. The
/// combined `dec mcp` registry folds these in alongside the `dec_*`
/// tools (TC-177 invariant).
#[must_use]
pub fn mcp_tool_entries() -> Vec<product_cli::McpToolEntry> {
    product_cli::tool_descriptors()
}

/// Run a `dec product …` invocation.
///
/// `args` is the **subcommand and arguments after `dec product`**, e.g.
/// `["feature", "show", "FT-001"]`. The function builds a fresh clap
/// `Command` rooted at `product` (so clap's error messages match what
/// a `product` invocation would emit) and delegates to the absorbed
/// dispatch.
pub fn run<I, S>(args: I) -> ExitCode
where
    I: IntoIterator<Item = S>,
    S: Into<OsString> + Clone,
{
    let cmd = product_cli::build_command();
    let mut argv: Vec<OsString> = vec!["product".into()];
    argv.extend(args.into_iter().map(Into::into));
    let matches = match cmd.try_get_matches_from(argv) {
        Ok(m) => m,
        Err(err) => {
            // clap renders the error itself; exit codes follow clap's
            // convention (2 on usage errors), matching what the
            // standalone `product` binary produces.
            err.print().ok();
            return match err.kind() {
                clap::error::ErrorKind::DisplayHelp
                | clap::error::ErrorKind::DisplayVersion => ExitCode::SUCCESS,
                _ => ExitCode::from(2),
            };
        }
    };
    product_cli::dispatch(&matches)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_tool_entries_match_product_cli() {
        let entries = mcp_tool_entries();
        assert!(!entries.is_empty(), "expected non-empty MCP tool set");
        for entry in entries {
            assert!(entry.name.starts_with("product_"));
        }
    }
}
