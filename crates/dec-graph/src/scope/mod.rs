//! Value-stream scope enforcement at command time (FT-010 / ADR-005).
//!
//! Every goal-driven `dec` invocation runs through [`ActiveScope`]
//! **before any role dispatches**:
//!
//! 1. The active [`ValueStream`](crate::vocab::IRI_DEC_VALUE_STREAM) is
//!    discovered from the working dir's persisted orchestration store
//!    (FT-009; per ADR-012 the directory is the identity).
//! 2. The cached authorized-goals set is consulted (a property of the
//!    persisted ValueStream artifact, not a runtime flag — ADR-005).
//! 3. An unauthorized goal verb produces a structured
//!    [`ScopeError::UnauthorizedGoal`] naming the goal, the authorized
//!    list, and the terminal ValueAction URI; no Session / Goal /
//!    Dispatch is ever written.
//!
//! Slice 1 has no `dec drive` (ADR-010 / ADR-011 / §6.2) — the same
//! enforcement gate is exposed through the hidden `_check-goal`
//! subcommand so TC-007 can exercise it without the higher-level
//! verb. When `dec drive` and `dec implement <goal>` land, they call
//! [`ActiveScope::validate_goal`] before constructing the Dispatch.

use std::path::{Path, PathBuf};

use oxigraph::io::RdfFormat;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use thiserror::Error;

const DEC_NS: &str = "https://decision-cli.dev/ns#";
const VA_BUNDLED_PREFIX: &str = "https://decision-cli.dev/ns/value-actions/";

/// A snapshot of the working dir's active value stream.
#[derive(Debug, Clone)]
pub struct ActiveScope {
    /// The ValueStream IRI persisted at init time.
    pub stream_iri: String,
    /// Optional `dec:name` literal for human-facing output.
    pub stream_name: Option<String>,
    /// The terminal ValueAction IRI (full form).
    pub value_action_iri: String,
    /// Authorized-goals literals copied off the persisted ValueStream
    /// artifact, in lexicographic order.
    pub authorized_goals: Vec<String>,
}

/// Structured scope-enforcement errors (ADR-005, §3.4).
#[derive(Debug, Error)]
pub enum ScopeError {
    /// No `.dec/` directory in the working dir — `dec init` first.
    #[error(
        "no orchestration store at {path}; run `dec init --template engineering-development` \
         (or `dec init --from <path>`) first"
    )]
    Uninitialized {
        /// Path the loader looked at.
        path: PathBuf,
    },

    /// The persisted store is unreadable / lacks a ValueStream artifact.
    #[error("failed to read orchestration store at {path}: {detail}")]
    StoreUnreadable {
        /// Path the loader looked at.
        path: PathBuf,
        /// Underlying error detail.
        detail: String,
    },

    /// The store opened but no ValueStream is persisted.
    #[error("orchestration store at {path} contains no dec:ValueStream artifact")]
    MissingValueStream {
        /// Path the loader looked at.
        path: PathBuf,
    },

    /// A goal verb is not in the stream's authorized-goals list.
    #[error(
        "This stream pursues `{value_action_display}`; `{goal}` is not an authorized goal \
         — try a stream with Discovery scope. Authorized goals: {authorized_display}. \
         ValueAction: <{value_action_iri}>."
    )]
    UnauthorizedGoal {
        /// The unauthorized goal verb.
        goal: String,
        /// Stream's authorized-goals list, joined as `a, b, c`.
        authorized_display: String,
        /// Raw authorized goals (so callers can format their own message).
        authorized: Vec<String>,
        /// Terminal ValueAction in prefixed form (e.g., `va:shipped-feature`).
        value_action_display: String,
        /// Terminal ValueAction full IRI.
        value_action_iri: String,
    },

    /// A caller tried to insert into a stream that is not the active one.
    #[error(
        "foreign-stream artifact insertion refused: active stream is <{active}>, but a write \
         supplied stream <{supplied}>"
    )]
    ForeignStream {
        /// Active stream IRI.
        active: String,
        /// Foreign stream IRI the caller tried to write.
        supplied: String,
    },
}

impl ActiveScope {
    /// Discover and load the active stream from `<workdir>/.dec/`.
    pub fn load(workdir: &Path) -> Result<Self, ScopeError> {
        let dec_dir = workdir.join(".dec");
        if !dec_dir.exists() {
            return Err(ScopeError::Uninitialized { path: dec_dir });
        }
        let dump_path = dec_dir.join("store").join("orchestration.nq");
        let store = load_store(&dump_path)?;
        let (stream_iri, stream_name, value_action_iri) = read_stream_record(&store, &dump_path)?;
        let authorized_goals = read_authorized_goals(&store, &stream_iri);

        Ok(Self {
            stream_iri,
            stream_name,
            value_action_iri,
            authorized_goals,
        })
    }

    /// Verify that `goal` appears in the active stream's authorized list.
    pub fn validate_goal(&self, goal: &str) -> Result<(), ScopeError> {
        if self.authorized_goals.iter().any(|g| g == goal) {
            return Ok(());
        }
        Err(ScopeError::UnauthorizedGoal {
            goal: goal.to_string(),
            authorized_display: self.authorized_goals.join(", "),
            authorized: self.authorized_goals.clone(),
            value_action_display: shorten_value_action(&self.value_action_iri),
            value_action_iri: self.value_action_iri.clone(),
        })
    }
}

fn load_store(dump_path: &Path) -> Result<Store, ScopeError> {
    let bytes = std::fs::read(dump_path).map_err(|e| ScopeError::StoreUnreadable {
        path: dump_path.to_path_buf(),
        detail: e.to_string(),
    })?;
    let store = Store::new().map_err(|e| ScopeError::StoreUnreadable {
        path: dump_path.to_path_buf(),
        detail: e.to_string(),
    })?;
    store
        .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
        .map_err(|e| ScopeError::StoreUnreadable {
            path: dump_path.to_path_buf(),
            detail: e.to_string(),
        })?;
    Ok(store)
}

fn read_stream_record(
    store: &Store,
    dump_path: &Path,
) -> Result<(String, Option<String>, String), ScopeError> {
    let q = format!(
        "PREFIX dec: <{ns}>
SELECT ?stream ?action ?name WHERE {{
  ?stream a dec:ValueStream ;
          dec:terminalValueAction ?action .
  OPTIONAL {{ ?stream dec:name ?name }}
}} LIMIT 1",
        ns = DEC_NS
    );
    match store.query(q.as_str()) {
        Ok(QueryResults::Solutions(mut sols)) => {
            let Some(first) = sols.next() else {
                return Err(ScopeError::MissingValueStream {
                    path: dump_path.to_path_buf(),
                });
            };
            let sol = first.map_err(|e| ScopeError::StoreUnreadable {
                path: dump_path.to_path_buf(),
                detail: e.to_string(),
            })?;
            let stream = match sol.get("stream") {
                Some(oxigraph::model::Term::NamedNode(n)) => n.as_str().to_string(),
                _ => {
                    return Err(ScopeError::MissingValueStream {
                        path: dump_path.to_path_buf(),
                    })
                }
            };
            let action = match sol.get("action") {
                Some(oxigraph::model::Term::NamedNode(n)) => n.as_str().to_string(),
                _ => {
                    return Err(ScopeError::MissingValueStream {
                        path: dump_path.to_path_buf(),
                    })
                }
            };
            let name = sol.get("name").and_then(|t| match t {
                oxigraph::model::Term::Literal(lit) => Some(lit.value().to_string()),
                _ => None,
            });
            Ok((stream, name, action))
        }
        _ => Err(ScopeError::MissingValueStream {
            path: dump_path.to_path_buf(),
        }),
    }
}

fn read_authorized_goals(store: &Store, stream_iri: &str) -> Vec<String> {
    let q = format!(
        "PREFIX dec: <{ns}>
SELECT ?goal WHERE {{ <{stream}> dec:authorizedGoals ?goal }}",
        ns = DEC_NS,
        stream = stream_iri,
    );
    let mut authorized: Vec<String> = Vec::new();
    if let Ok(QueryResults::Solutions(sols)) = store.query(q.as_str()) {
        for sol in sols.flatten() {
            if let Some(oxigraph::model::Term::Literal(lit)) = sol.get("goal") {
                let v = lit.value().to_string();
                if !v.is_empty() {
                    authorized.push(v);
                }
            }
        }
    }
    authorized.sort();
    authorized.dedup();
    authorized
}

/// Render a ValueAction IRI in its prefixed display form.
///
/// Bundled URIs under [`VA_BUNDLED_PREFIX`] shorten to `va:<local>`;
/// other IRIs are echoed verbatim so custom registries don't lose info.
#[must_use]
pub fn shorten_value_action(iri: &str) -> String {
    if let Some(local) = iri.strip_prefix(VA_BUNDLED_PREFIX) {
        format!("va:{local}")
    } else {
        iri.to_string()
    }
}
