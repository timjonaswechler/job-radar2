# Keep Search Request catalog ownership separate from execution

Status: accepted

A focused, Tauri-free `search-requests` crate owns the Search Request Catalog. Its deliberate Interface is `Catalog`: authored lifecycle and criteria, deterministic normalization and Validation, typed errors, concrete private SQLite persistence, and process-local admission for execution and mutation. Desktop is a host and transport adapter; it does not expose a second Search Request application Interface.

SQLite remains a private concrete dependency. There is no repository port because no second persistence adapter exists. The process-local activity gate is likewise private. `begin_execution` yields one immutable authored snapshot and an opaque RAII lease; dropping the lease releases admission on success, error, or cancellation. This prevents update or deletion during an admitted execution and prevents concurrent execution in one process without claiming distributed or multi-process coordination.

Search Run separately owns actual execution, Source Runs, cancellation, terminal outcomes, history, and the latest-run projection. Latest-run columns may remain physically colocated with Search Request rows so ADR 0008 can update them atomically, but they are absent from Catalog Record and Execution values. Desktop may compose a flat user-facing view from a Catalog Record and Search Run's batched or single latest summary; this host composition does not transfer projection ownership to the Catalog.

Search Request lifecycle is authored as `draft`, `active`, or `disabled`, while validity is derived from authored criteria. Invalid drafts remain authorable; activation and execution require valid criteria. A radius affects location matching only when explicitly authored. A missing radius is preserved across editing and reruns rather than being replaced by a later preference, so repeated executions reproduce the saved intent; preferences may seed creation only.

Deleting a Search Request continues to cascade its Search Runs and Matches while preserving Job Postings. This retention behavior is kept for now rather than redesigned as part of the ownership migration.

The Catalog excludes Search Run execution/history, Candidate Resolution, Sources, Source Behavior Language execution, Geo behavior, Background Tasks, and Tauri transport. `search-resolution` continues to own Match Rule executable meaning and Candidate Resolution; the Catalog stores only authored rule values and Source keys.

Rejected alternatives are:

- retaining Search Request behavior in Desktop, because that leaves the host as a second application core;
- introducing a broad `job-search` crate, because Search Request authoring and Search Run execution have different invariants and change drivers;
- merging Search Request ownership into Candidate Resolution, because authored lifecycle and persistence are independent of executable matching and resolution;
- adding a repository trait, because a hypothetical second adapter would widen the Interface without demonstrated variation.
