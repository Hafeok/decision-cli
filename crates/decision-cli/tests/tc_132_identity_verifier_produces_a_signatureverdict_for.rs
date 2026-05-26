//! TC-132 — identity-verifier produces a SignatureVerdict for each of the five outcome classes.
//!
//! Validates: FT-090 · ADR-017 · ADR-018.
//! Spec: `.product/tests/TC-132-identity-verifier-produces-a-signatureverdict-for.md`
//!
//! Pins down the FT-090 acceptance claim: for each of the five SignatureVerdict
//! outcome classes — `valid`, `invalid-signature`, `untrusted-identity`,
//! `image-not-found`, `rekor-entry-missing` — the identity-verifier produces
//! the expected verdict given the corresponding input conditions, and the
//! resulting `dec:SignatureVerdict` artifact carries both halves of the
//! ADR-038 dual-provenance discipline:
//!
//! - **mechanical** — `prov:wasGeneratedBy` → action session,
//!   `prov:wasAttributedTo` → agent, `prov:generatedAtTime` → RFC3339;
//! - **motivational** — `dec:respondsTo` → originating
//!   `dec:WorkerImageSubmission` (a `wasDerivedFrom` sub-property per
//!   ADR-039).
//!
//! Per ADR-013 we keep this file under the 400-line cap by routing each
//! per-class assertion through a small helper and consolidating the
//! exit-criterion into a single named test the product-cli cargo-test
//! runner identifies.

use oxigraph::model::{NamedNode, Term};
use oxigraph::store::Store;

use decision_cli::core::cosign_trust::{
    SignatureIdentity, TagPattern, TrustList, TrustListEntry, TrustOrigin,
    GITHUB_ACTIONS_ISSUER_URI,
};
use decision_cli::core::identity_verifier::{
    classify, CosignVerifyOutcome, IdentityVerificationEvidence, RegistryProbeOutcome,
    RekorLookupOutcome, SignatureVerdict, SignatureVerdictClass,
};
use decision_cli::vocab::{
    bundle_graph, IRI_DEC_RESPONDS_TO, IRI_DEC_SIGNATURE_VERDICT, IRI_DEC_SIGNATURE_VERDICT_CLASS,
    IRI_DEC_VERDICT_RATIONALE, IRI_PROV_GENERATED_AT_TIME, IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL,
    IRI_PROV_WAS_GENERATED_BY,
};

const WORKFLOW_PATH: &str = ".github/workflows/release-worker.yml";

fn submission_iri(id: &str) -> NamedNode {
    NamedNode::new_unchecked(format!(
        "https://decision-cli.dev/ns/worker-image-submission/{id}"
    ))
}

fn session_iri(id: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("https://decision-cli.dev/ns/session/{id}"))
}

fn agent_iri() -> NamedNode {
    NamedNode::new_unchecked("https://decision-cli.dev/ns/agent/identity-verifier")
}

fn populated_trust_list() -> TrustList {
    TrustList::from_entries(vec![TrustListEntry {
        origin: TrustOrigin::GithubActions {
            repo: "Hafeok/decision-cli-worker-implementer".to_string(),
            workflow_path: WORKFLOW_PATH.to_string(),
            tag_pattern: TagPattern::parse("implementer-v*.*.*")
                .expect("test tag pattern must parse"),
        },
        issuer: GITHUB_ACTIONS_ISSUER_URI.to_string(),
        note: Some("implementer release line".to_string()),
    }])
    .expect("trust list construction must succeed")
}

fn admitted_identity() -> SignatureIdentity {
    SignatureIdentity::new(
        format!(
            "https://github.com/Hafeok/decision-cli-worker-implementer/{WORKFLOW_PATH}@refs/tags/implementer-v1.2.3"
        ),
        GITHUB_ACTIONS_ISSUER_URI,
    )
}

fn off_list_identity() -> SignatureIdentity {
    SignatureIdentity::new(
        format!(
            "https://github.com/attacker/fork/{WORKFLOW_PATH}@refs/tags/implementer-v9.9.9"
        ),
        GITHUB_ACTIONS_ISSUER_URI,
    )
}

/// Build an evidence aggregate from the three component outcomes.
fn evidence(
    probe: RegistryProbeOutcome,
    cosign: CosignVerifyOutcome,
    rekor: RekorLookupOutcome,
) -> IdentityVerificationEvidence {
    IdentityVerificationEvidence::new(probe, cosign, rekor)
}

/// Construct the verdict artifact that the action's interpretation side
/// would emit for a given `(class, rationale)`. Provenance is uniform per
/// the FT-090 success criteria.
fn build_verdict(
    id: &str,
    class: SignatureVerdictClass,
    rationale: String,
    submission: &NamedNode,
) -> SignatureVerdict {
    SignatureVerdict {
        id: id.to_string(),
        verdict_class: class,
        rationale,
        generated_by_session: session_iri(&format!("identity-verifier-{id}")),
        attributed_to_agent: agent_iri(),
        generated_at_time: "2026-05-26T00:00:00Z".to_string(),
        responds_to_submission: submission.clone(),
    }
}

/// Assert that the verdict's serialised quads carry both halves of the
/// ADR-038 dual-provenance discipline pointing at the supplied subject IRIs.
fn assert_dual_provenance(verdict: &SignatureVerdict) {
    let quads = verdict.to_quads(bundle_graph());
    let subj = verdict.iri();

    // Mechanical: prov:wasGeneratedBy → action session.
    assert!(
        quads.iter().any(|q| {
            q.subject == subj.clone().into()
                && q.predicate.as_str() == IRI_PROV_WAS_GENERATED_BY
                && matches!(&q.object, Term::NamedNode(n) if n == &verdict.generated_by_session)
        }),
        "missing prov:wasGeneratedBy → action session for {}",
        verdict.id
    );

    // Mechanical: prov:wasAttributedTo → agent.
    assert!(
        quads.iter().any(|q| {
            q.subject == subj.clone().into()
                && q.predicate.as_str() == IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL
                && matches!(&q.object, Term::NamedNode(n) if n == &verdict.attributed_to_agent)
        }),
        "missing prov:wasAttributedTo → agent for {}",
        verdict.id
    );

    // Mechanical: prov:generatedAtTime → xsd:dateTime literal.
    assert!(
        quads.iter().any(|q| {
            q.subject == subj.clone().into()
                && q.predicate.as_str() == IRI_PROV_GENERATED_AT_TIME
        }),
        "missing prov:generatedAtTime literal for {}",
        verdict.id
    );

    // Motivational: dec:respondsTo → WorkerImageSubmission (wasDerivedFrom
    // sub-property per ADR-039).
    assert!(
        quads.iter().any(|q| {
            q.subject == subj.clone().into()
                && q.predicate.as_str() == IRI_DEC_RESPONDS_TO
                && matches!(&q.object, Term::NamedNode(n) if n == &verdict.responds_to_submission)
        }),
        "missing dec:respondsTo → WorkerImageSubmission for {}",
        verdict.id
    );
}

/// Drive the classifier through one (evidence, trust list, expected class)
/// tuple, build the verdict artifact, assert its provenance shape, and
/// return the verdict for downstream graph assertions.
fn run_class(
    id: &str,
    evidence: IdentityVerificationEvidence,
    trust_list: &TrustList,
    expected: SignatureVerdictClass,
    submission: &NamedNode,
) -> SignatureVerdict {
    let (class, rationale) = classify(&evidence, trust_list)
        .unwrap_or_else(|err| panic!("classifier failed for {id}: {err}"));
    assert_eq!(
        class, expected,
        "wrong verdict class for {id}: got {}, want {}",
        class.as_str(),
        expected.as_str()
    );
    assert!(
        !rationale.is_empty(),
        "rationale must be non-empty for {id}"
    );
    let verdict = build_verdict(id, class, rationale, submission);
    assert_dual_provenance(&verdict);
    verdict
}

/// Class 1 of 5: `valid` — all checks pass, identity on the trust list.
fn drives_valid_class() -> SignatureVerdict {
    run_class(
        "valid-001",
        evidence(
            RegistryProbeOutcome::Found,
            CosignVerifyOutcome::SignatureValid {
                identity: admitted_identity(),
            },
            RekorLookupOutcome::Confirmed,
        ),
        &populated_trust_list(),
        SignatureVerdictClass::Valid,
        &submission_iri("valid-001"),
    )
}

/// Class 2 of 5: `invalid-signature` — cosign verify failed cryptographically.
fn drives_invalid_signature_class() -> SignatureVerdict {
    run_class(
        "invalid-001",
        evidence(
            RegistryProbeOutcome::Found,
            CosignVerifyOutcome::SignatureInvalid {
                detail: "certificate chain failed verification".to_string(),
            },
            RekorLookupOutcome::Confirmed,
        ),
        &populated_trust_list(),
        SignatureVerdictClass::InvalidSignature,
        &submission_iri("invalid-001"),
    )
}

/// Class 3 of 5: `untrusted-identity` — signature valid, identity off-list.
fn drives_untrusted_identity_class() -> SignatureVerdict {
    run_class(
        "untrusted-001",
        evidence(
            RegistryProbeOutcome::Found,
            CosignVerifyOutcome::SignatureValid {
                identity: off_list_identity(),
            },
            RekorLookupOutcome::Confirmed,
        ),
        &populated_trust_list(),
        SignatureVerdictClass::UntrustedIdentity,
        &submission_iri("untrusted-001"),
    )
}

/// Class 4 of 5: `image-not-found` — registry returned 404.
fn drives_image_not_found_class() -> SignatureVerdict {
    run_class(
        "missing-image-001",
        evidence(
            RegistryProbeOutcome::NotFound,
            CosignVerifyOutcome::SignatureValid {
                identity: admitted_identity(),
            },
            RekorLookupOutcome::Confirmed,
        ),
        &populated_trust_list(),
        SignatureVerdictClass::ImageNotFound,
        &submission_iri("missing-image-001"),
    )
}

/// Class 5 of 5: `rekor-entry-missing` — referenced Rekor entry absent.
fn drives_rekor_entry_missing_class() -> SignatureVerdict {
    run_class(
        "missing-rekor-001",
        evidence(
            RegistryProbeOutcome::Found,
            CosignVerifyOutcome::SignatureValid {
                identity: admitted_identity(),
            },
            RekorLookupOutcome::Missing {
                detail: "rekor.sigstore.dev returned 404 for the referenced entry uuid".to_string(),
            },
        ),
        &populated_trust_list(),
        SignatureVerdictClass::RekorEntryMissing,
        &submission_iri("missing-rekor-001"),
    )
}

/// Final shape claim: each of the five verdicts inserts cleanly into an
/// Oxigraph store and is queryable by its `dec:signatureVerdictClass`
/// literal — the property the WorkerCurator bundle assembler will SELECT
/// on (FT-092).
fn five_verdicts_round_trip_through_store(verdicts: &[SignatureVerdict]) {
    let store = Store::new().expect("memory store must construct");
    for v in verdicts {
        for q in v.to_quads(bundle_graph()) {
            store.insert(&q).expect("insert quad");
        }
    }

    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT ?v ?class WHERE {{ GRAPH ?g {{ \
           ?v a <{IRI_DEC_SIGNATURE_VERDICT}> ; \
              <{IRI_DEC_SIGNATURE_VERDICT_CLASS}> ?class . \
         }} }}",
    );

    use oxigraph::sparql::QueryResults;
    let QueryResults::Solutions(sols) = store.query(q.as_str()).expect("query ok") else {
        panic!("expected solution stream");
    };
    let mut seen: Vec<String> = Vec::new();
    for sol in sols {
        let sol = sol.expect("solution");
        if let Some(Term::Literal(lit)) = sol.get("class") {
            seen.push(lit.value().to_string());
        }
    }
    seen.sort();
    let mut expected: Vec<String> = verdicts
        .iter()
        .map(|v| v.verdict_class.as_str().to_string())
        .collect();
    expected.sort();
    assert_eq!(seen, expected, "round-tripped verdict classes must match input");
}

/// Belt-and-braces — confirm each verdict's serialisation carries the
/// `dec:verdictRationale` literal so the Curator bundle (FT-092) can read
/// the rationale without re-running the classifier.
fn every_verdict_serialises_a_rationale_literal(verdicts: &[SignatureVerdict]) {
    for v in verdicts {
        let quads = v.to_quads(bundle_graph());
        let count = quads
            .iter()
            .filter(|q| q.predicate.as_str() == IRI_DEC_VERDICT_RATIONALE)
            .count();
        assert_eq!(
            count, 1,
            "verdict {} must emit exactly one dec:verdictRationale literal",
            v.id
        );
    }
}

/// Single-entry checkpoint test — the product-cli runner (cargo-test
/// runner) looks up TC-132 by this function name in `tests/*.rs` and
/// flips the TC to `passing` only when this test runs and exits 0. The
/// body drives the classifier through all five outcome classes, asserts
/// dual provenance per ADR-038 on each verdict, and round-trips every
/// verdict through an in-memory Oxigraph store.
#[test]
fn tc_132_identity_verifier_produces_a_signatureverdict_for() {
    let verdicts = vec![
        drives_valid_class(),
        drives_invalid_signature_class(),
        drives_untrusted_identity_class(),
        drives_image_not_found_class(),
        drives_rekor_entry_missing_class(),
    ];

    // FT-090 §Success criteria: each of the five classes is produced for
    // its corresponding input — assert all five appear exactly once.
    let mut seen: Vec<&'static str> = verdicts.iter().map(|v| v.verdict_class.as_str()).collect();
    seen.sort();
    let mut expected = vec![
        "image-not-found",
        "invalid-signature",
        "rekor-entry-missing",
        "untrusted-identity",
        "valid",
    ];
    expected.sort();
    assert_eq!(seen, expected, "all five outcome classes must appear exactly once");

    every_verdict_serialises_a_rationale_literal(&verdicts);
    five_verdicts_round_trip_through_store(&verdicts);
}
