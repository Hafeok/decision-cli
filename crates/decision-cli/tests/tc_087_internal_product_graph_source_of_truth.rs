//! TC-087 — internal product-cli graph is the source of truth for `dec preflight`.
//!
//! Validates: FT-052 · ADR-031, ADR-011.
//! Spec: `.product/tests/TC-087-internal-product-cli-graph-is-the-source-of-truth.md`
//!
//! The contract: `dec preflight FT-XXX` consults `.product/graph/index.ttl`
//! (the canonical projection) and ignores the markdown frontmatter under
//! `.product/features/`, `.product/adrs/`, and `.product/tests/`. The test
//! stages a projection that *disagrees* with the markdown on every key field
//! the report exposes (`cross_cutting_gaps`, `domain_gaps`, `dep_availability`)
//! and asserts the report mirrors the projection in every case.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use decision_cli::preflight;

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct WorkdirGuard(PathBuf);

impl WorkdirGuard {
    fn new(tag: &str) -> Self {
        let mut base = env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        base.push(format!(
            "decision-cli-tc087-{tag}-{}-{}-{}",
            std::process::id(),
            nanos,
            counter,
        ));
        fs::create_dir_all(&base).expect("create temp workdir");
        Self(base)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for WorkdirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Stage a Turtle projection whose contents we control. The text uses the
/// product-meta prefixes `product graph rebuild` emits today, so reads from
/// this fixture exercise the same query path the live tool produces.
fn write_projection(workdir: &Path, body: &str) -> PathBuf {
    let dir = workdir.join(".product/graph");
    fs::create_dir_all(&dir).expect("create projection dir");
    let path = dir.join("index.ttl");
    fs::write(&path, body).expect("write projection");
    path
}

/// Stage a markdown feature file under `.product/features/`. The body is
/// deliberately *inconsistent* with the projection so a markdown-leaking
/// implementation fails the assertions below.
fn write_feature_markdown(workdir: &Path, feature_id: &str, body: &str) {
    let dir = workdir.join(".product/features");
    fs::create_dir_all(&dir).expect("create features dir");
    fs::write(dir.join(format!("{feature_id}.md")), body).expect("write feature markdown");
}

/// Stage a markdown ADR file. Same source-of-truth principle as the
/// feature staging helper.
fn write_adr_markdown(workdir: &Path, adr_id: &str, body: &str) {
    let dir = workdir.join(".product/adrs");
    fs::create_dir_all(&dir).expect("create adrs dir");
    fs::write(dir.join(format!("{adr_id}.md")), body).expect("write adr markdown");
}

const FEATURE_FT007: &str = "FT-007";
const DEP_FT006: &str = "FT-006";
const ADR_LINKED: &str = "ADR-001";
const ADR_GAP: &str = "ADR-013";

fn projection_body() -> String {
    // Projection asserts:
    //   FT-007 depends on FT-006 (Complete).
    //   FT-007 is linked to ADR-001 via implementedBy.
    //   ADR-013 applies to some other feature (FT-099) — it is
    //   cross-cutting but NOT linked to FT-007. That makes it a
    //   `cross_cutting_gaps` row.
    //   ADR-001 applies to FT-007 (linked) — must NOT appear in gaps.
    format!(
        "@prefix pm:  <https://product-meta/ontology#> .\n\
         @prefix ft:  <https://product-meta/feature/> .\n\
         @prefix adr: <https://product-meta/adr/> .\n\
         \n\
         ft:{feature} a pm:Feature ;\n\
             pm:title \"projection title\" ;\n\
             pm:phase 1 ;\n\
             pm:status pm:Planned ;\n\
             pm:implementedBy adr:{linked} ;\n\
             pm:dependsOn ft:{dep} .\n\
         \n\
         ft:{dep} a pm:Feature ;\n\
             pm:status pm:Complete .\n\
         \n\
         ft:FT-099 a pm:Feature ;\n\
             pm:status pm:Planned .\n\
         \n\
         adr:{linked} a pm:ArchitecturalDecision ;\n\
             pm:appliesTo ft:{feature} .\n\
         \n\
         adr:{gap} a pm:ArchitecturalDecision ;\n\
             pm:appliesTo ft:FT-099 .\n",
        feature = FEATURE_FT007,
        dep = DEP_FT006,
        linked = ADR_LINKED,
        gap = ADR_GAP,
    )
}

/// Markdown that DISAGREES with the projection on every field. A
/// markdown-leaking implementation would report `FT-555` as a dep,
/// `ADR-666` as cross-cutting, and complete status — the assertions
/// below catch each leak independently.
fn markdown_body_disagreeing_with_projection() -> String {
    "---\n\
     id: FT-007\n\
     title: markdown title (differs from projection)\n\
     phase: 1\n\
     status: complete\n\
     depends-on:\n\
     - FT-555\n\
     adrs:\n\
     - ADR-666\n\
     tests: []\n\
     ---\n\n\
     Markdown body the projection does not see.\n"
        .to_string()
}

#[test]
fn tc_087_internal_product_graph_source_of_truth() {
    let guard = WorkdirGuard::new("graph-truth");
    let workdir = guard.path();

    // Stage a synthetic projection and DELIBERATELY DISAGREEING markdown.
    write_projection(workdir, &projection_body());
    write_feature_markdown(
        workdir,
        FEATURE_FT007,
        &markdown_body_disagreeing_with_projection(),
    );
    // ADR markdown stubs so a markdown-walking implementation could see
    // them; the projection still owns the truth.
    write_adr_markdown(
        workdir,
        ADR_LINKED,
        "---\nid: ADR-001\nscope: cross-cutting\n---\nstub\n",
    );
    write_adr_markdown(
        workdir,
        ADR_GAP,
        "---\nid: ADR-013\nscope: cross-cutting\n---\nstub\n",
    );

    let report = preflight::run(workdir, FEATURE_FT007).expect("preflight succeeds");

    // dep_availability — from projection, not markdown.
    // Markdown says depends-on: [FT-555]; projection says FT-006/Complete.
    assert_eq!(
        report.dep_availability.len(),
        1,
        "expected one dep from projection, got {:?}",
        report.dep_availability,
    );
    let dep = &report.dep_availability[0];
    assert_eq!(
        dep.feature_id, DEP_FT006,
        "dep_availability must mirror projection (FT-006), not markdown (FT-555); got {}",
        dep.feature_id,
    );
    assert_eq!(
        dep.status.as_deref(),
        Some("Complete"),
        "dep status from projection",
    );
    assert!(dep.is_available(), "FT-006 is Complete → available");

    // cross_cutting_gaps — ADR-013 in projection only applies to FT-099,
    // so for FT-007 it shows up as a gap. ADR-001 applies to FT-007, so
    // it is linked, not a gap.
    assert!(
        report.cross_cutting_gaps.contains(&ADR_GAP.to_string()),
        "ADR-013 must appear in cross_cutting_gaps (projection has it apply to FT-099 only); got {:?}",
        report.cross_cutting_gaps,
    );
    assert!(
        !report.cross_cutting_gaps.contains(&ADR_LINKED.to_string()),
        "ADR-001 is linked via projection (pm:appliesTo); must NOT be a gap; got {:?}",
        report.cross_cutting_gaps,
    );
    assert!(
        report
            .cross_cutting_linked
            .contains(&ADR_LINKED.to_string()),
        "ADR-001 must appear in cross_cutting_linked; got {:?}",
        report.cross_cutting_linked,
    );

    // domain_gaps — projection has no `pm:domainGap` triples → empty list.
    // (A markdown-leaking implementation would either skip this section
    // entirely or invent gaps from `domains:` in the FT-007 markdown.)
    assert!(
        report.domain_gaps.is_empty(),
        "domain_gaps must be empty when projection carries no domain info; got {:?}",
        report.domain_gaps,
    );

    // No blocking gaps — FT-006 is Complete and there are no domain gaps.
    assert!(
        !report.has_blocking_gaps(),
        "report should not block dispatch: {:?}",
        report,
    );

    // Source-of-truth contract: mutate the markdown to a value that
    // would change every section of the report if it were read.
    // Specifically swap depends-on to point at an *incomplete* feature
    // and add a contradictory ADR. The projection is unchanged, so the
    // report must remain identical.
    write_feature_markdown(
        workdir,
        FEATURE_FT007,
        "---\n\
         id: FT-007\n\
         title: post-mutation title\n\
         phase: 1\n\
         status: planned\n\
         depends-on:\n\
         - FT-998\n\
         - FT-999\n\
         adrs:\n\
         - ADR-777\n\
         - ADR-888\n\
         tests: []\n\
         ---\n\nMutated markdown.\n",
    );
    write_feature_markdown(
        workdir,
        "FT-998",
        "---\nid: FT-998\nstatus: planned\n---\nblocking dep that does NOT exist in projection.\n",
    );

    let after = preflight::run(workdir, FEATURE_FT007).expect("preflight succeeds after mutation");

    assert_eq!(
        after, report,
        "post-mutation report must equal pre-mutation report (projection is the source of truth, \
         markdown mutations between rebuilds must NOT influence the report)",
    );
}

#[test]
fn tc_087_missing_projection_is_an_actionable_error() {
    // Source-of-truth contract corollary: if the projection is missing,
    // `dec preflight` does NOT silently fall back to the markdown.
    let guard = WorkdirGuard::new("no-projection");
    let workdir = guard.path();
    write_feature_markdown(
        workdir,
        FEATURE_FT007,
        &markdown_body_disagreeing_with_projection(),
    );
    let err = preflight::run(workdir, FEATURE_FT007)
        .expect_err("preflight without projection must fail rather than read markdown");
    let msg = err.to_string();
    assert!(
        msg.contains("no graph projection") && msg.contains("product graph rebuild"),
        "error must point operator at the rebuild step; got {msg}",
    );
}

#[test]
fn tc_087_dep_blocks_when_projection_says_planned() {
    // Source-of-truth contract corollary: if the projection records the
    // dependency as Planned, the report blocks — regardless of what the
    // markdown says.
    let guard = WorkdirGuard::new("dep-blocks");
    let workdir = guard.path();

    let body = format!(
        "@prefix pm:  <https://product-meta/ontology#> .\n\
         @prefix ft:  <https://product-meta/feature/> .\n\
         \n\
         ft:{feature} a pm:Feature ;\n\
             pm:status pm:Planned ;\n\
             pm:dependsOn ft:{dep} .\n\
         \n\
         ft:{dep} a pm:Feature ;\n\
             pm:status pm:Planned .\n",
        feature = FEATURE_FT007,
        dep = DEP_FT006,
    );
    write_projection(workdir, &body);
    // Markdown lies — says dep is Complete.
    write_feature_markdown(
        workdir,
        DEP_FT006,
        "---\nid: FT-006\nstatus: complete\n---\nfake-completeness\n",
    );

    let report = preflight::run(workdir, FEATURE_FT007).expect("preflight succeeds");

    assert_eq!(report.dep_availability.len(), 1);
    assert_eq!(report.dep_availability[0].feature_id, DEP_FT006);
    assert_eq!(
        report.dep_availability[0].status.as_deref(),
        Some("Planned"),
        "status from projection, not markdown",
    );
    assert!(
        report.has_blocking_gaps(),
        "dep Planned in projection must block dispatch",
    );
}
