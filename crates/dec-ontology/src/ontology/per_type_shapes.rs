//! Embedded per-type SHACL shape catalog (FT-072 / ADR-038).
//!
//! Each per-type artifact shape composes the universal mechanical-
//! provenance fragment (FT-069) via `sh:and` and a BoundaryArtifact
//! escape hatch (FT-071) + motivational alternatives (FT-070) via
//! `sh:or`. This module exposes the catalog as constant slices so the
//! ontology loader, the dual-validator-agreement fitness check, and the
//! shape-file packager all consume one source of truth.

/// Filename / Turtle-bytes pairs for the FT-072 per-type shape catalog.
/// The mechanical / boundary / motivational *universal* blocks are NOT
/// listed here — they are loaded explicitly in the ontology loader
/// before this catalog so the per-type sh:and / sh:or references
/// resolve correctly when the per-type files parse.
pub const PER_TYPE_SHAPE_FILES: &[(&str, &str)] = &[
    (
        "acknowledgement.ttl",
        include_str!("assets/shapes/acknowledgement.ttl"),
    ),
    ("adr.ttl", include_str!("assets/shapes/adr.ttl")),
    ("brief.ttl", include_str!("assets/shapes/brief.ttl")),
    (
        "conformance-audit.ttl",
        include_str!("assets/shapes/conformance-audit.ttl"),
    ),
    (
        "dependency.ttl",
        include_str!("assets/shapes/dependency.ttl"),
    ),
    (
        "discovery-finding.ttl",
        include_str!("assets/shapes/discovery-finding.ttl"),
    ),
    ("dispatch.ttl", include_str!("assets/shapes/dispatch.ttl")),
    ("feature.ttl", include_str!("assets/shapes/feature.ttl")),
    ("feedback.ttl", include_str!("assets/shapes/feedback.ttl")),
    ("model.ttl", include_str!("assets/shapes/model.ttl")),
    ("policy.ttl", include_str!("assets/shapes/policy.ttl")),
    (
        "query-template.ttl",
        include_str!("assets/shapes/query-template.ttl"),
    ),
    ("question.ttl", include_str!("assets/shapes/question.ttl")),
    (
        "subscription.ttl",
        include_str!("assets/shapes/subscription.ttl"),
    ),
    ("tc.ttl", include_str!("assets/shapes/tc.ttl")),
    (
        "worker-image-submission.ttl",
        include_str!("assets/shapes/worker-image-submission.ttl"),
    ),
    (
        "worker-image.ttl",
        include_str!("assets/shapes/worker-image.ttl"),
    ),
];

/// FT-072 manifest TTL — declares the shape-file load order as an
/// rdf:Seq so non-Rust consumers (the Python defensive validator,
/// future packagers) can iterate without re-deriving the order.
pub const SHAPES_MANIFEST_TTL: &str = include_str!("assets/shapes/manifest.ttl");

/// Per-type shape files that are exempt from the BoundaryArtifact
/// first-branch invariant (FT-072 §Invariants). Session and Dispatch
/// carry their own mechanical block and are never boundary-originated;
/// their shapes compose mechanical via sh:and but omit the sh:or
/// motivational/boundary alternatives entirely.
pub const PER_TYPE_BOUNDARY_EXEMPT_FILES: &[&str] = &["dispatch.ttl"];

/// Type-shape IRIs the per-type catalog declares (used by TC-122 to
/// cross-check the embedded shapes match the published catalog). One
/// entry per file in [`PER_TYPE_SHAPE_FILES`].
pub const PER_TYPE_SHAPE_IRIS: &[(&str, &str)] = &[
    (
        "acknowledgement.ttl",
        "https://decision-cli.dev/ns#AcknowledgementShape",
    ),
    ("adr.ttl", "https://decision-cli.dev/ns#ADRShape"),
    ("brief.ttl", "https://decision-cli.dev/ns#BriefShape"),
    (
        "conformance-audit.ttl",
        "https://decision-cli.dev/ns#ConformanceAuditShape",
    ),
    (
        "dependency.ttl",
        "https://decision-cli.dev/ns#DependencyShape",
    ),
    (
        "discovery-finding.ttl",
        "https://decision-cli.dev/ns#DiscoveryFindingShape",
    ),
    (
        "dispatch.ttl",
        "https://decision-cli.dev/ns#DispatchProvenanceShape",
    ),
    ("feature.ttl", "https://decision-cli.dev/ns#FeatureShape"),
    (
        "feedback.ttl",
        "https://decision-cli.dev/ns#FeedbackProvenanceShape",
    ),
    ("model.ttl", "https://decision-cli.dev/ns#ModelShape"),
    ("policy.ttl", "https://decision-cli.dev/ns#PolicyShape"),
    (
        "query-template.ttl",
        "https://decision-cli.dev/ns#QueryTemplateShape",
    ),
    ("question.ttl", "https://decision-cli.dev/ns#QuestionShape"),
    (
        "subscription.ttl",
        "https://decision-cli.dev/ns#SubscriptionShape",
    ),
    ("tc.ttl", "https://decision-cli.dev/ns#TCShape"),
    (
        "worker-image-submission.ttl",
        "https://decision-cli.dev/ns#WorkerImageSubmissionShape",
    ),
    (
        "worker-image.ttl",
        "https://decision-cli.dev/ns#WorkerImageShape",
    ),
];
