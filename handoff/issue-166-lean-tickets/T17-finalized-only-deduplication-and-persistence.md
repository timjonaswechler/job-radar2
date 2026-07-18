# T17 — Finalize before cross-Source deduplication and persistence

## Result

A Search Run sends only T16-finalized candidates through the existing backend-owned cross-Source Job Posting merge and persists the resulting Job Postings and Match relationships atomically. Finalized candidates from `Complete` and executable budget-/ceiling-based `Partial` resolutions use the same path; rejected, unresolved, failed, budget-skipped, remaining, cancelled, and execution-aborted Candidate outcomes never become Job Postings, Job Posting Sources, or Matches.

Every committed terminal Search Run also receives durable Search Run identity and normalized Match history without changing existing Search Run or Source Run statuses.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#233/T16](https://github.com/timjonaswechler/job-radar2/issues/233).
- Blocking: none.
- Readiness: **Blocked**; re-baseline the exact landed Candidate Resolution types and tests after #233 completes and before assignment.
- Open decision: none. Durable Search Run and Match persistence, including identity, uniqueness, rerun, retention, cascade, and transaction semantics, is the selected contract.

## Consumed contracts

- #166 / PRD Decisions 41–46: Candidate Resolution owns finalizability, bounded batches, completion/count visibility, Cancellation, finalized-only persistence eligibility, and the separation between Source-local occurrence identity and cross-Source Job Posting deduplication.
- #233/T16 supplies the landed Source-scoped result. Only `SourceResolution.finalized` is eligible downstream; resolved Source Runs expose completion/counts and bounded sanitized Candidate diagnostics, while Cancellation and ordinary execution abort expose no persistable `SourceResolution`.
- T16 also owns normalization and final Include, Exclusion, and current Location evaluation. T17 neither repeats nor reinterprets them.
- `CONTEXT.md` owns the Match meaning: one persisted Job Posting matched one Search Request during one Search Run.
- `handoff/issue-166-delivery.md` supplies shared readiness, hard-cut, testing, migration, deletion, and PR-evidence rules.

## Current gap

This section describes the repository while #233 remains open and is provisional until readiness review.

- `src-tauri/src/search/run/service/runner.rs` currently executes each Source through `SourceExecutor`, collects every `SourceExecutionOutput.candidates` value, normalizes and evaluates rules/Locations in the runner, mutates per-Source `matched_count`, calls `merge_postings`, and conditionally imports postings before updating Search Request last-run metadata in the same SQLite transaction.
- `src-tauri/src/search/run/execution.rs` returns `SourceExecutionOutput { candidates: Vec<SourceCandidate>, diagnostics }`; `src-tauri/src/search/run/types.rs` has no finalized/non-final Candidate distinction, Resolution completion/count pair, or derived Search Run Resolution summary.
- `src-tauri/src/search/run/service/merging.rs` applies `search/posting/matching.rs::same_job_posting`, keeps first-representative order, merges Locations, and retains distinct Source rows.
- `src-tauri/src/search/posting/mod.rs` imports `SearchRunResult.postings`, resolves an existing Job Posting by Source URL plus `postingMeta` or by the backend deduplication rule, and inserts or updates `job_postings` and `job_posting_sources`. Its input cannot prove that a posting came from a finalized Candidate.
- `src-tauri/src/search/run/service/persistence.rs` updates Search Request last-run metadata and writes the optional post-commit development artifact; it does not persist Search Run identity or Matches.
- `src-tauri/migrations/20260609000000_current_schema.sql` contains `search_requests`, `job_postings`, and `job_posting_sources`, but no `search_runs` or `matches` tables.
- Current merge, useful-partial-failure, Cancellation, rollback, and real temporary-SQLite import behavior is covered by `search/run/tests/{deduping,failures,lifecycle}.rs` and `search/posting/tests/import_and_merge.rs`; deterministic execution support is in `search/run/tests/support.rs`.

The missing boundary is a single production construction path from T16 finalized values to cross-Source merge and atomic persistence, together with durable Search Run/Match representation. Exact future symbols and focused test names must follow #233's landed code without changing this responsibility.

## Target delta

T17 keeps persistence behind `SearchRunService::run_with_cancellation` and the existing internal transaction boundary. It adds no second Candidate Resolution and no public persistence facade.

### Finalized-only handoff and merge

1. For each successful T16 `SourceResolution`, convert only `resolution.finalized`, in selected-Source order and then provider/finalization order, into the input of `service/merging.rs`.
2. The conversion preserves finalized title, company, absolute URL, Locations, concrete Source key/name, and Source-local posting metadata required for Source-row identity and later lazy Detail reuse.
3. A finalized Candidate has already been normalized and passed final Search Request evaluation. T17 performs no second normalization, Include/Exclusion evaluation, Location evaluation, or finalizability check.
4. Collect eligible values from all resolved Sources before cross-Source merging. Preserve `same_job_posting`, first-representative ordering, additive Location merge, and distinct Source-row behavior. `PostingOccurrenceIdentity` does not become the cross-Source tolerance.
5. `Complete` and executable budget-/ceiling-based `Partial` resolutions use the same conversion, merge, and import path. Partial completion does not make finalized values provisional.
6. Candidate-scoped failure does not suppress other finalized values from that Source. Failed, skipped, or pre-resolution-aborted Sources contribute no finalized payload; other resolved Sources retain existing useful-result behavior.
7. Cancellation or ordinary execution abort releases no `SourceResolution`. A cancelled Search Run imports no Candidate-derived Job Posting, Job Posting Source, or Match, including work finalized earlier in the uncommitted run, and never synthesizes Partial completion.
8. `SearchRunResult.postings` contains post-finalization, post-cross-Source-merge values only. The importer receives no Candidate state, continuation, Detail-round, Partial-reason, or diagnostic-sampling payload and does not reconstruct eligibility.
9. T16 completion/counts, landed usage fields, derived Search Run Resolution summary, the immutable ten-entry per-Source Candidate Diagnostic sample limit, and `candidateDiagnosticsOmitted` naming remain caller-visible. They are not persisted or copied into an unbounded artifact collection. If `matched_count` remains, derive it from that Source's finalized values admitted before cross-Source collapse; it is not a mutable persistence authority.

### Durable persistence contract

The squashed development schema gains the following minimum model:

```sql
CREATE TABLE search_runs(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  search_request_id INTEGER NOT NULL
    REFERENCES search_requests(id) ON DELETE CASCADE,
  status TEXT NOT NULL,
  generated_at TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT(strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK(status IN('completed', 'completed_with_errors', 'failed', 'cancelled'))
);
CREATE INDEX idx_search_runs_search_request_id
  ON search_runs(search_request_id);

CREATE TABLE matches(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  search_run_id INTEGER NOT NULL
    REFERENCES search_runs(id) ON DELETE CASCADE,
  job_posting_id INTEGER NOT NULL
    REFERENCES job_postings(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL DEFAULT(strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(search_run_id, job_posting_id)
);
CREATE INDEX idx_matches_job_posting_id ON matches(job_posting_id);
```

Persistence invariants:

1. Each committed terminal `SearchRunResult` inserts exactly one Search Run row, including `completed`, `completed_with_errors`, `failed`, and `cancelled`.
2. The Search Run's required `search_request_id` is the single normalized path from a Match to its executing Search Request. `matches` must not duplicate `search_request_id`. Match also gains no status, score, Source key, Candidate state, or provider payload.
3. Completed and completed-with-errors runs may import finalized Job Postings/Sources and one Match per distinct post-merge posting. A posting merged from several Sources remains one Job Posting, several Source rows, and one Match for that run.
4. Failed and cancelled runs persist their Search Run row with zero Matches and import no Candidate-derived Job Postings or Source rows.
5. The importer resolves or inserts each post-merge Job Posting, obtains its ID without a second public repository interface, and inserts exactly one `(search_run_id, job_posting_id)` Match.
6. Each explicit rerun creates a new Search Run ID and may create a new Match for the same Search Request and existing Job Posting. There is no cross-run upsert or inferred idempotency. Duplicate insertion within one run fails through the unique constraint rather than silently upserting.
7. Search Runs and Matches are retained without automatic pruning. Deleting a Search Request cascades its Search Runs and Matches but not Job Postings; deleting a Job Posting cascades its Matches but not Search Runs.
8. Search Run insertion, Job Posting/Source import, Match insertion, and Search Request last-run update are one transaction. Any validation, SQL, foreign-key, uniqueness, or metadata error rolls back all of them. The optional development artifact is written only after commit and is non-authoritative.
9. T16 Resolution counts are not recomputed from Matches or merged postings. Cross-Source collapse may reduce `SearchRunResult.postings.len()` while Source-local finalized counts remain unchanged. For a committed completed/completed-with-errors run, Match count equals distinct post-merge posting count.
10. T17 adds no durable Source Run, Resolution, Diagnostic, usage, Candidate-outcome, continuation, provider-payload, or Search Request criteria-snapshot table and introduces no new status variant or production limit.

The selected persistence decision supersedes the conflicting “Search Runs are not historized” direction in `docs/adr/0008-persist-job-postings-as-work-items.md`; that ADR must be updated or superseded in the implementation slice so only one active decision remains.

## Dependency and deletion decision

- Finalized conversion, ordering, count association, and cross-Source merge remain concrete in-process logic.
- T16 `SourceResolution`/`FinalizedCandidate` values are typed immutable inputs, not a port.
- SQLite is local-substitutable persistence: production SQL and a real migrated temporary SQLite database are used in tests; no repository trait or persistence mock is introduced.
- Search Run setup tests use T16's landed deterministic Source execution implementation; HTTP/browser/provider execution is absent from T17 persistence logic.
- The optional artifact filesystem continues to use a temporary directory in tests and remains post-commit.

**Deletion test:** Without the finalized-only handoff, Search Run orchestration, cross-Source merging, and the importer would each need to reconstruct Candidate eligibility and Cancellation/Partial rules, permitting non-final Candidate payloads to reach SQLite. A wrapper that forwards an all-candidate vector or rechecks finalization inside SQL fails this test.

## Examples

1. **Complete and cross-Source merge:** Source A finalizes A1/A2 and Source B finalizes B1; A1 and B1 satisfy the existing backend tolerance. Merge input is A1, A2, B1. The run stores two Job Postings, three Source rows, and two Matches; unresolved report data stores no posting-shaped row.
2. **Useful Partial completion:** a budget-limited Source reports two finalized, one unresolved, and two budget-skipped Candidates. The two finalized values follow the normal merge/import path; all counts/completion remain visible, while the other three outcomes create no Job Posting, Source, or Match.
3. **Candidate failure:** A1 finalizes, A2 fails during Detail, and A3 finalizes. A1/A3 persist; A2 remains only in T16 counts and bounded diagnostics.
4. **Cancellation:** A1 finalizes and Cancellation is observed before commit. The Search Run commits one cancelled row, zero Matches, and no Candidate-derived Job Posting/Source import; no Partial completion is synthesized.
5. **Rerun and retention:** Run 41 and later Run 52 of the same Search Request both find Job Posting 12. Each has its own Match; duplicate `(52, 12)` fails. Deleting the Search Request removes both runs and Matches but leaves Job Posting 12.

## Scope

- Re-baseline against #233's landed Source Resolution, finalized Candidate, completion/count, diagnostics, usage, abort, and Search Run integration types.
- Route only finalized values into the existing cross-Source merge; preserve provenance, order, posting metadata, backend tolerance, and Source-row identity.
- Use one path for finalized values from Complete and executable Partial resolutions; keep Cancellation and execution abort outside that path.
- Add the minimum `search_runs` and `matches` tables, constraints, indexes, and cascades to the squashed development schema.
- Persist one Search Run per terminal committed result and one Match per distinct post-merge Job Posting where the status permits posting import.
- Return/resurface resolved Job Posting IDs internally as needed for Match insertion without adding a public repository facade or second deduplication query.
- Keep Search Run, posting/source, Match, and Search Request last-run writes atomic; retain artifact writing after commit.
- Update or supersede ADR 0008 for the durable history decision.
- Migrate production callers/tests directly and delete every all-candidate merge/import path, forwarding conversion, duplicate finalization gate, independently mutable duplicate count, persistence mock, and superseded fixture that can place a non-final Candidate in `SearchRunResult.postings`.

## Adjacent non-goals

- Candidate Resolution, batching, Detail requests/rounds, normalization, final Search Request evaluation, budgets, completion/count arithmetic, diagnostic sampling, or Source execution protocols: #233 owns them.
- Changes to cross-Source matching tolerance, Source-local occurrence identity, Source-row identity, Location normalization, or Job Posting update semantics.
- Structured Location semantics: [#57](https://github.com/timjonaswechler/job-radar2/issues/57).
- Durable Source Runs, Resolution/Diagnostic/usage snapshots, Candidate-outcome tables, criteria snapshots, continuations/checkpoints, retry queues, resumability, or automatic pruning.
- UI-only lazy Description loading or Description persistence.
- Parallel persistence or a provider-specific lifecycle branch.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| Complete resolution | Finalized values merge/import; completion/counts remain visible | `SearchRunService` temporary-SQLite integration |
| Unresolved Candidate | A finalized sibling persists; unresolved Candidate creates no posting/source/Match row | Search Run integration with exact table counts |
| Rejected Candidate | One terminal Search Run, zero Matches/posting/source rows, exact rejection count | Search Run integration with exact table counts |
| Budget-skipped Candidate | One terminal Search Run, zero Matches/posting/source rows; count invariants remain visible | Search Run integration with exact table counts |
| Provider-known remaining | No speculative rows; `remaining` stays report data only | Search Run result plus SQLite assertions |
| Candidate failure | Other finalized values persist; failed Candidate does not; count/Diagnostic remains | Search Run integration |
| Executable Partial | Finalized values use normal Match path; non-final values do not | Search Run integration |
| Resolved plus failed Source | Completed-with-errors run persists only resolved Source Matches | Search Run integration |
| Fully failed run | One failed Search Run, zero Matches/posting/source imports, atomic last-run update | Temporary-SQLite integration |
| Skipped Source | No Resolution or persisted Candidate-derived row for the skipped Source; other finalized work keeps existing status behavior | Source-key-specific SQLite assertions |
| Pre-resolution failure | No Resolution or persisted Candidate-derived row for the failed Source; other finalized work persists | Source-key-specific SQLite assertions |
| Cancellation after finalization | One cancelled Search Run, zero Candidate-derived posting/source/Match rows, no Partial | Cancellation integration with real SQLite |
| Cross-Source dedupe | Equivalent finalized postings become one Job Posting, multiple Source rows, one Match | Existing `deduping` regression plus SQLite assertions |
| Source-row identity | Exact Source URL/`postingMeta` behavior remains unchanged | `search::posting::tests::import_and_merge` |
| Count separation | Two Source-local finalized outcomes may collapse to one posting/Match without changing counts | Search Run result and SQLite assertion |
| Atomic rollback | Any invalid posting or SQL/constraint failure leaves no run/posting/source/Match/last-run subset | Transaction rollback integration |
| Rerun | Two invocations create two runs and two Matches to one durable Job Posting | Temporary-SQLite integration |
| Duplicate within run | Duplicate Match violates uniqueness and rolls back | Import transaction test |
| Request deletion | Runs/Matches cascade; Job Postings remain | Migrated-schema foreign-key test |
| Posting deletion | Matches cascade; Search Runs remain | Migrated-schema foreign-key test |
| Retention | Repeated runs remain until explicit deletion | Temporary-SQLite integration |
| Status schema | Four existing statuses succeed; unknown status fails | Direct migrated-schema SQL test |
| Required parent foreign keys | Inserts with missing Search Request, Search Run, or Job Posting parents fail with foreign-key enforcement enabled | Direct migrated-schema SQLite assertions |
| Normalized Match link | `matches` has run/posting FKs and no `search_request_id` | `PRAGMA table_info` / `foreign_key_list` assertions |
| Artifact | Post-commit artifact contains merged finalized postings and bounded Resolution metadata, no non-final payload collection | Temporary-file JSON assertion |
| Generic regression | Greenhouse/Workday/SuccessFactors-shaped finalized results use one path | Offline fixtures and call-graph review |
| Migration/deletion | One production construction path reaches importer; no duplicate gate/mock/wrapper/all-candidate path | Repository searches and manual call-graph review |

Tests cross `SearchRunService::run_with_cancellation` with T16's deterministic Source execution implementation while using real in-process merge/finalized handoff and a real migrated temporary SQLite database. Direct importer tests may construct valid already-finalized `NormalizedPosting` fixtures. No network-dependent test is added.

### Focused commands

Use #233's exact landed Candidate Resolution target/filter names after readiness re-baseline. Current and expected focused coverage is:

```bash
cargo test --manifest-path src-tauri/Cargo.toml search::run::tests::source_execution
cargo test --manifest-path src-tauri/Cargo.toml search::run::tests::matching
cargo test --manifest-path src-tauri/Cargo.toml search::run::tests::deduping
cargo test --manifest-path src-tauri/Cargo.toml search::run::tests::failures
cargo test --manifest-path src-tauri/Cargo.toml search::run::tests::lifecycle
cargo test --manifest-path src-tauri/Cargo.toml search::posting::tests::import_and_merge
cargo test --manifest-path src-tauri/Cargo.toml --test candidate_resolution
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
```

Ticket-specific searches, adapted to #233's landed names:

```bash
rg -n '\b(SourceResolution|FinalizedCandidate|ResolutionCompletion|ResolutionCounts|SearchRunResult|SourceRunResult)\b' src-tauri/src/search src-tauri/tests --glob '*.rs'
rg -n '\b(unresolved|failed|rejected|budget_skipped|budgetSkipped|remaining|finalized)\b' src-tauri/src/search/run src-tauri/src/search/posting --glob '*.rs'
rg -n 'merge_postings|same_job_posting|import_search_run_result_in_transaction|update_search_request_last_run|search_runs|matches' src-tauri/src/search src-tauri/migrations src-tauri/tests --glob '*.rs' --glob '*.sql'
rg -n '\b(candidate_count|matched_count|SourceStatus|SourceRunStatus|SearchRunStatus)\b' src-tauri/src/search src-tauri/tests --glob '*.rs'
rg -n '\b(SourceExecutor|SourceExecutionOutput|SourceCandidate)\b' src-tauri/src/search src-tauri/tests --glob '*.rs'
```

Every remaining hit requires manual classification; absence alone is not proof.

## Ticket-specific migration items

- [ ] Re-baseline exact T16 types, abort behavior, tests, and one finalized construction path after #233 closes.
- [ ] Update or supersede ADR 0008; leave no active conflicting Search Run persistence decision.
- [ ] Add `search_runs` and `matches` with the selected fields, status check, required foreign keys, indexes, uniqueness, and cascades.
- [ ] Route only T16 finalized values, preserving selected-Source/finalization order and Source provenance, into `merge_postings`.
- [ ] Persist one Search Run for every terminal committed result and one Match per eligible post-merge Job Posting; prove rerun, retention, and cascade behavior.
- [ ] Keep Search Run, Job Posting/Source, Match, and last-run writes in one transaction; preserve post-commit artifact behavior.
- [ ] Prove all non-final outcomes and cancelled/aborted work have no posting-shaped persistence path.
- [ ] Delete/derive any independently mutable `matched_count` and remove all-candidate conversions, duplicate eligibility gates, forwarding wrappers, persistence mocks, duplicate fakes, and superseded tests.
- [ ] Run and classify every ticket-specific search above, including status and old execution-type hits.

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
