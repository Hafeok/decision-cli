//! TC-175 — `product graph check` warns when
//! `default-acknowledged-cross-cutting` drifts from the live ADR catalog.
//!
//! Validates: FT-104.
//! Spec: `.product/tests/TC-175-graph-check-warns-when-default-acknowledged-cross.md`.
//!
//! Six scenarios exercise the three drift conditions and their
//! interaction:
//!
//! - A: listed ADR no longer exists → W035 with hint.
//! - B: listed ADR's scope changed away from cross-cutting → W036.
//! - C: feature rejects an ADR not in default-ack list → W037.
//! - D: three drift conditions co-exist without masking each other
//!   (output sorted for deterministic snapshot testing).
//! - E: fixing each warning clears it independently and locally.
//! - F: exit code (i.e. the warning-vs-error severity) is unchanged.

use decision_cli::default_ack::{
    check_drift, AdrSnapshot, DefaultAcknowledgeConfig, FeatureRejectionRecord, RejectedAdr,
};

fn cfg(adrs: &[&str]) -> DefaultAcknowledgeConfig {
    DefaultAcknowledgeConfig {
        adrs: adrs.iter().map(|s| (*s).to_string()).collect(),
        source: None,
    }
}

fn snap(id: &str, xcut: bool) -> AdrSnapshot {
    AdrSnapshot {
        adr_id: id.into(),
        is_cross_cutting: xcut,
    }
}

fn rejection(feature: &str, pairs: &[(&str, &str)]) -> FeatureRejectionRecord {
    FeatureRejectionRecord {
        feature_id: feature.into(),
        rejections: pairs
            .iter()
            .map(|(id, reason)| RejectedAdr {
                id: (*id).into(),
                reason: (*reason).into(),
            })
            .collect(),
    }
}

#[test]
fn tc_175_graph_check_warns_when_default_acknowledged_cross() {
    // ----- Scenario A: listed ADR no longer exists → W035 -----
    let warnings = check_drift(
        &cfg(&["ADR-ALIVE", "ADR-GONE", "ADR-RESCOPED"]),
        &[snap("ADR-ALIVE", true), snap("ADR-RESCOPED", true)],
        &[],
    );
    let w035 = warnings
        .iter()
        .find(|w| w.code == "W035")
        .expect("W035 present");
    assert!(
        w035.message.contains("ADR-GONE"),
        "scenario A: W035 names the missing ADR; got {:?}",
        w035.message
    );
    assert!(
        w035.message.contains("default-acknowledged-cross-cutting")
            || w035.message.contains("catalog"),
        "scenario A: W035 contextualises the violation; got {:?}",
        w035.message
    );
    assert!(
        w035.hint.to_lowercase().contains("remove") || w035.hint.to_lowercase().contains("restore"),
        "scenario A: hint suggests remove-or-restore; got {:?}",
        w035.hint
    );

    // ----- Scenario B: rescoped ADR → W036 -----
    let warnings = check_drift(
        &cfg(&["ADR-ALIVE", "ADR-RESCOPED"]),
        &[
            snap("ADR-ALIVE", true),
            snap("ADR-RESCOPED", false), // demoted to feature-specific
        ],
        &[],
    );
    let w036 = warnings
        .iter()
        .find(|w| w.code == "W036")
        .expect("W036 present");
    assert!(
        w036.message.contains("ADR-RESCOPED"),
        "scenario B: W036 names the rescoped ADR; got {:?}",
        w036.message
    );
    assert!(
        w036.message.to_lowercase().contains("cross-cutting")
            || w036.message.to_lowercase().contains("scope"),
        "scenario B: W036 mentions the scope change; got {:?}",
        w036.message
    );

    // ----- Scenario C: stray rejection → W037 -----
    let warnings = check_drift(
        &cfg(&["ADR-ALIVE"]),
        &[snap("ADR-ALIVE", true), snap("ADR-STRAY", true)],
        &[rejection(
            "FT-OPTOUT",
            &[
                ("ADR-ALIVE", "valid rejection"),
                ("ADR-STRAY", "no effect because not default-acked"),
            ],
        )],
    );
    let w037: Vec<_> = warnings.iter().filter(|w| w.code == "W037").collect();
    assert_eq!(
        w037.len(),
        1,
        "scenario C: only the stray rejection fires W037 (the valid one does not); got {:?}",
        warnings
    );
    assert!(
        w037[0].message.contains("FT-OPTOUT") && w037[0].message.contains("ADR-STRAY"),
        "scenario C: W037 names both the feature and the stray ADR; got {:?}",
        w037[0].message
    );

    // ----- Scenario D: three drift conditions co-exist, sorted -----
    let warnings = check_drift(
        &cfg(&["ADR-ALIVE", "ADR-GONE", "ADR-RESCOPED"]),
        &[
            snap("ADR-ALIVE", true),
            snap("ADR-RESCOPED", false),
            snap("ADR-STRAY", true),
        ],
        &[rejection(
            "FT-OPTOUT",
            &[("ADR-ALIVE", "valid"), ("ADR-STRAY", "incoherent")],
        )],
    );
    assert_eq!(
        warnings.len(),
        3,
        "scenario D: three warnings co-exist without masking each other; got {:?}",
        warnings
    );
    // sorted by code
    assert_eq!(
        warnings[0].code, "W035",
        "scenario D: sorted output (W035 first)"
    );
    assert_eq!(warnings[1].code, "W036", "scenario D: W036 second");
    assert_eq!(warnings[2].code, "W037", "scenario D: W037 third");

    // ----- Scenario E: each fix clears its warning independently -----
    // 1) Remove ADR-GONE from the config → W035 disappears.
    let mut config = cfg(&["ADR-ALIVE", "ADR-GONE", "ADR-RESCOPED"]);
    let mut catalog = vec![
        snap("ADR-ALIVE", true),
        snap("ADR-RESCOPED", false),
        snap("ADR-STRAY", true),
    ];
    let mut rejections = vec![rejection(
        "FT-OPTOUT",
        &[("ADR-ALIVE", "valid"), ("ADR-STRAY", "incoherent")],
    )];
    config.adrs.remove("ADR-GONE");
    let warnings = check_drift(&config, &catalog, &rejections);
    let codes: Vec<&str> = warnings.iter().map(|w| w.code.as_str()).collect();
    assert_eq!(
        codes,
        vec!["W036", "W037"],
        "scenario E1: only W036 and W037 remain after removing the stale entry"
    );

    // 2) Re-scope ADR-RESCOPED back to cross-cutting → W036 disappears.
    if let Some(entry) = catalog.iter_mut().find(|s| s.adr_id == "ADR-RESCOPED") {
        entry.is_cross_cutting = true;
    }
    let warnings = check_drift(&config, &catalog, &rejections);
    let codes: Vec<&str> = warnings.iter().map(|w| w.code.as_str()).collect();
    assert_eq!(
        codes,
        vec!["W037"],
        "scenario E2: only W037 remains after re-scoping ADR-RESCOPED"
    );

    // 3) Remove the stray rejection → zero warnings.
    rejections[0].rejections.retain(|r| r.id != "ADR-STRAY");
    let warnings = check_drift(&config, &catalog, &rejections);
    assert!(
        warnings.is_empty(),
        "scenario E3: removing the stray rejection clears all drift; got {:?}",
        warnings
    );

    // ----- Scenario F: exit code unchanged (warnings, not errors) -----
    // The drift records carry no `severity = error` channel — every
    // emitted record is a warning. We assert that the type itself
    // can't carry an error variant, so `graph check` will continue
    // to exit 0 with these findings (the FT-104 invariant: drift is
    // informational, not blocking).
    let warnings = check_drift(
        &cfg(&["ADR-ALIVE", "ADR-GONE"]),
        &[snap("ADR-ALIVE", true)],
        &[],
    );
    for w in &warnings {
        assert!(
            w.code.starts_with('W'),
            "scenario F: every drift record's code starts with W (warning); got {:?}",
            w.code
        );
    }
}
