//! Cross-subject uniqueness — `(capability_id, version)` unique within `status=active`.

use std::collections::BTreeMap;

use oxrdf::{NamedNode, Quad};

use crate::vocab::{
    CAPABILITY_STATUS_ACTIVE, IRI_DEC_CAPABILITY_ID, IRI_DEC_CAPABILITY_STATUS,
    IRI_DEC_CAPABILITY_VERSION,
};

use super::helpers::{literal_values, violation};
use super::CapabilityViolation;

pub(super) fn check_active_unique(
    quads: &[Quad],
    subjects: &[NamedNode],
) -> Vec<CapabilityViolation> {
    let mut by_key: BTreeMap<(String, String), Vec<NamedNode>> = BTreeMap::new();
    for s in subjects {
        let statuses = literal_values(quads, s, IRI_DEC_CAPABILITY_STATUS);
        if !statuses.iter().any(|v| v == CAPABILITY_STATUS_ACTIVE) {
            continue;
        }
        let ids = literal_values(quads, s, IRI_DEC_CAPABILITY_ID);
        let versions = literal_values(quads, s, IRI_DEC_CAPABILITY_VERSION);
        let Some(id) = ids.first() else { continue };
        let Some(version) = versions.first() else {
            continue;
        };
        by_key
            .entry((id.clone(), version.clone()))
            .or_default()
            .push(s.clone());
    }
    let mut out = Vec::new();
    for ((id, version), nodes) in by_key {
        if nodes.len() <= 1 {
            continue;
        }
        for n in &nodes {
            out.push(violation(
                n,
                IRI_DEC_CAPABILITY_ID,
                &format!(
                    "(capability_id={id:?}, version={version:?}) duplicated among active capabilities ({n_total} total)",
                    n_total = nodes.len(),
                ),
            ));
        }
    }
    out
}
