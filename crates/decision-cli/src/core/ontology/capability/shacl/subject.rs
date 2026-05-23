//! Per-subject orchestration of capability SHACL checks.

use oxigraph::model::{NamedNode, Quad};

use crate::core::vocab::{
    CAPABILITY_STATUS_ACTIVE, CAPABILITY_STATUS_CANDIDATE, CAPABILITY_STATUS_EOL,
    CAPABILITY_STATUS_PREVIEW, CURRENCY_EUR, CURRENCY_USD, ENDPOINT_ANTHROPIC, ENDPOINT_SCALEWAY,
    IRI_DEC_BOOTSTRAP_SOURCE, IRI_DEC_CAPABILITY_ENDPOINT, IRI_DEC_CAPABILITY_ID,
    IRI_DEC_CAPABILITY_NOTES, IRI_DEC_CAPABILITY_STATUS, IRI_DEC_CAPABILITY_SUPERSEDES,
    IRI_DEC_CONFIGURABLE_EFFORT, IRI_DEC_CONTEXT_WINDOW, IRI_DEC_COST_CACHE_HIT_PER_M,
    IRI_DEC_COST_CACHE_WRITE_5M, IRI_DEC_COST_CURRENCY, IRI_DEC_COST_INPUT_PER_M,
    IRI_DEC_COST_OUTPUT_PER_M, IRI_DEC_EXPOSES_REASONING_TRACE, IRI_DEC_MAX_OUTPUT,
    IRI_DEC_MODEL_IDENTIFIER, IRI_DEC_SUPPORTS_TOOL_CALLING, IRI_DEC_SUPPORTS_VISION,
};

use super::checks::{
    check_boolean_one, check_cache_cost_pair, check_enum_one, check_non_negative_decimal_one,
    check_non_negative_int_one, check_optional_iri, check_optional_string, check_string_one,
    check_tier_range, check_version_ge_one,
};
use super::CapabilityViolation;

pub(super) fn validate_subject(quads: &[Quad], subject: &NamedNode) -> Vec<CapabilityViolation> {
    let mut violations = Vec::new();
    check_string_one(
        quads,
        subject,
        IRI_DEC_CAPABILITY_ID,
        "dec:capability_id",
        true,
        &mut violations,
    );
    check_string_one(
        quads,
        subject,
        IRI_DEC_MODEL_IDENTIFIER,
        "dec:model_identifier",
        true,
        &mut violations,
    );
    check_enum_one(
        quads,
        subject,
        IRI_DEC_CAPABILITY_ENDPOINT,
        "dec:endpoint",
        &[ENDPOINT_SCALEWAY, ENDPOINT_ANTHROPIC],
        &mut violations,
    );
    check_enum_one(
        quads,
        subject,
        IRI_DEC_COST_CURRENCY,
        "dec:cost_currency",
        &[CURRENCY_EUR, CURRENCY_USD],
        &mut violations,
    );
    check_enum_one(
        quads,
        subject,
        IRI_DEC_CAPABILITY_STATUS,
        "dec:status",
        &[
            CAPABILITY_STATUS_ACTIVE,
            CAPABILITY_STATUS_PREVIEW,
            CAPABILITY_STATUS_EOL,
            CAPABILITY_STATUS_CANDIDATE,
        ],
        &mut violations,
    );
    check_integer_fields(quads, subject, &mut violations);
    check_decimal_fields(quads, subject, &mut violations);
    check_boolean_fields(quads, subject, &mut violations);
    check_optional_iri(
        quads,
        subject,
        IRI_DEC_CAPABILITY_SUPERSEDES,
        "dec:supersedes",
        &mut violations,
    );
    check_optional_string(
        quads,
        subject,
        IRI_DEC_BOOTSTRAP_SOURCE,
        "dec:bootstrap_source",
        &mut violations,
    );
    check_optional_string(
        quads,
        subject,
        IRI_DEC_CAPABILITY_NOTES,
        "dec:notes",
        &mut violations,
    );
    check_cache_cost_pair(quads, subject, &mut violations);
    violations
}

fn check_integer_fields(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<CapabilityViolation>,
) {
    check_non_negative_int_one(
        quads,
        subject,
        IRI_DEC_CONTEXT_WINDOW,
        "dec:context_window",
        true,
        violations,
    );
    check_non_negative_int_one(
        quads,
        subject,
        IRI_DEC_MAX_OUTPUT,
        "dec:max_output",
        true,
        violations,
    );
    check_version_ge_one(quads, subject, violations);
    check_tier_range(quads, subject, violations);
}

fn check_decimal_fields(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<CapabilityViolation>,
) {
    check_non_negative_decimal_one(
        quads,
        subject,
        IRI_DEC_COST_INPUT_PER_M,
        "dec:cost_input_per_m",
        true,
        violations,
    );
    check_non_negative_decimal_one(
        quads,
        subject,
        IRI_DEC_COST_OUTPUT_PER_M,
        "dec:cost_output_per_m",
        true,
        violations,
    );
    check_non_negative_decimal_one(
        quads,
        subject,
        IRI_DEC_COST_CACHE_HIT_PER_M,
        "dec:cost_cache_hit_per_m",
        false,
        violations,
    );
    check_non_negative_decimal_one(
        quads,
        subject,
        IRI_DEC_COST_CACHE_WRITE_5M,
        "dec:cost_cache_write_5m",
        false,
        violations,
    );
}

fn check_boolean_fields(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<CapabilityViolation>,
) {
    check_boolean_one(
        quads,
        subject,
        IRI_DEC_SUPPORTS_VISION,
        "dec:supports_vision",
        true,
        violations,
    );
    check_boolean_one(
        quads,
        subject,
        IRI_DEC_SUPPORTS_TOOL_CALLING,
        "dec:supports_tool_calling",
        true,
        violations,
    );
    check_boolean_one(
        quads,
        subject,
        IRI_DEC_CONFIGURABLE_EFFORT,
        "dec:configurable_effort",
        false,
        violations,
    );
    check_boolean_one(
        quads,
        subject,
        IRI_DEC_EXPOSES_REASONING_TRACE,
        "dec:exposes_reasoning_trace",
        false,
        violations,
    );
}
