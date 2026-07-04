# Slice 01 — Schema + DSL Document Model

## Scope
Reviewed the requested range `bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31` for:

- `src-tauri/src/schema/**`
- `src-tauri/src/profile_dsl/documents/**`
- `src-tauri/tests/schema_validation.rs`
- `src-tauri/tests/fixtures/source-profile-dsl/**`

Also inspected adjacent current document structs where needed to verify schema/document-model consistency.

## Commands/Inputs
- Read standards/spec sources: `AGENTS.md`, `README.md`, `CONTEXT.md`, `docs/prd/declarative-source-profile-dsl.md`, `docs/adr/0001-source-config-as-json-schema.md`, `docs/adr/0009-declarative-source-profile-dsl.md`.
- Ran: `git diff --stat bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- src-tauri/src/schema src-tauri/src/profile_dsl/documents src-tauri/tests/schema_validation.rs src-tauri/tests/fixtures/source-profile-dsl`
- Ran: `git diff --name-status bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- src-tauri/src/schema src-tauri/src/profile_dsl/documents src-tauri/tests/schema_validation.rs src-tauri/tests/fixtures/source-profile-dsl`
- Ran the requested diff inspection command for the same paths.
- Ran: `cargo test --manifest-path src-tauri/Cargo.toml` — timed out during compilation after 180s.
- Ran: `cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation` twice — timed out during compilation after 180s and 300s.
- Ran: `git diff --cached --quiet && echo no-staged || echo staged` — `no-staged`.

## Standards Findings
- No standards findings. The changed schemas and document fixtures use the canonical Source / Source Profile / Access Path / Source Config / Source Overrides / `postingDiscovery` / `postingDetail` terms from `CONTEXT.md`, and the added Rust tests are integration-style schema tests under `src-tauri/tests/`, matching `AGENTS.md` guidance.

## Spec Findings
- **major** — Detection probes are schema-valid without explicit timeouts, contrary to the PRD boundedness requirement. `docs/prd/declarative-source-profile-dsl.md` requires every network/browser primitive to have explicit bounds/timeouts. However, `src-tauri/src/schema/source-profile.schema.json:99-120` requires only `key` and `url` for `detectionHttpCheck` / `detectionBrowserProbe`; `timeoutMs` is optional. The corresponding document structs also model `timeout_ms` as optional at `src-tauri/src/source_profile/documents.rs:75-98`. This lets detection documents pass schema/serde without an explicit bound, reducing authoring-time feedback for a core safety invariant.
- **minor** — The Rust document model contains browser interaction variants that are not part of the public schema DSL. `src-tauri/src/schema/profile-dsl/fetch.schema.json:98-108` exposes only `click_if_visible` and `click_until_gone`, but `src-tauri/src/profile_dsl/documents/fetch.rs:91-110` also deserializes `execute_script`, `eval`, `mutate_dom`, `login_flow`, and `captcha_bypass`. Compiler security checks appear to reject these later, so this is not an execution bypass, but it leaves the schema and document model inconsistent for the Schema + DSL Document Model slice and should be covered by an explicit serde/compiler diagnostic test if intentional.

## Coverage Notes
- Positive coverage: `src-tauri/tests/schema_validation.rs:36-107` validates representative valid fixtures, invalid v1 vocabulary, missing support metadata, forbidden headers, invalid source status, posting-detail pagination, and unbounded pagination.
- Positive coverage: `src-tauri/src/profile_dsl/documents/serde_tests.rs:11-126` validates serde round-trips for reusable profiles, sources selecting reusable Access Paths, Source-owned Access Paths, Source Overrides, support levels, and source statuses.
- Gap: no schema fixture currently asserts that detection HTTP/browser probes must declare explicit `timeoutMs`; this aligns with the major finding above.
- Gap: no serde/compiler fixture in this slice documents the intended handling for schema-prohibited browser interactions deserialized by the Rust document model.
- Test execution could not be attested as passing because the cargo test commands timed out during compilation in this environment.

## Verdict
Not ready as-is for the Schema + DSL Document Model slice. The document model is broadly aligned with the new DSL and tests cover many required shape constraints, but the detection timeout gap should be fixed or explicitly justified because boundedness is a central PRD/security requirement.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Concrete findings include major detection-timeout schema/document-model gap at src-tauri/src/schema/source-profile.schema.json:99-120 and src-tauri/src/source_profile/documents.rs:75-98, plus minor schema/document-model mismatch for prohibited browser interactions at src-tauri/src/schema/profile-dsl/fetch.schema.json:98-108 and src-tauri/src/profile_dsl/documents/fetch.rs:91-110."
    }
  ],
  "changedFiles": [
    "docs/review-report/01-schema-documents.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "git diff --stat bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- src-tauri/src/schema src-tauri/src/profile_dsl/documents src-tauri/tests/schema_validation.rs src-tauri/tests/fixtures/source-profile-dsl",
      "result": "passed",
      "summary": "Reported 31 files changed, 1889 insertions, 6 deletions in the reviewed slice."
    },
    {
      "command": "git diff --name-status bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- src-tauri/src/schema src-tauri/src/profile_dsl/documents src-tauri/tests/schema_validation.rs src-tauri/tests/fixtures/source-profile-dsl",
      "result": "passed",
      "summary": "Listed added document-model modules, schema validation test/fixtures, and modified schema modules."
    },
    {
      "command": "cargo test --manifest-path src-tauri/Cargo.toml",
      "result": "timed_out",
      "summary": "Timed out after 180 seconds during compilation."
    },
    {
      "command": "cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation",
      "result": "timed_out",
      "summary": "Timed out during compilation after 180 seconds, then again after 300 seconds."
    },
    {
      "command": "git diff --cached --quiet && echo no-staged || echo staged",
      "result": "passed",
      "summary": "Output: no-staged."
    }
  ],
  "validationOutput": [
    "Schema/document inspection completed; cargo validation could not complete due to compile timeouts.",
    "git diff --stat for reviewed slice: 31 files changed, 1889 insertions, 6 deletions."
  ],
  "residualRisks": [
    "Cargo tests were not observed passing because compilation exceeded available timeouts.",
    "Worktree contains unrelated unstaged/untracked files outside this review slice."
  ],
  "noStagedFiles": true,
  "diffSummary": "Adds modular Profile DSL document structs, serde tests, schema validation tests/fixtures, and updates diagnostics, pagination, transform, and source-profile schemas.",
  "reviewFindings": [
    "major: src-tauri/src/schema/source-profile.schema.json:99-120 and src-tauri/src/source_profile/documents.rs:75-98 - detection HTTP/browser probes can omit explicit timeoutMs despite PRD boundedness requirements.",
    "minor: src-tauri/src/schema/profile-dsl/fetch.schema.json:98-108 and src-tauri/src/profile_dsl/documents/fetch.rs:91-110 - document model accepts schema-prohibited browser interaction variants; compiler appears to reject them later but schema/model intent is inconsistent."
  ],
  "manualNotes": "No source code was modified; only this Markdown review report was written."
}
```
