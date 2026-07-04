# Slice 07 — Frontend/API cleanup

## Scope
Reviewed the requested range `bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31` for the frontend/API Source/Profile DSL cleanup paths:

- `src/lib/api/sources.ts`
- `src/lib/api/search-requests.ts`
- `src/lib/api/job-postings.ts`
- `src/features/sources/**`
- `scripts/source-ui-contract-tests.mjs`
- `package.json`

## Commands/Inputs
- Read standards/spec sources: `AGENTS.md`, `README.md`, `CONTEXT.md`, `docs/prd/declarative-source-profile-dsl.md`, `docs/adr/0009-declarative-source-profile-dsl.md`.
- Ran requested diff command for the slice paths.
- Inspected current frontend/API files and backend command-boundary structs where needed.
- Ran `grep -RInE 'adapterKey|inventory|source_specific|SourceSpecific|listAdapters|createCustomSource|detectSourceFromUrl|SourceDetectionResult|SourceDetectionMatch|SourceRegistryDiagnostic' src/lib/api src/features/sources scripts/source-ui-contract-tests.mjs package.json || true` — no matches.
- Ran `npm run test:source-ui` — passed.
- Ran `npm run build` — passed.
- Ran `git diff --check ...` for the slice paths — no whitespace errors.

## Standards Findings
- No blocker/major standards findings.
- Correct: canonical Source/Profile DSL terms are used in the reviewed frontend API surface. `SourceStatus` is now only `draft | active | disabled` (`src/lib/api/sources.ts:13`), while derived validation is modeled separately as `ValidationStateKind` and `SourceValidationState` (`src/lib/api/sources.ts:191-199`).
- Correct: removed v1 UI/API concepts are not exposed in the reviewed paths. The grep above found no `adapterKey`, `inventory`, `source_specific`, `SourceSpecific`, old adapter APIs, or old detection/result diagnostic type names.
- Correct: the UI is wired through the new registry snapshot command (`src/features/sources/index.tsx:47-52`, `src/lib/api/sources.ts:262-285`) and the Source Registry view model summarizes support, validation, and capabilities instead of adapters (`src/features/sources/registry-view-model.ts:115-141`, `src/features/sources/registry-view-model.ts:311-347`).

## Spec Findings
- **minor — `src/lib/api/search-requests.ts:70-96`**: `SourceRunResult.diagnostics` and `BackgroundTaskSnapshot.diagnostics` are typed as `unknown[]`, but the backend command boundary serializes structured DSL diagnostics (`src-tauri/src/search/run/types.rs:86-93`, `src-tauri/src/background_tasks/mod.rs:92-100`). This does not break runtime behavior or the build, but it weakens the frontend/API contract for structured diagnostics compared with `src/lib/api/job-postings.ts:60-63`, which correctly uses `StructuredDiagnostic[]`.
- **minor — `src/features/sources/components/source-detection-panel.tsx:76-85` and `src/features/sources/components/source-add-drawer.tsx:238-257`**: the detection panel renders every non-`matched`/non-`ambiguous` result as “Kein vorhandenes Profil erkannt” and states that `startUrl` was adopted when possible. However the handler only inserts `startUrl` for `result.status === "unsupported"`; `failed` is a distinct Source Proposal status in `src/lib/api/sources.ts:248-259`. Failed detection therefore gets misleading unsupported copy and may claim a config value was adopted when it was not.
- Correct: Add Source now consumes `SourceProposal` and emits schema version 2 `SourceDocument` with `selectedAccessPath.type: "profile_access_path"` (`src/features/sources/source-add-model.ts:50-64`, `src/features/sources/source-add-model.ts:102-114`).
- Correct: source-owned Access Paths are represented with the new `source_owned_access_path` shape and support metadata in frontend types and row resolution (`src/lib/api/sources.ts:164-188`, `src/features/sources/registry-view-model.ts:317-328`).
- Correct: job posting detail diagnostics now match the structured diagnostic model (`src/lib/api/job-postings.ts:60-63`).

## Coverage Notes
- `src/features/sources/source-ui-contract-tests.ts` covers the new Source/Profile shapes, support/validation/capability row summaries, Source Proposal conversion, generated Source document shape, source-owned Access Path display, and absence of removed v1 fields (`src/features/sources/source-ui-contract-tests.ts:71-89`, `src/features/sources/source-ui-contract-tests.ts:127-177`, `src/features/sources/source-ui-contract-tests.ts:179-204`).
- `package.json:8-9` adds the source UI contract test script, and `scripts/source-ui-contract-tests.mjs:11-24` bundles/runs it successfully.
- Residual risk: coverage is model/contract-level only. It does not exercise Tauri command invocation, live registry snapshots, or the `failed` Source Proposal UI branch.

## Verdict
Attested with minor follow-ups. The slice substantially satisfies the DSL cleanup intent: old adapter/inventory/source-specific concepts are removed from the reviewed UI/API paths, Source Registry uses support/validation/capabilities, Add Source uses Source Proposal and the new Source shape, and build plus source UI contract tests pass. The two minor issues above should be addressed to tighten diagnostics typing and detection failure UX, but they are not blockers for the cleanup slice.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Reported two minor findings with file paths and line citations; also documented verified no-finding areas with file evidence."
    }
  ],
  "changedFiles": [
    "docs/review-report/07-frontend-api.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "git diff bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- src/lib/api/sources.ts src/lib/api/search-requests.ts src/lib/api/job-postings.ts src/features/sources scripts/source-ui-contract-tests.mjs package.json",
      "result": "passed",
      "summary": "Reviewed requested slice diff."
    },
    {
      "command": "grep -RInE 'adapterKey|inventory|source_specific|SourceSpecific|listAdapters|createCustomSource|detectSourceFromUrl|SourceDetectionResult|SourceDetectionMatch|SourceRegistryDiagnostic' src/lib/api src/features/sources scripts/source-ui-contract-tests.mjs package.json || true",
      "result": "passed",
      "summary": "No removed v1 frontend/API terms found in reviewed paths."
    },
    {
      "command": "npm run test:source-ui",
      "result": "passed",
      "summary": "source UI contract tests passed."
    },
    {
      "command": "npm run build",
      "result": "passed",
      "summary": "TypeScript and Vite production build passed."
    },
    {
      "command": "git diff --check bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- src/lib/api/sources.ts src/lib/api/search-requests.ts src/lib/api/job-postings.ts src/features/sources scripts/source-ui-contract-tests.mjs package.json",
      "result": "passed",
      "summary": "No whitespace errors in requested slice diff."
    }
  ],
  "validationOutput": [
    "npm run test:source-ui: source UI contract tests passed",
    "npm run build: tsc && vite build completed successfully",
    "grep removed terms: no matches"
  ],
  "residualRisks": [
    "No Tauri command/e2e invocation was run for the new frontend API commands.",
    "Source Proposal failed-status UI branch is not covered by the current contract test."
  ],
  "noStagedFiles": true,
  "diffSummary": "Frontend/API cleanup removes adapter/inventory/source_specific concepts, introduces Source Proposal and schemaVersion 2 Source/Profile types, shows support/validation/capabilities in Source Registry UI, adds structured posting detail diagnostics, background task API types, and source UI contract tests.",
  "reviewFindings": [
    "minor: src/lib/api/search-requests.ts:70-96 - diagnostics are typed as unknown[] despite backend returning structured Diagnostics.",
    "minor: src/features/sources/components/source-detection-panel.tsx:76-85 and src/features/sources/components/source-add-drawer.tsx:238-257 - failed Source Proposal detection is rendered with unsupported/startUrl-adopted copy."
  ],
  "manualNotes": "Existing working tree had unrelated unstaged/untracked files before writing this report; no source code was modified by this review."
}
```
