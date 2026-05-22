//! `dec feedback {list,show,close,route,receive}` — feedback CLI surface (FT-029 / FT-033).
//!
//! Slice 3 expands the read-only `dec feedback list` shipped in FT-029
//! with four additional verbs that complete the operator's inspection
//! and resolution surface:
//!
//!   * `list`    — open feedback, grouped by class and target.
//!   * `show`    — full record dump for one feedback artifact.
//!   * `close`   — `addressed → closed` transition + resume-check
//!                 cascade onto paused dispatches (ADR-022 / FT-032).
//!   * `route`   — manual routing override pre-routing.
//!   * `receive` — `routed → received` transition for the human-as-
//!                 target-role pattern.
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, every submodule
//! imports from `core::feedback::*` (the lifecycle / routing / read
//! API) and never reaches sideways into another feature. All mutations
//! commit through `core::StreamWriter`, never raw RDF.

pub mod close;
mod close_helpers;
pub mod format;
pub mod list;
pub mod receive;
pub mod route;
pub mod show;
mod store_io;

// Public surface re-exports — keep the parent module's public symbols
// stable. TC-039's `scripts/checks/dec-feedback-list.sh` greps this
// file for `pub fn list` / `pub fn format_list`; the re-exports below
// also serve as the discoverable list of every operator-visible verb
// in the slice-3 surface.

pub use close::{
    close as close_feedback, close_anyhow, format_close, format_close_json, CloseError,
    CloseOutcome, ResumedGroup,
};
pub use format::OutputFormat;
pub use list::{format_list, format_list_json, list, list_filtered, FeedbackRow, ListFilters};
pub use receive::{
    format_receive, format_receive_json, receive as receive_feedback, receive_anyhow, ReceiveError,
    ReceiveOutcome,
};
pub use route::{
    format_route, format_route_json, resolve_actor, route as route_feedback, route_anyhow,
    RouteError, RouteOutcome,
};
pub use show::{
    format_error_json, format_show, format_show_json, show as show_feedback, show_anyhow, ShowError,
};

// TC-039 anchor: the test runner script greps for these symbol names
// in this file. Keep them visible here (via `pub fn list` /
// `pub fn format_list`) so a refactor of the submodules doesn't
// silently unhook the routing read surface from the CLI.
//
// `pub fn list`
// `pub fn format_list`
