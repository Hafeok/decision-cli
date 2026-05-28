//! Read persisted subscriptions back from the `subscriptions` named graph.

use std::collections::BTreeSet;

use oxigraph::model::{NamedNode, Term};
use oxigraph::sparql::{QueryResults, QuerySolution};
use oxigraph::store::Store;

use crate::error::RegistryError;
use crate::vocab::{
    IRI_OXI_GRAPH_SUBSCRIPTIONS, IRI_OXI_SUBSCRIPTION, IRI_OXI_SUB_ASK_QUERY, IRI_OXI_SUB_HANDLER,
    IRI_OXI_SUB_MODE, IRI_OXI_SUB_SELECT_QUERY, IRI_OXI_SUB_TRIGGER,
};

use super::types::{
    DeliveryHandlerRef, Subscription, SubscriptionMode, SubscriptionQuery, TriggerType,
};

const RDFS_LABEL_IRI: &str = "http://www.w3.org/2000/01/rdf-schema#label";

pub(super) fn read_persisted_subscriptions(
    store: &Store,
) -> Result<Vec<Subscription>, RegistryError> {
    let q = build_subscription_select();
    let mut out = Vec::new();
    let QueryResults::Solutions(sols) = store.query(q.as_str())? else {
        return Ok(out);
    };
    for sol in sols {
        let sol = sol?;
        if let Some(sub) = decode_subscription_row(store, &sol)? {
            out.push(sub);
        }
    }
    Ok(out)
}

fn build_subscription_select() -> String {
    format!(
        "SELECT ?sub ?ask ?select ?mode ?handler ?label FROM <{graph}> WHERE {{ \
            ?sub a <{cls}> . \
            OPTIONAL {{ ?sub <{ask_pred}> ?ask }} \
            OPTIONAL {{ ?sub <{select_pred}> ?select }} \
            OPTIONAL {{ ?sub <{mode_pred}> ?mode }} \
            OPTIONAL {{ ?sub <{handler_pred}> ?handler }} \
            OPTIONAL {{ ?sub <{label_pred}> ?label }} \
         }}",
        graph = IRI_OXI_GRAPH_SUBSCRIPTIONS,
        cls = IRI_OXI_SUBSCRIPTION,
        ask_pred = IRI_OXI_SUB_ASK_QUERY,
        select_pred = IRI_OXI_SUB_SELECT_QUERY,
        mode_pred = IRI_OXI_SUB_MODE,
        handler_pred = IRI_OXI_SUB_HANDLER,
        label_pred = RDFS_LABEL_IRI,
    )
}

fn decode_subscription_row(
    store: &Store,
    sol: &QuerySolution,
) -> Result<Option<Subscription>, RegistryError> {
    let Some(Term::NamedNode(id)) = sol.get("sub").cloned() else {
        return Ok(None);
    };
    let id_str = id.as_str().to_string();
    let query = decode_subscription_query(sol, &id_str)?;
    let mode = decode_subscription_mode(sol, &id_str)?;
    let handler = decode_subscription_handler(sol);
    let label = decode_subscription_label(sol);
    let triggers = read_persisted_triggers(store, &id)?;
    Ok(Some(Subscription {
        id,
        label,
        query,
        triggers,
        mode,
        handler,
    }))
}

fn decode_subscription_query(
    sol: &QuerySolution,
    id_str: &str,
) -> Result<SubscriptionQuery, RegistryError> {
    match (sol.get("ask").cloned(), sol.get("select").cloned()) {
        (Some(Term::Literal(lit)), _) => Ok(SubscriptionQuery::Ask(lit.value().to_string())),
        (_, Some(Term::Literal(lit))) => Ok(SubscriptionQuery::Select(lit.value().to_string())),
        _ => Err(RegistryError::MalformedPersisted {
            subscription: id_str.to_string(),
            reason: "missing oxi:askQuery and oxi:selectQuery".to_string(),
        }),
    }
}

fn decode_subscription_mode(
    sol: &QuerySolution,
    id_str: &str,
) -> Result<SubscriptionMode, RegistryError> {
    match sol.get("mode").cloned() {
        Some(Term::Literal(lit)) => SubscriptionMode::parse(lit.value()).map_err(|reason| {
            RegistryError::MalformedPersisted {
                subscription: id_str.to_string(),
                reason,
            }
        }),
        _ => Ok(SubscriptionMode::default()),
    }
}

fn decode_subscription_handler(sol: &QuerySolution) -> Option<DeliveryHandlerRef> {
    match sol.get("handler").cloned() {
        Some(Term::Literal(lit)) => Some(DeliveryHandlerRef::new(lit.value().to_string())),
        Some(Term::NamedNode(n)) => Some(DeliveryHandlerRef::new(n.as_str().to_string())),
        _ => None,
    }
}

fn decode_subscription_label(sol: &QuerySolution) -> Option<String> {
    match sol.get("label").cloned() {
        Some(Term::Literal(lit)) => Some(lit.value().to_string()),
        _ => None,
    }
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
