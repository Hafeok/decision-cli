//! Gate-firing logic — FT-047 §Behaviour.
//!
//! Pure: takes a coverage report, an optional waiver intent, and the
//! supporting context (workdir, store handle, product-cli root,
//! attribution agent), and returns either a `Pass` (with the waiver IRI
//! when one was minted) or a structured error.
//!
//! The gate itself does *not* know about CLI flags, exit codes, or MCP
//! shapes — those map onto [`ChainIntegrityError`] / [`GateOutcome`] in
//! the dispatching verb. ADR-031 §"Gate runs on every dispatch" means
//! every feature-targeted dispatch passes through this function.

use std::path::Path;

use oxigraph::model::NamedNode;
use thiserror::Error;

use crate::core::verify::coverage::{
    feature_coverage, feature_resolver::feature_iri_for, CoverageError,
};
use crate::core::StreamWriter;

use super::waiver_intent::{validate_waiver_reason, WaiverIntent, WaiverReasonError};
use super::writer::{persist_waiver, PersistedWaiver, WaiverPersistError};

/// What the gate produces on a clean pass (with or without a waiver).
#[derive(Debug, Clone)]
pub enum GateOutcome {
    /// Coverage was already complete — no waiver written.
    Clean,
    /// A waiver was minted; the dispatch proceeds with the waiver IRI
    /// recorded on the session's PROV-O chain.
    Waived {
        /// The persisted waiver — IRI, on-disk path, and uncovered snapshot.
        waiver: PersistedWaiver,
    },
}

/// Structured failure surfaces — mapped to exit codes by the dispatching
/// verb per FT-047 §Error handling:
///
///   * `ChainIntegrity` → exit 1.
///   * `InvalidWaiverReason` → exit 2.
///   * `WaiverPersistFailed` / `StoreUnreachable` → exit 1.
#[derive(Debug, Error)]
pub enum ChainIntegrityError {
    /// Uncovered TCs and no waiver intent (ADR-031 §4).
    #[error("{}", render_chain_integrity(.feature, .uncovered_tcs, .known_envs.as_slice()))]
    ChainIntegrity {
        /// IRI of the gated feature.
        feature: String,
        /// Short ids of uncovered TCs (declaration order preserved).
        uncovered_tcs: Vec<String>,
        /// Optional list of env ids to suggest for `dec verify graph generate`.
        known_envs: Vec<String>,
    },
    /// The supplied waiver reason failed the minimum-length / whitespace check.
    #[error(transparent)]
    InvalidWaiverReason(#[from] WaiverReasonError),
    /// The waiver write failed (SHACL refused, I/O issue, etc.).
    #[error(transparent)]
    WaiverPersistFailed(#[from] WaiverPersistError),
    /// Underlying coverage primitive could not run (store / feature lookup failure).
    #[error(transparent)]
    Coverage(#[from] CoverageError),
}

fn render_chain_integrity(feature: &str, uncovered: &[String], known_envs: &[String]) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Error::ChainIntegrity: feature {feature} has uncovered TCs and no waiver."
    );
    let _ = writeln!(out);
    render_uncovered_block(&mut out, uncovered);
    let _ = writeln!(out);
    render_next_actions(&mut out, feature, known_envs);
    let _ = writeln!(out);
    render_escape_hatch(&mut out, feature);
    out
}

fn render_uncovered_block(out: &mut String, uncovered: &[String]) {
    use std::fmt::Write;
    let _ = writeln!(out, "Uncovered TCs ({}):", uncovered.len());
    for tc in uncovered {
        let _ = writeln!(out, "  - {tc}");
    }
}

fn render_next_actions(out: &mut String, feature: &str, known_envs: &[String]) {
    use std::fmt::Write;
    let _ = writeln!(out, "Next actions:");
    if known_envs.is_empty() {
        let _ = writeln!(
            out,
            "  * Author a verification graph that covers these TCs:"
        );
        let _ = writeln!(
            out,
            "      dec verify graph generate {feature} --bench BNCH-NNN"
        );
        return;
    }
    for env in known_envs {
        let _ = writeln!(out, "  * dec verify graph generate {feature} --bench {env}");
    }
}

fn render_escape_hatch(out: &mut String, feature: &str) {
    use std::fmt::Write;
    let _ = writeln!(out, "Escape hatch (records a CoverageWaiver artifact):");
    let _ = writeln!(
        out,
        "      dec implement {feature} --waive-coverage \"<reason ≥ 16 non-whitespace chars>\""
    );
}

/// Inputs for [`run_chain_integrity_gate`].
pub struct GateInputs<'a> {
    /// Working directory containing `.dec/`.
    pub workdir: &'a Path,
    /// `.product/` root used by the coverage primitive's TC resolver.
    pub product_root: &'a Path,
    /// The orchestration store handle.
    pub store: &'a oxigraph::store::Store,
    /// StreamWriter used for the waiver mutation (chokepoint).
    pub writer: &'a StreamWriter,
    /// Short id of the feature being dispatched (`FT-NNN`).
    pub feature_short_id: &'a str,
    /// Optional waiver intent supplied by the caller.
    pub waiver: Option<&'a WaiverIntent>,
    /// PROV-O actor (e.g. `urn:dec:agent:cli`).
    pub attributed_to: NamedNode,
    /// Known env ids — used to suggest concrete `dec verify graph generate`
    /// commands in the error message. Pass `&[]` if none.
    pub known_envs: &'a [String],
}

/// Run the chain-integrity gate. See module docs for the high-level
/// shape; the precondition is that the caller has resolved the target
/// to a feature artifact (the gate is feature-targeted only — non-
/// feature targets short-circuit upstream).
///
/// When the resolved `product_root` does not contain a `.product/features/`
/// directory, the gate short-circuits as "not applicable" — the same
/// fallback the `prepare_bundle` path takes for `product context`. This
/// matches FT-047 §Behaviour step 1's "not applicable" carve-out for
/// non-feature targets: with no product graph the orchestrator has no
/// source of truth for the feature's TC list and nothing to gate against.
pub fn run_chain_integrity_gate(
    inputs: GateInputs<'_>,
) -> Result<GateOutcome, ChainIntegrityError> {
    if !inputs
        .product_root
        .join(".product")
        .join("features")
        .is_dir()
    {
        return Ok(GateOutcome::Clean);
    }

    // Capture the snapshot BEFORE doing anything else; ADR-031 says the
    // waiver attests to *this* coverage gap as it existed when the
    // dispatch began.
    let coverage = feature_coverage(
        inputs.feature_short_id,
        None,
        inputs.store,
        inputs.product_root,
    )?;

    if coverage.uncovered.is_empty() {
        return Ok(GateOutcome::Clean);
    }

    let uncovered_iris: Vec<NamedNode> = coverage
        .uncovered
        .iter()
        .map(|s| NamedNode::new_unchecked(s.clone()))
        .collect();

    let Some(intent) = inputs.waiver else {
        return Err(no_waiver_error(&coverage, inputs.known_envs));
    };

    // Validate the reason BEFORE touching disk. Exit 2 path.
    let _ = validate_waiver_reason(&intent.reason)?;

    let persisted = mint_waiver(&inputs, intent, uncovered_iris)?;
    Ok(GateOutcome::Waived { waiver: persisted })
}

/// Build the structured "no waiver" failure. FT-047 §Error handling — exit 1.
fn no_waiver_error(
    coverage: &crate::core::verify::coverage::CoverageReport,
    known_envs: &[String],
) -> ChainIntegrityError {
    ChainIntegrityError::ChainIntegrity {
        feature: coverage.feature.clone(),
        uncovered_tcs: coverage.uncovered.iter().map(|iri| short_tc(iri)).collect(),
        known_envs: known_envs.to_vec(),
    }
}

fn mint_waiver(
    inputs: &GateInputs<'_>,
    intent: &WaiverIntent,
    uncovered_iris: Vec<NamedNode>,
) -> Result<PersistedWaiver, ChainIntegrityError> {
    let feature_iri = NamedNode::new_unchecked(feature_iri_for(inputs.feature_short_id));
    Ok(persist_waiver(
        inputs.workdir,
        inputs.writer,
        feature_iri,
        intent,
        uncovered_iris,
        inputs.attributed_to.clone(),
    )?)
}

/// Strip the canonical TC IRI prefix back to `TC-NNN` for the error
/// message. Passes any non-matching string through unchanged.
fn short_tc(iri: &str) -> String {
    iri.strip_prefix("https://decision-cli.dev/ns/tc/")
        .unwrap_or(iri)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_includes_all_required_strings() {
        let s = render_chain_integrity(
            "FT-U",
            &["TC-T2".to_string()],
            &["BNCH-001-ephemeral-cli".to_string()],
        );
        assert!(s.contains("Error::ChainIntegrity"));
        assert!(s.contains("FT-U"));
        assert!(s.contains("TC-T2"));
        assert!(s.contains("dec verify graph generate FT-U --bench BNCH-001-ephemeral-cli"));
        assert!(s.contains("--waive-coverage"));
    }

    #[test]
    fn render_when_no_known_envs_uses_placeholder_env() {
        let s = render_chain_integrity("FT-U", &["TC-T2".to_string()], &[]);
        assert!(s.contains("dec verify graph generate FT-U --bench BNCH-NNN"));
    }
}
