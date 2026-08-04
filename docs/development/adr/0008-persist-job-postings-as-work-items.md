# Persist job postings as work items

Job Radar persists found Job Postings as durable user workflow objects. Search Requests remain editable and rerunnable. Terminal Search Runs and their Match relationships are durable, while runtime Candidate Resolutions, criteria snapshots, provider payloads, and Diagnostic history are not.

ADR 0016 supersedes this ADR's original global found-URL identity, parent-owned primary-source pointer, and provisional comparison ownership. Canonical association now uses Source-local Posting Occurrence identity before semantic Job Posting equivalence.

## Decision

Persist normalized Job Postings in SQLite as the primary work items:

- `job_postings` stores durable Posting values, cached description, and manual workflow axes.
- `job_posting_sources` stores Source-local Posting Occurrences and owns the immutable primary marker.
- `search_runs` records terminal executions linked to Search Requests.
- `matches` links each Search Run to each finalized, cross-Source-merged Job Posting exactly once.
- `search_requests` retains its small overwritten latest-run projection.

Search Run history intentionally excludes criteria snapshots, Source Runs, runtime Candidate Resolutions, Diagnostics, usage, provider payloads, and checkpoints.

## Independent workflow axes and queues

Manual Posting state is stored as independent dimensions:

- `read_state`: `unread | read`
- `interest_state`: `undecided | interested | dismissed`
- `preparation_state`: `not_started | in_progress | ready`
- `application_state`: `not_applied | submitted | in_process | rejected_by_company | withdrawn_by_me | accepted`

New Postings default to unread, undecided, not started, and not applied. Rediscovery never resets those axes. User-facing queues and Counts are backend-derived projections of them; `all` is only a list scope and never a Posting's primary queue.

## Association and rediscovery

A Posting Occurrence is identified within one Source by provider posting ID when present, otherwise by normalized provider URL. Provider-ID and URL-fallback identities do not correlate merely by provider URL. Automatic import applies this order:

1. collect all existing owners for every exact Source-local occurrence identity;
2. fail and roll back if exact identities resolve to conflicting durable Job Postings;
3. if an exact owner exists, use it;
4. otherwise compare durable Job Postings by normalized company, title, and overlapping locations;
5. preserve the lowest-ID choice when several semantic candidates remain;
6. create a new Job Posting only when neither exact nor semantic association succeeds.

When an existing Posting is found again, import updates last-seen time, merges locations additively, and creates or updates the canonical occurrence. It does not overwrite title, company, manual workflow state, cached description, or the immutable primary occurrence. Provider URL and `postingMeta` may be updated as occurrence data; neither is an identity substitute.

Each occurrence records its Source key and display-name snapshot, canonical identity kind/value, current provider URL, `postingMeta`, primary marker, and first/last-seen timestamps. Source-name snapshots remain useful when a Source is renamed or removed.

## Search Run transaction

Only active, valid Search Requests may execute and persist results. Partial Source failures still persist successful finalized Postings. A fully failed run records its terminal result but contributes no Posting or Match input.

One private Search Run SQLite transaction atomically writes the terminal Search Run, imported/updated Job Postings and Posting Occurrences, one Match per finalized Posting, and the Search Request's latest-run projection. ADR 0015 owns this transaction; ADR 0016 owns Posting identity, Catalog, and Detail policy. No public repository, transaction, or commit-plan seam is exposed.

The bounded `search-run-result.json` artifact remains post-commit development output. Its failure cannot roll back committed SQLite state.

## Consequences

- Job Postings survive deleting or editing Search Requests.
- Dismissed Postings remain durable and do not return as new merely because they are rediscovered.
- Distinct occurrences from several Sources can belong to one Job Posting without losing Source-local identity.
- The normal UI consumes Job Posting Catalog lists, backend-derived queues/Counts, and Detail rather than treating Search Runs as the workflow.
- Richer activity/history remains a separate product decision.
