---
id: PAT-002
title: State-hash ring buffer for cycle detection in iterative drivers
status: live
domains:
- api
adrs: []
requires:
- PAT-001
examples:
- FT-110
- FT-111
- FT-119
---

## When to use

Any iterative driver loop that re-classifies state every round
and could plausibly visit the same state twice without making
progress — i.e. any planner where the same observable inputs
produce the same dispatch decision by construction. The pattern
catches multi-step rotations a pairwise "round N vs round N-1"
no-progress detector cannot see, because pairwise detection
needs the same role dispatched twice in a row.

Real cases: a worker dispatched on bootstrap (`prev.count == 0`)
that takes its early-return free pass and ships nothing; a
verifier→VGA→implementer rotation that ends back at the
verifier's starting observation; any non-converging chain whose
period is longer than 2.

The pattern terminates the loop with a clear, evidence-cited
diagnostic ("state-hash cycle of period N detected, inspect the
loop chain") instead of waiting for the iteration cap to fire
with no explanation.

## Prerequisites

- **PAT-001** — Inspector + Planner trait pair. The state-hash
  computation belongs on the inspector (it reads the same
  dimensions classify() reads); the ring buffer belongs on the
  planner. Without PAT-001 the hash function ends up coupled to a
  concrete store.
- Familiarity with `std::collections::hash_map::DefaultHasher` /
  `std::hash::Hasher`, and the deterministic-iteration discipline
  (`BTreeSet`, not `HashSet`) — same hash inputs must produce the
  same hash bits across processes.

## The pattern

Three pieces: a state-hash method on the inspector, a per-feature
ring buffer on the planner, and a backstop check at the end of
`classify()` that fires *only* when the pairwise no-progress
detector didn't already decide.

```rust
// Inspector trait gains one method.
pub trait GraphInspector {
    // ...existing methods...
    fn state_hash_for_feature(
        &self, feature_id: &str,
    ) -> Result<u64, InspectError>;
}

// ProductionInspector implementation:
fn state_hash_for_feature(&self, feature_id: &str) -> Result<u64, InspectError> {
    let verdict = self.aggregate_verdict_for_feature(feature_id)?;
    let open_iris: BTreeSet<String> = /* deterministic-iter set of open feedback */;
    let active_graphs: BTreeSet<String> = /* deterministic-iter set of non-superseded */;

    let mut h = DefaultHasher::new();
    format!("{verdict:?}").hash(&mut h);
    for iri in &open_iris { iri.hash(&mut h); }
    "::graphs::".hash(&mut h);                  // separator: no cross-set collision
    for iri in &active_graphs { iri.hash(&mut h); }
    Ok(h.finish())
}
```

Ring buffer on the planner, keyed by feature:

```rust
const STATE_HASH_BUFFER_LEN: usize = 8;

struct RecentHashes {
    feature_id: String,
    hashes: VecDeque<u64>,   // newest-first
}

pub struct FeatureShipPlanner<I: GraphInspector> {
    inspector: I,
    // ...other state...
    recent_hashes: RefCell<RecentHashes>,
}

// At the end of classify(), AFTER the pairwise detector has its turn:
let cycle_period = self.detect_state_hash_cycle(feature_id)?;
let pairwise_decided = matches!(final_action,
    Action::Stuck { .. } | Action::EscalateVgaToImplementer { .. }
    | Action::EscalateImplementerToVga { .. } | Action::Done);
if !pairwise_decided {
    if let Some(period) = cycle_period {
        final_action = Action::Stuck { reason: format!(
            "feature {feature_id}: state-hash cycle of period {period} detected...",
        )};
    }
}

fn detect_state_hash_cycle(&self, feature_id: &str) -> Result<Option<usize>, PlanError> {
    let hash = self.inspector.state_hash_for_feature(feature_id)?;
    let mut buf = self.recent_hashes.borrow_mut();
    if buf.feature_id != feature_id {
        buf.feature_id = feature_id.to_string();
        buf.hashes.clear();
    }
    if let Some(idx) = buf.hashes.iter().position(|&h| h == hash) {
        return Ok(Some(idx + 1));    // period 1 = immediate repeat
    }
    buf.hashes.push_front(hash);
    while buf.hashes.len() > STATE_HASH_BUFFER_LEN { buf.hashes.pop_back(); }
    Ok(None)
}
```

Three rules govern the ordering of the cycle detector vs the
pairwise no-progress detector:

1. **The hash is always recorded.** Even when pairwise decides to
   escalate or stuck, the hash gets pushed so the buffer stays
   continuous across escalation rounds.
2. **Pairwise gets the diagnostic.** Its reasons are more
   specific ("verifier dispatch did not change state",
   "escalation exhausted", etc.). The cycle backstop only
   overrides when pairwise returned `intended.clone()`.
3. **The buffer is per-feature.** If a planner instance is reused
   across `dec drive ship FT-A` and `dec drive ship FT-B`, the
   buffer resets on the feature-id change so FT-A's history can't
   false-positive on FT-B.

## Anti-patterns

- **Hashing with a `HashSet` or other non-deterministic
  collection.** Each process produces different bits for the same
  state; ring-buffer match is impossible. Always use `BTreeSet`
  (or `Vec` after explicit sort).
- **Hashing only the verdict + counts, not the actual IRI sets.**
  Two different graphs producing one open defect each look
  identical to the planner — false-positive cycle detection
  fires. Hash the *identity* of what's open (feedback IRIs,
  active-graph IRIs), not just the cardinality.
- **Letting the cycle detector outrank the pairwise reason.**
  Pairwise says "the implementer worker can't fix this" — that's
  the actionable diagnostic the operator needs. "State-hash cycle
  of period 1" replaces it with something weaker. Always check
  `pairwise_decided` first.
- **Skipping the hash record when the planner returns terminal
  `Done`.** Mostly harmless, but if a future change makes
  classify reusable across multiple goal states (e.g. the planner
  is asked to plan again after a Done), the buffer is now
  out-of-sync with the run's actual history.

## Worked example

`FeatureShipPlanner` in
`crates/decision-cli/src/features/drive/planners/feature_ship.rs`.
Five tests pin the behaviour:

- `period_two_cycle_between_impl_and_vga_returns_stuck` — pairwise
  blind because consecutive rounds dispatch different roles;
  hash detector terminates with period 2.
- `period_three_cycle_returns_stuck_with_period_three` — ABC
  rotation; the C→A transition shows impl progress (count drops)
  so pairwise misses it; hash detector terminates with period 3.
- `unique_state_every_round_does_not_false_positive` — 12
  distinct states (longer than the 8-slot buffer); no spurious
  stuck.
- `cycle_detector_defers_to_pairwise_diagnostic_when_pairwise_decides`
  — same state + same role twice; pairwise's
  `EscalateImplementerToVga` wins over the generic cycle stuck.
- `cycle_detector_resets_buffer_on_feature_id_change` — feature
  swap clears the buffer.

When this landed against the full 110-feature sweep, 46
previously-max-iter cases reclassified as terminal stuck with a
graph-theoretic proof of non-convergence — every one of them
caught on round 2 with a period-1 match, which immediately
exposed two root-cause bugs (the matcher's missing supersession
filter and the planner's too-narrow verdict query) that the
iteration cap had been silently swallowing.
