# T14d — Remove the replaced Detection execution path

## Result

Every production and deterministic-test caller enters Profile Detection through the one typed Strategy Set-based Detection operation landed by T14a–T14c. Repository checks prove that no imperative Detection executor, compatibility wrapper, duplicate acquisition dispatch, or superseded implementation-detail test remains.

If T14c already completed production convergence, this ticket creates no naming-only production change: it adds the missing committed convergence guard, or closes with verification evidence only when that complete guard and its self-tests already landed legitimately.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#207](https://github.com/timjonaswechler/job-radar2/issues/207) (T14c) only.
- Blocking: none currently recorded.
- Readiness: **Blocked**; #207 is open and this issue has no `ready-for-agent` label. Re-baseline the call graph and exact landed names after #207 completes.
- Open decision: none. If the post-#207 checks are already clean, verify or add the guard rather than inventing implementation churn.

## Consumed contracts

- #166 / PRD Decisions 8, 22–30, 43, and 48: typed immutable Detection plans, shared bounded Strategy execution, deterministic reducers, typed Cancellation outside persistable Partial Completion, and immutable browser ceilings.
- #166 / PRD Strategy Set Runtime module decision: callers use the typed Detection phase operation; policy execution, attempt history, budgets, provenance, and reducers remain private.
- T14a/#205 supplies compiled URL/HTTP Detection Strategies, the shared byte-preserving HTTP seam, cumulative budgets, typed Cancellation, and deletion of replaced URL/HTTP executors.
- T14b/#206 supplies conflict-safe Detection contribution reduction, ordered provenance, and one canonical Source Proposal constructor responsibility.
- T14c/#207 supplies browser Detection through the same operation and the managed/scripted browser lifecycle seam while deleting the imperative browser-probe path.
- Shared readiness, hard-cut, seam, testing, and evidence rules follow `handoff/issue-166-delivery.md`.

## Current gap

The repository is still pre-T14a–T14c, so this section is provisional until readiness review. Current production code has a parallel imperative orchestration path:

- `src-tauri/src/source_profile/detection/mod.rs` exports `detect_source_proposal`, `detect_source_proposal_with_http_client`, and `detect_source_proposal_with_clients`; they forward through `detect_source_proposal_internal`, which iterates profiles and calls `evaluate_profile`.
- That evaluator directly sequences `match_input_url_patterns`, `evaluate_http_checks`, optional `evaluate_browser_probes`, mutable evidence/capture aggregation, and proposal construction.
- `source_profile/detection/http.rs` owns the Detection-only `DetectionHttpClient`, response/error family, no-op/Reqwest implementations, and HTTP evaluator; `detection/browser.rs` owns the browser-probe evaluator and direct evidence mutation.
- `source_profile/documents.rs` and `schema/source-profile.schema.json` still expose the imperative `detect` URL/HTTP/browser-probe model.
- `src-tauri/src/lib.rs` re-exports the wrapper and Detection-only HTTP surfaces. `app/commands.rs::detect_source_proposal_from_url` selects clients and forwards through `detect_source_proposal_from_url_with_clients`.
- `src-tauri/tests/source_profile_detection.rs` exercises all wrapper variants with old-seam fakes; `workday_profile_dsl.rs` uses the no-client convenience operation.
- `source_profile/detection/proposal.rs` currently owns final proposal construction, but receives mutable inputs prepared by the imperative path.
- `src-tauri/tests/detection_convergence_evidence.sh` does not yet exist.

T14a–T14c are expected to replace most or all of this baseline. T14d closes only cross-slice residue: leftover caller knowledge, forwarding families, duplicate dispatch, old-seam tests/fakes, and the absence of a durable convergence guard.

## Target delta

Preserve the exact typed Detection operation landed by #205–#207. The responsibility-level shape is:

```rust
pub async fn detect_source_proposal(
    request: DetectionRequest<'_>,
    dependencies: DetectionDependencies<'_>,
    control: DetectionExecutionControl<'_>,
) -> Result<SourceProposalDetectionResult, DetectionCancelled>;
```

Landed names and argument grouping are authoritative. This sketch does not authorize a new facade, aggregate, overload, or forwarding wrapper.

Ticket-specific invariants:

1. The production Source-setup command supplies ordered immutable Registry/Profile input, shared production HTTP/browser dependencies, and runtime control, then calls the operation exactly once.
2. Deterministic tests use the same operation with the blocker-landed scripted HTTP/browser implementations. They cannot call URL, HTTP, or browser evaluators; reducers; proposal builders; or alternate convenience entry points.
3. URL-only plans use the same dependency and execution contract without a no-client overload, no-op HTTP client, regex executor, or second runtime path.
4. Runtime executes compiled immutable Detection plans. Profile/Strategy ordering, policy stopping, cumulative budgets, attempt history, reduction, provenance, Diagnostics, success/unsupported/ambiguous behavior, and serialization remain unchanged from the blockers.
5. Accepted URL, HTTP, and browser contributions enter one conflict-safe reducer. Conflicting non-empty captures, Source Config contributions, or Access Path recommendations fail the profile rather than using last-write-wins.
6. `source_profile/detection/proposal.rs`, or its directly renamed canonical successor, remains the sole final Source Proposal constructor responsibility. Acquisition code and callers cannot construct or mutate final proposals.
7. Cancellation remains typed control flow, suppresses later Strategies and proposal construction, and is never inferred from Diagnostics or represented as persistable `ResolutionCompletion::Partial`.
8. HTTP/browser representations and errors are translated at the blocker-landed adapter edges. Compilation, policy state, dispatch, reducers, aggregation, budgets, and Diagnostic projection remain private.
9. No serialized result, status, DTO field, browser ceiling, teardown rule, policy rule, reducer rule, or provenance contract changes in this cleanup.

Add `src-tauri/tests/detection_convergence_evidence.sh` as the single owner of the focused static guard and changed-path evidence logic. It must expose `--self-test` and real-repository `--check` modes that invoke the same internal functions and:

- search Rust under `src-tauri/src` and `src-tauri/tests` for exact retired wrappers, evaluator signatures, URL matcher, browser-probe authored names, and applicable Detection-only HTTP/document symbols;
- treat `rg` exit 0 as a forbidden hit, 1 as clean, and greater than 1 as a propagated tool failure;
- inspect committed changes from validated `IMPLEMENTATION_BASE...HEAD` (default `origin/main`) plus staged, unstaged, and untracked paths, checking each Git producer independently;
- retain NUL-delimited paths through collection and consume them with `read -r -d ''`, including nested paths and whitespace; an empty path set performs zero inspections and succeeds;
- retain non-Rust changed paths during collection but skip their Rust inspection;
- reject, in changed production Rust, names where `legacy`, `compat`, `migration`, `alias`, or `forwarder` is joined to `detection`;
- reject explicit Greenhouse/Workday/SuccessFactors literal comparisons or match arms and direct single-line `if`/`match` dispatch on `profile_key` or `source_key`, without rejecting ordinary key access, provider fixture data, historical Markdown, legitimate shared adapters, or bare words such as `host`, `company`, or `forward`; multiline/indirect dispatch remains a call-graph review responsibility;
- print the canonical proposal-constructor and Detection-operation call-graph inventories for review, with explicit hit/no-hit/tool-error handling separate from forbidden-search exit semantics.

`--self-test` must use temporary repositories and controlled `rg`/`git` shims while invoking the production guard, Git-producer, NUL-path, and changed-file inspection functions. It proves that an invalid base exits before any producer runs; each of the comparison-range, staged, unstaged, and untracked producers propagates failure independently; a nested whitespace path reaches inspection unchanged as one path; and a collected non-Rust path is preserved but skipped by Rust checks. It also covers empty input, committed/staged/unstaged/untracked Rust changes, the full retired-symbol table, shared-adapter exclusions, migration names, each direct-dispatch shape, and non-triggering ordinary key/fixture use.

The retired-symbol table must cover each still-applicable old symbol named in this ticket's migration items. If a blocker legitimately reuses a spelling for a shared adapter, narrow the check to the retired module/signature/call edge and record the classification; never suppress the whole family.

## Dependency and deletion decision

Compiled plans, policy state, reducers, provenance, aggregation, Diagnostics, control, and immutable Registry/Profile inputs stay in-process. HTTP reuses the shared production/deterministic external seam from T14a; browser execution reuses T14c's managed/scripted lifecycle seam. The Tauri command translates application input and dependencies once. Detection uses no SQLite dependency.

No new port, facade, clock-policy trait, ledger trait, fake Cancellation provider, or browser seam is introduced.

**Deletion test:** Removing the canonical typed Detection operation would force the Source-setup command, adapters, acceptance fixtures, and tests to relearn profile/Strategy ordering, policy transitions, cumulative limits, Cancellation, contribution reduction, provenance, proposal construction, and deterministic Diagnostics. Removing any leftover wrapper or imperative evaluator must make no required behavior disappear.

## Examples

1. **Production:** the command replaces wrapper selection with one call to the landed operation and does not sequence profiles, Strategies, or acquisition kinds.
2. **URL-only:** an accepted compiled URL Strategy completes through the same operation without invoking HTTP/browser dependencies or selecting a no-client compatibility path.
3. **Conflict:** differing accepted HTTP and browser values for one Source Config property produce the landed ordered conflict Diagnostic and no proposal for that profile.
4. **Cancellation:** Cancellation after HTTP evidence but before a browser Strategy returns the typed Cancellation result; no later Strategy or proposal construction runs and prior contributions are not released.
5. **Already converged:** if T14c left no product-code residue, add only the missing complete guard. True no-code closure is allowed only if that committed guard and its self-tests already exist and all checks pass.

## Scope

- Re-inventory post-#207 exports, callers, tests, compiler/registry entry, Strategy kernel, external adapters, control, reducer/provenance, proposal construction, and serialized results.
- Move every residual production and deterministic-test caller directly to the one landed Detection operation.
- Delete forwarding overloads, optional/no-client wrappers, command-local forwarding helpers, compatibility dispatch, per-profile orchestration, acquisition evaluator bridges, test-only production entry points, duplicate dispatch, and replaced re-exports.
- Delete Detection-only HTTP/browser types only where the shared blocker-landed seam replaced their external role; preserve legitimate shared adapters.
- Delete direct capture/evidence/proposal mutation outside the canonical contribution → reducer → constructor flow.
- Replace superseded implementation-detail tests and old-seam fakes with equivalent coverage through the public typed operation.
- Add the complete convergence guard and its self-tests when absent.
- Update only active canonical Detection documentation that still describes an executable replaced path.
- Avoid production-code churn if the landed call graph is already converged.

## Adjacent non-goals

- Changing URL/HTTP Strategy execution (#205), Detection reduction/provenance (#206), or browser execution, lifecycle, ceilings, accounting, and teardown (#207).
- Changing Strategy Policy, cumulative budget, Diagnostic ordering/codes, Registry ordering, support/ambiguity behavior, Source Proposal serialization, or Cancellation semantics.
- Introducing Detection Source specialization, Candidate Resolution, Search Run persistence, Source Live Check changes, Source lifecycle changes, structured Location work, UI redesign, parallel execution, or resumability.
- Creating another facade, service, port, adapter family, compatibility runtime, migration alias, or provider-specific Rust dispatch.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| URL/HTTP and browser success | One public operation, reducer/provenance path, and proposal constructor; landed ordering and result stay stable | `source_profile_detection` plus call-graph guard |
| URL-only | Same operation succeeds with no external call and no no-client/no-op compatibility path | Deterministic public Detection test |
| Ambiguous profiles | Existing deterministic ambiguous result and proposal order | `source_profile_detection` |
| Conflicting contributions | Landed conflict Diagnostic; no partial proposal or last-write-wins | Public Detection reducer regression |
| Recovered attempt | Earlier failure remains observable; later acceptance follows landed policy | Public Detection policy regression |
| One-over budget | Typed budget terminal; no alternate executor or later Strategy | Blocker-landed public budget regression |
| Cancellation | Typed Cancellation; no later work/proposal and no persistable Partial Completion | Blocker-landed public Cancellation regression |
| Retired authored/runtime symbol | Schema/Serde or convergence guard fails | `schema_validation`; guard `--check` |
| Caller/constructor ownership | All callers use one operation; only canonical constructor responsibility builds proposals | Guard inventory plus reviewed call graph |
| Guard mechanics | Hit/no-hit/tool-error, invalid base, each producer failure, empty/NUL path, committed/staged/unstaged/untracked, exclusions, and dispatch checks behave exactly as specified | Guard `--self-test` |
| Acceptance profiles | Greenhouse, Workday, and SuccessFactors remain data-driven and serialized results remain stable | Three profile targets |
| Already-converged repository | Guard-only change when absent; no product-code change solely to create a diff | Git diff and call-graph review |

Tests cross the final typed Detection operation with the real in-process compiler/kernel/reducer/constructor and blocker-landed deterministic HTTP/browser adapters. They do not inject policy, budget, acceptance, or Cancellation results through private helpers. No network- or installed-browser-dependent test enters default CI.

### Focused commands

```bash
bash src-tauri/tests/detection_convergence_evidence.sh --self-test
IMPLEMENTATION_BASE="${IMPLEMENTATION_BASE:-origin/main}" \
  bash src-tauri/tests/detection_convergence_evidence.sh --check
cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation
cargo test --manifest-path src-tauri/Cargo.toml --test source_profile_detection
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
git diff --check
```

Also run the exact post-blocker Detection budget, Cancellation, Strategy Set, and browser-lifecycle targets if separate; use landed target names rather than inventing aliases.

## Ticket-specific migration items

- [ ] Record the exact final operation and direct call graph landed by #207.
- [ ] Move `app/commands.rs::detect_source_proposal_from_url` and all deterministic/acceptance tests directly to that operation; delete `detect_source_proposal_from_url_with_clients`.
- [ ] Delete residual `detect_source_proposal_with_clients`, `detect_source_proposal_with_http_client`, `detect_source_proposal_internal`, path/signature-constrained `source_profile/detection::Candidate`, retired-signature `evaluate_profile`, `match_input_url_patterns`, `evaluate_http_checks`, and `evaluate_browser_probes` where still present.
- [ ] Delete replaced `DetectionHttpClient`, `DetectionHttpError`, `DetectionHttpResponse`, `BoxedDetectionHttpFuture`, `NoopDetectionHttpClient`, `ReqwestDetectionHttpClient`, `DetectionHttpCheck`, `DetectionBrowserProbe`, `DetectionBrowserInteraction`, `ProfileDetectionDocument`, and `InputUrlPattern` where the blockers replaced them; classify any legitimate shared-name survivor narrowly.
- [ ] Delete `browser_probe_*`, `browserProbes`/`browser_probes`, `browser_probe_unavailable_diagnostics`, and `render_detection_template_with_source_config` when retained only by the retired probe bridge, plus replaced wrappers, aliases, re-exports, old fakes, and superseded tests.
- [ ] Prove only one contribution → reducer/provenance → proposal-constructor responsibility remains and runtime receives compiled typed plans only.
- [ ] Add or update `src-tauri/tests/detection_convergence_evidence.sh`; prove `--self-test` and `--check` share guard, producer, path-collection, and changed-file inspection functions.
- [ ] Classify every remaining retired-symbol, proposal-constructor, Detection-operation, migration-name, and direct provider/key-dispatch inventory hit.
- [ ] If production is already converged, make no naming-only product change; use guard-only delivery or the strictly conditioned no-code closure.

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
