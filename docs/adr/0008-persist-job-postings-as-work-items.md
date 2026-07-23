# Persist job postings as work items

Job Radar persists found job postings as durable work items. A Search Request remains editable and rerunnable. This ADR's original no-history/no-persisted-Search-Request-link claims are superseded narrowly: terminal Search Runs and their Match relationships are durable, while Job Postings remain the user workflow objects and no criteria snapshot or runtime Resolution history is stored.

## Context

Search execution already produces normalized `SearchRunResult.postings`: postings are filtered by include/exclude rules, normalized, deduplicated across selected sources, and contain source references. The development JSON output is useful for inspecting runs, but it is not the product persistence model.

The user-facing work is centered on reviewing and processing job postings:

- identify newly found postings;
- decide whether a posting is interesting;
- prepare an application;
- track whether an application was submitted and what happened next.

It is not currently important to reconstruct which search configuration found which posting. Search requests can be edited over time and rerun from their latest configuration.

## Decision

Persist normalized job postings in SQLite as the primary work items:

- `job_postings` stores the deduplicated posting and manual workflow state.
- `job_posting_sources` stores the found source/link occurrences for a posting.
- `job_postings.primary_source_id` points to the source/link used for the primary “open posting” action.
- `search_runs` records terminal executions linked to `search_requests`.
- `matches` links each Search Run to each finalized, cross-Source-merged Job Posting exactly once.
- `search_requests` also retains its small overwritten last-run status for current-state display.

Search Run history is intentionally narrow: it does not persist criteria snapshots, Source Runs, runtime Resolutions, Diagnostics, usage, provider payloads, or checkpoints.

## Job posting state

Manual posting state is stored as independent dimensions rather than one overloaded status:

- `read_state`: `unread | read`
- `interest_state`: `undecided | interested | dismissed`
- `preparation_state`: `not_started | in_progress | ready`
- `application_state`: `not_applied | submitted | in_process | rejected_by_company | withdrawn_by_me | accepted`

Newly imported postings default to:

```txt
read_state = unread
interest_state = undecided
preparation_state = not_started
application_state = not_applied
```

When an existing posting is found again, manual state is not reset.

## Matching existing postings

Importing a normalized posting uses this order:

1. Find an existing `job_posting_sources.url` with the exact found URL.
2. Otherwise compare against existing `job_postings` using the same title/company/location dedupe semantics as search-run result merging.
3. Create a new posting only if neither step finds a match.

URLs are stored and compared exactly as received from normalized search results. There is no URL normalization key.

When an existing posting is found again:

- update `job_postings.last_seen_at`;
- merge newly observed locations additively;
- create or update the relevant `job_posting_sources` row;
- do not overwrite title, company, primary source, or manual workflow state.

## Source/link occurrences

Each `job_posting_sources` row records one found source/link occurrence:

- `posting_id`
- `source_key`
- `source_name_snapshot`
- `url`
- `first_seen_at`
- `last_seen_at`

`source_name_snapshot` is a display fallback because sources are authoritative JSON documents and may be renamed or removed later.

Duplicate occurrences are prevented per posting/source/url. The same URL may still appear for different source keys so origin remains visible.

## Search-run persistence behavior

Running a search request will automatically persist normalized postings. Only active search requests may run and write postings. Draft, disabled, and invalid search requests must fail with a clear error instead of silently producing persisted data.

Partial source failures still persist successful normalized postings. A fully failed run updates the search request's last-run fields but leaves job postings unchanged.

Persistence of the terminal Search Run, finalized merged postings and posting sources, one Match per posting, and last-run metadata happens in exactly one database transaction.

## Development JSON output

`search-run-result.json` is a bounded post-commit development/debug summary. It is never persistence authority or a release prerequisite; write failure cannot roll back committed SQLite state.

## Consequences

- Job postings survive deleting or editing search requests.
- Dismissed postings stay in the database so they do not reappear as new when found again.
- The normal UI should list and update job postings directly rather than treating search runs as the main workflow.
- Historical analytics over runs are intentionally out of scope for now.
- If future UI needs run history or richer activity, it should be added as a separate product decision rather than inferred from the posting store.
