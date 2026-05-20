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
    for required in ["feature-spec-changes", "adr-changes", "cross-cutting-policy"] {
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
