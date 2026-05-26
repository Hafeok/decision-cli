//! In-memory shape of `worker.toml` (FT-093).
//!
//! Mirrors the declarative TOML the FT-093 feature_spec defines:
//!
//! ```toml
//! [worker]
//! name = "implementer"
//! sdk_version = "0.3.0"
//! wire_protocol = "1.0"
//!
//! [capabilities]
//! tags = ["code-writer", "frontier-reasoning"]
//! compatible_roles = ["engineering.implementer"]
//!
//! [runtime]
//! kind = "subscribed"     # vs "invoked" if Dagger lands later
//! entrypoint = "implementer.main:run"
//! ```
//!
//! No defaults are baked into the parser — every required field MUST be
//! present in the file. The single exception is [`WorkerSection::wire_protocol`],
//! which falls back to [`DEFAULT_WIRE_PROTOCOL_VERSION`] when omitted so
//! older worker manifests authored before the wire-protocol pin landed
//! continue to load.

/// Default wire-protocol version when `[worker].wire_protocol` is omitted
/// from the manifest. Matches the SSE/POST contract version slice-1
/// workers speak (per `feature:manual-runtime-stance`).
pub const DEFAULT_WIRE_PROTOCOL_VERSION: &str = "1.0";

/// `[worker]` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerSection {
    /// Short worker identifier — drives the path-filtered release
    /// trigger in monorepo layouts (`workers/<name>/**`) and the scoped
    /// semver tag prefix (`<name>-v<semver>`).
    pub name: String,
    /// Worker SDK version baked into the image. Surfaces as the
    /// `ddd.sdk-version` OCI label per FT-088.
    pub sdk_version: String,
    /// SSE/POST wire-protocol version baked into the image. Surfaces as
    /// the `ddd.wire-protocol` OCI label per FT-088. Defaults to
    /// [`DEFAULT_WIRE_PROTOCOL_VERSION`] when omitted.
    pub wire_protocol: String,
}

/// `[capabilities]` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    /// Capability-tag claims. Each tag becomes a separate
    /// `ddd.capability-tag.<tag>=true` OCI label per ADR-057 and a
    /// `dec:claimed_capability_tag` literal on the Submission.
    pub tags: Vec<String>,
    /// `dec:Role` identifiers the worker claims compatibility with.
    /// Empty is permitted in slice 1 (the WorkerCurator decides
    /// bindings); the assembler still threads the empty list onto
    /// the Submission.
    pub compatible_roles: Vec<String>,
}

/// Worker runtime kind. Slice 1 supports only the long-running
/// subscribed shape; `invoked` is reserved for a future Dagger landing
/// (per ADR-065). An unknown value is refused at parse time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind {
    /// Long-running worker that opens an SSE connection to pipeline-cli
    /// on start and POSTs artifacts back over HTTP — the slice-1 shape.
    Subscribed,
    /// Stateless RPC-style worker invoked per dispatch. Reserved for
    /// a Dagger landing per ADR-065.
    Invoked,
}

impl RuntimeKind {
    /// Stable wire string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Subscribed => "subscribed",
            Self::Invoked => "invoked",
        }
    }

    /// Parse the wire string. Returns `None` for any unrecognised value
    /// so the manifest parser can surface the typo with its key path.
    #[must_use]
    pub fn try_from_str(s: &str) -> Option<Self> {
        match s {
            "subscribed" => Some(Self::Subscribed),
            "invoked" => Some(Self::Invoked),
            _ => None,
        }
    }
}

/// `[runtime]` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSpec {
    /// Runtime shape. See [`RuntimeKind`].
    pub kind: RuntimeKind,
    /// Module / function reference the container entrypoint invokes.
    /// Format is worker-language-specific; the parser stores it
    /// verbatim and the workflow consumes it when generating the
    /// container's CMD.
    pub entrypoint: String,
}

/// Parsed `worker.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerManifest {
    /// `[worker]` table.
    pub worker: WorkerSection,
    /// `[capabilities]` table.
    pub capabilities: Capabilities,
    /// `[runtime]` table.
    pub runtime: RuntimeSpec,
}

impl WorkerManifest {
    /// Stable scoped-tag prefix per FT-093 ("monorepo with scoped semver
    /// tags `implementer-v1.2.0`"). Used by the release workflow's tag
    /// filter and by the assembler to derive the build run URL when the
    /// caller does not provide one explicitly.
    #[must_use]
    pub fn tag_prefix(&self) -> String {
        format!("{name}-v", name = self.worker.name)
    }
}
