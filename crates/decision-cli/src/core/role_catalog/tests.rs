//! Unit tests covering catalog lookup, listing, authority round-trips,
//! and embedded-seed TTL invariants (FT-019 + FT-030).

use oxigraph::store::Store;

use super::*;
use crate::core::feedback::FeedbackClass;

fn seeded_store() -> Store {
    let store = Store::new().expect("in-memory store");
    store
        .transaction(|mut tx| {
            for q in verifier_seed_quads() {
                tx.insert(q.as_ref())?;
            }
            for q in implementer_seed_quads() {
                tx.insert(q.as_ref())?;
            }
            Ok::<_, oxigraph::store::StorageError>(())
        })
        .expect("seed role catalog quads");
    store
}

#[test]
fn lookup_verifier_returns_role_with_authority() {
    let store = seeded_store();
    let role = lookup(&store, VERIFIER_ROLE_ID)
        .expect("lookup ok")
        .expect("verifier present");
    assert_eq!(role.role_id, VERIFIER_ROLE_ID);
    assert_eq!(role.iri, VERIFIER_ROLE_IRI);
    assert_eq!(role.output_type, VERIFICATION_VERDICT_IRI);
    assert_eq!(role.model_binding, "claude-sonnet-4-5");
    let authority = role.authority.expect("verifier authority present");
    assert_eq!(authority.iri, VERIFIER_AUTHORITY_IRI);
    assert!(authority
        .may_decide
        .iter()
        .any(|c| c == "verdict-classification"));
    assert!(authority
        .must_escalate
        .iter()
        .any(|c| c == "feature-spec-changes"));
    assert!(!authority.rationale.is_empty());
    assert!(authority
        .escalate_via
        .iter()
        .any(|h| h.category == "feature-spec-changes" && h.class == FeedbackClass::Gap));
}

#[test]
fn lookup_implementer_returns_role_with_authority() {
    let store = seeded_store();
    let role = lookup(&store, IMPLEMENTER_ROLE_ID)
        .expect("lookup ok")
        .expect("implementer present");
    assert_eq!(role.role_id, IMPLEMENTER_ROLE_ID);
    assert_eq!(role.iri, IMPLEMENTER_ROLE_IRI);
    assert_eq!(role.output_type, "https://decision-cli.dev/ns#CodeChange");
    let authority = role.authority.expect("implementer authority present");
    assert!(authority.may_decide.iter().any(|c| c == "code-style"));
    for required in [
        "feature-spec-changes",
        "adr-changes",
        "cross-cutting-policy",
    ] {
        assert!(
            authority.must_escalate.iter().any(|c| c == required),
            "implementer authority must escalate {required}"
        );
    }
    // Authority invariant (ADR-027): mayDecide and mustEscalate are disjoint.
    for cat in &authority.may_decide {
        assert!(
            !authority.must_escalate.contains(cat),
            "category {cat:?} appears in both lists"
        );
    }
}

#[test]
fn lookup_returns_none_on_empty_store() {
    let store = Store::new().expect("in-memory store");
    let role = lookup(&store, VERIFIER_ROLE_ID).expect("lookup ok");
    assert!(role.is_none());
}

#[test]
fn lookup_returns_none_for_unknown_role() {
    let store = seeded_store();
    let role = lookup(&store, "no-such-role").expect("lookup ok");
    assert!(role.is_none());
}

#[test]
fn list_roles_returns_both_phase_a_entries() {
    let store = seeded_store();
    let roles = list_roles(&store).expect("list ok");
    assert_eq!(roles.len(), 2, "Phase A baseline is implementer + verifier");
    let ids: Vec<&str> = roles.iter().map(|r| r.role_id.as_str()).collect();
    assert!(ids.contains(&IMPLEMENTER_ROLE_ID));
    assert!(ids.contains(&VERIFIER_ROLE_ID));
    for r in &roles {
        assert!(
            r.authority.is_some(),
            "FT-030 invariant: every role carries an authority declaration"
        );
    }
}

#[test]
fn every_must_escalate_category_has_a_hint() {
    let store = seeded_store();
    for role in list_roles(&store).expect("list ok") {
        let authority = role.authority.expect("authority present");
        for category in &authority.must_escalate {
            assert!(
                authority
                    .escalate_via
                    .iter()
                    .any(|h| &h.category == category),
                "role {} mustEscalate category {category:?} has no escalateVia hint",
                role.role_id
            );
        }
    }
}

#[test]
fn seed_ttl_mentions_required_predicates() {
    for pred in [
        "dec:roleId",
        "dec:roleInputType",
        "dec:roleOutputType",
        "dec:roleModelBinding",
        "dec:authority",
    ] {
        assert!(
            VERIFIER_SEED_TTL.contains(pred),
            "verifier seed missing {pred}"
        );
        assert!(
            IMPLEMENTER_SEED_TTL.contains(pred),
            "implementer seed missing {pred}"
        );
    }
}

#[test]
fn authority_seed_ttl_carries_required_predicates() {
    for ttl in [VERIFIER_AUTHORITY_TTL, IMPLEMENTER_AUTHORITY_TTL] {
        for pred in [
            "dec:mayDecide",
            "dec:mustEscalate",
            "dec:escalateVia",
            "dec:rationale",
        ] {
            assert!(ttl.contains(pred), "authority TTL missing {pred}");
        }
    }
}

// FT-121 / TC-266: role_catalog::lookup returns seeded allowed_tools for implementer role.
#[test]
fn tc_266_role_catalog_lookup_returns_seeded_allowed_tools() {
    let store = seeded_store();
    let implementer = lookup(&store, IMPLEMENTER_ROLE_ID)
        .expect("lookup ok")
        .expect("implementer present");
    let verifier = lookup(&store, VERIFIER_ROLE_ID)
        .expect("lookup ok")
        .expect("verifier present");

    // Implementer gets the canonical five-tool list.
    assert_eq!(implementer.allowed_tools.len(), 5);
    let expected_implementer = vec![
        "read_file",
        "write_file",
        "run_build",
        "run_lint",
        "run_tests",
    ];
    for tool in &expected_implementer {
        assert!(
            implementer.allowed_tools.contains(&tool.to_string()),
            "implementer missing tool {tool}"
        );
    }

    // Verifier gets the four-tool subset (no write_file).
    assert_eq!(verifier.allowed_tools.len(), 4);
    let expected_verifier = vec!["read_file", "run_build", "run_lint", "run_tests"];
    for tool in &expected_verifier {
        assert!(
            verifier.allowed_tools.contains(&tool.to_string()),
            "verifier missing tool {tool}"
        );
    }
    assert!(
        !verifier.allowed_tools.contains(&"write_file".to_string()),
        "verifier should not have write_file"
    );

    // Both roles have read_file (sanity check the seeding covers shared tools).
    assert!(implementer.allowed_tools.contains(&"read_file".to_string()));
    assert!(verifier.allowed_tools.contains(&"read_file".to_string()));
}

// FT-121 / TC-267: SHACL shape file requires at least one dec:roleTool.
#[test]
fn tc_267_shacl_refuses_role_without_role_tool() {
    const ROLE_SHAPE_TTL: &str = include_str!("seeds/role.shacl.ttl");

    // Assert the shape file contains the dec:roleTool constraint with minCount 1.
    assert!(
        ROLE_SHAPE_TTL.contains("dec:roleTool"),
        "role shape missing dec:roleTool predicate"
    );
    assert!(
        ROLE_SHAPE_TTL.contains("sh:minCount 1") || ROLE_SHAPE_TTL.contains("sh:minCount 1 ;"),
        "role shape missing minCount 1 constraint"
    );

    // Assert the shape is well-formed Turtle that references all required predicates.
    for pred in [
        "dec:roleId",
        "dec:roleInputType",
        "dec:roleOutputType",
        "dec:roleModelBinding",
        "dec:authority",
        "dec:roleTool",
    ] {
        assert!(
            ROLE_SHAPE_TTL.contains(pred),
            "role shape missing predicate {pred}"
        );
    }

    // Assert the targetClass is dec:Role.
    assert!(
        ROLE_SHAPE_TTL.contains("sh:targetClass dec:Role"),
        "role shape missing targetClass declaration"
    );
}

// FT-121 / TC-268: Legacy stores without dec:roleTool quads return empty allowed_tools and do not panic.
#[test]
fn tc_268_legacy_store_returns_empty_allowed_tools() {
    use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

    let store = Store::new().expect("in-memory store");

    // Create a role instance with all predicates EXCEPT dec:roleTool (simulating legacy).
    let role_iri = NamedNode::new_unchecked("https://decision-cli.dev/ns/role/legacy");
    let g: GraphName = crate::core::vocab::orchestration_graph()
        .into_owned()
        .into();
    let rdf_type = NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type");
    let role_class = NamedNode::new_unchecked(ROLE_CLASS_IRI);
    let role_id_pred = NamedNode::new_unchecked(ROLE_ID_IRI);
    let role_input_pred = NamedNode::new_unchecked(ROLE_INPUT_TYPE_IRI);
    let role_output_pred = NamedNode::new_unchecked(ROLE_OUTPUT_TYPE_IRI);
    let role_model_pred = NamedNode::new_unchecked(ROLE_MODEL_BINDING_IRI);

    let feature_spec = NamedNode::new_unchecked("https://decision-cli.dev/ns#FeatureSpec");
    let bundle_hash = NamedNode::new_unchecked("https://decision-cli.dev/ns#BundleHash");
    let code_change = NamedNode::new_unchecked("https://decision-cli.dev/ns#CodeChange");

    store
        .transaction(|mut tx| {
            tx.insert(
                Quad::new(
                    role_iri.clone(),
                    rdf_type.clone(),
                    role_class.clone(),
                    g.clone(),
                )
                .as_ref(),
            )?;
            tx.insert(
                Quad::new(
                    role_iri.clone(),
                    role_id_pred.clone(),
                    Literal::new_simple_literal("legacy"),
                    g.clone(),
                )
                .as_ref(),
            )?;
            tx.insert(
                Quad::new(
                    role_iri.clone(),
                    role_input_pred.clone(),
                    feature_spec.clone(),
                    g.clone(),
                )
                .as_ref(),
            )?;
            tx.insert(
                Quad::new(
                    role_iri.clone(),
                    role_input_pred.clone(),
                    bundle_hash.clone(),
                    g.clone(),
                )
                .as_ref(),
            )?;
            tx.insert(
                Quad::new(
                    role_iri.clone(),
                    role_output_pred.clone(),
                    code_change.clone(),
                    g.clone(),
                )
                .as_ref(),
            )?;
            tx.insert(
                Quad::new(
                    role_iri.clone(),
                    role_model_pred.clone(),
                    Literal::new_simple_literal("claude-sonnet-4-5"),
                    g.clone(),
                )
                .as_ref(),
            )?;
            // No dec:roleTool quads, no dec:authority (simulating pre-FT-121 seed).
            Ok::<_, oxigraph::store::StorageError>(())
        })
        .expect("insert legacy role");

    // Lookup should succeed and return a Role with empty allowed_tools.
    let role = lookup(&store, "legacy")
        .expect("lookup ok")
        .expect("legacy role present");

    assert_eq!(
        role.allowed_tools,
        Vec::<String>::new(),
        "legacy role should have empty allowed_tools"
    );
    assert_eq!(role.role_id, "legacy");
    assert_eq!(role.output_type, code_change.as_str());
    assert_eq!(role.model_binding, "claude-sonnet-4-5");
    assert_eq!(role.input_types.len(), 2);

    // Calling lookup twice should return identical values.
    let role2 = lookup(&store, "legacy")
        .expect("second lookup ok")
        .expect("legacy role still present");
    assert_eq!(role.allowed_tools, role2.allowed_tools);
    assert_eq!(role.role_id, role2.role_id);
}
