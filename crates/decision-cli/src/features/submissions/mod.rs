//! `POST /submissions` HTTP endpoint on pipeline-cli (FT-094).
//!
//! Producer-side CI posts a `dec:WorkerImageSubmission` payload here
//! after a worker repo's release pipeline assembles its OCI artifact,
//! SBOM referrer URI, and sigstore identity. The endpoint:
//!
//! 1. Authenticates the Bearer token against the in-memory
//!    [`TokenStore`], resolving the calling repo's identity.
//! 2. Refuses the request if the declared source repo doesn't match the
//!    token's bound identity (403).
//! 3. Constructs a `WorkerImageSubmission` artifact in the `received`
//!    lifecycle state and validates it against the FT-087 SHACL shape
//!    plus the FT-071 `BoundaryArtifact` escape-hatch shape (422 on any
//!    violation).
//! 4. Commits the Submission's RDF quads into the orchestration store
//!    through the FT-001 `GraphWriter` chokepoint, attaching a stable
//!    cause label so the mutation is traceable.
//! 5. Returns the canonical Submission IRI plus the synthetic
//!    `dispatch event id` that the `WorkerCurator` dispatch subscription
//!    (FT-092) will consume.
//!
//! Authentication, identity resolution, and rate-limiting are slice-1
//! shapes only: tokens are long-lived strings hashed into the token
//! store at startup; rate limiting is a token-bucket on a per-repo
//! key. Token rotation, multi-tenant scoping, and submission-level
//! idempotency are explicitly out of scope per the `feature_spec`.

pub mod auth;
pub mod handler;
pub mod payload;
pub mod rate_limit;

#[cfg(test)]
mod tests;

pub use auth::{RepoIdentity, TokenStore, TokenStoreError};
pub use handler::{
    router, AppState, SubmissionAccepted, SubmissionRejection, SubmissionsService,
    SubmissionsServiceError,
};
pub use payload::SubmissionPayload;
pub use rate_limit::{RateLimitConfig, RateLimiter, DEFAULT_RATE_LIMIT_PER_MINUTE};
