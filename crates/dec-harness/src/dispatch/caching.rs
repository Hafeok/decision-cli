//! Anthropic prompt-cache breakpoint placement per FT-065 / ADR-037.
//!
//! Pure helpers consumed by the dispatcher escalation loop. The split is
//! deterministic from `(bundle, prior_attempt)` — no graph reads, no
//! capability inspection beyond what is passed in.
//!
//! Boundaries (per FT-065 §Behaviour and §Invariants):
//!
//! - [`split_bundle_for_caching`] returns exactly two
//!   [`CacheableBlock`]s: the stable prefix (cacheable) and the
//!   per-attempt suffix (not cacheable). Future generalisation to four
//!   breakpoints is out of scope per PRD §9.4.
//! - [`should_cache`] returns `true` iff the resolved capability has
//!   `endpoint = anthropic` and a non-null `cost_cache_hit_per_m`. The
//!   dispatcher refuses to silently enable caching on capabilities
//!   that lack the cost fields.

use crate::dispatch::capability_resolver::ResolvedCapability;
use dec_graph::bundle::Bundle;
use dec_graph::ontology::capability::Endpoint;

use super::escalation::bundle_enrich::render_prior_attempt_block;
use super::escalation::types::DispatchAttempt;

/// A single content block tagged with whether it is cacheable.
///
/// The dispatcher emits exactly two blocks per Anthropic dispatch: a
/// cacheable stable prefix and a non-cacheable per-attempt suffix
/// (which may be empty on the first attempt in a chain).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheableBlock {
    /// Block content as conveyed to the model (markdown for slice-2;
    /// translated to provider-specific message blocks downstream).
    pub content: String,
    /// True iff the dispatcher requests a `cache_control` marker on
    /// the last segment of this block. False on the per-attempt
    /// suffix.
    pub cacheable: bool,
}

/// Compute the cache-breakpoint split for `bundle` plus an optional
/// `prior_attempt` (None for the first attempt in a chain).
///
/// Always returns exactly two blocks:
///
/// 1. `cacheable = true` — the bundle's stable prefix (focal artifact,
///    linked ADRs, tool definitions, system framing). Byte-for-byte
///    identical across attempts in the same escalation chain — this is
///    what makes the Anthropic cache hit on the second attempt.
/// 2. `cacheable = false` — the per-attempt suffix (prior-attempt
///    enrichment block from ADR-034, current step's framing). May be
///    an empty string on the first attempt; the marker is still false.
///
/// The function is pure: bundle in, two blocks out. It does **not**
/// read the graph or the prior session record beyond what is already
/// in the `DispatchAttempt` passed to it.
#[must_use]
pub fn split_bundle_for_caching(
    bundle: &Bundle,
    prior_attempt: Option<&DispatchAttempt>,
) -> Vec<CacheableBlock> {
    let prefix = render_stable_prefix(bundle);
    let suffix = render_per_attempt_suffix(prior_attempt);
    vec![
        CacheableBlock {
            content: prefix,
            cacheable: true,
        },
        CacheableBlock {
            content: suffix,
            cacheable: false,
        },
    ]
}

/// True iff the dispatcher should compute cache blocks for `capability`.
///
/// Per FT-065 §Invariants the breakpoint is set iff:
///
/// - `capability.endpoint == Anthropic`, AND
/// - `capability.cost_cache_hit_per_m.is_some()`.
///
/// Scaleway capabilities and any Anthropic capability missing the cost
/// fields are excluded — no silent enablement, no silent disablement.
#[must_use]
pub fn should_cache(capability: &ResolvedCapability) -> bool {
    capability.endpoint == Endpoint::Anthropic && capability.cost_cache_hit_per_m.is_some()
}

fn render_stable_prefix(bundle: &Bundle) -> String {
    // The dispatch metadata + focal IRI line forms the stable prefix.
    // The exact bytes here MUST be deterministic from `bundle` so
    // identity holds across attempts in the same escalation chain.
    let mut prefix = bundle.dispatch_metadata_markdown();
    prefix.push_str(&format!(
        "\nFocal artifact: <{focal}>\nBundle hash: {hash}\n",
        focal = bundle.focal.as_str(),
        hash = bundle.hash,
    ));
    prefix
}

fn render_per_attempt_suffix(prior_attempt: Option<&DispatchAttempt>) -> String {
    match prior_attempt {
        Some(prior) => render_prior_attempt_block(prior, 1),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::escalation::types::WorkerResult;
    use dec_graph::bundle::{Bundle, Stakes};
    use dec_graph::ontology::capability::Endpoint;
    use dec_graph::ontology::verdict::Verdict;
    use oxigraph::model::NamedNode;

    fn cap(endpoint: Endpoint, cache_hit: Option<&str>) -> ResolvedCapability {
        ResolvedCapability {
            capability_id: "any".to_string(),
            capability_version: 1,
            endpoint,
            model_identifier: "model".to_string(),
            max_output: 1024,
            supports_tool_calling: true,
            configurable_effort: false,
            binding_version: 1,
            cost_cache_hit_per_m: cache_hit.map(str::to_string),
        }
    }

    fn bundle(hash: &str, stakes: Stakes) -> Bundle {
        Bundle {
            hash: hash.to_string(),
            focal: NamedNode::new_unchecked("https://example.com/focal"),
            stakes,
        }
    }

    fn prior(conf: f32) -> DispatchAttempt {
        DispatchAttempt {
            session_id: NamedNode::new_unchecked("https://example.com/s1"),
            capability: cap(Endpoint::Scaleway, None),
            result: WorkerResult::Verdict {
                kind: Verdict::AmendmentRequired,
                confidence: Some(conf),
            },
            feedback: vec![],
            audit_outcome: None,
        }
    }

    #[test]
    fn always_returns_exactly_two_blocks() {
        let b = bundle("h", Stakes::Foundational);
        let blocks = split_bundle_for_caching(&b, None);
        assert_eq!(blocks.len(), 2);
        let blocks_with_prior = split_bundle_for_caching(&b, Some(&prior(0.4)));
        assert_eq!(blocks_with_prior.len(), 2);
    }

    #[test]
    fn first_block_is_cacheable_second_is_not() {
        let b = bundle("h", Stakes::Routine);
        let blocks = split_bundle_for_caching(&b, None);
        assert!(blocks[0].cacheable);
        assert!(!blocks[1].cacheable);
    }

    #[test]
    fn empty_suffix_on_first_attempt() {
        let b = bundle("h", Stakes::Routine);
        let blocks = split_bundle_for_caching(&b, None);
        assert!(blocks[1].content.is_empty());
    }

    #[test]
    fn suffix_carries_prior_attempt_block() {
        let b = bundle("h", Stakes::Routine);
        let blocks = split_bundle_for_caching(&b, Some(&prior(0.6)));
        assert!(blocks[1].content.contains("## Prior attempt"));
        assert!(blocks[1].content.contains("agree, refute, or refine"));
    }

    #[test]
    fn prefix_is_byte_stable_across_attempts() {
        let b = bundle("samehash", Stakes::Foundational);
        let a = split_bundle_for_caching(&b, None);
        let p = prior(0.4);
        let c = split_bundle_for_caching(&b, Some(&p));
        assert_eq!(a[0].content, c[0].content);
    }

    #[test]
    fn should_cache_true_only_for_anthropic_with_cache_rate() {
        assert!(should_cache(&cap(Endpoint::Anthropic, Some("0.50"))));
        assert!(!should_cache(&cap(Endpoint::Anthropic, None)));
        assert!(!should_cache(&cap(Endpoint::Scaleway, Some("0.50"))));
        assert!(!should_cache(&cap(Endpoint::Scaleway, None)));
    }
}
