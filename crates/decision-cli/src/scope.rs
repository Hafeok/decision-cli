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
///
/// Loaded once at process start (ADR-012) from the persisted N-Quads
/// dump produced by [`crate::init::run`]. The authorized-goals list is
/// a property of the persisted artifact — there is no `--stream` or
/// `--authorized-goals` runtime override (ADR-005).
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
    ///
    /// The display message matches the shape from
    /// `decision-cli-slice-1-bounds.md` §3.4 so operators see a
    /// reproducible, structured refusal. The `Display` impl renders the
    /// terminal ValueAction in its prefixed form (`va:<local>`) when it
    /// belongs to the bundled URI namespace; bare IRIs fall through.
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
    ///
    /// Reserved for the writer-middleware path; surfaced here so the
    /// vocabulary is consistent with §3.4 even though FT-010's
    /// [`crate::StreamWriter`] does not currently emit it.
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
    ///
    /// Returns [`ScopeError::Uninitialized`] when no `.dec/` is present,
    /// and structured errors for store / artifact corruption.
    pub fn load(workdir: &Path) -> Result<Self, ScopeError> {
        let dec_dir = workdir.join(".dec");
        if !dec_dir.exists() {
            return Err(ScopeError::Uninitialized { path: dec_dir });
        }
        let dump_path = dec_dir.join("store").join("orchestration.nq");
        let bytes = std::fs::read(&dump_path).map_err(|e| ScopeError::StoreUnreadable {
            path: dump_path.clone(),
            detail: e.to_string(),
        })?;
        let store = Store::new().map_err(|e| ScopeError::StoreUnreadable {
            path: dump_path.clone(),
            detail: e.to_string(),
        })?;
        store
            .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
            .map_err(|e| ScopeError::StoreUnreadable {
                path: dump_path.clone(),
                detail: e.to_string(),
            })?;

        // Stream + ValueAction in one query (the persisted ValueStream
        // must reference exactly one terminal ValueAction per ADR-006).
        let stream_q = format!(
            "PREFIX dec: <{ns}>
SELECT ?stream ?action ?name WHERE {{
  ?stream a dec:ValueStream ;
          dec:terminalValueAction ?action .
  OPTIONAL {{ ?stream dec:name ?name }}
}} LIMIT 1",
            ns = DEC_NS
        );

        let (stream_iri, stream_name, value_action_iri) = match store.query(stream_q.as_str()) {
            Ok(QueryResults::Solutions(mut sols)) => {
                let Some(first) = sols.next() else {
                    return Err(ScopeError::MissingValueStream { path: dump_path });
                };
                let sol = first.map_err(|e| ScopeError::StoreUnreadable {
                    path: dump_path.clone(),
                    detail: e.to_string(),
                })?;
                let stream = match sol.get("stream") {
                    Some(oxigraph::model::Term::NamedNode(n)) => n.as_str().to_string(),
                    _ => return Err(ScopeError::MissingValueStream { path: dump_path }),
                };
                let action = match sol.get("action") {
                    Some(oxigraph::model::Term::NamedNode(n)) => n.as_str().to_string(),
                    _ => return Err(ScopeError::MissingValueStream { path: dump_path }),
                };
                let name = sol.get("name").and_then(|t| match t {
                    oxigraph::model::Term::Literal(lit) => Some(lit.value().to_string()),
                    _ => None,
                });
                (stream, name, action)
            }
            Ok(_) | Err(_) => {
                return Err(ScopeError::MissingValueStream { path: dump_path });
            }
        };

        // Authorized-goals (slice 1 persists them as repeated literal triples).
        let goals_q = format!(
            "PREFIX dec: <{ns}>
SELECT ?goal WHERE {{ <{stream}> dec:authorizedGoals ?goal }}",
            ns = DEC_NS,
            stream = stream_iri,
        );
        let mut authorized: Vec<String> = Vec::new();
        if let Ok(QueryResults::Solutions(sols)) = store.query(goals_q.as_str()) {
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

        Ok(Self {
            stream_iri,
            stream_name,
            value_action_iri,
            authorized_goals: authorized,
        })
    }

    /// Verify that `goal` appears in the active stream's authorized list.
    ///
    /// This is the chokepoint per ADR-005 / §3.4: a refusal here happens
    /// **before** any Session / Goal / Dispatch is written. Callers that
    /// invoke dispatch (`dec implement`, future `dec drive`) must call
    /// this first and abort on `Err`.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundled;
    use crate::init::{self, DefinitionSource};

    fn init_dev_stream(workdir: &Path) {
        init::run(
            workdir,
            DefinitionSource::Template("engineering-development".to_string()),
        )
        .expect("init succeeds");
    }

    #[test]
    fn load_after_init_caches_authorized_goals() {
        let tmp = tempdir();
        init_dev_stream(tmp.path());
        let scope = ActiveScope::load(tmp.path()).expect("scope loads");
        assert!(scope.authorized_goals.contains(&"ship".to_string()));
        assert!(scope.authorized_goals.contains(&"land".to_string()));
        assert_eq!(
            scope.value_action_iri,
            bundled::SHIPPED_FEATURE_IRI.to_string()
        );
    }

    #[test]
    fn authorized_goal_passes() {
        let tmp = tempdir();
        init_dev_stream(tmp.path());
        let scope = ActiveScope::load(tmp.path()).expect("scope loads");
        assert!(scope.validate_goal("ship").is_ok());
        assert!(scope.validate_goal("land").is_ok());
    }

    #[test]
    fn unauthorized_goal_is_refused_with_full_diagnostic() {
        let tmp = tempdir();
        init_dev_stream(tmp.path());
        let scope = ActiveScope::load(tmp.path()).expect("scope loads");
        let err = scope.validate_goal("prioritize").expect_err("refused");
        let msg = err.to_string();
        assert!(msg.contains("prioritize"), "names goal: {msg}");
        assert!(msg.contains("ship"), "names authorized goal ship: {msg}");
        assert!(msg.contains("land"), "names authorized goal land: {msg}");
        assert!(
            msg.contains("va:shipped-feature"),
            "names ValueAction in prefixed form: {msg}"
        );
        assert!(
            msg.contains(bundled::SHIPPED_FEATURE_IRI),
            "names ValueAction full IRI: {msg}"
        );
        assert!(
            msg.contains("This stream pursues"),
            "matches §3.4 phrasing: {msg}"
        );
    }

    #[test]
    fn uninitialized_workdir_errors_clearly() {
        let tmp = tempdir();
        let err = ActiveScope::load(tmp.path()).expect_err("uninitialised");
        assert!(matches!(err, ScopeError::Uninitialized { .. }));
    }

    /// Tiny tempdir helper so we don't drag in `tempfile` as a dep
    /// for a single test module.
    fn tempdir() -> TempDir {
        let mut p = std::env::temp_dir();
        let pid = std::process::id();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        p.push(format!("dec-scope-test-{pid}-{nonce}"));
        std::fs::create_dir_all(&p).expect("create tempdir");
        TempDir { path: p }
    }

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}
