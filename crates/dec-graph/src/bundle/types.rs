//! In-memory `dec:Bundle` representation per FT-056 / ADR-035.
//!
//! Defines the `Bundle` shape, the `Stakes` enum, the `BundleBuilder`
//! constructor.

use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};
use thiserror::Error;

use dec_ontology::vocab::{
    bundle_class, focal_pred, stakes_pred, IRI_DEC_BUNDLE_PREFIX, STAKES_ELEVATED,
    STAKES_FOUNDATIONAL, STAKES_ROUTINE,
};

pub(super) const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Closed-vocabulary stakes per ADR-035. Default at composition is
/// [`Stakes::Routine`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Stakes {
    /// Standard incremental work within established patterns.
    #[default]
    Routine,
    /// Cross-cutting refactor, schema change, or other elevated blast radius.
    Elevated,
    /// Touches the framework itself or long-lived structures.
    Foundational,
}

impl Stakes {
    /// Stable wire string (`routine` / `elevated` / `foundational`).
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Routine => STAKES_ROUTINE,
            Self::Elevated => STAKES_ELEVATED,
            Self::Foundational => STAKES_FOUNDATIONAL,
        }
    }

    /// Parse a stakes value from its wire string. Returns
    /// [`StakesParseError::Unknown`] for any input outside the closed enum.
    pub fn try_from_str(s: &str) -> Result<Self, StakesParseError> {
        match s {
            STAKES_ROUTINE => Ok(Self::Routine),
            STAKES_ELEVATED => Ok(Self::Elevated),
            STAKES_FOUNDATIONAL => Ok(Self::Foundational),
            other => Err(StakesParseError::Unknown(other.to_string())),
        }
    }
}

/// Failure mode for [`Stakes::try_from_str`].
#[derive(Debug, Error)]
pub enum StakesParseError {
    /// The input literal was not in the ADR-035 closed enum.
    #[error("unknown stakes literal {0:?}; expected one of routine | elevated | foundational")]
    Unknown(String),
}

/// In-memory `dec:Bundle` artifact (FT-056).
///
/// Bundles are immutable after composition; identity is the content hash.
/// The canonical IRI is `https://decision-cli.dev/ns/bundle/<hash>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bundle {
    /// SHA-256 (hex) of the serialised bundle bytes. Identity field.
    pub hash: String,
    /// The focal artifact this bundle was composed around.
    pub focal: NamedNode,
    /// Required stakes per ADR-035 — never `Option`; default is `Routine`.
    pub stakes: Stakes,
}

impl Bundle {
    /// Canonical IRI: `https://decision-cli.dev/ns/bundle/<hash>`.
    #[must_use]
    pub fn iri(&self) -> NamedNode {
        NamedNode::new_unchecked(format!(
            "{prefix}{hash}",
            prefix = IRI_DEC_BUNDLE_PREFIX,
            hash = self.hash,
        ))
    }

    /// Serialise the bundle to RDF quads in the supplied named graph.
    #[must_use]
    pub fn to_quads(&self, graph: NamedNodeRef<'_>) -> Vec<Quad> {
        let g: GraphName = graph.into_owned().into();
        let subject = self.iri();
        let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
        vec![
            Quad::new(subject.clone(), rdf_type, bundle_class(), g.clone()),
            named_quad(&subject, focal_pred(), &self.focal, &g),
            literal_quad(&subject, stakes_pred(), self.stakes.as_str(), &g),
        ]
    }

    /// Markdown-form preamble for worker dispatch (FT-056 §Behaviour bullet 6).
    ///
    /// Bundles are conveyed to workers as markdown. The dispatch metadata
    /// section surfaces stakes so workers and humans reading session logs
    /// see it — the worker contract is not contingent on it.
    #[must_use]
    pub fn dispatch_metadata_markdown(&self) -> String {
        format!(
            "## Dispatch metadata\n\nStakes: {stakes}\n",
            stakes = self.stakes.as_str()
        )
    }
}

/// Builder for [`Bundle`]. Constructs a routine bundle by default; the
/// composer (or the meta-loop) may override via [`BundleBuilder::with_stakes`]
/// before [`BundleBuilder::build`].
#[derive(Debug, Clone)]
pub struct BundleBuilder {
    hash: String,
    focal: NamedNode,
    stakes: Option<Stakes>,
}

impl BundleBuilder {
    /// Open a builder with the bundle's content hash and focal artifact.
    /// Stakes is unset; [`BundleBuilder::build`] will fall back to
    /// [`Stakes::default`] (Routine) unless [`BundleBuilder::with_stakes`]
    /// or a default-ladder consumer overrides it.
    #[must_use]
    pub fn new(hash: impl Into<String>, focal: NamedNode) -> Self {
        Self {
            hash: hash.into(),
            focal,
            stakes: None,
        }
    }

    /// Override the stakes (typically used by the meta-loop or by callers
    /// that have run the default ladder externally).
    #[must_use]
    pub fn with_stakes(mut self, stakes: Stakes) -> Self {
        self.stakes = Some(stakes);
        self
    }

    /// Materialise the [`Bundle`]. Uses [`Stakes::default`] when no override
    /// was supplied.
    #[must_use]
    pub fn build(self) -> Bundle {
        Bundle {
            hash: self.hash,
            focal: self.focal,
            stakes: self.stakes.unwrap_or_default(),
        }
    }
}

pub(super) fn literal_quad(s: &NamedNode, p: NamedNodeRef<'_>, value: &str, g: &GraphName) -> Quad {
    Quad::new(
        s.clone(),
        p.into_owned(),
        Literal::new_simple_literal(value),
        g.clone(),
    )
}

pub(super) fn named_quad(s: &NamedNode, p: NamedNodeRef<'_>, o: &NamedNode, g: &GraphName) -> Quad {
    Quad::new(s.clone(), p.into_owned(), o.clone(), g.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_routine() {
        assert_eq!(Stakes::default(), Stakes::Routine);
    }

    #[test]
    fn as_str_round_trips() {
        for stakes in [Stakes::Routine, Stakes::Elevated, Stakes::Foundational] {
            let s = stakes.as_str();
            assert_eq!(Stakes::try_from_str(s).expect("parse"), stakes);
        }
    }

    #[test]
    fn parse_rejects_unknown_value() {
        let err = Stakes::try_from_str("critical").expect_err("must reject");
        assert!(format!("{err}").contains("critical"));
    }

    #[test]
    fn builder_default_yields_routine_bundle() {
        let b = BundleBuilder::new(
            "abc123",
            NamedNode::new("https://example.com/focal").expect("focal iri"),
        )
        .build();
        assert_eq!(b.stakes, Stakes::Routine);
        assert_eq!(b.hash, "abc123");
        assert_eq!(
            b.iri().as_str(),
            "https://decision-cli.dev/ns/bundle/abc123"
        );
    }

    #[test]
    fn builder_with_stakes_overrides() {
        let b = BundleBuilder::new(
            "abc123",
            NamedNode::new("https://example.com/focal").expect("focal iri"),
        )
        .with_stakes(Stakes::Foundational)
        .build();
        assert_eq!(b.stakes, Stakes::Foundational);
    }

    #[test]
    fn dispatch_metadata_markdown_includes_stakes() {
        let b = BundleBuilder::new(
            "deadbeef",
            NamedNode::new("https://example.com/focal").expect("focal iri"),
        )
        .with_stakes(Stakes::Elevated)
        .build();
        let md = b.dispatch_metadata_markdown();
        assert!(md.contains("Stakes: elevated"));
        assert!(md.contains("## Dispatch metadata"));
    }
}
