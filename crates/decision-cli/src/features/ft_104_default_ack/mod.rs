//! FT-104 — default-acknowledge cross-cutting ADRs via `product.toml`, with
//! explicit per-feature opt-out.
//!
//! The slice extends three small touches on the product-cli preflight
//! surface (this is the algorithm reference implementation that the
//! cross-repo product-cli feature mirrors — see the FT-104 spec):
//!
//! 1. `[features] default-acknowledged-cross-cutting` — a list of ADR
//!    IDs in `product.toml`/`config.toml` that are inherited by every
//!    feature as a *virtual* acknowledgement. Preflight stops flagging
//!    them as gaps per-feature without each feature listing them in
//!    `adrs:`.
//!
//! 2. `adrs-rejected:` frontmatter field on features — explicit
//!    per-feature opt-out for a default-acknowledged ADR. Carrying a
//!    `reason` string is mandatory; preflight re-flags any ADR listed
//!    here as a gap with `severity = intentional`.
//!
//! 3. `product graph check` drift validators — three warnings ride on
//!    the existing CheckResult shape: stale entry in
//!    `default-acknowledged-cross-cutting` (W035), entry whose scope
//!    has changed away from `cross-cutting` (W036), and a feature's
//!    `adrs-rejected:` entry that references an ADR not in the
//!    default-acknowledge list (W037).
//!
//! No subcommand, no CLI surface in decision-cli — the algorithm and
//! its types are exposed for use by `dec preflight` (when the projection
//! is extended to carry the new triples) and for the cargo-test runner
//! that validates TC-173/174/175 in this workspace. The product-cli
//! implementation mirrors this shape.
//!
//! See `.product/features/FT-104-*.md` for the full functional spec.

pub mod algorithm;
pub mod config;
pub mod drift;
pub mod frontmatter;

pub use algorithm::{evaluate_cross_cutting, CrossCuttingRow, CoverageStatus};
pub use config::{load_default_acknowledge, DefaultAcknowledgeConfig};
pub use drift::{check_drift, AdrSnapshot, DriftWarning, FeatureRejectionRecord};
pub use frontmatter::{parse_adrs_rejected, AdrsRejectedError, RejectedAdr};
