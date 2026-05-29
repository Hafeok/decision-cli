//! `dec migrate env-to-bench` — rewrite legacy ENV-NNN vocabulary to
//! BNCH-NNN in the live orchestration store (FT-117).
//!
//! FT-112 specced the migration but the implementation shipped only
//! as a callable function exercised by TC-210 against an isolated
//! test store; no CLI was landed and the live workdir's store was
//! never migrated. This module ships the real migration logic and
//! exposes it as `dec migrate env-to-bench` so the live store can
//! be brought up to the BNCH vocabulary.
//!
//! The migration is composed of four idempotent SPARQL UPDATE
//! operations applied in a single store transaction:
//! 1. Class assertion: `dec:VerificationEnvironment` →
//!    `dec:VerificationBench`.
//! 2. Predicate `dec:envType` → `dec:benchType`.
//! 3. Predicate `dec:ranInEnvironment` → `dec:ranOnBench`.
//! 4. Subject + object IRI prefix `/ns/env/ENV-` →
//!    `/ns/bench/BNCH-` for instance references.
//!
//! After the rewrite, a cross-check query counts residual
//! `dec:VerificationEnvironment` instances — non-zero indicates a
//! UPDATE-pattern bug and exits non-zero (defence in depth).

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use crate::core::store::{open_orchestration_store, orchestration_dump_path, persist_store};

/// Rewrite summary the CLI prints to stdout.
#[derive(Debug, Default)]
pub struct MigrationReport {
    /// Class-assertion triples rewritten
    /// (`?s a dec:VerificationEnvironment` → `dec:VerificationBench`).
    pub class_assertions: usize,
    /// Triples whose predicate was `dec:envType` → `dec:benchType`.
    pub env_type_predicates: usize,
    /// Triples whose predicate was `dec:ranInEnvironment` →
    /// `dec:ranOnBench`.
    pub ran_in_environment_predicates: usize,
    /// Triples with `/ns/env/ENV-` IRIs (subject or object) rewritten
    /// to `/ns/bench/BNCH-`.
    pub instance_iri_rewrites: usize,
    /// Quads moved from the legacy `verify-env` named graph to the
    /// canonical `verify-bench` named graph.
    pub named_graph_moves: usize,
}

impl MigrationReport {
    /// True iff every counter is zero (nothing to migrate or already
    /// migrated).
    #[must_use]
    pub fn is_noop(&self) -> bool {
        self.class_assertions == 0
            && self.env_type_predicates == 0
            && self.ran_in_environment_predicates == 0
            && self.instance_iri_rewrites == 0
            && self.named_graph_moves == 0
    }

    /// Render a human-readable summary.
    #[must_use]
    pub fn render(&self, dry_run: bool) -> String {
        let prefix = if dry_run { "[DRY-RUN] " } else { "" };
        let mut out = String::new();
        out.push_str(&format!("{prefix}migration plan:\n"));
        out.push_str(&format!(
            "  dec:VerificationEnvironment → dec:VerificationBench   {n} class assertions\n",
            n = self.class_assertions
        ));
        out.push_str(&format!(
            "  dec:envType → dec:benchType                            {n} predicate rewrites\n",
            n = self.env_type_predicates
        ));
        out.push_str(&format!(
            "  dec:ranInEnvironment → dec:ranOnBench                  {n} predicate rewrites\n",
            n = self.ran_in_environment_predicates
        ));
        out.push_str(&format!(
            "  /ns/env/ENV- → /ns/bench/BNCH-                         {n} instance IRI rewrites\n",
            n = self.instance_iri_rewrites
        ));
        out.push_str(&format!(
            "  named graph: verify-env → verify-bench                 {n} quads moved\n",
            n = self.named_graph_moves
        ));
        if self.is_noop() {
            out.push_str("  → no-op (store has no legacy env vocabulary)\n");
        } else if dry_run {
            out.push_str("  → dry-run: no writes performed\n");
        } else {
            out.push_str("  ✓ migration complete; idempotent on re-run\n");
        }
        out
    }
}

/// Migrate `<workdir>/.dec/store/orchestration.nq` from legacy ENV
/// vocabulary to BNCH. If `dry_run`, count what would change without
/// writing.
pub fn migrate(workdir: &Path, dry_run: bool) -> Result<MigrationReport> {
    let store = open_orchestration_store(workdir)?;
    let report = migrate_store(&store, dry_run)?;
    if !dry_run && !report.is_noop() {
        let dump = orchestration_dump_path(workdir);
        persist_store(&store, &dump)
            .with_context(|| format!("persisting migrated store to {}", dump.display()))?;
        cross_check(&store)?;
    }
    Ok(report)
}

/// Migrate an in-memory store. Tests use this directly without
/// touching the filesystem.
pub fn migrate_store(store: &Store, dry_run: bool) -> Result<MigrationReport> {
    let report = MigrationReport {
        class_assertions: count_class_assertions(store)?,
        env_type_predicates: count_predicate(store, "envType")?,
        ran_in_environment_predicates: count_predicate(store, "ranInEnvironment")?,
        instance_iri_rewrites: count_instance_iris(store)?,
        named_graph_moves: count_named_graph_moves(store)?,
    };
    if dry_run || report.is_noop() {
        return Ok(report);
    }
    // Order matters: rewrite inside the verify-env named graph first
    // (class, predicates, IRIs all in place), THEN move every quad
    // from verify-env → verify-bench named graph. This way the
    // canonical bench data lands in the canonical named graph that
    // `dec verify bench list` queries against.
    apply_class_rewrite(store)?;
    apply_predicate_rewrite(store, "envType", "benchType")?;
    apply_predicate_rewrite(store, "ranInEnvironment", "ranOnBench")?;
    apply_instance_iri_rewrite(store)?;
    apply_named_graph_move(store)?;
    Ok(report)
}

fn count_class_assertions(store: &Store) -> Result<usize> {
    let q = "PREFIX dec: <https://decision-cli.dev/ns#> \
             SELECT (COUNT(*) AS ?n) WHERE { \
               GRAPH ?g { ?s a dec:VerificationEnvironment } \
             }";
    count_query(store, q)
}

fn count_predicate(store: &Store, local: &str) -> Result<usize> {
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT (COUNT(*) AS ?n) WHERE {{ \
           GRAPH ?g {{ ?s dec:{local} ?o }} \
         }}"
    );
    count_query(store, &q)
}

fn count_named_graph_moves(store: &Store) -> Result<usize> {
    let q = "SELECT (COUNT(*) AS ?n) WHERE { \
             GRAPH <https://decision-cli.dev/ns/graph/verify-env> { ?s ?p ?o } \
             }";
    count_query(store, q)
}

fn count_instance_iris(store: &Store) -> Result<usize> {
    let q = "SELECT (COUNT(*) AS ?n) WHERE { \
             GRAPH ?g { ?s ?p ?o } \
             FILTER( \
               (isIRI(?s) && STRSTARTS(STR(?s), \"https://decision-cli.dev/ns/env/\")) || \
               (isIRI(?o) && STRSTARTS(STR(?o), \"https://decision-cli.dev/ns/env/\")) \
             ) \
             }";
    count_query(store, q)
}

fn count_query(store: &Store, query: &str) -> Result<usize> {
    match store.query(query) {
        Ok(QueryResults::Solutions(sols)) => {
            for sol in sols {
                let sol = sol.with_context(|| format!("evaluating count: {query}"))?;
                if let Some(oxigraph::model::Term::Literal(lit)) = sol.get("n") {
                    return lit
                        .value()
                        .parse::<usize>()
                        .with_context(|| format!("parsing count from {}", lit.value()));
                }
            }
            Ok(0)
        }
        Ok(_) => Err(anyhow!("unexpected query shape for count: {query}")),
        Err(e) => Err(anyhow!("count query failed: {e}; query={query}")),
    }
}

fn apply_class_rewrite(store: &Store) -> Result<()> {
    let u = "PREFIX dec: <https://decision-cli.dev/ns#> \
             PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> \
             DELETE { GRAPH ?g { ?s a dec:VerificationEnvironment } } \
             INSERT { GRAPH ?g { ?s a dec:VerificationBench } } \
             WHERE  { GRAPH ?g { ?s a dec:VerificationEnvironment } }";
    store
        .update(u)
        .with_context(|| "applying class rewrite".to_string())
}

fn apply_predicate_rewrite(store: &Store, old: &str, new: &str) -> Result<()> {
    let u = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         DELETE {{ GRAPH ?g {{ ?s dec:{old} ?o }} }} \
         INSERT {{ GRAPH ?g {{ ?s dec:{new} ?o }} }} \
         WHERE  {{ GRAPH ?g {{ ?s dec:{old} ?o }} }}"
    );
    store
        .update(u.as_str())
        .with_context(|| format!("applying predicate rewrite {old} → {new}"))
}

fn apply_instance_iri_rewrite(store: &Store) -> Result<()> {
    // Rewrite subjects: <.../ns/env/ENV-NNN> → <.../ns/bench/BNCH-NNN>.
    let subj = "DELETE { GRAPH ?g { ?old ?p ?o } } \
                INSERT { GRAPH ?g { ?new ?p ?o } } \
                WHERE  { \
                  GRAPH ?g { ?old ?p ?o } \
                  FILTER(isIRI(?old) && STRSTARTS(STR(?old), \"https://decision-cli.dev/ns/env/\")) \
                  BIND(IRI(REPLACE(\
                    REPLACE(STR(?old), \"/ns/env/\", \"/ns/bench/\"), \
                    \"/bench/ENV-\", \"/bench/BNCH-\"\
                  )) AS ?new) \
                }";
    store
        .update(subj)
        .with_context(|| "rewriting subject IRIs".to_string())?;
    // Rewrite objects: same pattern but on ?o.
    let obj = "DELETE { GRAPH ?g { ?s ?p ?old } } \
               INSERT { GRAPH ?g { ?s ?p ?new } } \
               WHERE  { \
                 GRAPH ?g { ?s ?p ?old } \
                 FILTER(isIRI(?old) && STRSTARTS(STR(?old), \"https://decision-cli.dev/ns/env/\")) \
                 BIND(IRI(REPLACE(\
                   REPLACE(STR(?old), \"/ns/env/\", \"/ns/bench/\"), \
                   \"/bench/ENV-\", \"/bench/BNCH-\"\
                 )) AS ?new) \
               }";
    store
        .update(obj)
        .with_context(|| "rewriting object IRIs".to_string())
}

fn apply_named_graph_move(store: &Store) -> Result<()> {
    let u = "DELETE { GRAPH <https://decision-cli.dev/ns/graph/verify-env> { ?s ?p ?o } } \
             INSERT { GRAPH <https://decision-cli.dev/ns/graph/verify-bench> { ?s ?p ?o } } \
             WHERE  { GRAPH <https://decision-cli.dev/ns/graph/verify-env> { ?s ?p ?o } }";
    store
        .update(u)
        .with_context(|| "moving quads from verify-env to verify-bench named graph".to_string())
}

/// Defence-in-depth: after migration, confirm zero residual
/// `dec:VerificationEnvironment` instances remain. The UPDATE should
/// always cover every match; this cross-check catches subtle bugs in
/// the query template.
fn cross_check(store: &Store) -> Result<()> {
    let n = count_class_assertions(store)?;
    if n != 0 {
        return Err(anyhow!(
            "cross-check failed: {n} dec:VerificationEnvironment instances remain after migration"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxigraph::io::RdfFormat;

    fn seeded_store() -> Store {
        let store = Store::new().unwrap();
        store
            .load_from_reader(
                RdfFormat::TriG,
                r#"
                @prefix dec: <https://decision-cli.dev/ns#> .
                GRAPH <urn:graph:test> {
                    <https://decision-cli.dev/ns/env/ENV-001>
                        a dec:VerificationEnvironment ;
                        dec:envType "ephemeral-tempdir" .
                    <https://decision-cli.dev/ns/graph/VG-001>
                        dec:environment <https://decision-cli.dev/ns/env/ENV-001> .
                    <https://decision-cli.dev/ns/result/VGR-001>
                        dec:ranInEnvironment <https://decision-cli.dev/ns/env/ENV-001> .
                }
                "#
                .as_bytes(),
            )
            .unwrap();
        store
    }

    #[test]
    fn migrate_store_rewrites_all_four_classes() {
        let store = seeded_store();
        let report = migrate_store(&store, false).unwrap();
        assert_eq!(report.class_assertions, 1);
        assert_eq!(report.env_type_predicates, 1);
        assert_eq!(report.ran_in_environment_predicates, 1);
        assert!(report.instance_iri_rewrites >= 3);

        // Post-migration: no VerificationEnvironment left.
        assert_eq!(count_class_assertions(&store).unwrap(), 0);
        // Post-migration: one VerificationBench exists.
        let q = "PREFIX dec: <https://decision-cli.dev/ns#> \
                 SELECT (COUNT(*) AS ?n) WHERE { \
                   GRAPH ?g { ?s a dec:VerificationBench } \
                 }";
        assert_eq!(count_query(&store, q).unwrap(), 1);
        // Post-migration: bench/BNCH-001 subject exists.
        let q = "SELECT (COUNT(*) AS ?n) WHERE { \
                 GRAPH ?g { <https://decision-cli.dev/ns/bench/BNCH-001> ?p ?o } \
                 }";
        assert!(count_query(&store, q).unwrap() >= 2);
    }

    #[test]
    fn dry_run_reports_counts_without_writing() {
        let store = seeded_store();
        let report = migrate_store(&store, true).unwrap();
        assert_eq!(report.class_assertions, 1);
        // Store should be unchanged.
        assert_eq!(count_class_assertions(&store).unwrap(), 1);
    }

    #[test]
    fn migration_is_idempotent() {
        let store = seeded_store();
        let r1 = migrate_store(&store, false).unwrap();
        assert!(!r1.is_noop());
        let r2 = migrate_store(&store, false).unwrap();
        assert!(r2.is_noop(), "second migration should be no-op");
    }

    #[test]
    fn empty_store_is_noop() {
        let store = Store::new().unwrap();
        let report = migrate_store(&store, false).unwrap();
        assert!(report.is_noop());
    }
}
