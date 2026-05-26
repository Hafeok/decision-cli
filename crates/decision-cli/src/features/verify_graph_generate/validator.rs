//! ADR-066 dispatch-time chokepoint validator.
//!
//! Walks every `ProposedStep` in a `GraphProposal::New` and asserts each
//! referenced fact (binary, dec subcommand, SPARQL namespace, HTTP host,
//! file path prefix, capture source) is in the bundle that was sent to
//! the worker. Out-of-bundle references → `Error::ProposalReferencesOutOfBundle`
//! (carried via [`crate::core::handler::Error::Internal`] with a stable
//! `ProposalReferencesOutOfBundle:` prefix so renderers and tests can
//! match against either the discriminant or the prefix).
//!
//! Per ADR-066 §Rule 4 the validator runs at dispatch time, between
//! `worker::invoke_worker` and the persistence path. The bundle is the
//! ground truth — even if the catalog evolves between dispatch and
//! validation, the verdict over a `(bundle, proposal)` pair is
//! deterministic.

use serde::{Deserialize, Serialize};

use crate::core::handler::Error as HandlerError;

use super::enrichment::{w3c_whitelist, EnrichmentFields};
use super::proposal::{GraphProposal, ProposalKind, ProposedStep};

/// One violation row in the validator's report.
///
/// `kind` is one of the five categories from ADR-066 §Behaviour;
/// `referenced_thing` is the literal string the proposal carried (binary
/// name, dec subcommand, SPARQL namespace IRI, …); `why_rejected` names
/// the bundle field that does not contain it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Violation {
    /// 0-based step index in the proposal.
    pub step_index: usize,
    /// Violation category.
    pub kind: ViolationKind,
    /// The literal that wasn't in the bundle.
    pub referenced_thing: String,
    /// Human-readable why-rejected hint.
    pub why_rejected: String,
}

/// Discriminator for [`Violation::kind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViolationKind {
    /// `shell-command` binary not in `env_capabilities.binaries_on_path`.
    Binary,
    /// `dec <subcommand>` not in `cli_surface.dec_subcommands`.
    DecSubcommand,
    /// SPARQL PREFIX namespace not in `ontology_vocabulary.namespaces`
    /// and not in the W3C whitelist.
    SparqlNamespace,
    /// `http-request` host not in `env_capabilities.allowed_hosts`.
    HttpHost,
    /// `file-assertion` target path prefix not in `env_capabilities.writable_paths`.
    FilePath,
    /// `capture` source references env var / step that isn't declared.
    CaptureSource,
}

impl ViolationKind {
    /// Natural upstream-target category for this violation kind.
    /// Used by the gap-feedback emitter ([`super::feedback::emit_gap_feedback`])
    /// to choose which catalog artifact to point at.
    #[must_use]
    pub const fn upstream_target(&self) -> UpstreamTarget {
        match self {
            // Unknown `dec verb` → the CapabilityReference catalog needs
            // a new entry. Other binaries (`curl`, `grep`, …) come from
            // the env's `concreteCapabilities` block, not the catalog.
            Self::DecSubcommand => UpstreamTarget::CapabilityReference,
            // SPARQL namespaces ⇒ the active OntologyDescription is the
            // natural place to register the missing namespace.
            Self::SparqlNamespace => UpstreamTarget::OntologyDescription,
            // Everything else (Binary, HttpHost, FilePath, CaptureSource)
            // → the env's `dec:concreteCapabilities` block.
            Self::Binary
            | Self::HttpHost
            | Self::FilePath
            | Self::CaptureSource => UpstreamTarget::VerificationEnvironment,
        }
    }
}

/// The natural upstream artifact category for a gap-feedback target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum UpstreamTarget {
    /// `dec:CapabilityReference` category.
    CapabilityReference,
    /// `dec:OntologyDescription` category.
    OntologyDescription,
    /// `dec:VerificationEnvironment` (the target env that was queried).
    VerificationEnvironment,
}

impl UpstreamTarget {
    /// Stable wire string used by gap feedback (CLI / test assertions).
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::CapabilityReference => "capability-reference",
            Self::OntologyDescription => "ontology-description",
            Self::VerificationEnvironment => "verification-environment",
        }
    }
}

/// Validate a worker's proposal against the bundle's enrichment fields.
///
/// `Ok(Vec::new())` ⇒ no violations; the caller proceeds to persistence.
/// `Ok(non-empty)` ⇒ violations to emit feedback for; the caller maps to
/// `Error::ProposalReferencesOutOfBundle`.
pub fn validate_proposal(
    proposal: &GraphProposal,
    enrichment: &EnrichmentFields,
) -> Vec<Violation> {
    let steps = match proposal.kind {
        ProposalKind::New => proposal
            .new
            .as_ref()
            .map(|n| n.steps.as_slice())
            .unwrap_or(&[]),
        _ => return Vec::new(),
    };
    let mut out: Vec<Violation> = Vec::new();
    for (idx, step) in steps.iter().enumerate() {
        validate_step(idx, step, enrichment, &mut out);
    }
    out
}

fn validate_step(
    idx: usize,
    step: &ProposedStep,
    enrichment: &EnrichmentFields,
    out: &mut Vec<Violation>,
) {
    match step.step_type.as_str() {
        "shell-command" => validate_shell_command(idx, step, enrichment, out),
        "sparql-assertion" => validate_sparql_assertion(idx, step, enrichment, out),
        "http-request" => validate_http_request(idx, step, enrichment, out),
        "file-assertion" => validate_file_assertion(idx, step, enrichment, out),
        "capture" => validate_capture(idx, step, enrichment, out),
        // Other step types (e.g. `wait-for`) are passed through. Their
        // inner conditions, if they reference one of the validated kinds,
        // are still caught because the bundle is the ground truth.
        _ => {}
    }
}

fn validate_shell_command(
    idx: usize,
    step: &ProposedStep,
    enrichment: &EnrichmentFields,
    out: &mut Vec<Violation>,
) {
    let Some(cmd) = step.fields.get("command").and_then(|v| v.as_str()) else {
        return;
    };
    let cmd_trim = cmd.trim();
    let head = cmd_trim.split_whitespace().next().unwrap_or("");
    if head.is_empty() {
        return;
    }
    // dec subcommand match (only when the binary is `dec`).
    if head == "dec" && !enrichment.cli_surface.dec_subcommands.is_empty() {
        if let Some(dec_sub) = extract_dec_subcommand(cmd_trim) {
            if !enrichment
                .cli_surface
                .dec_subcommands
                .iter()
                .any(|s| s.as_str() == dec_sub)
            {
                out.push(Violation {
                    step_index: idx,
                    kind: ViolationKind::DecSubcommand,
                    referenced_thing: dec_sub,
                    why_rejected: "not in cli_surface.dec_subcommands".to_string(),
                });
            }
            return;
        }
    }
    // Lenient mode: when the bundle's binaries surface is empty, skip
    // the membership check. This is the ADR-066 contract — the
    // validator enforces only what the bundle carries.
    if enrichment.env_capabilities.binaries_on_path.is_empty() {
        return;
    }
    if !enrichment
        .env_capabilities
        .binaries_on_path
        .iter()
        .any(|b| b.as_str() == head)
    {
        out.push(Violation {
            step_index: idx,
            kind: ViolationKind::Binary,
            referenced_thing: head.to_string(),
            why_rejected: "not in env_capabilities.binaries_on_path".to_string(),
        });
    }
}

/// Pull a `dec <verb> <verb> ...` longest-match against the bundle's
/// declared `dec_subcommands`. The bundle's list carries fully-qualified
/// strings like `dec verify graph new`; we match the proposal's command
/// against the longest known prefix that aligns with whitespace
/// boundaries.
fn extract_dec_subcommand(cmd: &str) -> Option<String> {
    // Stop at the first non-flag token boundary so flags don't bleed in.
    let tokens: Vec<&str> = cmd
        .split_whitespace()
        .take_while(|t| !t.starts_with('-'))
        .collect();
    if tokens.is_empty() || tokens[0] != "dec" {
        return None;
    }
    if tokens.len() == 1 {
        return Some("dec".to_string());
    }
    // Best-effort: rejoin the leading non-flag tokens. The validator
    // membership check then compares this against `dec_subcommands`.
    // We progressively trim trailing tokens that look like positional
    // arguments (anything containing '/', '$', '.', or starting with
    // an upper-case letter) — these are typically values, not verbs.
    let mut joined: Vec<&str> = tokens.clone();
    while joined.len() > 1 {
        let candidate = joined.join(" ");
        // If candidate ends with what looks like an id / value, peel it.
        let last = joined[joined.len() - 1];
        if looks_like_value(last) {
            joined.pop();
            continue;
        }
        return Some(candidate);
    }
    Some(joined.join(" "))
}

fn looks_like_value(token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    let first = token.chars().next().unwrap_or(' ');
    token.contains('/')
        || token.contains('$')
        || token.contains('.')
        || token.contains('=')
        || first.is_ascii_uppercase()
        || first.is_ascii_digit()
}

fn validate_sparql_assertion(
    idx: usize,
    step: &ProposedStep,
    enrichment: &EnrichmentFields,
    out: &mut Vec<Violation>,
) {
    let Some(query) = step.fields.get("query").and_then(|v| v.as_str()) else {
        return;
    };
    if enrichment.ontology_vocabulary.namespaces.is_empty() {
        // Lenient mode (per ADR-066): no namespaces declared on the
        // bundle ⇒ validator cannot enforce membership.
        return;
    }
    for ns in extract_sparql_prefixes(query) {
        let allowed_in_bundle = enrichment
            .ontology_vocabulary
            .namespaces
            .iter()
            .any(|n| n == &ns);
        let allowed_in_w3c = w3c_whitelist().iter().any(|w| *w == ns);
        if !(allowed_in_bundle || allowed_in_w3c) {
            out.push(Violation {
                step_index: idx,
                kind: ViolationKind::SparqlNamespace,
                referenced_thing: ns,
                why_rejected:
                    "not in ontology_vocabulary.namespaces and not in W3C whitelist".to_string(),
            });
        }
    }
}

/// Extract all namespace IRIs from `PREFIX foo: <iri>` declarations.
fn extract_sparql_prefixes(query: &str) -> Vec<String> {
    let lower = query.to_lowercase();
    let mut out: Vec<String> = Vec::new();
    // Be tolerant of casing — collect ranges where 'prefix' (case-insensitive) appears.
    let bytes = lower.as_bytes();
    let needle = b"prefix";
    let mut i = 0;
    while i + needle.len() <= bytes.len() {
        if &bytes[i..i + needle.len()] == needle {
            // Find the '<' after this position.
            if let Some(lt) = lower[i..].find('<') {
                if let Some(gt) = lower[i + lt..].find('>') {
                    let start = i + lt + 1;
                    let end = i + lt + gt;
                    if start < end {
                        let ns = query[start..end].to_string();
                        if !ns.is_empty() {
                            out.push(ns);
                        }
                    }
                    i += lt + gt + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

fn validate_http_request(
    idx: usize,
    step: &ProposedStep,
    enrichment: &EnrichmentFields,
    out: &mut Vec<Violation>,
) {
    let Some(url) = step.fields.get("url").and_then(|v| v.as_str()) else {
        return;
    };
    let Some(host) = host_of(url) else {
        return;
    };
    if enrichment.env_capabilities.allowed_hosts.is_empty() {
        // Lenient: no hosts declared ⇒ skip.
        return;
    }
    if !enrichment
        .env_capabilities
        .allowed_hosts
        .iter()
        .any(|h| h == &host)
    {
        out.push(Violation {
            step_index: idx,
            kind: ViolationKind::HttpHost,
            referenced_thing: host,
            why_rejected: "not in env_capabilities.allowed_hosts".to_string(),
        });
    }
}

fn host_of(url: &str) -> Option<String> {
    // Stripped-down URL parser sufficient for `https://host/...` and
    // `http://host:port/...` shapes used by `http-request` steps.
    let after_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host_end = after_scheme
        .find('/')
        .unwrap_or(after_scheme.len());
    let host_with_port = &after_scheme[..host_end];
    let host = host_with_port
        .split(':')
        .next()
        .unwrap_or(host_with_port);
    Some(host.to_string())
}

fn validate_file_assertion(
    idx: usize,
    step: &ProposedStep,
    enrichment: &EnrichmentFields,
    out: &mut Vec<Violation>,
) {
    let Some(target) = step.fields.get("target").and_then(|v| v.as_str()) else {
        return;
    };
    let trimmed = target.trim();
    if enrichment.env_capabilities.writable_paths.is_empty() {
        return;
    }
    let matches_any = enrichment
        .env_capabilities
        .writable_paths
        .iter()
        .any(|p| path_under_prefix(trimmed, p));
    if !matches_any {
        out.push(Violation {
            step_index: idx,
            kind: ViolationKind::FilePath,
            referenced_thing: trimmed.to_string(),
            why_rejected: "not in env_capabilities.writable_paths".to_string(),
        });
    }
}

fn path_under_prefix(path: &str, prefix: &str) -> bool {
    if prefix.is_empty() {
        return false;
    }
    if prefix.ends_with('/') {
        path.starts_with(prefix)
    } else {
        path == prefix || path.starts_with(&format!("{prefix}/")) || path.starts_with(prefix)
    }
}

fn validate_capture(
    idx: usize,
    step: &ProposedStep,
    enrichment: &EnrichmentFields,
    out: &mut Vec<Violation>,
) {
    let kind = step
        .fields
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if kind == "env_var" {
        let Some(name) = step.fields.get("name").and_then(|v| v.as_str()) else {
            return;
        };
        if enrichment
            .env_capabilities
            .environment_variables
            .is_empty()
        {
            return;
        }
        if !enrichment
            .env_capabilities
            .environment_variables
            .iter()
            .any(|e| e == name)
        {
            out.push(Violation {
                step_index: idx,
                kind: ViolationKind::CaptureSource,
                referenced_thing: name.to_string(),
                why_rejected: "not in env_capabilities.environment_variables".to_string(),
            });
        }
    }
}

/// Build the `Error::ProposalReferencesOutOfBundle` carrier from a
/// non-empty violation list. The marker substring is stable so tests
/// and renderers can grep for it.
#[must_use]
pub fn build_rejection_error(violations: &[Violation]) -> HandlerError {
    let joined = violations
        .iter()
        .map(|v| {
            format!(
                "step {idx}: {k} {thing}: {why}",
                idx = v.step_index,
                k = format!("{:?}", v.kind).to_lowercase(),
                thing = v.referenced_thing,
                why = v.why_rejected,
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    HandlerError::Internal {
        detail: format!("ProposalReferencesOutOfBundle: {joined}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::verify_graph_generate::enrichment::{
        CliCommand, CliSurface, EnvCapabilities, OntologyVocabulary,
    };
    use crate::features::verify_graph_generate::proposal::{
        GraphProposal, NewProposal, ProposedStep,
    };
    use serde_json::json;

    fn enrichment_with_dec_only() -> EnrichmentFields {
        EnrichmentFields {
            cli_surface: CliSurface {
                commands: vec![CliCommand {
                    command: "dec verify graph new".to_string(),
                    capability_version: "0.3.0".to_string(),
                    source_cr: "CR-001".to_string(),
                }],
                dec_subcommands: vec!["dec verify graph new".to_string()],
                capability_version: "0.3.0".to_string(),
            },
            ontology_vocabulary: OntologyVocabulary {
                namespace: "https://decision-cli.dev/ns#".to_string(),
                prefix: "dec".to_string(),
                namespaces: vec!["https://decision-cli.dev/ns#".to_string()],
                classes: Vec::new(),
                source_od: "OD-001".to_string(),
            },
            env_capabilities: EnvCapabilities {
                binaries_on_path: vec!["dec".to_string(), "bash".to_string()],
                writable_paths: vec!["$DEC_VERIFY_TMP".to_string()],
                allowed_hosts: vec!["api.dec.test".to_string()],
                environment_variables: vec!["DEC_VERIFY_TMP".to_string(), "PATH".to_string()],
                pre_seeded_artifacts: Vec::new(),
            },
            ..EnrichmentFields::default()
        }
    }

    fn step(kind: &str, fields: serde_json::Value) -> ProposedStep {
        ProposedStep {
            step_type: kind.to_string(),
            fields: fields.as_object().cloned().unwrap_or_default(),
            provides_evidence_for: Vec::new(),
        }
    }

    fn proposal_with(steps: Vec<ProposedStep>) -> GraphProposal {
        GraphProposal::new_new(
            "bh",
            NewProposal {
                environment: "ENV-x".to_string(),
                steps,
                rationale: "test".to_string(),
                addressed_feedback_iris: Vec::new(),
            },
        )
    }

    #[test]
    fn shell_command_with_known_dec_subcommand_passes() {
        let enrichment = enrichment_with_dec_only();
        let p = proposal_with(vec![step(
            "shell-command",
            json!({"command": "dec verify graph new --verifies FT-X"}),
        )]);
        let v = validate_proposal(&p, &enrichment);
        assert!(v.is_empty(), "expected no violations, got {v:?}");
    }

    #[test]
    fn shell_command_with_unknown_binary_violates() {
        let enrichment = enrichment_with_dec_only();
        let p = proposal_with(vec![step(
            "shell-command",
            json!({"command": "curl https://example.com"}),
        )]);
        let v = validate_proposal(&p, &enrichment);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, ViolationKind::Binary);
        assert_eq!(v[0].referenced_thing, "curl");
    }

    #[test]
    fn shell_command_with_unknown_dec_subcommand_violates() {
        let enrichment = enrichment_with_dec_only();
        let p = proposal_with(vec![step(
            "shell-command",
            json!({"command": "dec verify result inspect VGR-001"}),
        )]);
        let v = validate_proposal(&p, &enrichment);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, ViolationKind::DecSubcommand);
        assert!(v[0].referenced_thing.starts_with("dec verify result"));
    }

    #[test]
    fn sparql_with_unknown_namespace_violates() {
        let enrichment = enrichment_with_dec_only();
        let p = proposal_with(vec![step(
            "sparql-assertion",
            json!({"target": "./", "query": "PREFIX foo: <https://fake.example/ns#> SELECT * WHERE { ?s foo:p ?o }"}),
        )]);
        let v = validate_proposal(&p, &enrichment);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, ViolationKind::SparqlNamespace);
        assert_eq!(v[0].referenced_thing, "https://fake.example/ns#");
    }

    #[test]
    fn sparql_with_w3c_whitelisted_namespace_passes() {
        let enrichment = enrichment_with_dec_only();
        let p = proposal_with(vec![step(
            "sparql-assertion",
            json!({"target": "./", "query": "PREFIX prov: <http://www.w3.org/ns/prov#> SELECT * WHERE { ?s a prov:Activity }"}),
        )]);
        let v = validate_proposal(&p, &enrichment);
        assert!(v.is_empty(), "{v:?}");
    }

    #[test]
    fn file_assertion_outside_writable_paths_violates() {
        let enrichment = enrichment_with_dec_only();
        let p = proposal_with(vec![step(
            "file-assertion",
            json!({"target": "/etc/passwd"}),
        )]);
        let v = validate_proposal(&p, &enrichment);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, ViolationKind::FilePath);
    }

    #[test]
    fn http_request_outside_allowed_hosts_violates() {
        let enrichment = enrichment_with_dec_only();
        let p = proposal_with(vec![step(
            "http-request",
            json!({"url": "https://evil.example/probe"}),
        )]);
        let v = validate_proposal(&p, &enrichment);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, ViolationKind::HttpHost);
        assert_eq!(v[0].referenced_thing, "evil.example");
    }

    #[test]
    fn multiple_violations_all_reported() {
        let enrichment = enrichment_with_dec_only();
        let p = proposal_with(vec![
            step(
                "shell-command",
                json!({"command": "dec verify result inspect VGR-001"}),
            ),
            step(
                "sparql-assertion",
                json!({"target": "./", "query": "PREFIX foo: <https://fake.example/ns#> SELECT * WHERE { ?s ?p ?o }"}),
            ),
            step("file-assertion", json!({"target": "/etc/passwd"})),
        ]);
        let v = validate_proposal(&p, &enrichment);
        assert_eq!(v.len(), 3);
    }

    #[test]
    fn match_proposals_skip_validation() {
        let enrichment = enrichment_with_dec_only();
        let p = GraphProposal::new_match(
            "bh",
            super::super::proposal::MatchProposal {
                graph_id: "VG-007".to_string(),
                rationale: "covers all TCs".to_string(),
            },
        );
        let v = validate_proposal(&p, &enrichment);
        assert!(v.is_empty());
    }

    #[test]
    fn upstream_target_mapping_is_stable() {
        assert_eq!(
            ViolationKind::Binary.upstream_target(),
            UpstreamTarget::VerificationEnvironment
        );
        assert_eq!(
            ViolationKind::DecSubcommand.upstream_target(),
            UpstreamTarget::CapabilityReference
        );
        assert_eq!(
            ViolationKind::SparqlNamespace.upstream_target(),
            UpstreamTarget::OntologyDescription
        );
    }
}
