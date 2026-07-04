# Slice 08 — v1 removal, cleanup, docs/vocabulary

## Scope

Reviewed range `bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31` for the requested v1-removal, cleanup, vocabulary, and module/file-size concerns. Focused on the requested paths plus current module ownership and the `8e1756f` module split.

## Commands/Inputs

- Read standards/spec inputs: `AGENTS.md`, `README.md`, `CONTEXT.md`, `docs/prd/declarative-source-profile-dsl.md`, `docs/adr/0001-source-config-as-json-schema.md`, `docs/adr/0009-declarative-source-profile-dsl.md`.
- `git diff --stat bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- <requested paths>`.
- `git diff --name-status bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- <requested paths>`.
- `git diff --find-renames ... -- AGENTS.md CONTEXT.md README.md docs/prd/declarative-source-profile-dsl.md src-tauri/src/lib.rs src-tauri/resources/profiles/{greenhouse,successfactors,workday}.json`.
- `rg -n "adapterKey|declarative_endpoint_inventory|declarative_sitemap_inventory|declarative_browser_inventory|\binventory\b|schemaVersion\"\s*:\s*1|\"status\"\s*:\s*\"invalid\"|list_adapters|create_custom_source|detect_source_from_url|source::registry|source::detection|adapter_registry|mod declarative" src-tauri/src src-tauri/resources README.md AGENTS.md CONTEXT.md docs/prd/declarative-source-profile-dsl.md docs/adr/0001-source-config-as-json-schema.md docs/adr/0009-declarative-source-profile-dsl.md -g '!src-tauri/target'`.
- `test ! -e src-tauri/src/adapter_registry.rs && test ! -d src-tauri/src/declarative && test ! -d src-tauri/src/source/registry && test ! -d src-tauri/src/source/detection && test ! -e docs/source-registry-json-model.md && test ! -e src-tauri/resources/sources/stepstone_de.json` plus `ls -1 src-tauri/resources/profiles`.
- `for f in src-tauri/resources/profiles/*.json; do jq -r '[.schemaVersion, .key, .support.level, ([.accessPaths[].key] | join(","))] | @tsv' "$f"; done`.
- `find src-tauri/src -type f -name '*.rs' -print0 | xargs -0 wc -l | sort -nr | head -25`.
- `git show --stat --oneline --name-status 8e1756f -- src-tauri/src`.
- `cargo test --manifest-path src-tauri/Cargo.toml`.
- `git diff --cached --quiet && echo 'no staged files'`.

## Standards Findings

- **blocker/major: none.** The standards sources now point contributors at the canonical domain vocabulary and DSL specs: `AGENTS.md:14-19`, `AGENTS.md:31-37`, and `README.md:71-80` reference `CONTEXT.md`, the Profile DSL PRD, and ADRs instead of the removed source-registry model document.
- **correct:** Canonical vocabulary is represented in the reviewed standards/spec-facing docs. `CONTEXT.md:49-59` defines Profile Compiler, Execution Plan, and Structured Diagnostic; `CONTEXT.md:77-99` defines `postingDiscovery`, `postingDetail`, `postingMeta`, Support Level, Validation State, and Source Status with explicit avoid-list entries for old inventory/status wording.
- **correct:** Public Tauri command names in the current module entrypoint no longer expose the removed adapter/registry API. `src-tauri/src/lib.rs:113-118` registers `get_source_profile_registry_snapshot`, `list_source_profiles`, `list_sources`, `list_source_diagnostics`, `detect_source_proposal_from_url`, and `create_source`; there are no `list_adapters`, `list_source_registry_*`, `detect_source_from_url`, or `create_custom_source` registrations in the current `lib.rs`.
- **minor note:** File-size cleanup improved the reviewed v1/migration areas, especially via `8e1756f` splitting posting discovery/detail runtime, detection, and test modules. No file-size threshold is documented in `AGENTS.md`/`README.md`/`CONTEXT.md`; current largest Rust files still include `src-tauri/src/app/commands.rs` at 991 LOC and `src-tauri/src/background_tasks/mod.rs` at 864 LOC from the `wc -l` inspection. I am not treating this as a slice blocker because the removed v1 monoliths are gone and these files are outside the old v1 registry/runtime ownership.

## Spec Findings

- **blocker/major: none.** The implementation direction matches the hard-cut spec. The PRD requires no compatibility layer, no automatic migration, no parallel v1/v2 runtime, and no legacy warnings (`docs/prd/declarative-source-profile-dsl.md:121-123`, `docs/prd/declarative-source-profile-dsl.md:243-248`); ADR 0009 says the same at `docs/adr/0009-declarative-source-profile-dsl.md:3-9`.
- **correct:** Removed v1 paths are absent in the current tree: `src-tauri/src/adapter_registry.rs`, `src-tauri/src/declarative/`, `src-tauri/src/source/registry/`, `src-tauri/src/source/detection/`, `docs/source-registry-json-model.md`, and `src-tauri/resources/sources/stepstone_de.json` all failed existence checks as expected. The current resource profiles directory contains only `greenhouse.json`, `successfactors.json`, and `workday.json`.
- **correct:** Built-in profile resources are rewritten as schemaVersion 2 DSL profiles with support evidence and Access Paths, not adapter-key profiles: `src-tauri/resources/profiles/greenhouse.json:1-9` and `:59-64`, `src-tauri/resources/profiles/workday.json:1-9` and `:28-35`, and `src-tauri/resources/profiles/successfactors.json:1-9` and `:28-35`. The `jq` inspection reported `greenhouse/boards_api`, `successfactors/rmk_sitemap_html`, and `workday/cxs_api`, all with `schemaVersion` 2 and `verified` support.
- **correct:** Runtime/API ownership aligns with the new model. `src-tauri/src/lib.rs:1-9` declares `profile_dsl`, `source`, and `source_profile` modules; `src-tauri/src/profile_dsl/mod.rs:3-8` owns compiler/diagnostics/documents/execution_plan/runtime/template; `src-tauri/src/source/mod.rs:1-4` owns Source documents/validation; and `src-tauri/src/source_profile/mod.rs:3-5` owns detection, profile documents, and registry.
- **correct:** Source and Source Profile document shapes reflect the spec separation. `src-tauri/src/source/documents.rs:28-34` persists only `draft`, `active`, or `disabled` Source statuses; `src-tauri/src/source/documents.rs:36-60` models either a selected profile Access Path or one Source-owned Access Path. `src-tauri/src/source_profile/documents.rs:10-24` models Source Profiles with support metadata, detection, Source Config schema, and Access Paths.
- **correct:** Targeted v1-term search found no active v1 runtime/resource/API leftovers. Remaining relevant matches are intentional spec text/avoid-list references (`CONTEXT.md:77-83`, `docs/prd/declarative-source-profile-dsl.md:121-134`, `docs/prd/declarative-source-profile-dsl.md:210-219`, `docs/adr/0009-declarative-source-profile-dsl.md:3-5`) or negative tests (`src-tauri/src/profile_dsl/documents/serde_tests.rs`, `src-tauri/src/app/commands.rs:878-895`). Browser-runtime `schemaVersion: 1` test fixtures are unrelated to Source/Profile v1 documents.
- **correct:** Tests explicitly guard the v1 removal. `src-tauri/tests/source_profile_registry.rs:12-60` checks built-in resource documents have no v1 vocabulary and are schemaVersion 2; `src-tauri/tests/source_profile_registry.rs:102-142` asserts old backend v1 entrypoint paths are removed and compiled plans do not serialize `adapterKey`/`list_adapters`; `src-tauri/tests/source_profile_registry.rs:144-197` covers derived Source validation and forbidden Search Request criteria in Source Config.

## Coverage Notes

- `cargo test --manifest-path src-tauri/Cargo.toml` passed: 110 unit tests passed, 2 ignored network smoke tests; integration suites for compiler, runtime, schema validation, detection, source-profile registry, and built-in Greenhouse/SuccessFactors/Workday profiles all passed.
- The targeted search still sees `v1` and `inventory` in PRD/ADR removal-scope prose and in invalid/negative tests. I did not classify these as leftovers because they document or assert rejection of the old model.
- Worktree had pre-existing unstaged/untracked files before this report write; `git diff --cached --quiet` returned `no staged files`.

## Verdict

Accepted for slice 08. I found no blockers or major issues in the reviewed cleanup. The requested v1 registry/runtime/API/resource removals are present, documentation points to the new DSL/ADR vocabulary, current module ownership matches `profile_dsl` / `source_profile` / `source` responsibilities, and the relevant Rust test suite passes.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Review-only task completed without modifying source code; only this Markdown report was written. Scope stayed on slice 08 v1 removal, cleanup, docs/vocabulary, and module-size checks."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Report includes diff inputs, targeted v1-term searches, path/resource absence checks, module/file-size inspection, cited file/line evidence, cargo test output summary, residual risks, and no-staged-files check."
    }
  ],
  "changedFiles": [
    "docs/review-report/08-v1-removal-cleanup-docs.md"
  ],
  "testsAddedOrUpdated": [
    "none by reviewer; reviewed existing slice tests including src-tauri/tests/source_profile_registry.rs and built-in profile DSL integration tests"
  ],
  "commandsRun": [
    {
      "command": "git diff --stat/name-status bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- <requested paths>",
      "result": "passed",
      "summary": "Confirmed requested slice deletes old v1 registry/runtime/docs/resources and rewrites remaining built-in profiles/docs."
    },
    {
      "command": "rg targeted v1/API/resource terms over src-tauri/src src-tauri/resources README.md AGENTS.md CONTEXT.md PRD/ADR files",
      "result": "passed",
      "summary": "No active v1 leftovers found; matches are intentional spec/avoid-list references or negative tests."
    },
    {
      "command": "test removed paths absent; ls -1 src-tauri/resources/profiles",
      "result": "passed",
      "summary": "Removed v1 paths are absent; remaining built-in profiles are greenhouse.json, successfactors.json, workday.json."
    },
    {
      "command": "jq profile summary for src-tauri/resources/profiles/*.json",
      "result": "passed",
      "summary": "All remaining built-in profiles are schemaVersion 2, verified, and expose new Access Path keys."
    },
    {
      "command": "find src-tauri/src -type f -name '*.rs' -print0 | xargs -0 wc -l | sort -nr | head -25",
      "result": "passed",
      "summary": "Inspected current largest Rust files after module split; noted remaining large non-v1 files."
    },
    {
      "command": "git show --stat --oneline --name-status 8e1756f -- src-tauri/src",
      "result": "passed",
      "summary": "Verified module split commit added focused compiler/runtime/detection/test submodules."
    },
    {
      "command": "cargo test --manifest-path src-tauri/Cargo.toml",
      "result": "passed",
      "summary": "All Rust tests passed: 110 unit tests passed, 2 ignored; integration tests for compiler/runtime/schema/detection/registry/built-in profiles passed."
    },
    {
      "command": "git diff --cached --quiet && echo 'no staged files'",
      "result": "passed",
      "summary": "No staged files."
    }
  ],
  "validationOutput": [
    "cargo test --manifest-path src-tauri/Cargo.toml: passed; 110 unit tests passed, 2 ignored network smoke tests; all integration suites passed.",
    "Removed path check: removed v1 paths absent; remaining resource profiles are greenhouse.json, successfactors.json, workday.json.",
    "Targeted rg: no active adapterKey/inventory/list_adapters/create_custom_source/detect_source_from_url/source::registry/source::detection leftovers outside intentional specs and negative tests.",
    "No staged files."
  ],
  "residualRisks": [
    "Current worktree had pre-existing unstaged/untracked files unrelated to this review; no staged files were present.",
    "No explicit repository file-size threshold exists; app/commands.rs and background_tasks/mod.rs remain large but outside the old v1 runtime/registry ownership."
  ],
  "noStagedFiles": true,
  "diffSummary": "Slice removes v1 adapter registry, declarative runtime, source registry/detection modules, old source-registry docs, and old resources; rewrites remaining built-in profiles to schemaVersion 2 DSL; updates docs/vocabulary; and splits DSL/runtime/detection/test modules.",
  "reviewFindings": [
    "no blockers",
    "no major findings",
    "minor note: current largest Rust files still include app/commands.rs and background_tasks/mod.rs, but no documented size threshold or v1-ownership issue was found"
  ],
  "manualNotes": "Report written to docs/review-report/08-v1-removal-cleanup-docs.md as requested."
}
```
