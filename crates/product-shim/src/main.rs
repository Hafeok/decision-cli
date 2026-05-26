//! Deprecation shim binary producing the standalone `product` command.
//!
//! Per FT-105 §Phase 5 / ADR-067, the standalone `product` binary
//! continues to ship for the deprecation window but is no longer the
//! primary entry point — operators should prefer `dec product <verb>`.
//! The shim's behaviour: emit a one-line warning to stderr, then
//! delegate to the absorbed product-cli crate's `dispatch` so the verb
//! produces the same stdout as `dec product <verb>` (TC-176).

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!("warning: 'product' is deprecated; prefer 'dec product <verb>'");
    let matches = product_cli::build_command().get_matches();
    product_cli::dispatch(&matches)
}
