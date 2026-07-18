# T14a — Run URL and HTTP Profile Detection through Strategy Sets

## Result

Every valid reusable Source Profile executes URL matching and bounded HTTP evidence as one ordered, compiled Detection Strategy Set under `all_required`. Existing caller-visible `SourceProposalDetectionResult`, proposal ordering, Source Config, key/name candidates, support handling, and browser-probe behavior remain unchanged; replaced flat URL/HTTP authoring and imperative Detection HTTP execution are removed.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#178/T10](https://github.com/timjonaswechler/job-radar2/issues/178), [#192/T11c](https://github.com/timjonaswechler/job-radar2/issues/192), and [#202/T13a](https://github.com/timjonaswechler/job-radar2/issues/202).
- Blocking: [#206/T14b](https://github.com/timjonaswechler/job-radar2/issues/206).
- Readiness: **Blocked** pending all three direct blockers. Re-baseline paths, names, and test targets after they land.
- Open decision: none.

T13b/#203 and T13c/#204 are intentionally not blockers: this migration uses `all_required`, not `at_least(count)` or `collect_all(minAccepted)`.

## Consumed contracts

- #166 / PRD Decisions 2–10, 26–30, and 48: typed phase Strategy Sets, acceptance-driven policy execution, deterministic diagnostics, cumulative bounded work, byte-preserving HTTP, and immutable typed runtime plans.
- #178 supplies the single byte-preserving bounded HTTP seam, strict decoding, sanitizer, cumulative response-byte accounting, and production/scripted implementations. T14a must not retain a Detection-specific HTTP family.
- #192 supplies compiled shared values and placement-aware typed contexts. T14a adds the closed Detection context without cloning Discovery/Detail evaluation or introducing an open value map.
- #202 supplies the one crate-private Strategy Set kernel, compiled `all_required`, sequential fail-fast, typed Cancellation, policy outcomes, and exact internal budget reports. T14a adds a Detection phase adapter, not another loop.
- `handoff/issue-166-delivery.md` owns the shared hard-cut, testing, migration, deletion, and PR-evidence requirements.

## Current gap

This section describes the current pre-blocker tree and is provisional until readiness review.

- `source_profile/documents.rs::ProfileDetectionDocument` and `schema/source-profile.schema.json` expose flat optional `inputUrlPatterns`, `httpChecks`, and `browserProbes`; there is no authored/compiled Detection Strategy Set or cumulative per-profile Strategy Set budget.
- `source_profile/detection/mod.rs` exposes `detect_source_proposal_with_http_client`, `detect_source_proposal_with_clients`, and `detect_source_proposal`, then imperatively runs `match_input_url_patterns`, `evaluate_http_checks`, browser probes, and `proposal::build_source_proposal`.
- `source_profile/detection/http.rs` owns `DetectionHttpClient`, string-bodied `DetectionHttpResponse`, reqwest/no-op clients, runtime regex compilation, and free-form transport errors. `Response::text()` loses the byte/metadata/strict-decoding contract required from #178.
- URL alternatives currently stop at the first match. HTTP checks run sequentially, can consume prior captures, fail fast, and replace earlier same-key captures. Evidence preserves execution order.
- `app/commands.rs`, `lib.rs`, and `tests/source_profile_detection.rs` consume or export the wrapper/client family. Built-in Greenhouse and Workday profiles use URL alternatives; SuccessFactors uses URL matching followed by an HTTP sitemap check.
- `proposal.rs::build_source_proposal` is the sole proposal constructor, and frontend callers consume only the unchanged serialized result.

The missing slice is compiled URL/HTTP Detection through the shared Strategy Set runtime. It is not proposal redesign, conflict-safe reduction, browser migration, or profile scoring.

## Target delta

### Authored and compiled contract

A schema-v3 Detection document contains one finite ordered Strategy Set with mandatory `{ "type": "all_required" }` policy. Its first Strategy contains exactly one internally tagged `urlInput`:

```json
{ "key": "input_url", "urlInput": { "type": "patterns", "patterns": [/* non-empty ordered alternatives */] } }
```

or:

```json
{ "key": "input_url", "urlInput": { "type": "absolute_url" } }
```

`patterns` tries alternatives in authored order and contributes only the first accepted match's explicitly listed, non-empty named captures plus one synthetic URL-match evidence entry. `absolute_url` accepts the already validated absolute input without captures or synthetic URL evidence. Profiles that currently omit URL patterns migrate explicitly to `absolute_url`; empty-list or implicit pass-through is invalid.

Each existing HTTP check becomes one following Strategy with its stable key and order. Proposal metadata and profile-level evidence remain Detection metadata. Profile-level evidence seeds the private working context exactly once and is not duplicated by Strategies or `proposal.rs`. Browser probes stay in their Detection-owned location and existing execution path until T14c/#207.

The old `inputUrlPatterns` and `httpChecks` fields are rejected and deleted without aliases, dual parsing, converters, or runtime fallback. Other fetch, value, predicate, regex-capture, acceptance, evidence, and limit members must use blocker-landed shared Primitive spelling; only the submitted-URL operation may be new.

Before runtime, schema/Serde/compiler validation rejects missing/unknown/`null` `urlInput`, empty patterns, variant-incompatible members, duplicate Strategy keys, missing/unsupported policy, invalid regex/template/value references, unavailable Detection contexts, and unbounded work. Runtime receives compiled regexes, templates, acceptance, bounds, values, and policy only—never raw profile JSON. Detection context exposes the typed input URL, immutable proposal-relevant profile metadata, and captures from earlier accepted Strategies; it exposes no Source, specialization, posting data, Search Request, Discovery item, or Detail patch.

### Typed operation and execution

One canonical typed Detection operation replaces the wrapper family. Exact Rust grouping follows the landed runtime, but responsibility is:

```rust
async fn detect_source_proposal(
    input: DetectionInput<'_>,
    profiles: &[CompiledDetectionProfile],
    clients: DetectionClients<'_>,
    control: RuntimeExecutionContext<'_>,
) -> Result<SourceProposalDetectionResult, DetectionCancelled>;
```

The public boundary trims once, parses through the canonical URL type, and requires an absolute URL before profile iteration. Empty, relative, base-dependent, or malformed input returns the existing `Failed` DTO with one stable `detection` Diagnostic and performs no Strategy, HTTP, or browser work.

Profiles execute in Registry Snapshot order; Strategies execute in authored order through the one kernel. `all_required` stops on the first rejected, failed, exhausted, or cancelled Strategy. Accepted contributions advance a private working context so later HTTP templates can use prior captures. No accepted prefix is returned; it is discarded if universal acceptance is not reached.

For T14a only, later non-empty same-key captures replace earlier values and evidence remains ordered, preserving current behavior. T14b/#206 replaces this isolated transition with conflict-safe reduction and provenance. No public reducer trait, serialized contribution, or accepted-prefix list is introduced.

HTTP Strategies use #178 acquisition, byte accounting, strict decoding, sanitizer, Cancellation, and shared compiled values. An authored expected status must match before body contribution. If expected status is absent, a bounded non-2xx response is not rejected by status alone; body acceptance decides. This parity rule does not alter Discovery/Detail HTTP status behavior.

After all URL/HTTP Strategies accept, existing browser probes run once with accumulated captures, then `proposal.rs::build_source_proposal` constructs the proposal. Browser attempts do not enter Strategy Attempt History in this ticket.

### Results, bounds, Diagnostics, and Cancellation

`SourceProposalDetectionResult` and `SourceProposal` do not change. A failed/exhausted profile contributes ordered Diagnostics. If another profile proposes, the aggregate remains `Matched` or `Ambiguous`; with no proposal and any failed/exhausted profile it is `Failed`; otherwise it is `Unsupported`.

Each profile uses one #178/T9 cumulative ledger and unchanged immutable ceilings across its entire Detection Strategy Set. Inspected URL alternatives debit `fan_out`; attempts, requests, response bytes, and elapsed duration never reset per Strategy. An authored pattern set that cannot fit the compiled ceiling is rejected, not truncated. Exact-boundary completion succeeds; denied required work is exhaustion. Typed budget reports remain internal; one terminal Diagnostic exposes only sanitized exact dimension, used, requested, effective limit, and profile/Strategy path. No budget field, status, persistence, or Partial Completion is added.

Cancellation reaches profile iteration, alternatives, kernel, HTTP/stream/decode/value work, browser handoff, and proposal construction. Check it before each profile, URL alternative, Strategy, external side effect, decode/value phase, browser handoff, and proposal construction, and around awaited work where cancellation-safe interruption is possible. Low-level execution returns typed control and does not fabricate a terminal Diagnostic. Cancellation stops later work, discards all contributions and proposals accumulated by the invocation, and returns `DetectionCancelled`; it is never failed-profile aggregation, a DTO, Diagnostic-inferred control flow, or `ResolutionCompletion::Partial`.

Runtime Diagnostics remain ordered by profile, Strategy, and operation. Mismatches are rejected attempts; transport/decode/value errors are failed attempts. Raw bodies, headers, cookies, query values, credentials, captures, and unnecessary Source Config values must not enter Diagnostics or logs.

## Dependency and deletion decision

Authored documents, compiled plans, matching, accumulation, policy state, values, proposal construction, and Registry Snapshot entries are in-process. Provider HTTP is the existing #178 true-external seam with reqwest production and scripted deterministic implementations. Browser remains on its landed production/deterministic seam until T14c. SQLite is not involved.

**Deletion test:** Without the Detection adapter, ordered policy transitions, typed attempts, cumulative budget/Cancellation handling, HTTP decoding, capture context, browser handoff, and Diagnostic ordering would spread into the command/Profile Detection callers or recreate a duplicate URL/HTTP executor. A forwarding adapter fails this test.

## Examples

1. **Workday-shaped URL-only profile:** the first pattern rejects and the second accepts; its captures produce the same Source Config and candidates. The single required Strategy is enough because the profile explicitly authors only it.
2. **SuccessFactors-shaped profile:** the URL Strategy captures the host; the following HTTP Strategy renders a sitemap URL, accepts status/body/regex evidence, and only universal acceptance reaches `proposal.rs`.
3. **Same-key transition:** URL contributes `tenant=first`, later HTTP contributes `tenant=second`, and T14a exposes `second` to later templates/proposal. This is the explicit T14b replacement seam.
4. **Exhaustion/Cancellation:** cumulative byte exhaustion fails that profile and suppresses later work; Cancellation during streaming aborts all profiles and returns only typed Cancellation.

## Scope

- Migrate flat Detection URL/HTTP authoring, Built-in profiles, Custom Profile fixtures, and deterministic acceptance fixtures to ordered `all_required` Strategy Sets.
- Add the minimum typed URL-input capability and Detection value context; compile URL/HTTP regexes, templates, acceptance, evidence, limits, and references.
- Add Detection to the one private Strategy Set kernel and reuse the landed ledger, HTTP, decoder, sanitizer, values, Diagnostics, and Cancellation.
- Preserve transitional ordered captures/evidence, existing failed-profile aggregation, proposal/status/DTO behavior, and browser behavior.
- Move `app/commands.rs`, crate exports, and tests to one typed Detection operation.
- Delete replaced flat fields, Detection HTTP types/clients/fakes, imperative URL/HTTP executors, wrapper operations, runtime-only static validation, and superseded tests.
- Update only active canonical docs made false by this hard move.

## Adjacent non-goals

- Conflict-safe Detection reduction and full contribution/proposal provenance: T14b/#206.
- Browser Strategy migration and immutable Detection browser ceilings: T14c/#207; final remaining Detection-path convergence: T14d.
- `at_least`/`collect_all`, profile scoring/ranking/tie-breaking, Source-specializable or Source-owned Detection, proposal/frontend/persistence redesign, new shared Primitives beyond current evidence needs, parallel/resumable Strategies, or live-network CI.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| Ordered URL alternatives | First authored match alone supplies captures/evidence; non-match suppresses HTTP/browser/proposal | External Detection test with call log |
| Explicit pass-through | Valid `absolute_url` accepts without captures/synthetic evidence; profile evidence occurs once | External Detection test |
| Invalid input/shape | Non-absolute input fails before profile work; invalid variant/policy/old fields fail schema/Serde/compiler validation | Boundary and parity tests |
| Compiled safety | Invalid regex/template/context or excessive fan-out yields pre-runtime Diagnostic and no plan/request | Registry/compiler tests |
| HTTP order/context | Every required check runs in order and sees prior accepted captures | Scripted HTTP log test |
| Status parity | Authored mismatch rejects; absent status permits bounded non-2xx body evaluation | External parity test |
| HTTP rejection/failure | Fail-fast, sanitized typed attempt, no later HTTP/browser/proposal | External Detection test |
| Transitional same-key capture | Later non-empty accepted value replaces earlier value only in T14a accumulator | Focused regression |
| Cumulative budget | Cross-Strategy exhaustion fails profile with internal report and sanitized exact terminal usage; equality succeeds | Detection budget tests |
| Multi-profile aggregation | Exhausted profile plus proposal remains Matched/Ambiguous; no proposal plus failure is Failed | External multi-profile test |
| Ambiguity and order | Two successful profiles produce the existing `Ambiguous` result in Registry Snapshot order | External Detection test |
| Unsupported support | An accepted unsupported profile produces the existing unsupported-profile result | External Detection test |
| Cancellation | Every named checkpoint returns typed Cancellation, discards proposals, and suppresses later work | Runtime/Detection cancellation tests |
| Browser handoff | Existing browser behavior runs only after URL/HTTP acceptance | Existing browser regressions/call log |
| Proposal/DTO | `proposal.rs` remains sole constructor; serialized result and ordering are unchanged | Static search plus external result assertion |
| Data minimization | Sentinel secrets/body/header/query/capture values do not leak | Sanitization test/static review |
| Generic fixtures | Greenhouse, Workday, and SuccessFactors proposals remain equivalent through authored data only | Existing profile regressions |
| Detection ownership | Source fragment cannot author Detection | Schema/Serde/compiler test |
| Hard deletion | No flat URL/HTTP shape, Detection HTTP family, imperative executor, wrapper, or provider dispatch remains | Reviewed repository searches |

Primary tests load complete validated profiles and cross the one typed Detection operation using the real compiler/kernel/proposal constructor and deterministic HTTP/browser implementations. Private tests are limited to narrow alternative/accumulator edges not economically visible there.

### Focused commands

Re-baseline target names after blockers land:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation
cargo test --manifest-path src-tauri/Cargo.toml --test source_profile_registry
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_semantic_validation
cargo test --manifest-path src-tauri/Cargo.toml --test primitive_registry
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_set_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_set_budget
cargo test --manifest-path src-tauri/Cargo.toml --test http_response_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test source_profile_detection
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
npm run build
```

## Ticket-specific migration items

- [ ] Replace `inputUrlPatterns`/`httpChecks` with exact `urlInput.patterns`/`urlInput.absolute_url` and one HTTP Strategy per check under `all_required`; migrate Built-ins and fixtures.
- [ ] Parse one absolute submitted URL at the boundary; compile Detection plans and add the closed typed Detection context/adapter.
- [ ] Reuse one cumulative ledger and the shared HTTP/value implementations; preserve optional-status, aggregation, Cancellation, and browser-handoff behavior.
- [ ] Keep `source_profile/detection/proposal.rs` as the sole Source Proposal constructor.
- [ ] Migrate `app/commands.rs`, `lib.rs`, and external tests to one canonical operation.
- [ ] Delete `BoxedDetectionHttpFuture`, `DetectionHttpClient`, `DetectionHttpResponse`, `DetectionHttpError`, `ReqwestDetectionHttpClient`, `NoopDetectionHttpClient`, Detection fakes, `evaluate_http_checks`, `match_input_url_patterns`, `detect_source_proposal_with_http_client`, `detect_source_proposal_with_clients`, `detect_source_proposal_internal`, runtime regex validation, aliases, and compatibility paths.
- [ ] Review and classify every hit from:

```bash
rg -n 'Response::text|\.text\(\)\.await|from_utf8_lossy|DetectionHttp(Response|Client|Error)' \
  src-tauri/src/source_profile/detection src-tauri/tests/source_profile_detection.rs --glob '*.rs'
rg -n 'urlInput|absolute_url|inputUrlPatterns|httpChecks|Compiled.*Detection|DetectionContribution' \
  src-tauri/src/source_profile src-tauri/src/schema/source-profile.schema.json src-tauri/resources/profiles src-tauri/tests/source_profile_detection.rs \
  --glob '*.rs' --glob '*.json'
rg -n 'detect_source_proposal_with_http_client|detect_source_proposal_with_clients|detect_source_proposal_internal|evaluate_http_checks|match_input_url_patterns' \
  src-tauri/src/source_profile/detection src-tauri/src/lib.rs src-tauri/src/app/commands.rs src-tauri/tests/source_profile_detection.rs --glob '*.rs'
rg -n 'greenhouse|workday|successfactors|profile_key\s*(==|match)|source_key\s*(==|match)' \
  src-tauri/src/source_profile/detection src-tauri/src/profile_dsl/runtime src-tauri/src/profile_dsl/primitives --glob '*.rs'
```

Expected remaining hits are canonical schema-v3 Detection documents/plans, one URL capability owner, shared HTTP/value call sites, browser code awaiting T14c, and provider names in fixture data/tests only.

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
