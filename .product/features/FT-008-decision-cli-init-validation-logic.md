---
id: FT-008
title: 'decision-cli: Init validation logic'
phase: 1
status: complete
depends-on:
- FT-006
- FT-007
- FT-009
adrs:
- ADR-004
- ADR-005
- ADR-006
- ADR-002
- ADR-011
- ADR-012
- ADR-008
- ADR-001
tests:
- TC-001
- TC-002
- TC-003
- TC-004
- TC-005
- TC-006
- TC-015
domains: []
domains-acknowledged: {}
---

## Description

`dec init` runs the **ADR-006** validation pipeline against a ValueStream definition document: parse, SHACL-validate against the embedded ontology (FT-006), resolve the referenced ValueAction URI against the bundled set (FT-007), cross-validate authorized goals against the ValueAction's compatible-goals, persist the validated artifacts (FT-009), and record a bootstrap session (`dec:session/init-001`) with PROV-O lineage per **ADR-004**.

See `decision-cli-slice-1-bounds.md` §3.2, §3.3.

## Functional Specification

### Inputs

- One of: `--template <bundled-template-name>` resolved via FT-007, or `--from <path>` to a Turtle/JSON-LD file on disk.
- The ontology handle from FT-006.
- The bundled definition library from FT-007.
- The orchestration store handle from FT-009.

### Outputs

- A persisted `ValueStream` artifact and a persisted `ValueAction` artifact.
- A bootstrap session record with full PROV-O metadata (ADR-004).
- Success report on stdout; structured errors on failure.

### State

- Reads from input source; writes to orchestration store **only on full success** (ADR-006).

### Behaviour

1. Read source bytes (template → bundled; `--from` → file).
2. Parse as Turtle or JSON-LD (auto-detected).
3. SHACL-validate against FT-006 shapes; on violation, abort with field-naming message.
4. Resolve `dec:terminalValueAction` URI against FT-007; on miss, abort naming the URI.
5. Cross-validate: every `dec:authorizedGoals` entry must appear in the ValueAction's compatible-goals; on miss, abort naming the goal and compatible set.
6. Compute the content hash of the source bytes.
7. Persist `ValueStream`, a copy of the resolved `ValueAction`, and the bootstrap session record with `prov:wasDerivedFrom` (source), `prov:value` (hash), validation outcome, ontology version, timestamp.

### Invariants

- No state written unless all five validation steps pass (ADR-006).
- The initial init in a fresh store always produces session id `dec:session/init-001`.
- After init, `dec status` (FT-012) can reproduce the source path/template and content hash exactly.

### Error handling

- `InitError::Parse(_)` — malformed Turtle/JSON-LD.
- `InitError::Shacl(report)` — SHACL violation with structured report.
- `InitError::UnknownValueAction(uri)` — bundled-set miss.
- `InitError::UnauthorizedGoal { goal, compatible }` — cross-validation failure.
- `InitError::Persist(_)` — exceptional; surfaces underlying store failure.

### Boundaries

- Does NOT define the ontology (FT-006) or bundled artifacts (FT-007).
- Does NOT enforce scope at command time — FT-010.
- Does NOT manage store lifecycle beyond writing initial artifacts (FT-009).

## Out of scope

- Re-init / store migration commands.
- URL-fetched definitions (ADR-006 defers to later slices).
- Interactive prompts — fail fast, never prompt.
