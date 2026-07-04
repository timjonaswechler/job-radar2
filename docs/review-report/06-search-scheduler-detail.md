# Slice 06 — Search Run, lazy detail, scheduler, smoke tooling

## Scope
Reviewed range `bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31` for:

- `src-tauri/src/search/run/**`
- `src-tauri/src/search/posting/**`
- `src-tauri/src/background_tasks/**`
- `src-tauri/src/app/commands.rs`
- `src-tauri/src/app/state.rs`
- `src-tauri/src/browser_runtime/**`
- `src-tauri/src/search/smoke/**`
- `docs/dev-search-run-smoke.md`

## Commands/Inputs
- Read standards/spec sources: `AGENTS.md`, `README.md`, `CONTEXT.md`, `docs/prd/declarative-source-profile-dsl.md`, `docs/adr/0003-managed-browser-runtime.md`, `docs/adr/0008-persist-job-postings-as-work-items.md`, `docs/adr/0009-declarative-source-profile-dsl.md`.
- Ran requested diff into `/tmp/job-radar-slice06.diff`: 11,735 lines.
- Ran `git diff --stat ...` and `git diff --name-only ...` for the requested slice paths.
- Ran `cargo test --manifest-path src-tauri/Cargo.toml` — passed: 220 non-ignored Rust tests passed, 2 ignored smoke tests.

## Standards Findings
- No blocker/major standards findings. The slice preserves canonical domain terms such as Source, Source Profile, Access Path, Search Request, Search Run, `postingDiscovery`, `postingDetail`, and `postingMeta` in the reviewed Rust/docs paths.
- Coverage is substantial and relevant to the slice. The changed tests include Search Run source selection/failure paths, lazy detail loading, posting import/merge, scheduler artifacts, browser detail seams, and smoke tooling.

## Spec Findings
- Correct: Search Runs resolve and compile selected Sources from one registry snapshot at run start. Evidence: `resolve_selected_sources_with_options` builds a `ProfileCompilerSnapshot` once from the loaded registry snapshot before iterating selected source keys (`src-tauri/src/search/run/service/selection.rs:42-53`), and `SearchRunService::run_with_cancellation` loads the snapshot before resolving selected sources (`src-tauri/src/search/run/service/runner.rs:87-93`).
- Correct: missing Sources fail with structured diagnostics, while draft/disabled Sources are skipped with structured `source_not_active` diagnostics. Evidence: missing source handling emits `source_not_found` (`src-tauri/src/search/run/service/selection.rs:58-71`); non-active handling emits `source_not_active` and returns `Skipped` (`src-tauri/src/search/run/service/selection.rs:74-97`); skipped diagnostics are copied into `SourceRunResult` (`src-tauri/src/search/run/service/source_runs.rs:92-106`).
- Correct: invalid selected Sources fail at source-run level and successful Sources can still produce persisted postings. Evidence: non-executable validation diagnostics are returned as `FailedWithDiagnostics` (`src-tauri/src/search/run/service/selection.rs:100-109`), overall status becomes `completed_with_errors` when at least one source completes and at least one does not (`src-tauri/src/search/run/service/source_runs.rs:117-127`), and successful postings are imported for `Completed` or `CompletedWithErrors` results only (`src-tauri/src/search/run/service/runner.rs:188-194`).
- Correct: lazy `postingDetail` uses persisted posting-source occurrence context. Evidence: detail loading iterates persisted posting sources (`src-tauri/src/search/posting/service.rs:179-241`), builds `PostingDetailPostingOccurrence` from persisted URL/title/company/locations/description/postingMeta (`src-tauri/src/search/posting/service.rs:473-484`), and annotates diagnostics with posting-source context (`src-tauri/src/search/posting/service.rs:493-520`).
- Major: running Search Run cancellation is only cooperative between Sources and is not observable as a cancellation request while one Source is executing. `cancel()` for a running task only flips the token and returns the unchanged running snapshot (`src-tauri/src/background_tasks/mod.rs:324-326`). `SearchRunService` checks the token before starting each selected Source (`src-tauri/src/search/run/service/runner.rs:98-102`) but then awaits `self.source_executor.execute(input)` without passing cancellation into the executor (`src-tauri/src/search/run/service/runner.rs:137-154`). The public `SourceExecutionInput` contains only the Source (`src-tauri/src/search/run/execution.rs:69-74`), and `DefaultSourceExecutor` calls the posting-discovery runtime without a cancellation token (`src-tauri/src/search/run/execution.rs:89-99`). Impact: a long HTTP/browser `postingDiscovery` cannot be cancelled until it returns, and the task snapshot remains `running` with no `cancelling`/cancel-request marker. This falls short of the slice focus that background scheduler cancellation be observable/non-blocking for Search Runs.

## Coverage Notes
- `cargo test --manifest-path src-tauri/Cargo.toml` passed fully for deterministic tests.
- The ignored smoke tests remain manual/network-dependent, consistent with the PRD/README/docs guidance.
- Residual coverage risk: I did not find Search Run tests that cancel while a Source executor is actively blocked; existing scheduler cancellation tests cover generic cooperative work, not the Search Run executor seam.

## Verdict
Needs follow-up for running Search Run cancellation propagation/observability. The Search Run/source selection, partial-failure preservation, lazy detail context, diagnostics, and smoke tooling otherwise align with the reviewed specs.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Reported one major spec finding with file/line citations for running Search Run cancellation propagation/observability; also documented correct behavior with citations."
    }
  ],
  "changedFiles": [
    "docs/review-report/06-search-scheduler-detail.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "git diff --stat bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- src-tauri/src/search/run src-tauri/src/search/posting src-tauri/src/background_tasks src-tauri/src/app/commands.rs src-tauri/src/app/state.rs src-tauri/src/browser_runtime src-tauri/src/search/smoke docs/dev-search-run-smoke.md",
      "result": "passed",
      "summary": "Reviewed slice diff statistics."
    },
    {
      "command": "git diff --name-only bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- src-tauri/src/search/run src-tauri/src/search/posting src-tauri/src/background_tasks src-tauri/src/app/commands.rs src-tauri/src/app/state.rs src-tauri/src/browser_runtime src-tauri/src/search/smoke docs/dev-search-run-smoke.md",
      "result": "passed",
      "summary": "Confirmed changed files in the requested slice."
    },
    {
      "command": "git diff bf0a3ba49ea5f555cad1f53dcc4ab8d6ad1407be..28bfd67f5f6a7688640f925442d2019a05732b31 -- src-tauri/src/search/run src-tauri/src/search/posting src-tauri/src/background_tasks src-tauri/src/app/commands.rs src-tauri/src/app/state.rs src-tauri/src/browser_runtime src-tauri/src/search/smoke docs/dev-search-run-smoke.md > /tmp/job-radar-slice06.diff && wc -l /tmp/job-radar-slice06.diff",
      "result": "passed",
      "summary": "Requested diff captured for inspection; 11,735 lines."
    },
    {
      "command": "cargo test --manifest-path src-tauri/Cargo.toml",
      "result": "passed",
      "summary": "All deterministic Rust tests passed; 220 passed, 2 ignored smoke tests."
    }
  ],
  "validationOutput": [
    "cargo test --manifest-path src-tauri/Cargo.toml: test result ok across lib, integration, and doc tests; 2 network smoke tests ignored."
  ],
  "residualRisks": [
    "Running Search Run cancellation is not propagated into active SourceExecutor/runtime work and has no immediate observable cancel-request state.",
    "Manual live smoke was not run because it is network-dependent and intentionally outside normal CI."
  ],
  "noStagedFiles": true,
  "diffSummary": "Slice adds Search Run integration with compiled Source plans, lazy postingDetail service, background task scheduler/commands, browser runtime seams, smoke tooling, and tests/docs.",
  "reviewFindings": [
    "major: src-tauri/src/search/run/service/runner.rs:98 - cancellation is checked only before each selected Source; src-tauri/src/search/run/execution.rs:69 - SourceExecutionInput has no cancellation token; src-tauri/src/background_tasks/mod.rs:324 - running task cancellation returns unchanged running snapshot."
  ],
  "manualNotes": "Source code was not modified; only this Markdown review report was written."
}
```
