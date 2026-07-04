# Slice 04 — Acceptance profiles and fixtures

## Scope
Reviewed slice `bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31` for:

- Built-in acceptance profiles: `src-tauri/resources/profiles/{greenhouse,workday,successfactors}.json`
- Acceptance fixtures under `src-tauri/tests/fixtures/{greenhouse,workday,successfactors}/`
- Acceptance tests: `src-tauri/tests/{greenhouse,workday,successfactors}_profile_dsl.rs`

## Commands/Inputs
- Read standards/spec sources: `AGENTS.md`, `README.md`, `CONTEXT.md`, `docs/prd/declarative-source-profile-dsl.md`, `docs/adr/0009-declarative-source-profile-dsl.md`.
- Inspected requested diff/stat with:
  - `git diff --stat bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- <slice paths>`
  - `git diff --name-status bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- <slice paths>`
- Inspected current profile/test/fixture contents and relevant runtime/compiler definitions.
- Searched for ATS-specific runtime branches with `git grep -n -E 'greenhouse|workday|successfactors|Greenhouse|Workday|SuccessFactors|SAP SuccessFactors' -- src-tauri/src`.
- Searched relevant slice for v1 vocabulary with `git grep -n -E 'adapterKey|inventory|SourceSpecific|source_specific' -- <slice profile/test paths>`.
- Ran targeted tests:
  - `cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl --test workday_profile_dsl --test successfactors_profile_dsl`

## Standards Findings
- No blocker/major standards findings.
- Correct: The profiles are declarative JSON documents with `schemaVersion: 2` and profile-level `support.level: verified` (`greenhouse.json:1-21`, `workday.json:1-27`, `successfactors.json:1-27`), aligning with the Source/Profile DSL vocabulary in `CONTEXT.md:45-55` and support-level expectations in `CONTEXT.md:89-90`.
- Correct: The slice uses external Rust integration tests under `src-tauri/tests/*.rs`, matching the AGENTS.md Rust-test guidance (`AGENTS.md:39-41`).
- Correct: I did not find ATS-specific Rust adapters in runtime code. The only current `greenhouse`/`workday`/`successfactors` Rust references are built-in embedding, registry/app tests, and smoke/test helpers, not profile-specific execution branches.
- Correct: Strategies are bounded with explicit HTTP timeouts and pagination limits where applicable (`greenhouse.json:68-75,126-133`; `workday.json:95-121,179-186`; `successfactors.json:98-116,216-223,243-250`), consistent with AGENTS.md bounded-strategy guidance (`AGENTS.md:35-36`).

## Spec Findings
- **major** — Template filter pipes remain in profile JSON detection candidates, contrary to the DSL PRD. The PRD requires transform logic to be explicit in `transforms[]` and says templates must not contain transform pipes (`docs/prd/declarative-source-profile-dsl.md:51-53`, reiterated at `docs/prd/declarative-source-profile-dsl.md:168-169`). The changed profiles use pipe filters in detection candidate templates:
  - `src-tauri/resources/profiles/greenhouse.json:28-29` — `{{capture:boardSlug|technicalKey}}`, `{{capture:boardSlug|slugToTitle}}`
  - `src-tauri/resources/profiles/workday.json:36-37` — `{{capture:tenant|technicalKey}}`, `{{capture:site|technicalKey}}`, `{{capture:tenant|slugToTitle}}`, `{{capture:site|slugToTitle}}`
  - `src-tauri/resources/profiles/successfactors.json:34-35` — `{{capture:successFactorsHost|domainKey}}`, `{{capture:successFactorsHost|domainTitle}}`
  These are still declarative, but they hide transform behavior in template expressions rather than the explicit transform pipeline required by the spec.

## Coverage Notes
- **Correct:** Greenhouse fixture coverage compiles through the public Profile Compiler and executes posting discovery plus lazy detail via the shared runtime (`greenhouse_profile_dsl.rs:39-49`, `greenhouse_profile_dsl.rs:62-87`). It also asserts only the discovery request and one lazy detail request occurred, which guards against discovery-time detail fanout (`greenhouse_profile_dsl.rs:89-95`).
- **Correct:** Workday coverage exercises HTTP POST discovery, JSON request body offset/limit pagination, normalized candidates/posting metadata, and detail GET with HTML-in-JSON normalization (`workday_profile_dsl.rs:44-84`, `workday_profile_dsl.rs:86-117`, `workday_profile_dsl.rs:119-147`).
- **Correct:** SuccessFactors coverage exercises sitemap/XML discovery, posting metadata, primary HTML detail extraction, fallback detail extraction, and preservation of the failed primary strategy diagnostic (`successfactors_profile_dsl.rs:43-74`, `successfactors_profile_dsl.rs:76-115`).
- **major coverage gap:** The Workday acceptance spec says the profile must exercise “Source Config from detection” (`docs/prd/declarative-source-profile-dsl.md:239`). The Workday profile declares detection-derived Source Config (`workday.json:28-35`), but the slice test only checks that capture names are present (`workday_profile_dsl.rs:18-20`, `workday_profile_dsl.rs:164-177`) and then constructs a `SourceDocument` manually (`workday_profile_dsl.rs:25-41`). It does not call profile detection or assert the proposed Source Config, so this acceptance requirement is not actually covered.
- Note: Browser detail is not covered for SuccessFactors, but the PRD qualifies this as “where needed” (`docs/prd/declarative-source-profile-dsl.md:240`); the current profile uses HTTP HTML primary/fallback strategies, so I am not flagging that as a defect.

## Verdict
Targeted tests pass and the core acceptance fixtures demonstrate compiled Execution Plan execution for posting discovery/detail across Greenhouse, Workday, and SuccessFactors. I found no ATS-specific Rust adapters in the runtime. However, two acceptance issues remain: profile detection templates still use PRD-prohibited pipe filters, and Workday does not test Source Config produced by detection.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Reported concrete major findings with file/line citations for template pipes in the three profile JSON files and missing Workday detection Source Config coverage."
    }
  ],
  "changedFiles": [
    "docs/review-report/04-acceptance-profiles.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "git diff --stat bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- <slice paths>",
      "result": "passed",
      "summary": "Diff contains 3 modified built-in profiles plus new fixtures/tests; 1619 insertions and 271 deletions across 21 files."
    },
    {
      "command": "cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl --test workday_profile_dsl --test successfactors_profile_dsl",
      "result": "passed",
      "summary": "All three targeted acceptance test binaries passed (1 test each)."
    },
    {
      "command": "git grep -n -E 'greenhouse|workday|successfactors|Greenhouse|Workday|SuccessFactors|SAP SuccessFactors' -- src-tauri/src",
      "result": "passed",
      "summary": "No profile-specific runtime adapters found; matches are built-in embedding, registry/app tests, and smoke/test helpers."
    }
  ],
  "validationOutput": [
    "greenhouse_builtin_profile_compiles_and_executes_offline_fixtures ... ok",
    "workday_builtin_profile_compiles_and_executes_cxs_offline_fixtures ... ok",
    "successfactors_builtin_profile_compiles_and_executes_sitemap_html_fallback_fixtures ... ok"
  ],
  "residualRisks": [
    "Detection candidate template pipes remain contrary to the PRD template/transform model.",
    "Workday detection-derived Source Config is declared but not exercised by the slice acceptance test."
  ],
  "noStagedFiles": true,
  "diffSummary": "Rewrites Greenhouse, Workday, and SuccessFactors built-in profiles to schemaVersion 2 DSL and adds deterministic fixtures plus acceptance tests for discovery/detail execution.",
  "reviewFindings": [
    "major: src-tauri/resources/profiles/greenhouse.json:28-29, src-tauri/resources/profiles/workday.json:36-37, src-tauri/resources/profiles/successfactors.json:34-35 - detection candidate templates use PRD-prohibited transform pipes instead of explicit transforms[] behavior.",
    "major: src-tauri/tests/workday_profile_dsl.rs:18-20 and 164-177 - Workday acceptance test checks capture declarations but does not run detection/assert Source Config from detection required by docs/prd/declarative-source-profile-dsl.md:239."
  ],
  "manualNotes": "Source code was not modified; only this Markdown review report was written as requested."
}
```
