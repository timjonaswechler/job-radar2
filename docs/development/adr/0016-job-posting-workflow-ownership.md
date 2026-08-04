# Keep durable Job Posting workflow ownership in `job-postings`

Status: accepted

A focused, Tauri-free `job-postings` crate owns three Modules: `identity`, `Catalog`, and `Detail`. Desktop owns Tauri transport, composition, and native HTTP/Browser Adapters. Search Run owns execution and its atomic terminal transaction, but not Job Posting identity or workflow policy.

`identity` owns durable Job Posting association policy. A Posting Occurrence has canonical Source-local identity: the concrete Source key plus its case-sensitive provider posting ID when present, otherwise the Source key plus a conservatively normalized absolute HTTP(S) provider URL. Provider-ID and URL-fallback identities do not correlate merely because their provider URLs match. Import checks every exact occurrence identity before semantic company/title/location equivalence. Exact identities must resolve to at most one durable Job Posting; conflicting exact owners fail and roll back the terminal Search Run transaction. Without an exact owner, semantic ambiguity deliberately preserves the existing lowest-ID selection. Provider URL and `postingMeta` are mutable occurrence data used for opening and Detail; neither substitutes for canonical identity.

`Catalog` owns persisted workflow records, batched Posting Occurrence hydration, partial workflow changes, and backend-derived queue lists and Counts. Read, interest, preparation, and application remain independent axes. `all` is only a list scope; each Posting transports exactly one primary workflow queue. The primary Posting Occurrence is occurrence-owned and immutable after creation. Rediscovery may update occurrence data and last-seen time, but does not replace the primary occurrence, title, company, cached description, or manual workflow state.

`Detail` owns opening one Job Posting. Opening marks it read before Source preparation or description acquisition. A failure after that point reports the committed partial effect distinctly. Detail tries the immutable primary occurrence first and then the remaining occurrences, using installed Source behavior and host-supplied HTTP/Browser Adapters. The first successful description is cached indefinitely; concurrent successes retain the first committed value. Refresh, provenance, cancellation, deletion, and semantic-ambiguity repair are excluded.

Search Run retains private automatic-import SQL because one non-cancellable DB01 transaction must atomically persist the terminal Search Run, imported or updated Job Postings and Posting Occurrences, Matches, and latest-run projection. `search-runs::Runner` consumes `job-postings::identity`; it does not call a public repository, transaction, commit-plan, or import Interface. This private SQL integration is the explicit atomicity exception, not a second owner of identity or workflow behavior.

The dependency direction is:

```text
Desktop -> job-postings
Desktop -> search-runs
search-runs -> job-postings::identity
job-postings -> sources + source-engine + sqlx
search-runs -> search-resolution
```

Rejected alternatives were:

- a broad Search Run/workflow owner, because execution history and manual Posting workflow have different invariants and change drivers;
- Desktop-only ownership, because the reusable application behavior would remain coupled to Tauri and native composition;
- a broad shared domain crate, because it would collect vocabulary without one coherent Interface;
- a public repository, transaction, or commit-plan seam, because only one SQLite Adapter exists and exposing transaction mechanics would weaken atomic commit locality;
- moving automatic import behind `Catalog`, because Catalog changes would then have to participate in a caller-owned Search Run transaction.

The canonical identity and occurrence-owned primary schema are a hard development-schema cut. ADR 0005 therefore requires an explicit development database reset/rebaseline after the squashed schema change; compatibility rows must not be assigned invented identities or primary occurrences. ADR 0008 is superseded for old global URL and parent-owned primary language. ADR 0015 retains Search Run transaction ownership while delegating Posting identity policy to this crate.
