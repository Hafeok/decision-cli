//! SPARQL queries against the product-cli graph projection (FT-052).
//!
//! Each function takes a loaded [`Store`] and the target feature id and
//! returns one section of the report. The queries deliberately use
//! only predicates that `product graph rebuild` emits today so the
//! reader degrades gracefully when the projection has not yet been
//! extended with `pm:domain` or `pm:scope`.

use oxigraph::model::Term;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use super::report::{CrossCuttingRow, DependencyStatus};
use super::PreflightError;

/// Product-meta IRI prefixes — kept in one place so callers (and tests)
/// can build matching IRIs without re-typing the namespace.
pub mod iris {
    /// product-meta ontology namespace used by `product graph rebuild`.
    pub const PM: &str = "https://product-meta/ontology#";
    /// product-meta feature subject prefix.
    pub const PM_FEATURE: &str = "https://product-meta/feature/";
    /// product-meta ADR subject prefix.
    pub const PM_ADR: &str = "https://product-meta/adr/";
}

use iris::{PM, PM_ADR, PM_FEATURE};

/// Enumerate ADRs the projection records as cross-cutting (any
/// `pm:appliesTo` link to *some* feature) and report whether each is
/// linked to the target.
pub(super) fn query_cross_cutting(
    store: &Store,
    feature_id: &str,
) -> Result<Vec<CrossCuttingRow>, PreflightError> {
    let q = format!(
        "PREFIX pm: <{PM}>\n\
         SELECT DISTINCT ?adr (EXISTS {{ \
            {{ <{PM_FEATURE}{feature_id}> pm:implementedBy ?adr }} UNION \
            {{ ?adr pm:appliesTo <{PM_FEATURE}{feature_id}> }} \
         }} AS ?linked) WHERE {{ \
            {{ ?adr pm:appliesTo ?anyfeature }} UNION \
            {{ ?adr pm:appliesTo <{PM_FEATURE}{feature_id}> }} . \
            ?adr a pm:ArchitecturalDecision . \
         }} ORDER BY ?adr",
    );
    let mut out = Vec::new();
    let sols = solutions(store, &q, "cross-cutting")?;
    for sol in sols {
        let sol = sol.map_err(|e| PreflightError::StoreError {
            detail: format!("cross-cutting solution: {e}"),
        })?;
        let adr = sol.get("adr").and_then(term_iri).and_then(strip_pm_adr);
        let linked = sol.get("linked").is_some_and(term_bool);
        if let Some(adr_id) = adr {
            out.push(CrossCuttingRow { adr_id, linked });
        }
    }
    Ok(out)
}

/// Partition cross-cutting rows into `(linked, gaps)` ADR-id lists.
pub(super) fn split_cross_cutting(rows: Vec<CrossCuttingRow>) -> (Vec<String>, Vec<String>) {
    let mut linked = Vec::new();
    let mut gaps = Vec::new();
    for row in rows {
        if row.linked {
            linked.push(row.adr_id);
        } else {
            gaps.push(row.adr_id);
        }
    }
    (linked, gaps)
}

/// Read `pm:domainGap` triples from the projection. The current
/// product-cli rebuild does not emit this predicate; the query is
/// shaped so adding it upstream lights this section up without any
/// code changes here.
pub(super) fn query_domain_gaps(
    store: &Store,
    feature_id: &str,
) -> Result<Vec<String>, PreflightError> {
    let q = format!(
        "PREFIX pm: <{PM}>\n\
         SELECT DISTINCT ?domain WHERE {{ \
            <{PM_FEATURE}{feature_id}> pm:domainGap ?domain . \
         }} ORDER BY ?domain",
    );
    let mut out = Vec::new();
    let sols = solutions(store, &q, "domain gap")?;
    for sol in sols {
        let sol = sol.map_err(|e| PreflightError::StoreError {
            detail: format!("domain solution: {e}"),
        })?;
        if let Some(d) = sol.get("domain").and_then(term_string) {
            out.push(d);
        }
    }
    Ok(out)
}

/// Walk `feature pm:dependsOn ?dep` and collect each dep with its
/// projected `pm:status`.
pub(super) fn query_dep_availability(
    store: &Store,
    feature_id: &str,
) -> Result<Vec<DependencyStatus>, PreflightError> {
    let q = format!(
        "PREFIX pm: <{PM}>\n\
         SELECT ?dep ?status WHERE {{ \
            <{PM_FEATURE}{feature_id}> pm:dependsOn ?dep . \
            OPTIONAL {{ ?dep pm:status ?status }} . \
         }} ORDER BY ?dep",
    );
    let mut out = Vec::new();
    let sols = solutions(store, &q, "dep")?;
    for sol in sols {
        let sol = sol.map_err(|e| PreflightError::StoreError {
            detail: format!("dep solution: {e}"),
        })?;
        let dep = sol.get("dep").and_then(term_iri).and_then(strip_pm_feature);
        let status = sol.get("status").and_then(term_status_label);
        if let Some(feature_id) = dep {
            out.push(DependencyStatus { feature_id, status });
        }
    }
    Ok(out)
}

fn solutions(
    store: &Store,
    query: &str,
    label: &str,
) -> Result<oxigraph::sparql::QuerySolutionIter, PreflightError> {
    let res = store.query(query).map_err(|e| PreflightError::StoreError {
        detail: format!("{label} query: {e}"),
    })?;
    match res {
        QueryResults::Solutions(s) => Ok(s),
        _ => Err(PreflightError::StoreError {
            detail: format!("{label} query returned non-solutions"),
        }),
    }
}

fn term_iri(t: &Term) -> Option<String> {
    if let Term::NamedNode(n) = t {
        Some(n.as_str().to_string())
    } else {
        None
    }
}

fn term_string(t: &Term) -> Option<String> {
    match t {
        Term::Literal(l) => Some(l.value().to_string()),
        Term::NamedNode(n) => Some(n.as_str().to_string()),
        _ => None,
    }
}

/// Coerce a projected status term into a short human label. The
/// projection encodes statuses as `pm:Complete`, `pm:Planned`, etc.;
/// strip the namespace so the report reads naturally.
fn term_status_label(t: &Term) -> Option<String> {
    match t {
        Term::NamedNode(n) => {
            let iri = n.as_str();
            iri.strip_prefix(PM)
                .map(str::to_string)
                .or_else(|| Some(iri.to_string()))
        }
        Term::Literal(l) => Some(l.value().to_string()),
        _ => None,
    }
}

fn term_bool(t: &Term) -> bool {
    if let Term::Literal(l) = t {
        matches!(l.value(), "true" | "1")
    } else {
        false
    }
}

fn strip_pm_adr(iri: String) -> Option<String> {
    iri.as_str().strip_prefix(PM_ADR).map(str::to_string)
}

fn strip_pm_feature(iri: String) -> Option<String> {
    iri.as_str().strip_prefix(PM_FEATURE).map(str::to_string)
}
