# Issue #242 — Atomic Source Schema-v3 Activation

Issue: https://github.com/timjonaswechler/job-radar2/issues/242

## Delivery contract

This implementation is developed in internal vertical TDD steps but lands as one productive commit. No compatibility adapter, version dispatcher, old/new compiler facade, positive v2 fixture, or dual fingerprint route may remain. Existing app-data schema-v2 Source JSON is rejected and must be recreated manually.

## Confirmed test seams

Tests are added or migrated only at these public boundaries:

1. JSON Schema and Rust Serde acceptance/rejection for authored Source/Profile documents.
2. `compile_source(...) -> CompileSourceOutcome` for effective behavior, plans, provenance, and diagnostics.
3. Tauri command/filesystem registry round trip for typed Source documents.
4. Search Run source selection and lazy posting Detail service behavior.
5. Source Live Check operations and Check Report freshness behavior.
6. Frontend Create/Edit/Details model and API payload behavior through `test:source-ui`.

Each implementation step follows red → green at its applicable seam. Existing C01–C08 tests remain regression contracts.

## Implementation plan and status

| Step | Scope | Completion evidence | Status |
|---|---|---|---|
| 0 | Re-baseline C01–C08 and inventory all A01-owned surfaces | Foundation tests green; current symbols/callers/files recorded | Completed 2026-07-20 |
| 1 | Activate the strict schema-v3 authored graph | v3 Source/Profile and typed Policy accept and round-trip; v2, old phases/wrappers, Retry, and invalid Policy reject in Schema and Serde | Completed 2026-07-20 |
| 2 | Rewrite schemas, positive fixtures, and built-in profiles | Greenhouse, Workday, SuccessFactors and all positive fixtures use only v3/final vocabulary; required negatives remain explicit | Completed 2026-07-20 |
| 3 | Remove old overrides/compiler facade and migrate validation, registry, commands, persistence | `compile_source_execution_plan`, snapshots, override modules/schema/UI helpers deleted; non-empty direct-specialization filesystem round trip green | Completed 2026-07-20 |
| 4 | Migrate Search Run and lazy Detail | Both callers compile authoritative Sources through `compile_source`; runtime receives immutable policy-bearing plans | Completed 2026-07-20 |
| 5 | Migrate Source Live Check and activate C08 as sole freshness path | Each operation compiles/prepares once; checked fingerprints persist unchanged; raw identities and reload routes deleted | Completed 2026-07-20 |
| 6 | Migrate frontend Create/Edit/Details | Explicit v3 TypeScript types and final fragment editor; base/path/detection clearing preserved; no override vocabulary | Completed 2026-07-20 |
| 7 | Remove Profile-DSL Retry ghost and classify residue | No Profile DSL Retry representation; unrelated agent/UI retry occurrences classified | Completed 2026-07-20 |
| 8 | Update active domain/PRD/ADR/agent documentation | Active guidance uses direct specialization, effective behavior, final phases, C01 compiler, and C08 freshness vocabulary | Completed 2026-07-20 |
| 9 | Full validation, residue audit, code review, atomic delivery | Focused suites, `npm run build`, full Cargo suite, residue searches, `/code-review`, one commit | Completed 2026-07-20 |

## Step 0 baseline

- C01–C08 issues #235–#241 are closed.
- `main` and `origin/main` both started at `f75a37e`; worktree was clean.
- The corrected C05 contract is present as commit `718da33`.
- Focused foundation suites pass: effective compiler (18), provenance (7), phase naming (1), canonical fingerprints (9), policy/runtime (6).
- Current productive old-route inventory before edits:
  - 14 Rust production files reference old compiler/override concepts.
  - 6 schema/resource files contain v2 or old authored vocabulary.
  - 5 Profile DSL/schema/resource files contain Retry ghost material.
  - 2 check files still contain raw freshness identities.
  - Active documentation contains old Source Override and phase vocabulary that A01 owns.

## Completion log

### Step 0 — Readiness and re-baseline

Completed 2026-07-20. The earlier C05 discrepancy is fixed and published. Focused C01–C08 regression tests passed on the synchronized branch. No additional foundation blocker was found, so A01 implementation may proceed.

### Step 1 — Strict schema-v3 authored graph

Completed 2026-07-20. Canonical Rust Source/Profile documents now enforce schema version 3, final `detection`/`discovery`/`detail` keys, direct root `accessPaths`, structural-null rejection, and mandatory typed Policy on complete Strategy Sets. The new hard-cut integration test was red before activation and is green afterward.

### Step 2 — Schemas, fixtures, and built-ins

Completed 2026-07-20. Source/Profile/policy/fragment schemas are strict v3, the override schema is deleted, all positive fixtures and Greenhouse/Workday/SuccessFactors resources were rewritten in place, and explicit old-shape negatives remain rejection-only.

### Step 3 — Compiler, registry, commands, and persistence

Completed 2026-07-20. `compile_source` now consumes canonical documents directly; schema-v2 conversion, old snapshots/facade, override modules, and policy-less plans were deleted. Registry loading stores the exact outcome and compiler-effective profile. Command persistence round-trips a non-empty direct specialization.

### Step 4 — Search Run and lazy Detail

Completed 2026-07-20. Search Run selection and lazy Detail call `compile_source` with authoritative Sources and pass only immutable policy-bearing plans to runtime. Draft admission and lazy existing-description behavior remain caller-owned.

### Step 5 — Source Live Check and C08

Completed 2026-07-20. Registry loading compiles each Source once; check/status/activate/reactivate reuse that exact outcome and prepare canonical C08 fingerprints once. Raw document/override/logic fingerprints, status mutation, and activation reload/reprepare were removed; the checked set is persisted unchanged.

### Step 6 — Frontend Source UI

Completed 2026-07-20. API types, Create/Edit builders and hooks, schema catalogue, editor, registry view models, and Details use schema-v3 direct specialization. The old override helper/editor files are gone. Details separates authored fragments from backend-compiled effective profile behavior.

### Step 7 — Profile-DSL Retry removal

Completed 2026-07-20. Retry fields/types/compiler branches/plan members/provenance material/tests were removed. The only retained Retry concepts belong to unrelated AI-provider transport, persistence collision, or user-triggered UI actions.

### Step 8 — Active documentation

Completed 2026-07-20. Domain language, canonical PRDs, ADRs, production-agent guidance, fingerprint/provenance guidance, and Search Run smoke instructions now describe the active v3 model. Older evidence/handoff documents with old literals are explicitly marked historical.

### Step 9 — Full validation, review, and atomic delivery

Completed 2026-07-20. Focused Schema/Serde, compiler, registry, runtime, Search Run, lazy Detail, Live Check, fingerprint, built-in-profile, and frontend contract suites passed. `npm run test:source-ui`, `npm run build`, formatting, `git diff --check`, and the complete Cargo suite passed (all non-network tests green; the two documented network smoke tests remain ignored). Required old-route, Profile-DSL Retry, and raw-fingerprint residue searches are zero-hit; old literals remain only in explicit rejection or clearly marked historical material. The two-axis code review found and prompted fixes for DTO strictness, active vocabulary, fragment/schema alignment, and stale comments; the final review reported no remaining Standards or Spec blocker/major finding. This completion log ships with all productive changes in the single atomic #242 commit.

## Final verification evidence

- Focused Rust suites listed in #242, including Search Run and lazy Detail targets: passed.
- `npm run test:source-ui`: passed.
- `npm run build`: passed.
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check`: passed.
- `cargo test --manifest-path src-tauri/Cargo.toml`: passed.
- All #242 zero-hit/classified residue searches: passed.
- Standards and spec review via the repository code-review workflow: no remaining blocker/major finding.
