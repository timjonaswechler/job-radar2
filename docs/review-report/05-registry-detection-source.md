# Slice 05 — Source Profile registry, Source validation, Source Proposal detection

## Scope
Reviewed slice `bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31` for:

- `src-tauri/src/source_profile/**`
- `src-tauri/src/source/**`
- `src-tauri/tests/source_profile_registry.rs`
- `src-tauri/tests/source_profile_detection.rs`

Spec/standards inputs read: `AGENTS.md`, `README.md`, `CONTEXT.md`, `docs/prd/declarative-source-profile-dsl.md`, `docs/adr/0001-source-config-as-json-schema.md`, `docs/adr/0009-declarative-source-profile-dsl.md`.

## Commands/Inputs
- `git diff --stat bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- src-tauri/src/source_profile src-tauri/src/source src-tauri/tests/source_profile_registry.rs src-tauri/tests/source_profile_detection.rs`
- `git diff bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- src-tauri/src/source_profile src-tauri/src/source src-tauri/tests/source_profile_registry.rs src-tauri/tests/source_profile_detection.rs > /tmp/slice05.diff && wc -l /tmp/slice05.diff` → `9961 /tmp/slice05.diff`
- `cargo test --manifest-path src-tauri/Cargo.toml --test source_profile_registry --test source_profile_detection` → passed, 22 tests total.
- Targeted inspections with line-numbered reads of the changed registry, source validation, detection, proposal, HTTP/browser probe, and test files.

## Standards Findings
- No standards blockers found.
- Correct: terminology and document model align with `CONTEXT.md`: Source, Source Profile, Access Path, Source Config, Source Proposal, `validationState`, and `postingDiscovery` are used consistently in the reviewed Rust documents and tests.
- Correct: source code was not modified. Only this Markdown report was written.

## Spec Findings
- **major: `src-tauri/src/source_profile/registry/loading.rs:79-95`, `src-tauri/src/source_profile/registry/loading.rs:216-239` — registry loading does not apply Profile Compiler rules to Source Profiles unless a Source references them.** `load_profile_documents` parses and basic-checks profiles, then pushes them into the registry snapshot. The only compiler invocation in this registry path is via `derive_source_validation_state` for loaded Sources. Because built-in sources are empty in this slice, built-in profiles and unreferenced custom profiles can be exposed without the semantic/boundedness/security compiler checks that the PRD requires for built-in and custom profiles using the same DSL/compiler rules. This leaves invalid custom profiles diagnosable only later, after a Source is created.
- **major: `src-tauri/src/source_profile/detection/mod.rs:219-228`, `src-tauri/src/source_profile/detection/mod.rs:341-355` — invalid `inputUrlPatterns` regexes are silently ignored.** `match_input_url_patterns` handles `Regex::new` errors with `Err(_) => continue`, and `evaluate_profile` then returns a non-match with no diagnostic. This conflicts with the structured diagnostic contract for profile detection and makes an authoring error indistinguishable from an unsupported URL. HTTP and browser regex errors do produce structured diagnostics, so this gap is specific to input URL detection patterns.
- **major: `src-tauri/src/source_profile/detection/proposal.rs:84-131`, `src-tauri/src/source_profile/detection/proposal.rs:189-206` — Source Proposal validation is shallow and can return a proposal whose Source Config does not satisfy the profile/path schema.** `validate_source_config_for_detection` checks only object-ness, forbidden Search Request criteria keys, and presence of required keys; it does not validate property types, enums, patterns, or other JSON Schema constraints from the profile and Access Path. Additionally, the default Source Config builder only reads profile-level schema properties, not Access Path-level schema properties. A detection result can therefore be `matched` while the proposed Source would fail later Source validation, weakening the PRD goal that detection returns an actionable Source Proposal.

## Coverage Notes
- Covered by tests and passing: custom profile key collision keeps the built-in profile and emits `duplicate_source_profile_key`; persisted `"status": "invalid"` is rejected by the Source document shape; derived Source validation state is exercised through compiler diagnostics; Source Proposal serialization avoids `adapterKey`; HTTP/browser detection probe diagnostics and bounded browser waits/interactions are covered.
- Missing/weak coverage matching the findings:
  - No registry test proves unreferenced built-in/custom profiles are compiler-validated before being exposed.
  - No detection test covers an invalid `detect.inputUrlPatterns[].pattern` regex.
  - No detection test covers a proposed Source Config with the wrong JSON Schema type/enum/pattern or an Access Path-only schema property populated by defaults.

## Verdict
Do not accept slice 05 as complete yet. The implementation is directionally aligned and the focused tests pass, but the registry and detection paths still miss important PRD guarantees around same-rule profile compiler validation, structured diagnostics for detection authoring errors, and actionable Source Proposal Source Config validation.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Concrete findings are listed with severity and file/line citations under Spec Findings."
    }
  ],
  "changedFiles": [
    "docs/review-report/05-registry-detection-source.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "git diff --stat bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- src-tauri/src/source_profile src-tauri/src/source src-tauri/tests/source_profile_registry.rs src-tauri/tests/source_profile_detection.rs",
      "result": "passed",
      "summary": "Reviewed slice diff summary."
    },
    {
      "command": "git diff bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- src-tauri/src/source_profile src-tauri/src/source src-tauri/tests/source_profile_registry.rs src-tauri/tests/source_profile_detection.rs > /tmp/slice05.diff && wc -l /tmp/slice05.diff",
      "result": "passed",
      "summary": "Captured exact requested diff for inspection; 9961 lines."
    },
    {
      "command": "cargo test --manifest-path src-tauri/Cargo.toml --test source_profile_registry --test source_profile_detection",
      "result": "passed",
      "summary": "22 focused tests passed."
    },
    {
      "command": "git diff --cached --name-only",
      "result": "passed",
      "summary": "No staged files."
    }
  ],
  "validationOutput": [
    "source_profile_detection: 17 passed",
    "source_profile_registry: 5 passed"
  ],
  "residualRisks": [
    "Registry may expose unreferenced profiles without compiler validation.",
    "Invalid input URL detection regexes currently produce no structured diagnostic.",
    "Detection can return Source Config proposals that later fail full Source validation."
  ],
  "noStagedFiles": true,
  "diffSummary": "Slice replaces v1 source registry/detection with source_profile registry/detection, adds Source documents/derived validation, and adds registry/detection tests.",
  "reviewFindings": [
    "major: src-tauri/src/source_profile/registry/loading.rs:79-95 and :216-239 - registry loading does not compiler-validate profiles unless a Source references them.",
    "major: src-tauri/src/source_profile/detection/mod.rs:219-228 and :341-355 - invalid detect.inputUrlPatterns regexes are silently ignored without structured diagnostics.",
    "major: src-tauri/src/source_profile/detection/proposal.rs:84-131 and :189-206 - Source Proposal Source Config validation is shallow and can return non-actionable configs."
  ],
  "manualNotes": "No source code was modified; only the required Markdown report was written."
}
```
