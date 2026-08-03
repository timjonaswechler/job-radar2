# Keep Search Run execution and history together

Status: accepted

A focused, Tauri-free `search-runs` crate owns two host-facing Modules: `Runner` and `History`. `Runner` owns one admitted Search Request execution from requirements coordination through terminal commit. `History` owns the single and batched latest-run projection reads. Desktop remains the host for Tauri transport, generic Background Task scheduling and notification, process-local global Search Run serialization, and native Adapters; it does not expose a second Search Run application Interface.

`Runner` consumes the immutable `search_requests::Execution` admitted by the Search Request Catalog. The Catalog retains admission and RAII lease ownership. Desktop acquires the lease before scheduling and holds it until the scheduled work ends: queued cancellation or admission/setup failure can release it without entering `Runner`; once `Runner` starts, its `Outcome` or operation error returns before release. A run uses one operation-local installed Source snapshot. `sources` retains installed Source state and exact preparation, `source-engine` retains declarative Source execution behavior, and `Runner` owns the active-only versus development-smoke Source admission policy for the run. `search-resolution` retains Candidate Resolution, `geo` retains geographic resolution, and the Search Request Catalog retains authored lifecycle and validity.

The concrete SQLite implementation is private because no second persistence Adapter exists. One non-cancellable terminal transaction atomically writes the terminal Search Run, automatically imported or updated Job Postings and their Source occurrences, one Match per finalized cross-Source-merged Posting, and the Search Request's overwritten latest-run projection. Failed or cancelled runs contribute no Posting or Match input. Search Request deletion continues to cascade Search Runs and Matches while durable Job Postings survive.

For work that has entered `Runner`, cancellation linearizes at the final cancellation-token observation immediately before that transaction. Cancellation observed before the cutoff persists a cancelled terminal Outcome; after the cutoff, the committed Outcome is authoritative even if Desktop has already requested cancellation. Desktop may cancel a queued Background Task before `Runner` starts; that generic task cancellation has no Search Run Outcome or durable Search Run. Requirements, Geo, installed-state, timestamp, and storage failures before a successful terminal commit are non-durable operation errors.

Source outcomes, runtime Candidate Resolutions, Structured Diagnostics, usage, provider payloads, criteria snapshots, and checkpoints remain ephemeral. Partial Source failures can produce `completed_with_errors`; only finalized Candidate Resolution values enter cross-Source merge and persistence. Richer durable Search Run or Source Run history is deferred until required by a product workflow.

`History` owns latest-run status decoding, missing-value defaults, corruption and storage distinctions, and bounded batched reads, even though the physical latest-run columns remain on `search_requests` for atomic commit locality. `Runner` and `History` exclude Search Request CRUD and admission, Source behavior, Candidate Resolution internals, Geo behavior, generic Background Tasks, Tauri transport, and Job Posting workflow operations.

Automatic Posting import is private `Runner` implementation required by the atomic terminal transaction. Job Posting listing, queues, manual workflow changes, and detail loading remain separate Desktop workflow ownership pending the Job Posting Workflow investigation. ADR 0008's exact-URL lookup wording and the current `(source_key, url)` lookup behavior are not reconciled here; that mismatch is deferred to the same investigation.

The bounded `search-run-result.json` artifact is a Desktop post-commit Adapter. It is never persistence authority, and artifact failure can add a warning but cannot change the committed Outcome. The development smoke runner is a second productive `Runner` consumer without making Tauri or Desktop a crate dependency.

The frontend separately owns one feature-local Search Run operation lifecycle. It composes the Search Run transport with generic Background Task polling while preserving the current one-second polling interval and lack of navigation recovery. Push notification, task listing/recovery, retention, and panic supervision remain Platform Integration questions. Concurrent and multi-process Search Run policy also remains deferred; the current Desktop process-local serialization is unchanged.

Rejected alternatives are:

- splitting execution from a public ledger/commit-plan Module, because it would expose transaction mechanics and weaken commit locality;
- retaining a Desktop-only Search Run Module, because Desktop and the smoke runner need the same execution behavior and Desktop would remain a second application core;
- introducing a broad `job-search` crate, because Search Request authoring, Search Run execution, and Candidate Resolution have different invariants and change drivers;
- introducing a premature `job-postings` crate, because automatic import is currently transaction-private while the wider Job Posting workflow boundary has not been researched.
