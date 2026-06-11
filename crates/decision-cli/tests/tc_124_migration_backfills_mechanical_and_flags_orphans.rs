//! TC-124 — FT-074 migration end-to-end against a three-class fixture.
//!
//! Exit criterion for FT-074. The fixture seeds three artifacts:
//!
//! - A **conformant** ADR carrying both mechanical and motivational blocks.
//! - A **backfillable** ADR carrying motivational triples (the `:decidesFor`
//!   edges synthesised from the historical `features:` front-matter list)
//!   but no mechanical block.
//! - An **orphan** Feature carrying neither block and no informal edges
//!   that the FT-074 mapping table can map.
//!
//! The test then:
//!
//! 1. Runs the audit in dry-run mode and checks the three verdicts.
//! 2. Runs the apply path and asserts:
//!    - the backfillable ADR carries `prov:wasGeneratedBy` pointing at a
//!      `:HistoricalSession` flagged `:isMigrationBackfill true`,
//!    - the orphan Feature carries `:isMigrationOrphan true` plus a
//!      `migration-orphan-needs-repair` Feedback artifact.
//! 3. Re-runs apply and asserts idempotence — no new feedback, no new
//!    historical sessions.
//! 4. Runs cutover with one orphan present → must error and leave
//!    warn-only mode intact.
//! 5. Simulates orphan repair (removes the `:isMigrationOrphan` annotation)
//!    and re-runs cutover → succeeds and flips warn-only mode off.

use anyhow::Result;
use oxigraph::model::{GraphName, Literal, NamedNode, Quad};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use decision_cli::migrate_provenance::{
    audit_store, run_cutover, run_migration, warn_only_mode, AuditVerdict, MigrateArgs,
    HISTORICAL_AGENT_IRI, HISTORICAL_SESSION_CLASS, IRI_DEC_IS_MIGRATION_ORPHAN,
    MIGRATION_ORPHAN_FEEDBACK_CLASS,
};
use decision_cli::vocab::{
    IRI_DEC_FEEDBACK, IRI_DEC_FEEDBACK_CLASS, IRI_DEC_GRAPH_ORCHESTRATION, IRI_DEC_SOURCE_ARTIFACT,
    IRI_PROV_GENERATED_AT_TIME, IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL, IRI_PROV_WAS_GENERATED_BY,
};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const XSD_DATE_TIME: &str = "http://www.w3.org/2001/XMLSchema#dateTime";
const XSD_BOOLEAN: &str = "http://www.w3.org/2001/XMLSchema#boolean";

const ADR_CLASS: &str = "https://decision-cli.dev/ns#ADR";
const FEATURE_CLASS: &str = "https://decision-cli.dev/ns#Feature";

const IRI_DECIDES_FOR: &str = "https://decision-cli.dev/ns#decidesFor";
const IRI_IS_MIGRATION_BACKFILL: &str = "https://decision-cli.dev/ns#isMigrationBackfill";

const ADR_CONFORMANT: &str = "https://decision-cli.dev/ns/adr/tc124-ADR-conformant";
const ADR_BACKFILL: &str = "https://decision-cli.dev/ns/adr/tc124-ADR-backfill";
const FEATURE_ORPHAN: &str = "https://decision-cli.dev/ns/feature/tc124-FT-orphan";
const FEATURE_TARGET: &str = "https://decision-cli.dev/ns/feature/tc124-FT-target";

const SESSION_REAL: &str = "https://decision-cli.dev/ns/session/tc124-real";
const AGENT_REAL: &str = "https://decision-cli.dev/ns/agent/tc124-real";

const REAL_TIMESTAMP: &str = "2026-05-25T19:00:00Z";
const FALLBACK_TIMESTAMP: &str = "2026-05-25T20:30:00Z";

// ---------------------------------------------------------------------------
// TC-124 — the headline test the TC frontmatter points at.
// ---------------------------------------------------------------------------

#[test]
fn tc_124_migration_backfills_mechanical_and_flags_orphans() {
    let store = Store::new().expect("in-memory store");
    seed_fixture(&store).expect("seed fixture");

    // ----- (1) Audit + dry-run produces three verdicts ------------------
    let entries = audit_store(&store).expect("audit ok");
    let by_artifact = index_by_artifact(&entries);
    let conformant = by_artifact
        .get(ADR_CONFORMANT)
        .expect("conformant entry present");
    assert!(
        matches!(conformant, AuditVerdict::Conformant),
        "conformant ADR must classify as Conformant; got {:?}",
        conformant
    );
    let backfill = by_artifact
        .get(ADR_BACKFILL)
        .expect("backfill entry present");
    let AuditVerdict::BackfillableMechanical { edges } = backfill else {
        panic!(
            "backfill ADR must classify as BackfillableMechanical; got {:?}",
            backfill
        );
    };
    assert!(
        edges.iter().any(|e| e.predicate == IRI_DECIDES_FOR),
        "edges must include :decidesFor"
    );
    let orphan = by_artifact.get(FEATURE_ORPHAN).expect("orphan present");
    let AuditVerdict::Orphan { reasons } = orphan else {
        panic!("orphan Feature must classify as Orphan; got {:?}", orphan);
    };
    assert!(
        !reasons.is_empty(),
        "orphan classification must carry reasons"
    );

    // Verify the dry-run path produces a parallel summary. The fixture
    // has four FT-074-audited artifacts: ADR_CONFORMANT (conformant),
    // FEATURE_TARGET (conformant — supports ADR_CONFORMANT's :decidesFor),
    // ADR_BACKFILL (backfillable), FEATURE_ORPHAN (orphan).
    let dry = run_migration(&store, &dry_args()).expect("dry-run ok");
    assert_eq!(
        dry.summary.conformant, 2,
        "two conformant artifacts in the fixture (got summary={:?})",
        dry.summary
    );
    assert_eq!(dry.summary.backfilled, 1, "one backfillable artifact");
    assert_eq!(dry.summary.orphan, 1, "one orphan artifact");
    // No new triples committed by dry-run.
    assert!(
        !subject_present(&store, &session_for_artifact(ADR_BACKFILL, "run-tc124")),
        "dry-run must not commit synthetic session"
    );

    // ----- (2) Apply path commits backfill + orphan feedback ------------
    let applied = run_migration(&store, &apply_args()).expect("apply ok");
    assert_eq!(applied.summary.backfilled, 1);
    assert_eq!(applied.summary.orphan, 1);
    let session_iri = session_for_artifact(ADR_BACKFILL, "run-tc124");
    assert!(
        subject_present(&store, &session_iri),
        "synthetic :HistoricalSession must be committed"
    );
    assert!(
        triple_present(
            &store,
            ADR_BACKFILL,
            IRI_PROV_WAS_GENERATED_BY,
            &session_iri
        ),
        "backfilled ADR must have prov:wasGeneratedBy pointing at the :HistoricalSession"
    );
    assert!(
        is_typed_as(&store, &session_iri, HISTORICAL_SESSION_CLASS),
        ":HistoricalSession must carry rdf:type :HistoricalSession"
    );
    assert!(
        bool_flag_set(&store, &session_iri, IRI_IS_MIGRATION_BACKFILL),
        ":HistoricalSession must carry :isMigrationBackfill true"
    );
    assert!(
        triple_present(
            &store,
            ADR_BACKFILL,
            IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL,
            HISTORICAL_AGENT_IRI
        ),
        "backfilled ADR must be attributed to the shared :HistoricalAgent"
    );

    // Orphan annotations + feedback artifact.
    assert!(
        bool_flag_set(&store, FEATURE_ORPHAN, IRI_DEC_IS_MIGRATION_ORPHAN),
        "orphan Feature must carry :isMigrationOrphan true"
    );
    let feedback_iri = find_orphan_feedback(&store, FEATURE_ORPHAN)
        .expect("migration-orphan-needs-repair feedback must be emitted");
    assert!(
        is_typed_as(&store, &feedback_iri, IRI_DEC_FEEDBACK),
        "feedback artifact must carry rdf:type dec:Feedback"
    );
    assert_eq!(
        literal_value_of(&store, &feedback_iri, IRI_DEC_FEEDBACK_CLASS).as_deref(),
        Some(MIGRATION_ORPHAN_FEEDBACK_CLASS),
        "feedback class must be `migration-orphan-needs-repair`"
    );

    // ----- (3) Idempotence — re-running produces no new triples --------
    let triples_before = count_all_triples(&store);
    let re_applied = run_migration(&store, &apply_args()).expect("re-apply ok");
    let triples_after = count_all_triples(&store);
    assert_eq!(
        triples_before, triples_after,
        "idempotence: second --apply run must not write any new quads (before={triples_before}, after={triples_after})"
    );
    // Re-run sees the existing orphan feedback and the backfilled ADR.
    assert_eq!(re_applied.summary.orphan, 0, "no fresh orphan emissions");
    assert!(
        re_applied.summary.already_orphan == 1 || re_applied.summary.conformant >= 2,
        "re-run must either route through SkippedAlreadyOrphanFlagged or treat as conformant"
    );

    // ----- (4) Cutover refuses while an orphan remains -----------------
    assert!(
        warn_only_mode(&store).expect("warn_only query"),
        "warn-only mode defaults true during the migration window"
    );
    let cutover_err = run_cutover(&store, /*threshold=*/ 0)
        .expect_err("cutover must refuse while orphans remain");
    let msg = cutover_err.to_string();
    assert!(
        msg.contains("cutover refused"),
        "error must mention cutover refused: {msg}"
    );
    assert!(
        msg.contains(FEATURE_ORPHAN),
        "error must list the unrepaired orphan IRI: {msg}"
    );
    assert!(
        warn_only_mode(&store).expect("warn_only query"),
        "warn-only mode must remain true after a refused cutover"
    );

    // ----- (5) Repair the orphan, retry cutover, flip warn-only off ----
    remove_orphan_marker(&store, FEATURE_ORPHAN).expect("repair orphan");
    let outcome =
        run_cutover(&store, /*threshold=*/ 0).expect("cutover must succeed once orphans drop to 0");
    assert_eq!(
        outcome.orphan_count, 0,
        "cutover outcome must report zero unrepaired orphans"
    );
    assert!(outcome.flipped, "cutover must flip warn-only mode");
    assert!(
        !warn_only_mode(&store).expect("warn_only query"),
        "warn-only mode must be false after cutover"
    );
}

// ---------------------------------------------------------------------------
// Fixture seeding.
// ---------------------------------------------------------------------------

fn seed_fixture(store: &Store) -> Result<()> {
    let g = orchestration_graph();
    let mut quads: Vec<Quad> = Vec::new();

    // FT-target: the Feature the conformant ADR decides for (also lets
    // the backfillable ADR's :decidesFor edge land on a real subject).
    // Conformant (mechanical + motivational) so it does not show up as
    // a separate orphan in the audit scope — we want exactly one orphan
    // (FEATURE_ORPHAN) in the fixture.
    quads.push(typed_quad(
        FEATURE_TARGET,
        RDF_TYPE,
        FEATURE_CLASS,
        g.clone(),
    ));
    quads.push(named_quad(
        FEATURE_TARGET,
        IRI_PROV_WAS_GENERATED_BY,
        SESSION_REAL,
        g.clone(),
    ));
    quads.push(named_quad(
        FEATURE_TARGET,
        IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL,
        AGENT_REAL,
        g.clone(),
    ));
    quads.push(datetime_quad(
        FEATURE_TARGET,
        IRI_PROV_GENERATED_AT_TIME,
        REAL_TIMESTAMP,
        g.clone(),
    ));
    // Motivational edge: Feature `addresses` a synthetic Feedback IRI.
    let synth_feedback = "https://decision-cli.dev/ns/feedback/tc124-target-source";
    quads.push(typed_quad(
        synth_feedback,
        RDF_TYPE,
        IRI_DEC_FEEDBACK,
        g.clone(),
    ));
    quads.push(named_quad(
        FEATURE_TARGET,
        "https://decision-cli.dev/ns#addresses",
        synth_feedback,
        g.clone(),
    ));

    // Real session + agent used by the conformant ADR — gives it a
    // mechanical block without flagging it as a migration backfill.
    quads.push(typed_quad(
        SESSION_REAL,
        RDF_TYPE,
        "https://decision-cli.dev/ns#Session",
        g.clone(),
    ));
    quads.push(typed_quad(
        AGENT_REAL,
        RDF_TYPE,
        "https://decision-cli.dev/ns#Agent",
        g.clone(),
    ));

    // --- Conformant ADR (mechanical + motivational both present) --------
    quads.push(typed_quad(ADR_CONFORMANT, RDF_TYPE, ADR_CLASS, g.clone()));
    quads.push(named_quad(
        ADR_CONFORMANT,
        IRI_PROV_WAS_GENERATED_BY,
        SESSION_REAL,
        g.clone(),
    ));
    quads.push(named_quad(
        ADR_CONFORMANT,
        IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL,
        AGENT_REAL,
        g.clone(),
    ));
    quads.push(datetime_quad(
        ADR_CONFORMANT,
        IRI_PROV_GENERATED_AT_TIME,
        REAL_TIMESTAMP,
        g.clone(),
    ));
    quads.push(named_quad(
        ADR_CONFORMANT,
        IRI_DECIDES_FOR,
        FEATURE_TARGET,
        g.clone(),
    ));

    // --- Backfillable ADR (motivational present, no mechanical) ---------
    quads.push(typed_quad(ADR_BACKFILL, RDF_TYPE, ADR_CLASS, g.clone()));
    quads.push(named_quad(
        ADR_BACKFILL,
        IRI_DECIDES_FOR,
        FEATURE_TARGET,
        g.clone(),
    ));

    // --- Orphan Feature (neither block, no mappable informal field) -----
    quads.push(typed_quad(FEATURE_ORPHAN, RDF_TYPE, FEATURE_CLASS, g));

    store.transaction(|mut tx| {
        for q in &quads {
            tx.insert(q.as_ref())?;
        }
        Ok::<(), oxigraph::store::StorageError>(())
    })?;
    Ok(())
}

// ---------------------------------------------------------------------------
// MigrateArgs factories — keep the test deterministic across CI runs.
// ---------------------------------------------------------------------------

fn dry_args() -> MigrateArgs {
    MigrateArgs {
        run_id: "run-tc124".to_string(),
        fallback_timestamp: FALLBACK_TIMESTAMP.to_string(),
        external_origin: format!("FT-074 provenance migration tool run at {FALLBACK_TIMESTAMP}"),
        cutover_threshold: 0,
        dry_run: true,
    }
}

fn apply_args() -> MigrateArgs {
    MigrateArgs {
        run_id: "run-tc124".to_string(),
        fallback_timestamp: FALLBACK_TIMESTAMP.to_string(),
        external_origin: format!("FT-074 provenance migration tool run at {FALLBACK_TIMESTAMP}"),
        cutover_threshold: 0,
        dry_run: false,
    }
}

fn session_for_artifact(artifact: &str, run_id: &str) -> String {
    decision_cli::migrate_provenance::historical_session_iri(artifact, run_id)
        .as_str()
        .to_string()
}

// ---------------------------------------------------------------------------
// SPARQL probes + helpers.
// ---------------------------------------------------------------------------

fn index_by_artifact(
    entries: &[decision_cli::migrate_provenance::AuditEntry],
) -> std::collections::BTreeMap<&str, &AuditVerdict> {
    let mut out = std::collections::BTreeMap::new();
    for e in entries {
        out.insert(e.artifact.as_str(), &e.verdict);
    }
    out
}

fn count_all_triples(store: &Store) -> usize {
    let mut n = 0usize;
    if let Ok(QueryResults::Solutions(sols)) = store
        .query("SELECT (COUNT(*) AS ?c) WHERE { { ?s ?p ?o } UNION { GRAPH ?g { ?s ?p ?o } } }")
    {
        for sol in sols.flatten() {
            if let Some(oxigraph::model::Term::Literal(lit)) = sol.get("c") {
                n = lit.value().parse::<usize>().unwrap_or(0);
            }
        }
    }
    n
}

fn subject_present(store: &Store, subject: &str) -> bool {
    let q = format!(
        "ASK {{ {{ <{s}> ?p ?o }} UNION {{ GRAPH ?g {{ <{s}> ?p ?o }} }} }}",
        s = subject
    );
    matches!(store.query(q.as_str()), Ok(QueryResults::Boolean(true)))
}

fn triple_present(store: &Store, subject: &str, predicate: &str, object: &str) -> bool {
    let q = format!(
        "ASK {{ \
           {{ <{s}> <{p}> <{o}> }} UNION {{ GRAPH ?g {{ <{s}> <{p}> <{o}> }} }} \
         }}",
        s = subject,
        p = predicate,
        o = object
    );
    matches!(store.query(q.as_str()), Ok(QueryResults::Boolean(true)))
}

fn is_typed_as(store: &Store, subject: &str, class: &str) -> bool {
    triple_present(store, subject, RDF_TYPE, class)
}

fn bool_flag_set(store: &Store, subject: &str, predicate: &str) -> bool {
    let q = format!(
        "ASK {{ \
           {{ <{s}> <{p}> ?v . FILTER(?v = true || str(?v) = \"true\") }} \
           UNION \
           {{ GRAPH ?g {{ <{s}> <{p}> ?v . FILTER(?v = true || str(?v) = \"true\") }} }} \
         }}",
        s = subject,
        p = predicate
    );
    matches!(store.query(q.as_str()), Ok(QueryResults::Boolean(true)))
}

fn literal_value_of(store: &Store, subject: &str, predicate: &str) -> Option<String> {
    let q = format!(
        "SELECT ?v WHERE {{ {{ <{s}> <{p}> ?v }} UNION {{ GRAPH ?g {{ <{s}> <{p}> ?v }} }} }}",
        s = subject,
        p = predicate
    );
    if let Ok(QueryResults::Solutions(sols)) = store.query(q.as_str()) {
        for sol in sols.flatten() {
            if let Some(oxigraph::model::Term::Literal(lit)) = sol.get("v") {
                return Some(lit.value().to_string());
            }
        }
    }
    None
}

fn find_orphan_feedback(store: &Store, orphan_artifact: &str) -> Option<String> {
    let q = format!(
        "SELECT ?f WHERE {{ \
           {{ ?f a <{ft}> ; <{fc}> ?cls ; <{sa}> <{a}> . FILTER(str(?cls) = \"{cls_lit}\") }} \
           UNION \
           {{ GRAPH ?g {{ ?f a <{ft}> ; <{fc}> ?cls ; <{sa}> <{a}> . FILTER(str(?cls) = \"{cls_lit}\") }} }} \
         }}",
        ft = IRI_DEC_FEEDBACK,
        fc = IRI_DEC_FEEDBACK_CLASS,
        sa = IRI_DEC_SOURCE_ARTIFACT,
        a = orphan_artifact,
        cls_lit = MIGRATION_ORPHAN_FEEDBACK_CLASS,
    );
    if let Ok(QueryResults::Solutions(sols)) = store.query(q.as_str()) {
        for sol in sols.flatten() {
            if let Some(oxigraph::model::Term::NamedNode(n)) = sol.get("f") {
                return Some(n.as_str().to_string());
            }
        }
    }
    None
}

fn remove_orphan_marker(store: &Store, artifact: &str) -> Result<()> {
    let q = format!(
        "DELETE {{ GRAPH <{g}> {{ <{a}> <{p}> ?v }} }} \
         WHERE  {{ GRAPH <{g}> {{ <{a}> <{p}> ?v }} }}",
        g = IRI_DEC_GRAPH_ORCHESTRATION,
        a = artifact,
        p = IRI_DEC_IS_MIGRATION_ORPHAN
    );
    store.update(q.as_str())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Quad helpers — mirror the same construction style as ft_073's fixture.
// ---------------------------------------------------------------------------

fn orchestration_graph() -> GraphName {
    GraphName::NamedNode(NamedNode::new_unchecked(IRI_DEC_GRAPH_ORCHESTRATION))
}

fn typed_quad(subject: &str, predicate: &str, object: &str, g: GraphName) -> Quad {
    Quad::new(
        NamedNode::new_unchecked(subject),
        NamedNode::new_unchecked(predicate),
        NamedNode::new_unchecked(object),
        g,
    )
}

fn named_quad(subject: &str, predicate: &str, object: &str, g: GraphName) -> Quad {
    Quad::new(
        NamedNode::new_unchecked(subject),
        NamedNode::new_unchecked(predicate),
        NamedNode::new_unchecked(object),
        g,
    )
}

fn datetime_quad(subject: &str, predicate: &str, value: &str, g: GraphName) -> Quad {
    Quad::new(
        NamedNode::new_unchecked(subject),
        NamedNode::new_unchecked(predicate),
        Literal::new_typed_literal(value, NamedNode::new_unchecked(XSD_DATE_TIME)),
        g,
    )
}

#[allow(dead_code)]
fn boolean_quad(subject: &str, predicate: &str, value: bool, g: GraphName) -> Quad {
    Quad::new(
        NamedNode::new_unchecked(subject),
        NamedNode::new_unchecked(predicate),
        Literal::new_typed_literal(
            if value { "true" } else { "false" },
            NamedNode::new_unchecked(XSD_BOOLEAN),
        ),
        g,
    )
}
