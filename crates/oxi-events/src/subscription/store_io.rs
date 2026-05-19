//! Read persisted subscriptions back from the `subscriptions` named graph.

use std::collections::BTreeSet;

use oxigraph::model::{NamedNode, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use crate::error::RegistryError;
use crate::vocab::{
    IRI_OXI_GRAPH_SUBSCRIPTIONS, IRI_OXI_SUBSCRIPTION, IRI_OXI_SUB_ASK_QUERY, IRI_OXI_SUB_HANDLER,
    IRI_OXI_SUB_MODE, IRI_OXI_SUB_SELECT_QUERY, IRI_OXI_SUB_TRIGGER,
};

use super::types::{
    DeliveryHandlerRef, Subscription, SubscriptionMode, SubscriptionQuery, TriggerType,
};

pub(super) fn read_persisted_subscriptions(
    store: &Store,
) -> Result<Vec<Subscription>, RegistryError> {
    let graph = IRI_OXI_GRAPH_SUBSCRIPTIONS;
    let cls = IRI_OXI_SUBSCRIPTION;
    let ask_pred = IRI_OXI_SUB_ASK_QUERY;
    let select_pred = IRI_OXI_SUB_SELECT_QUERY;
    let mode_pred = IRI_OXI_SUB_MODE;
    let handler_pred = IRI_OXI_SUB_HANDLER;
    let label_pred = "http://www.w3.org/2000/01/rdf-schema#label";

    let q = format!(
        "SELECT ?sub ?ask ?select ?mode ?handler ?label FROM <{graph}> WHERE {{ \
            ?sub a <{cls}> . \
            OPTIONAL {{ ?sub <{ask_pred}> ?ask }} \
            OPTIONAL {{ ?sub <{select_pred}> ?select }} \
            OPTIONAL {{ ?sub <{mode_pred}> ?mode }} \
            OPTIONAL {{ ?sub <{handler_pred}> ?handler }} \
            OPTIONAL {{ ?sub <{label_pred}> ?label }} \
         }}"
    );

    let mut out = Vec::new();
    let QueryResults::Solutions(sols) = store.query(q.as_str())? else {
        return Ok(out);
    };
    for sol in sols {
        let sol = sol?;
        let Some(Term::NamedNode(id)) = sol.get("sub").cloned() else {
            continue;
        };
        let id_str = id.as_str().to_string();
        let query = match (sol.get("ask").cloned(), sol.get("select").cloned()) {
            (Some(Term::Literal(lit)), _) => SubscriptionQuery::Ask(lit.value().to_string()),
            (_, Some(Term::Literal(lit))) => SubscriptionQuery::Select(lit.value().to_string()),
            _ => {
                return Err(RegistryError::MalformedPersisted {
                    subscription: id_str,
                    reason: "missing oxi:askQuery and oxi:selectQuery".to_string(),
                });
            }
        };
        let mode = match sol.get("mode").cloned() {
            Some(Term::Literal(lit)) => SubscriptionMode::parse(lit.value()).map_err(|reason| {
                RegistryError::MalformedPersisted {
                    subscription: id_str.clone(),
                    reason,
                }
            })?,
            _ => SubscriptionMode::default(),
        };
        let handler = match sol.get("handler").cloned() {
            Some(Term::Literal(lit)) => Some(DeliveryHandlerRef::new(lit.value().to_string())),
            Some(Term::NamedNode(n)) => Some(DeliveryHandlerRef::new(n.as_str().to_string())),
            _ => None,
        };
        let label = match sol.get("label").cloned() {
            Some(Term::Literal(lit)) => Some(lit.value().to_string()),
            _ => None,
        };
        let triggers = read_persisted_triggers(store, &id)?;
        out.push(Subscription {
            id,
            label,
            query,
            triggers,
            mode,
            handler,
        });
    }
    Ok(out)
}

fn read_persisted_triggers(
    store: &Store,
    sub_id: &NamedNode,
) -> Result<BTreeSet<TriggerType>, RegistryError> {
    let graph = IRI_OXI_GRAPH_SUBSCRIPTIONS;
    let trigger_pred = IRI_OXI_SUB_TRIGGER;
    let q = format!(
        "SELECT ?t FROM <{graph}> WHERE {{ <{sub}> <{trigger_pred}> ?t }}",
        sub = sub_id.as_str()
    );
    let mut out = BTreeSet::new();
    let QueryResults::Solutions(sols) = store.query(q.as_str())? else {
        return Ok(out);
    };
    for sol in sols {
        let sol = sol?;
        if let Some(Term::Literal(lit)) = sol.get("t").cloned() {
            out.insert(lit.value().to_string());
        }
    }
    Ok(out)
}
