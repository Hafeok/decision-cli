//! FT-036 / ADR-028 — verification-graph artifact vocabulary.
//!
//! Split out of `core::vocab` to keep per-file size within the ADR-013
//! 400-line ceiling. Re-exported from the parent module so callers
//! continue to import from `decision_cli::vocab`.

#![allow(missing_docs)]

use oxrdf::NamedNodeRef;

// --- Class IRIs --------------------------------------------------------------

/// Class IRI for `dec:VerificationGraph` (ADR-028 §VerificationGraph).
pub const IRI_DEC_VERIFICATION_GRAPH: &str = "https://decision-cli.dev/ns#VerificationGraph";

/// Class IRI for `dec:VerificationStep` (ADR-028 §VerificationGraph).
pub const IRI_DEC_VERIFICATION_STEP: &str = "https://decision-cli.dev/ns#VerificationStep";

// --- Graph-level structural predicates --------------------------------------

/// `dec:verifies` predicate — graph → feature or TC (polymorphic).
pub const IRI_DEC_VERIFIES: &str = "https://decision-cli.dev/ns#verifies";

/// `dec:bench` predicate — graph → VerificationBench (renamed from environment by FT-112).
pub const IRI_DEC_BENCH: &str = "https://decision-cli.dev/ns#bench";

/// `dec:steps` predicate — graph → rdf:List head of ordered steps.
pub const IRI_DEC_STEPS: &str = "https://decision-cli.dev/ns#steps";

// --- Step-level structural predicates ---------------------------------------

/// `dec:stepType` predicate — step → kind tag literal.
pub const IRI_DEC_STEP_TYPE: &str = "https://decision-cli.dev/ns#stepType";

/// `dec:requiredOps` predicate — declared on the step (rdf:List); consumed
/// by FT-037 (safety enforcement).
pub const IRI_DEC_REQUIRED_OPS: &str = "https://decision-cli.dev/ns#requiredOps";

/// `dec:providesEvidenceFor` predicate — step → TC IRI; optional, multi-valued.
pub const IRI_DEC_PROVIDES_EVIDENCE_FOR: &str = "https://decision-cli.dev/ns#providesEvidenceFor";

// --- Per-step-kind field predicates -----------------------------------------

/// `dec:command` predicate — shell-command step body.
pub const IRI_DEC_COMMAND: &str = "https://decision-cli.dev/ns#command";

/// `dec:expectExitCode` predicate — shell-command exit-code expectation.
pub const IRI_DEC_EXPECT_EXIT_CODE: &str = "https://decision-cli.dev/ns#expectExitCode";

/// `dec:captureOutput` predicate — shell-command bind-stdout flag.
pub const IRI_DEC_CAPTURE_OUTPUT: &str = "https://decision-cli.dev/ns#captureOutput";

/// `dec:target` predicate — sparql-assertion target store or endpoint.
pub const IRI_DEC_TARGET: &str = "https://decision-cli.dev/ns#target";

/// `dec:query` predicate — sparql-assertion query text.
pub const IRI_DEC_QUERY: &str = "https://decision-cli.dev/ns#query";

/// `dec:expectRows` predicate — sparql-assertion row-count expectation.
pub const IRI_DEC_EXPECT_ROWS: &str = "https://decision-cli.dev/ns#expectRows";

/// `dec:path` predicate — file-assertion target path.
pub const IRI_DEC_PATH: &str = "https://decision-cli.dev/ns#path";

/// `dec:expectHash` predicate — file-assertion hash expectation.
pub const IRI_DEC_EXPECT_HASH: &str = "https://decision-cli.dev/ns#expectHash";

/// `dec:expectContent` predicate — file-assertion content expectation.
pub const IRI_DEC_EXPECT_CONTENT: &str = "https://decision-cli.dev/ns#expectContent";

/// `dec:method` predicate — http-request method.
pub const IRI_DEC_METHOD: &str = "https://decision-cli.dev/ns#method";

/// `dec:url` predicate — http-request target URL.
pub const IRI_DEC_URL: &str = "https://decision-cli.dev/ns#url";

/// `dec:expectStatus` predicate — http-request status-code expectation.
pub const IRI_DEC_EXPECT_STATUS: &str = "https://decision-cli.dev/ns#expectStatus";

/// `dec:condition` predicate — wait-for sub-condition reference.
pub const IRI_DEC_CONDITION: &str = "https://decision-cli.dev/ns#condition";

/// `dec:timeout` predicate — wait-for timeout (ISO 8601 duration).
pub const IRI_DEC_TIMEOUT: &str = "https://decision-cli.dev/ns#timeout";

/// `dec:fromStep` predicate — capture step source step reference.
pub const IRI_DEC_FROM_STEP: &str = "https://decision-cli.dev/ns#fromStep";

/// `dec:bindAs` predicate — capture step binding name.
pub const IRI_DEC_BIND_AS: &str = "https://decision-cli.dev/ns#bindAs";

// --- Named graph + IRI prefixes ---------------------------------------------

/// Named graph holding verification-graph projections (ADR-028 §State).
pub const IRI_DEC_GRAPH_VERIFY_GRAPH: &str = "https://decision-cli.dev/ns/graph/verify-graph";

/// IRI prefix for minted graph IRIs (`https://decision-cli.dev/ns/graph/<id>`).
pub const IRI_DEC_VERIFY_GRAPH_PREFIX: &str = "https://decision-cli.dev/ns/graph/";

/// IRI prefix for step IRIs (`https://decision-cli.dev/ns/step/<graph-id>/<index>`).
pub const IRI_DEC_STEP_PREFIX: &str = "https://decision-cli.dev/ns/step/";

// --- Step-kind literal tokens -----------------------------------------------

/// Seed step kind: shell-command.
pub const STEP_KIND_SHELL_COMMAND: &str = "shell-command";
/// Seed step kind: sparql-assertion.
pub const STEP_KIND_SPARQL_ASSERTION: &str = "sparql-assertion";
/// Seed step kind: file-assertion.
pub const STEP_KIND_FILE_ASSERTION: &str = "file-assertion";
/// Seed step kind: http-request.
pub const STEP_KIND_HTTP_REQUEST: &str = "http-request";
/// Seed step kind: wait-for.
pub const STEP_KIND_WAIT_FOR: &str = "wait-for";
/// Seed step kind: capture.
pub const STEP_KIND_CAPTURE: &str = "capture";

/// All seed step kinds in declaration order. Consumed by the SHACL
/// shape (sh:in) and the step parser.
pub const SEED_STEP_KINDS: &[&str] = &[
    STEP_KIND_SHELL_COMMAND,
    STEP_KIND_SPARQL_ASSERTION,
    STEP_KIND_FILE_ASSERTION,
    STEP_KIND_HTTP_REQUEST,
    STEP_KIND_WAIT_FOR,
    STEP_KIND_CAPTURE,
];

// --- NamedNodeRef helpers ---------------------------------------------------

#[must_use]
pub fn verification_graph_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_VERIFICATION_GRAPH)
}

#[must_use]
pub fn verification_step_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_VERIFICATION_STEP)
}

#[must_use]
pub fn verifies_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_VERIFIES)
}

#[must_use]
pub fn bench_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_BENCH)
}

#[must_use]
pub fn steps_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_STEPS)
}

#[must_use]
pub fn step_type_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_STEP_TYPE)
}

#[must_use]
pub fn required_ops_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_REQUIRED_OPS)
}

#[must_use]
pub fn provides_evidence_for_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_PROVIDES_EVIDENCE_FOR)
}

#[must_use]
pub fn verify_graph_named_graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_GRAPH_VERIFY_GRAPH)
}
