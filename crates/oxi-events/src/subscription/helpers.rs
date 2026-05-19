//! SPARQL helpers and binding-diff utilities used by the registry.

use std::collections::{BTreeMap, BTreeSet};

use oxigraph::model::{NamedNode, Term};
use oxigraph::sparql::{Query, QueryResults};
use oxigraph::store::Store;

use crate::error::RegistryError;

use super::types::{Binding, Delta, SubscriptionQuery, TriggerType};

pub(super) fn triggers_compatible(
    sub_triggers: &BTreeSet<TriggerType>,
    mutation_triggers: &BTreeSet<TriggerType>,
) -> bool {
    if sub_triggers.is_empty() || mutation_triggers.is_empty() {
        return true;
    }
    !sub_triggers.is_disjoint(mutation_triggers)
}

pub(super) fn validate_query(
    id: &NamedNode,
    query: &SubscriptionQuery,
) -> Result<(), RegistryError> {
    Query::parse(query.as_str(), None).map_err(|source| RegistryError::InvalidQuery {
        subscription: id.as_str().to_string(),
        source,
    })?;
    Ok(())
}

pub(super) fn collect_bindings(
    store: &Store,
    query: &str,
) -> Result<Vec<Binding>, oxigraph::sparql::EvaluationError> {
    let results = store.query(query)?;
    let mut out: Vec<Binding> = Vec::new();
    let QueryResults::Solutions(sols) = results else {
        return Ok(out);
    };
    for sol in sols {
        let sol = sol?;
        let mut binding: Binding = BTreeMap::new();
        for (var, term) in sol.iter() {
            binding.insert(var.as_str().to_string(), term_repr(term));
        }
        out.push(binding);
    }
    Ok(out)
}

fn term_repr(term: &Term) -> String {
    match term {
        Term::NamedNode(n) => format!("<{}>", n.as_str()),
        Term::BlankNode(b) => format!("_:{}", b.as_str()),
        Term::Literal(l) => l.to_string(),
        Term::Triple(t) => format!("<<{}>>", t.as_ref()),
    }
}

pub(super) fn diff_bindings(prior: &[Binding], current: &[Binding]) -> Delta {
    let prior_set: BTreeSet<&Binding> = prior.iter().collect();
    let current_set: BTreeSet<&Binding> = current.iter().collect();
    let added: Vec<Binding> = current_set
        .difference(&prior_set)
        .copied()
        .cloned()
        .collect();
    let removed: Vec<Binding> = prior_set
        .difference(&current_set)
        .copied()
        .cloned()
        .collect();
    match (added.is_empty(), removed.is_empty()) {
        (true, true) => Delta::Added(Vec::new()),
        (false, true) => Delta::Added(added),
        (true, false) => Delta::Removed(removed),
        (false, false) => Delta::Changed { added, removed },
    }
}
