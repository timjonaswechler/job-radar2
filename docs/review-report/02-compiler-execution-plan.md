# Slice 02 — Profile Compiler + typed Execution Plan

## Scope
Reviewed range `bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31` for:

- `src-tauri/src/profile_dsl/compiler/**`
- `src-tauri/src/profile_dsl/execution_plan/**`
- `src-tauri/tests/compiler_resolution.rs`
- `src-tauri/tests/compiler_semantic_validation.rs`
- `src-tauri/tests/compiler_security_boundedness.rs`

Spec/standards inputs read: `AGENTS.md`, `README.md`, `CONTEXT.md`, `docs/prd/declarative-source-profile-dsl.md`, `docs/adr/0001-source-config-as-json-schema.md`, and `docs/adr/0009-declarative-source-profile-dsl.md`.

## Commands/Inputs
- `git diff --stat bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- ...slice paths...` — inspected 20 added files / 3992 insertions.
- `git diff --name-status bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- ...slice paths...` — all slice files are additions.
- `grep -RInEi 'workday|greenhouse|personio|successfactors|sap|adapterKey|inventory' src-tauri/src/profile_dsl/compiler src-tauri/src/profile_dsl/execution_plan src-tauri/tests/compiler_resolution.rs src-tauri/tests/compiler_semantic_validation.rs src-tauri/tests/compiler_security_boundedness.rs` — no output.
- `cargo test --manifest-path src-tauri/Cargo.toml --test compiler_resolution --test compiler_semantic_validation --test compiler_security_boundedness` — passed: 5 + 8 + 7 tests.
- `git diff --cached --quiet` — exit 0, no staged files.

## Standards Findings
- **nit — clean:** The slice uses canonical Source/Profile DSL vocabulary consistently in the reviewed compiler and execution-plan modules, matching `CONTEXT.md` and `AGENTS.md` terminology.
- **nit — clean:** The new behavioral tests are integration tests under `src-tauri/tests/*.rs`, which matches the Rust testing preference in `AGENTS.md`.
- **minor — clean:** No profile-specific ATS branches were found in the reviewed slice; the targeted grep for Workday/Greenhouse/Personio/SuccessFactors/SAP and v1 adapter/inventory vocabulary returned no matches.

## Spec Findings
- **blocker — Source Overrides are not applied or fully validated before producing the Execution Plan.** The PRD requires Source Overrides to be “applied to a selected profile Access Path before compilation” and says the compiler validates them “by compiling the final effective Execution Plan” (`docs/prd/declarative-source-profile-dsl.md:35-36`, `docs/prd/declarative-source-profile-dsl.md:127-129`); ADR 0009 likewise lists override application as a compiler responsibility (`docs/adr/0009-declarative-source-profile-dsl.md:9`). In the implementation, `StrategyOverride` can override executable behavior (`fetch`, `select`, `extract`, `transforms`, `acceptWhen`) (`src-tauri/src/profile_dsl/documents/overrides.rs:20-32`), but `validate_source_overrides` only checks duplicate and unknown strategy keys (`src-tauri/src/profile_dsl/compiler/overrides.rs:10-80`). Subsequent template, capability, boundedness, and security checks still run against the original `access_path.posting_discovery` / `access_path.posting_detail`, not an effective overridden step (`src-tauri/src/profile_dsl/compiler/resolution.rs:191-225`), and compilation also uses the original access path steps (`src-tauri/src/profile_dsl/compiler/resolution.rs:63-80`). The produced `SourceExecutionPlan` then carries raw `SourceOverrides` as a side field (`src-tauri/src/profile_dsl/execution_plan/mod.rs:15-23`, `src-tauri/src/profile_dsl/compiler/resolution.rs:82-97`) instead of exposing one strict, effective plan. This means an override fetch with forbidden headers, missing timeouts, prohibited browser behavior, bad templates, or incompatible selectors is not validated at the compiler boundary; conversely, valid overrides do not affect execution. The existing fixture demonstrates the gap: it overrides `postingDiscovery.json_api.acceptWhen.minResults` to `0` (`src-tauri/tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json:15-22`), while the profile strategy remains `minResults: 1` (`src-tauri/tests/fixtures/source-profile-dsl/valid/simple-source-profile.json:98-100`), and the resolution test only asserts that overrides are present, not that the effective plan changed (`src-tauri/tests/compiler_resolution.rs:35-49`).
- **minor — clean:** The runtime entry points reviewed execute `SourceExecutionPlan` rather than accepting raw profile JSON (`src-tauri/src/profile_dsl/runtime/posting_discovery.rs:197-217`, `src-tauri/src/profile_dsl/runtime/posting_detail.rs:191-218`), and Search Run source selection compiles before execution (`src-tauri/src/search/run/service/selection.rs:122-132`). This is aligned with the raw-JSON execution boundary, subject to the override blocker above.
- **minor — clean:** Security and boundedness checks cover the base Access Path for timeouts/retries, pagination limits, browser waits/interactions, forbidden headers/body fields, and prohibited browser behaviors (`src-tauri/src/profile_dsl/compiler/boundedness.rs`, `src-tauri/src/profile_dsl/compiler/security.rs`), with passing tests in `compiler_security_boundedness.rs`.

## Coverage Notes
- Added/updated tests cover reusable profile resolution, Source-owned Access Path resolution, missing profile/path diagnostics, inactive source rejection, capability compatibility, templates, source config criteria checks, support metadata, duplicate keys, security, and boundedness.
- Coverage gap: no test asserts that `sourceOverrides` are applied into the compiled strategy, and no test checks semantic/security/boundedness/template validation of override contents. This is the main missing acceptance coverage for this slice.
- Validation command passed for the three requested compiler test files. A first attempted `cargo test --manifest-path src-tauri/Cargo.toml compiler_resolution compiler_semantic_validation compiler_security_boundedness` failed due Cargo accepting only one test filter; a subsequent combined `--test ...` command passed.

## Verdict
**Blocked.** The compiler/runtime boundary is mostly moving in the intended direction, and the reviewed tests pass, but Source Overrides are currently stored as raw overlay data and are neither applied nor compiled into the effective typed Execution Plan. This violates the PRD/ADR requirement for override application and compiler validation before execution.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Reviewed only the requested slice paths and wrote this Markdown report; no source code changes were made."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Report includes file/line citations, commands run, validation output, changed files, coverage notes, and residual risks."
    }
  ],
  "changedFiles": [
    "src-tauri/src/profile_dsl/compiler/boundedness.rs",
    "src-tauri/src/profile_dsl/compiler/capabilities.rs",
    "src-tauri/src/profile_dsl/compiler/keys.rs",
    "src-tauri/src/profile_dsl/compiler/mod.rs",
    "src-tauri/src/profile_dsl/compiler/overrides.rs",
    "src-tauri/src/profile_dsl/compiler/resolution.rs",
    "src-tauri/src/profile_dsl/compiler/security.rs",
    "src-tauri/src/profile_dsl/compiler/source_config.rs",
    "src-tauri/src/profile_dsl/compiler/support.rs",
    "src-tauri/src/profile_dsl/compiler/templates.rs",
    "src-tauri/src/profile_dsl/compiler/templates/fetch.rs",
    "src-tauri/src/profile_dsl/compiler/templates/fields.rs",
    "src-tauri/src/profile_dsl/compiler/templates/validation.rs",
    "src-tauri/src/profile_dsl/execution_plan/capabilities.rs",
    "src-tauri/src/profile_dsl/execution_plan/mod.rs",
    "src-tauri/src/profile_dsl/execution_plan/posting_detail.rs",
    "src-tauri/src/profile_dsl/execution_plan/posting_discovery.rs",
    "src-tauri/tests/compiler_resolution.rs",
    "src-tauri/tests/compiler_semantic_validation.rs",
    "src-tauri/tests/compiler_security_boundedness.rs"
  ],
  "testsAddedOrUpdated": [
    "src-tauri/tests/compiler_resolution.rs",
    "src-tauri/tests/compiler_semantic_validation.rs",
    "src-tauri/tests/compiler_security_boundedness.rs"
  ],
  "commandsRun": [
    {
      "command": "git diff --stat bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- src-tauri/src/profile_dsl/compiler src-tauri/src/profile_dsl/execution_plan src-tauri/tests/compiler_resolution.rs src-tauri/tests/compiler_semantic_validation.rs src-tauri/tests/compiler_security_boundedness.rs",
      "result": "passed",
      "summary": "20 files added, 3992 insertions."
    },
    {
      "command": "grep -RInEi 'workday|greenhouse|personio|successfactors|sap|adapterKey|inventory' src-tauri/src/profile_dsl/compiler src-tauri/src/profile_dsl/execution_plan src-tauri/tests/compiler_resolution.rs src-tauri/tests/compiler_semantic_validation.rs src-tauri/tests/compiler_security_boundedness.rs",
      "result": "passed",
      "summary": "No profile-specific ATS or v1 adapter/inventory matches in reviewed slice."
    },
    {
      "command": "cargo test --manifest-path src-tauri/Cargo.toml compiler_resolution compiler_semantic_validation compiler_security_boundedness",
      "result": "failed",
      "summary": "Incorrect Cargo invocation: unexpected extra test-filter arguments."
    },
    {
      "command": "cargo test --manifest-path src-tauri/Cargo.toml --test compiler_resolution --test compiler_semantic_validation --test compiler_security_boundedness",
      "result": "passed",
      "summary": "20 tests passed across the three compiler test files."
    },
    {
      "command": "git diff --cached --quiet",
      "result": "passed",
      "summary": "Exit 0; no staged files."
    }
  ],
  "validationOutput": [
    "compiler_resolution: 5 passed, 0 failed",
    "compiler_security_boundedness: 8 passed, 0 failed",
    "compiler_semantic_validation: 7 passed, 0 failed",
    "git diff --cached --quiet: exit 0"
  ],
  "residualRisks": [
    "blocker: Source Overrides are not applied to, or fully validated as part of, the effective compiled Execution Plan.",
    "Working tree contained unrelated unstaged/untracked files before report writing; no files were staged."
  ],
  "noStagedFiles": true,
  "diffSummary": "Adds Profile Compiler modules for resolution, semantic/security/boundedness/template/support/source-config validation, typed execution-plan structs for discovery/detail capabilities, and three compiler integration test files.",
  "reviewFindings": [
    "blocker: src-tauri/src/profile_dsl/compiler/resolution.rs:63-80 and src-tauri/src/profile_dsl/compiler/overrides.rs:10-80 - Source Overrides are only key-validated and then ignored during compilation; raw overrides remain on SourceExecutionPlan at src-tauri/src/profile_dsl/execution_plan/mod.rs:15-23.",
    "no additional blockers found in standards checks"
  ],
  "manualNotes": "Report written to docs/review-report/02-compiler-execution-plan.md as requested."
}
```
