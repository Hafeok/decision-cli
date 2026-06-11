use oxigraph::model::NamedNode;

pub const MECHANICAL_PROVENANCE: NamedNode = NamedNode::new_unchecked("https://decisionframework.org/ns/decision#mechanical");
pub const MOTIVATIONAL_PROVENANCE: NamedNode = NamedNode::new_unchecked("https://decisionframework.org/ns/decision#motivational");
pub const MECHANICAL_GENERATED_BY: NamedNode = NamedNode::new_unchecked("https://decisionframework.org/ns/decision#mechanicalGeneratedBy");
pub const MECHANICAL_GENERATED_AT: NamedNode = NamedNode::new_unchecked("https://decisionframework.org/ns/decision#mechanicalGeneratedAt");
pub const MECHANICAL_GENERATED_VIA: NamedNode = NamedNode::new_unchecked("https://decisionframework.org/ns/decision#mechanicalGeneratedVia");
pub const MOTIVATIONAL_MOTIVATED_BY: NamedNode = NamedNode::new_unchecked("https://decisionframework.org/ns/decision#motivationalMotivatedBy");
pub const MOTIVATIONAL_MOTIVATED_VIA: NamedNode = NamedNode::new_unchecked("https://decisionframework.org/ns/decision#motivationalMotivatedVia");