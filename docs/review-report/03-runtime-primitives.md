# Slice 03 — Declarative Runtime primitives

## Scope
Reviewed diff range `bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31` for:
- `src-tauri/src/profile_dsl/runtime/**`
- `src-tauri/tests/posting_discovery_runtime.rs`
- `src-tauri/tests/posting_discovery_runtime/**`
- `src-tauri/tests/posting_detail_runtime.rs`

## Commands/Inputs
- Read standards/spec sources: `AGENTS.md`, `README.md`, `CONTEXT.md`, `docs/prd/declarative-source-profile-dsl.md`, `docs/adr/0009-declarative-source-profile-dsl.md`.
- Ran required diff command: `git diff bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- src-tauri/src/profile_dsl/runtime src-tauri/tests/posting_discovery_runtime.rs src-tauri/tests/posting_discovery_runtime src-tauri/tests/posting_detail_runtime.rs`.
- Ran `git diff --stat ...` for the same path set: 36 files, 9543 insertions.
- Ran targeted tests:
  - `cargo test --manifest-path src-tauri/Cargo.toml posting_discovery_runtime` — passed, 46 runtime tests.
  - `cargo test --manifest-path src-tauri/Cargo.toml posting_detail_runtime` — passed, 15 runtime tests.
- Initial combined test-filter command `cargo test --manifest-path src-tauri/Cargo.toml posting_discovery_runtime posting_detail_runtime` failed because Cargo accepts only one test name filter.

## Standards Findings
- No blocker/major standards findings.
- Correct: runtime code stays generic. Grep over `src-tauri/src/profile_dsl/runtime` found no ATS-specific branches or old `adapterKey`/`inventory` routing terms.
- Correct: runtime diagnostics are structured through `DiagnosticCategory::Runtime`, stable codes, paths, severities, and strategy keys in the inspected runtime modules.
- Correct: runtime tests are integration tests under `src-tauri/tests/*.rs`, matching the Rust test guidance in `AGENTS.md`.

## Spec Findings
- **major — `where` / filter conditions are compiled but not executed.** The PRD requires `Where / filter` to keep or reject selected items before extraction (`docs/prd/declarative-source-profile-dsl.md:45-46`). The model defines filters (`src-tauri/src/profile_dsl/documents/select.rs:35-42`) and the execution plans carry `conditions` for both discovery and detail (`src-tauri/src/profile_dsl/execution_plan/posting_discovery.rs:35-37`, `src-tauri/src/profile_dsl/execution_plan/posting_detail.rs:31-34`). The discovery runtime selects items and immediately extracts candidates with no condition step (`src-tauri/src/profile_dsl/runtime/posting_discovery/strategy.rs:192-210`); detail similarly selects and proceeds to matching/extraction without applying `conditions` (`src-tauri/src/profile_dsl/runtime/posting_detail/strategy.rs:67-82`). Profiles using `where` would silently return items they declared should be filtered.
- **major — fallback can be bypassed by partially failed discovery strategies.** The PRD says a strategy succeeds only when required sub-primitives and acceptance validation succeed, and failed fallback diagnostics must be preserved (`docs/prd/declarative-source-profile-dsl.md:145-148`). Discovery acceptance only treats a strategy as failed when it has both zero candidates and at least one error (`src-tauri/src/profile_dsl/runtime/posting_discovery/strategy.rs:36-45`, `src-tauri/src/profile_dsl/runtime/posting_discovery/strategy.rs:68-87`). Therefore a paginated strategy that fetches page 1 successfully and fails page 2 can still be accepted with partial candidates, preventing fallback strategies from running.
- **major — postingDetail collection matching is JSON-array-only, despite XML/JSON requirement.** The PRD explicitly calls for posting detail collection matching from XML or JSON feeds (`docs/prd/declarative-source-profile-dsl.md:97`) and says detail may fetch a collection/feed and match one item (`docs/prd/declarative-source-profile-dsl.md:61`, `docs/prd/declarative-source-profile-dsl.md:162-163`). Runtime matching rejects anything except `RuntimeItem::Json(Value::Array(_))` and emits `detail_match_unsupported_selection` (`src-tauri/src/profile_dsl/runtime/posting_detail/strategy.rs:160-168`). XML collection detail strategies cannot satisfy the declared DSL behavior.
- **minor — schema/model exposes transforms that runtime rejects.** The PRD lists join and regex replace as explicit transform examples (`docs/prd/declarative-source-profile-dsl.md:51`), and the document model includes `Transform::Join` and `Transform::RegexReplace` (`src-tauri/src/profile_dsl/documents/transform.rs:25-30`). The runtime pipeline returns `unsupported_transform` for both (`src-tauri/src/profile_dsl/runtime/transform.rs:81-85`). Either implement these generic transforms or reject them during compilation so executable plans cannot contain schema-supported but runtime-unsupported transforms.

## Coverage Notes
- Existing runtime coverage is broad for JSON/XML/HTML parsing, HTTP/browser fetch, request bodies, pagination bounds, fallback acceptance, explicit transforms, combine behavior, locations, and detail diagnostics.
- Missing coverage follows the findings: no tests exercise `where` filters, discovery fallback after partial paginated fetch/parse failures, XML collection matching for `postingDetail`, or runtime behavior/compile rejection for `join` and `regex_replace` transforms.

## Verdict
Not ready to accept as fully spec-compliant. The implementation is generic and bounded in many paths, and targeted runtime tests pass, but the ignored filter conditions and unsupported XML detail collection matching are material DSL semantic gaps. Address the major findings before relying on these runtime primitives for declarative profiles.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Concrete findings include severities and citations for ignored filters, partial failed discovery strategy acceptance, JSON-only detail matching, and unsupported transforms."
    }
  ],
  "changedFiles": [
    "docs/review-report/03-runtime-primitives.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "git diff bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- src-tauri/src/profile_dsl/runtime src-tauri/tests/posting_discovery_runtime.rs src-tauri/tests/posting_discovery_runtime src-tauri/tests/posting_detail_runtime.rs",
      "result": "passed",
      "summary": "Inspected required diff; output was large and truncated by tool."
    },
    {
      "command": "git diff --stat bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- src-tauri/src/profile_dsl/runtime src-tauri/tests/posting_discovery_runtime.rs src-tauri/tests/posting_discovery_runtime src-tauri/tests/posting_detail_runtime.rs",
      "result": "passed",
      "summary": "36 files changed, 9543 insertions."
    },
    {
      "command": "cargo test --manifest-path src-tauri/Cargo.toml posting_discovery_runtime posting_detail_runtime",
      "result": "failed",
      "summary": "Cargo rejected the second test-name argument; reran filters separately."
    },
    {
      "command": "cargo test --manifest-path src-tauri/Cargo.toml posting_discovery_runtime",
      "result": "passed",
      "summary": "46 posting discovery runtime tests passed."
    },
    {
      "command": "cargo test --manifest-path src-tauri/Cargo.toml posting_detail_runtime",
      "result": "passed",
      "summary": "15 posting detail runtime tests passed."
    }
  ],
  "validationOutput": [
    "posting_discovery_runtime: 46 passed",
    "posting_detail_runtime: 15 passed"
  ],
  "residualRisks": [
    "Review was limited to the requested slice paths and targeted runtime test filters; full cargo test was not run.",
    "Findings are based on current source inspection and spec comparison; no source code was modified."
  ],
  "noStagedFiles": true,
  "diffSummary": "Adds declarative profile DSL runtime primitives for postingDiscovery/postingDetail fetch, parse, select, extract, transforms, pagination, browser client integration, diagnostics, and integration tests.",
  "reviewFindings": [
    "major: src-tauri/src/profile_dsl/runtime/posting_discovery/strategy.rs:192-210 and src-tauri/src/profile_dsl/runtime/posting_detail/strategy.rs:67-82 - compiled `where` filter conditions are ignored at runtime.",
    "major: src-tauri/src/profile_dsl/runtime/posting_discovery/strategy.rs:36-87 - discovery strategies with candidates and error diagnostics can still be accepted, bypassing fallback after partial failures.",
    "major: src-tauri/src/profile_dsl/runtime/posting_detail/strategy.rs:160-168 - postingDetail collection matching supports only JSON arrays, not XML collections required by the PRD.",
    "minor: src-tauri/src/profile_dsl/runtime/transform.rs:81-85 - schema-supported join and regex_replace transforms are rejected by runtime instead of implemented or compile-rejected."
  ],
  "manualNotes": "Report written to docs/review-report/03-runtime-primitives.md as requested."
}
```
