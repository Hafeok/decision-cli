//! In-memory `dec:RoleBinding`, `dec:EscalationStep`, `dec:EscalationTrigger`
//! shapes plus their RDF serialisation (FT-055 / ADR-033 / ADR-034).

use oxrdf::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

use crate::vocab::{
    bootstrap_source_pred, capability_supersedes_pred, capability_version_pred,
    default_capability_pred, escalation_step_class, escalation_steps_pred,
    escalation_trigger_class, role_binding_active_pred, role_binding_class,
    role_binding_role_id_pred, step_capability_pred, trigger_signal_pred, triggers_pred,
    IRI_DEC_ROLE_BINDING_PREFIX, TRIGGER_AUDIT_FAIL, TRIGGER_AUDIT_PASS,
    TRIGGER_CONFIDENCE_BELOW_05, TRIGGER_CONFIDENCE_BELOW_07, TRIGGER_CONFIDENCE_BELOW_09,
    TRIGGER_FEEDBACK_CONTRADICTION, TRIGGER_FEEDBACK_GAP, TRIGGER_FEEDBACK_SCOPE_ISSUE,
    TRIGGER_FEEDBACK_UNIMPLEMENTABLE_CRITICAL, TRIGGER_PRIOR_ATTEMPTS_GE_1,
    TRIGGER_PRIOR_ATTEMPTS_GE_2, TRIGGER_PRIOR_ATTEMPTS_GE_3, TRIGGER_PRIOR_ATTEMPTS_GE_4,
    TRIGGER_PRIOR_ATTEMPTS_GE_5, TRIGGER_STAKES_ELEVATED, TRIGGER_STAKES_FOUNDATIONAL,
    TRIGGER_STAKES_ROUTINE,
};

/// IRI of `rdf:type`.
pub const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
/// IRI of `rdf:first` (RDF collection head).
pub const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
/// IRI of `rdf:rest` (RDF collection tail).
pub const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
/// IRI of `rdf:nil` (RDF collection terminator).
pub const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";
/// IRI of the `xsd:integer` datatype.
pub const XSD_INTEGER: &str = "http://www.w3.org/2001/XMLSchema#integer";
/// IRI of the `xsd:boolean` datatype.
pub const XSD_BOOLEAN: &str = "http://www.w3.org/2001/XMLSchema#boolean";

/// Closed-vocabulary trigger signal per ADR-034.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TriggerSignal {
    /// Bundle stakes: routine.
    StakesRoutine,
    /// Bundle stakes: elevated.
    StakesElevated,
    /// Bundle stakes: foundational.
    StakesFoundational,
    /// Prior verifier confidence < 0.5.
    ConfidenceBelow05,
    /// Prior verifier confidence < 0.7.
    ConfidenceBelow07,
    /// Prior verifier confidence < 0.9.
    ConfidenceBelow09,
    /// Audit check passed.
    AuditPass,
    /// Audit check failed.
    AuditFail,
    /// Prior attempts in this escalation chain ≥ 1.
    PriorAttemptsGe1,
    /// Prior attempts ≥ 2.
    PriorAttemptsGe2,
    /// Prior attempts ≥ 3.
    PriorAttemptsGe3,
    /// Prior attempts ≥ 4.
    PriorAttemptsGe4,
    /// Prior attempts ≥ 5.
    PriorAttemptsGe5,
    /// Feedback class `contradiction`.
    FeedbackContradiction,
    /// Feedback class `unimplementable` with severity `critical`.
    FeedbackUnimplementableCritical,
    /// Feedback class `gap`.
    FeedbackGap,
    /// Feedback class `scope-issue`.
    FeedbackScopeIssue,
}

impl TriggerSignal {
    /// Stable wire string per ADR-034.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::StakesRoutine => TRIGGER_STAKES_ROUTINE,
            Self::StakesElevated => TRIGGER_STAKES_ELEVATED,
            Self::StakesFoundational => TRIGGER_STAKES_FOUNDATIONAL,
            Self::ConfidenceBelow05 => TRIGGER_CONFIDENCE_BELOW_05,
            Self::ConfidenceBelow07 => TRIGGER_CONFIDENCE_BELOW_07,
            Self::ConfidenceBelow09 => TRIGGER_CONFIDENCE_BELOW_09,
            Self::AuditPass => TRIGGER_AUDIT_PASS,
            Self::AuditFail => TRIGGER_AUDIT_FAIL,
            Self::PriorAttemptsGe1 => TRIGGER_PRIOR_ATTEMPTS_GE_1,
            Self::PriorAttemptsGe2 => TRIGGER_PRIOR_ATTEMPTS_GE_2,
            Self::PriorAttemptsGe3 => TRIGGER_PRIOR_ATTEMPTS_GE_3,
            Self::PriorAttemptsGe4 => TRIGGER_PRIOR_ATTEMPTS_GE_4,
            Self::PriorAttemptsGe5 => TRIGGER_PRIOR_ATTEMPTS_GE_5,
            Self::FeedbackContradiction => TRIGGER_FEEDBACK_CONTRADICTION,
            Self::FeedbackUnimplementableCritical => TRIGGER_FEEDBACK_UNIMPLEMENTABLE_CRITICAL,
            Self::FeedbackGap => TRIGGER_FEEDBACK_GAP,
            Self::FeedbackScopeIssue => TRIGGER_FEEDBACK_SCOPE_ISSUE,
        }
    }

    /// Parse a trigger signal from its wire string. Returns `None` for
    /// unknown values.
    #[must_use]
    pub fn try_from_str(s: &str) -> Option<Self> {
        match s {
            TRIGGER_STAKES_ROUTINE => Some(Self::StakesRoutine),
            TRIGGER_STAKES_ELEVATED => Some(Self::StakesElevated),
            TRIGGER_STAKES_FOUNDATIONAL => Some(Self::StakesFoundational),
            TRIGGER_CONFIDENCE_BELOW_05 => Some(Self::ConfidenceBelow05),
            TRIGGER_CONFIDENCE_BELOW_07 => Some(Self::ConfidenceBelow07),
            TRIGGER_CONFIDENCE_BELOW_09 => Some(Self::ConfidenceBelow09),
            TRIGGER_AUDIT_PASS => Some(Self::AuditPass),
            TRIGGER_AUDIT_FAIL => Some(Self::AuditFail),
            TRIGGER_PRIOR_ATTEMPTS_GE_1 => Some(Self::PriorAttemptsGe1),
            TRIGGER_PRIOR_ATTEMPTS_GE_2 => Some(Self::PriorAttemptsGe2),
            TRIGGER_PRIOR_ATTEMPTS_GE_3 => Some(Self::PriorAttemptsGe3),
            TRIGGER_PRIOR_ATTEMPTS_GE_4 => Some(Self::PriorAttemptsGe4),
            TRIGGER_PRIOR_ATTEMPTS_GE_5 => Some(Self::PriorAttemptsGe5),
            TRIGGER_FEEDBACK_CONTRADICTION => Some(Self::FeedbackContradiction),
            TRIGGER_FEEDBACK_UNIMPLEMENTABLE_CRITICAL => {
                Some(Self::FeedbackUnimplementableCritical)
            }
            TRIGGER_FEEDBACK_GAP => Some(Self::FeedbackGap),
            TRIGGER_FEEDBACK_SCOPE_ISSUE => Some(Self::FeedbackScopeIssue),
            _ => None,
        }
    }
}

/// One escalation step within a `RoleBinding` (ADR-034).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscalationStep {
    /// Capability promoted to when this step's triggers fire.
    pub step_capability: NamedNode,
    /// Triggers selecting this step (OR-evaluated; non-empty).
    pub triggers: Vec<TriggerSignal>,
}

/// In-memory `dec:RoleBinding` artifact (FT-055).
///
/// Identity is `(role_id, version)`; canonical IRI is
/// `https://decision-cli.dev/ns/binding/<role_id>/v<version>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleBinding {
    /// Lookup key into the FT-030 role catalog.
    pub role_id: String,
    /// Default capability (dispatcher's first choice).
    pub default_capability: NamedNode,
    /// Ordered escalation chain. Empty for bounded-classification roles.
    pub escalation_steps: Vec<EscalationStep>,
    /// Monotonically increasing version (≥ 1).
    pub version: u32,
    /// True iff this binding is the dispatcher's authoritative choice.
    pub active: bool,
    /// Optional link to the prior version this supersedes.
    pub supersedes: Option<NamedNode>,
    /// Optional audit trail — seed YAML hash if catalog-bootstrapped.
    pub bootstrap_source: Option<String>,
}

impl RoleBinding {
    /// Canonical IRI: `https://decision-cli.dev/ns/binding/<role_id>/v<version>`.
    #[must_use]
    pub fn iri(&self) -> NamedNode {
        NamedNode::new_unchecked(format!(
            "{prefix}{id}/v{version}",
            prefix = IRI_DEC_ROLE_BINDING_PREFIX,
            id = self.role_id,
            version = self.version,
        ))
    }

    /// Canonical IRI for the `i`th escalation step (zero-indexed).
    #[must_use]
    pub fn step_iri(&self, i: usize) -> NamedNode {
        NamedNode::new_unchecked(format!(
            "{prefix}{id}/v{version}/step/{i}",
            prefix = IRI_DEC_ROLE_BINDING_PREFIX,
            id = self.role_id,
            version = self.version,
        ))
    }

    /// Canonical IRI for the `j`th trigger inside the `i`th step.
    #[must_use]
    pub fn trigger_iri(&self, i: usize, j: usize) -> NamedNode {
        NamedNode::new_unchecked(format!(
            "{prefix}{id}/v{version}/step/{i}/trigger/{j}",
            prefix = IRI_DEC_ROLE_BINDING_PREFIX,
            id = self.role_id,
            version = self.version,
        ))
    }

    /// Canonical IRI for the `k`th rdf:List node carrying the escalation chain.
    /// (k == 0 is the head; k == n-1 has rdf:rest rdf:nil.)
    #[must_use]
    pub fn list_node_iri(&self, k: usize) -> NamedNode {
        NamedNode::new_unchecked(format!(
            "{prefix}{id}/v{version}/list/{k}",
            prefix = IRI_DEC_ROLE_BINDING_PREFIX,
            id = self.role_id,
            version = self.version,
        ))
    }

    /// Serialise the binding (with its escalation steps and triggers) to
    /// RDF quads in the supplied named graph.
    #[must_use]
    pub fn to_quads(&self, graph: NamedNodeRef<'_>) -> Vec<Quad> {
        let g: GraphName = graph.into_owned().into();
        let subject = self.iri();
        let mut quads = self.binding_quads(&subject, &g);
        quads.extend(self.list_quads(&subject, &g));
        for (i, step) in self.escalation_steps.iter().enumerate() {
            quads.extend(self.step_quads(i, step, &g));
        }
        quads
    }

    fn binding_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
        let mut out = vec![
            Quad::new(subject.clone(), rdf_type, role_binding_class(), g.clone()),
            literal_quad(subject, role_binding_role_id_pred(), &self.role_id, g),
            named_quad(
                subject,
                default_capability_pred(),
                &self.default_capability,
                g,
            ),
            typed_literal_quad(
                subject,
                role_binding_active_pred(),
                if self.active { "true" } else { "false" },
                XSD_BOOLEAN,
                g,
            ),
            typed_literal_quad(
                subject,
                capability_version_pred(),
                &self.version.to_string(),
                XSD_INTEGER,
                g,
            ),
        ];
        if let Some(s) = &self.supersedes {
            out.push(named_quad(subject, capability_supersedes_pred(), s, g));
        }
        if let Some(s) = &self.bootstrap_source {
            out.push(literal_quad(subject, bootstrap_source_pred(), s, g));
        }
        out
    }

    fn list_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        let mut out = Vec::new();
        if self.escalation_steps.is_empty() {
            // rdf:nil — empty list; bindings without escalation_steps are
            // valid per FT-055 §Behaviour (bounded-classification roles).
            out.push(named_quad(
                subject,
                escalation_steps_pred(),
                &NamedNode::new_unchecked(RDF_NIL),
                g,
            ));
            return out;
        }
        // Non-empty rdf:List. Mint named nodes for each cell.
        out.push(named_quad(
            subject,
            escalation_steps_pred(),
            &self.list_node_iri(0),
            g,
        ));
        for (k, _step) in self.escalation_steps.iter().enumerate() {
            let node = self.list_node_iri(k);
            let step_iri = self.step_iri(k);
            out.push(named_quad(
                &node,
                NamedNodeRef::new_unchecked(RDF_FIRST),
                &step_iri,
                g,
            ));
            let rest = if k + 1 == self.escalation_steps.len() {
                NamedNode::new_unchecked(RDF_NIL)
            } else {
                self.list_node_iri(k + 1)
            };
            out.push(named_quad(
                &node,
                NamedNodeRef::new_unchecked(RDF_REST),
                &rest,
                g,
            ));
        }
        out
    }

    fn step_quads(&self, i: usize, step: &EscalationStep, g: &GraphName) -> Vec<Quad> {
        let step_iri = self.step_iri(i);
        let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
        let mut out = vec![
            Quad::new(
                step_iri.clone(),
                rdf_type,
                escalation_step_class(),
                g.clone(),
            ),
            named_quad(&step_iri, step_capability_pred(), &step.step_capability, g),
        ];
        for (j, t) in step.triggers.iter().enumerate() {
            let trigger_iri = self.trigger_iri(i, j);
            out.push(Quad::new(
                trigger_iri.clone(),
                rdf_type,
                escalation_trigger_class(),
                g.clone(),
            ));
            out.push(literal_quad(
                &trigger_iri,
                trigger_signal_pred(),
                t.as_str(),
                g,
            ));
            out.push(named_quad(&step_iri, triggers_pred(), &trigger_iri, g));
        }
        out
    }
}

/// Build a plain-literal quad `(s, p, "value", g)`.
pub fn literal_quad(s: &NamedNode, p: NamedNodeRef<'_>, value: &str, g: &GraphName) -> Quad {
    Quad::new(
        s.clone(),
        p.into_owned(),
        Literal::new_simple_literal(value),
        g.clone(),
    )
}

/// Build a typed-literal quad `(s, p, "value"^^datatype, g)`.
pub fn typed_literal_quad(
    s: &NamedNode,
    p: NamedNodeRef<'_>,
    value: &str,
    datatype: &str,
    g: &GraphName,
) -> Quad {
    Quad::new(
        s.clone(),
        p.into_owned(),
        Literal::new_typed_literal(value, NamedNode::new_unchecked(datatype)),
        g.clone(),
    )
}

/// Build an IRI-object quad `(s, p, o, g)`.
pub fn named_quad(s: &NamedNode, p: NamedNodeRef<'_>, o: &NamedNode, g: &GraphName) -> Quad {
    Quad::new(s.clone(), p.into_owned(), o.clone(), g.clone())
}
