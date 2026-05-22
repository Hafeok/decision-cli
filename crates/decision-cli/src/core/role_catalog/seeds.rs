//! Quad builders that seed the role catalog into the orchestration store.
//!
//! Slice 2 baseline (FT-030): two roles — implementer (code-writer) and
//! verifier — each carrying a `dec:Authority` declaration per ADR-027.
//! The TTL assets live alongside this module under `seeds/`; the quads
//! below are hand-mirrored so the init pipeline can write them through a
//! standard transaction (without dragging in a Turtle parser at seed
//! time).

use oxigraph::model::{BlankNode, GraphName, Literal, NamedNode, NamedNodeRef, Quad};

use crate::core::vocab::orchestration_graph;

use super::authority::{
    AUTHORITY_CLASS_IRI, AUTHORITY_PRED_IRI, ESCALATE_CATEGORY_IRI, ESCALATE_CLASS_IRI,
    ESCALATE_TARGET_ROLE_IRI, ESCALATE_VIA_IRI, IMPLEMENTER_AUTHORITY_IRI, MAY_DECIDE_IRI,
    MUST_ESCALATE_IRI, RATIONALE_IRI, VERIFIER_AUTHORITY_IRI,
};
use super::role::{
    ROLE_CLASS_IRI, ROLE_ID_IRI, ROLE_INPUT_TYPE_IRI, ROLE_MODEL_BINDING_IRI, ROLE_OUTPUT_TYPE_IRI,
    VERIFICATION_VERDICT_IRI,
};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Stable IRI minted for the implementer role catalog entry (FT-030).
pub const IMPLEMENTER_ROLE_IRI: &str = "https://decision-cli.dev/ns/role/implementer";

/// Stable IRI minted for the verifier role catalog entry (FT-019).
pub const VERIFIER_ROLE_IRI: &str = "https://decision-cli.dev/ns/role/verifier";

/// Stable string id (`dec:roleId`) used to dispatch the implementer.
/// Mirrors the worker manifest entry `code-writer` (ADR-008 / FT-016).
pub const IMPLEMENTER_ROLE_ID: &str = "code-writer";

/// Stable string id (`dec:roleId`) used to dispatch the verifier.
pub const VERIFIER_ROLE_ID: &str = "verifier";

/// Build the quads that seed the verifier role + its authority into the
/// orchestration store.
#[must_use]
pub fn verifier_seed_quads() -> Vec<Quad> {
    let g: GraphName = orchestration_graph().into_owned().into();
    let role = NamedNode::new_unchecked(VERIFIER_ROLE_IRI);
    let authority = NamedNode::new_unchecked(VERIFIER_AUTHORITY_IRI);
    let mut quads = verifier_role_quads(&role, &authority, &g);
    quads.extend(verifier_authority_quads(&authority, &g));
    quads
}

/// Build the quads that seed the implementer role + its authority into
/// the orchestration store (FT-030).
#[must_use]
pub fn implementer_seed_quads() -> Vec<Quad> {
    let g: GraphName = orchestration_graph().into_owned().into();
    let role = NamedNode::new_unchecked(IMPLEMENTER_ROLE_IRI);
    let authority = NamedNode::new_unchecked(IMPLEMENTER_AUTHORITY_IRI);
    let mut quads = implementer_role_quads(&role, &authority, &g);
    quads.extend(implementer_authority_quads(&authority, &g));
    quads
}

fn verifier_role_quads(role: &NamedNode, authority: &NamedNode, g: &GraphName) -> Vec<Quad> {
    let mut quads = base_role_typing_quads(role, VERIFIER_ROLE_ID, g);
    let role_in = NamedNodeRef::new_unchecked(ROLE_INPUT_TYPE_IRI).into_owned();
    let code_change = NamedNode::new_unchecked("https://decision-cli.dev/ns#CodeChange");
    let feature_spec = NamedNode::new_unchecked("https://decision-cli.dev/ns#FeatureSpec");
    let bundle_hash = NamedNode::new_unchecked("https://decision-cli.dev/ns#BundleHash");
    quads.push(Quad::new(
        role.clone(),
        role_in.clone(),
        code_change,
        g.clone(),
    ));
    quads.push(Quad::new(
        role.clone(),
        role_in.clone(),
        feature_spec,
        g.clone(),
    ));
    quads.push(Quad::new(role.clone(), role_in, bundle_hash, g.clone()));
    quads.extend(role_output_and_model_quads(
        role,
        VERIFICATION_VERDICT_IRI,
        g,
    ));
    quads.push(authority_link_quad(role, authority, g));
    quads
}

fn implementer_role_quads(role: &NamedNode, authority: &NamedNode, g: &GraphName) -> Vec<Quad> {
    let mut quads = base_role_typing_quads(role, IMPLEMENTER_ROLE_ID, g);
    let role_in = NamedNodeRef::new_unchecked(ROLE_INPUT_TYPE_IRI).into_owned();
    let feature_spec = NamedNode::new_unchecked("https://decision-cli.dev/ns#FeatureSpec");
    let bundle_hash = NamedNode::new_unchecked("https://decision-cli.dev/ns#BundleHash");
    quads.push(Quad::new(
        role.clone(),
        role_in.clone(),
        feature_spec,
        g.clone(),
    ));
    quads.push(Quad::new(role.clone(), role_in, bundle_hash, g.clone()));
    quads.extend(role_output_and_model_quads(
        role,
        "https://decision-cli.dev/ns#CodeChange",
        g,
    ));
    quads.push(authority_link_quad(role, authority, g));
    quads
}

fn base_role_typing_quads(role: &NamedNode, role_id: &str, g: &GraphName) -> Vec<Quad> {
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    let role_class = NamedNodeRef::new_unchecked(ROLE_CLASS_IRI).into_owned();
    let role_id_pred = NamedNodeRef::new_unchecked(ROLE_ID_IRI).into_owned();
    vec![
        Quad::new(role.clone(), rdf_type, role_class, g.clone()),
        Quad::new(
            role.clone(),
            role_id_pred,
            Literal::new_simple_literal(role_id),
            g.clone(),
        ),
    ]
}

fn role_output_and_model_quads(role: &NamedNode, output_iri: &str, g: &GraphName) -> Vec<Quad> {
    let role_out = NamedNodeRef::new_unchecked(ROLE_OUTPUT_TYPE_IRI).into_owned();
    let role_model = NamedNodeRef::new_unchecked(ROLE_MODEL_BINDING_IRI).into_owned();
    let output = NamedNode::new_unchecked(output_iri);
    vec![
        Quad::new(role.clone(), role_out, output, g.clone()),
        Quad::new(
            role.clone(),
            role_model,
            Literal::new_simple_literal("claude-sonnet-4-5"),
            g.clone(),
        ),
    ]
}

fn authority_link_quad(role: &NamedNode, authority: &NamedNode, g: &GraphName) -> Quad {
    let pred = NamedNodeRef::new_unchecked(AUTHORITY_PRED_IRI).into_owned();
    Quad::new(role.clone(), pred, authority.clone(), g.clone())
}

fn verifier_authority_quads(authority: &NamedNode, g: &GraphName) -> Vec<Quad> {
    let may_decide = &[
        "verdict-classification",
        "rationale-content",
        "cited-references",
    ];
    let must_escalate = &[
        "feature-spec-changes",
        "adr-changes",
        "cross-cutting-policy",
    ];
    let escalate_via: &[(&str, &str, &str)] = &[
        ("feature-spec-changes", "gap", "spec-author"),
        ("adr-changes", "contradiction", "architect"),
        ("cross-cutting-policy", "contradiction", "architect"),
    ];
    let rationale = "Verifier issues a verdict on whether the produced artifact satisfies the feature_spec and TCs. Verdict classification, rationale wording, and cited references are within authority. Changes to the feature_spec, ADRs, or cross-cutting policy must be escalated via feedback rather than encoded in the verdict.";
    build_authority_quads(
        authority,
        may_decide,
        must_escalate,
        escalate_via,
        rationale,
        g,
    )
}

fn implementer_authority_quads(authority: &NamedNode, g: &GraphName) -> Vec<Quad> {
    let may_decide = &[
        "code-style",
        "naming-within-feature",
        "internal-data-shape",
        "test-cases-for-this-feature",
    ];
    let must_escalate = &[
        "feature-spec-changes",
        "adr-changes",
        "new-artifact-types",
        "cross-cutting-policy",
    ];
    let escalate_via: &[(&str, &str, &str)] = &[
        ("feature-spec-changes", "gap", "spec-author"),
        ("adr-changes", "contradiction", "architect"),
        ("new-artifact-types", "capability-request", "architect"),
        ("cross-cutting-policy", "contradiction", "architect"),
    ];
    let rationale = "Implementer writes code from a feature_spec; structural changes to the spec or to ADRs go through their authoring roles. Implementer self-decides anything that does not survive the file boundary of the feature.";
    build_authority_quads(
        authority,
        may_decide,
        must_escalate,
        escalate_via,
        rationale,
        g,
    )
}

fn build_authority_quads(
    authority: &NamedNode,
    may_decide: &[&str],
    must_escalate: &[&str],
    escalate_via: &[(&str, &str, &str)],
    rationale: &str,
    g: &GraphName,
) -> Vec<Quad> {
    let mut quads = authority_typing_quads(authority, rationale, g);
    quads.extend(authority_category_quads(
        authority,
        MAY_DECIDE_IRI,
        may_decide,
        g,
    ));
    quads.extend(authority_category_quads(
        authority,
        MUST_ESCALATE_IRI,
        must_escalate,
        g,
    ));
    for (category, class, target_role) in escalate_via {
        quads.extend(escalation_hint_quads(
            authority,
            category,
            class,
            target_role,
            g,
        ));
    }
    quads
}

fn authority_typing_quads(authority: &NamedNode, rationale: &str, g: &GraphName) -> Vec<Quad> {
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    let class = NamedNodeRef::new_unchecked(AUTHORITY_CLASS_IRI).into_owned();
    let rationale_pred = NamedNodeRef::new_unchecked(RATIONALE_IRI).into_owned();
    vec![
        Quad::new(authority.clone(), rdf_type, class, g.clone()),
        Quad::new(
            authority.clone(),
            rationale_pred,
            Literal::new_simple_literal(rationale),
            g.clone(),
        ),
    ]
}

fn authority_category_quads(
    authority: &NamedNode,
    predicate_iri: &str,
    values: &[&str],
    g: &GraphName,
) -> Vec<Quad> {
    let pred = NamedNodeRef::new_unchecked(predicate_iri).into_owned();
    values
        .iter()
        .map(|v| {
            Quad::new(
                authority.clone(),
                pred.clone(),
                Literal::new_simple_literal(*v),
                g.clone(),
            )
        })
        .collect()
}

fn escalation_hint_quads(
    authority: &NamedNode,
    category: &str,
    class: &str,
    target_role: &str,
    g: &GraphName,
) -> Vec<Quad> {
    let hint = escalation_hint_blank_node(authority, category);
    let via_quad = Quad::new(
        authority.clone(),
        NamedNodeRef::new_unchecked(ESCALATE_VIA_IRI).into_owned(),
        hint.clone(),
        g.clone(),
    );
    vec![
        via_quad,
        hint_literal_quad(&hint, ESCALATE_CATEGORY_IRI, category, g),
        hint_literal_quad(&hint, ESCALATE_CLASS_IRI, class, g),
        hint_literal_quad(&hint, ESCALATE_TARGET_ROLE_IRI, target_role, g),
    ]
}

fn escalation_hint_blank_node(authority: &NamedNode, category: &str) -> BlankNode {
    // Deterministic blank-node id so re-seeding the catalog is idempotent.
    BlankNode::new_unchecked(format!(
        "authority-{}-{}",
        short_iri_tail(authority),
        category
    ))
}

fn hint_literal_quad(hint: &BlankNode, pred_iri: &str, value: &str, g: &GraphName) -> Quad {
    Quad::new(
        hint.clone(),
        NamedNodeRef::new_unchecked(pred_iri).into_owned(),
        Literal::new_simple_literal(value),
        g.clone(),
    )
}

fn short_iri_tail(iri: &NamedNode) -> String {
    iri.as_str()
        .rsplit('/')
        .next()
        .unwrap_or("authority")
        .replace(|c: char| !c.is_ascii_alphanumeric(), "-")
}
