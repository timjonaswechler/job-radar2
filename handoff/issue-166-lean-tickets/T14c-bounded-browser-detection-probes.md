# T14c — Execute bounded browser Detection Strategies

## Result

A browser-backed Detection Strategy executes through the same typed Strategy Set operation as URL and HTTP Detection, contributes only typed Detection values and evidence through T14b's conflict-safe reducer, and cannot exceed the approved immutable navigation, action, wait, duration, rendered-document-byte, or Cancellation limits. The separate imperative browser-probe authoring and execution path is deleted.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#206](https://github.com/timjonaswechler/job-radar2/issues/206) (T14b).
- Blocking: [#218](https://github.com/timjonaswechler/job-radar2/issues/218) (T14d).
- Readiness: **Blocked — not ready for agent execution**. Re-baseline against #206 and its landed transitive prerequisites before assignment; the immutable limits are already decided and must not be reopened.
- Open decision: none.

## Consumed contracts

- #166 / PRD Decisions 2–10 and the Strategy Set Runtime module decision: Detection uses one typed phase operation, ordered Strategy attempts, cumulative accounting, typed Cancellation, phase acceptance, and immutable compiled plans.
- #166 / PRD Decision 48: the browser ceilings, conservative multi-scope accounting, rendered-byte handling, and in-ceiling teardown reserve below are binding.
- T14b/#206 provides the conflict-safe `DetectionContribution` reducer, ordered contribution provenance, shared kernel/ledger/terminals, and `source_profile/detection/proposal.rs` as the sole Source Proposal constructor. Browser execution must use those landed contracts rather than define parallel shapes.
- T14a/T14b and their prerequisites provide compiled Detection Strategies, canonical parse/select/value Primitives, policy semantics, attempt ordering, Diagnostic conventions, and the single public Detection operation. Exact future names and paths in this blocked draft are provisional and must be adapted directly at readiness review without changing responsibility.
- [ADR 0003](../../docs/adr/0003-managed-browser-runtime.md): production uses pinned managed Chrome behind a Job Radar-owned browser boundary, not a system-browser or provider-specific path.

## Current gap

The current repository is still pre-T14a/T14b and uses a separate schema-v2 browser-probe slice:

- `src-tauri/src/source_profile/documents.rs` defines `ProfileDetectionDocument::browser_probes`, `DetectionBrowserProbe`, and `DetectionBrowserInteraction`; `src-tauri/src/schema/source-profile.schema.json` admits `detect.browserProbes`.
- `src-tauri/src/source_profile/detection/mod.rs` evaluates URL patterns and HTTP checks, then calls `source_profile/detection/browser.rs::evaluate_browser_probes` before building a proposal. That evaluator constructs browser requests, mutates captures/evidence, and maps browser errors outside a Strategy Set.
- `src-tauri/src/profile_dsl/runtime/browser.rs` exposes `ProfileBrowserClient::render`/`render_with_context`; its response owns an unbounded `String`, and Cancellation is checked only around the high-level call.
- `src-tauri/src/browser_runtime/control.rs` launches Chrome, navigates, waits/clicks, calls `page.content()`, then awaits `Browser::close` and the handler. It has local timeouts and cancellable sleeps but no all-scope navigation/action/wait/byte ledger, no work/teardown deadline split, no forced kill/reap path, and no bounded cleanup report. `browser_runtime/types.rs::BrowserRuntimeRenderRequest` carries no cumulative control or lifecycle terminal.
- `src-tauri/src/profile_dsl/runtime/cancellation.rs` currently carries Cancellation and a Discovery request budget rather than the prerequisite shared Strategy Set ledger.
- `src-tauri/src/app/commands.rs::detect_source_proposal_from_url` constructs the HTTP and managed-browser clients for Source setup. `src/lib/api/sources.ts` still serializes `DetectionBrowserProbe` authoring shapes.
- `src-tauri/tests/source_profile_detection.rs` covers current browser evidence, captures, request construction, local bound validation, unavailable runtime, and errors. `browser_runtime/tests.rs` covers pre-Cancellation and cleanup precedence, but there is no deterministic proof of cumulative limits, byte rejection, mid-await Cancellation, bounded forced teardown, or shared reducer integration.

This Current Gap is drafting-time evidence only. Because #206 is open, readiness review must replace stale symbols with their landed equivalents and identify any transitional browser bridge to delete.

## Target delta

### One Detection path and one browser lifecycle seam

Preserve the blocker-landed public typed Detection operation; do not add a browser-specific public entry point or compatibility wrapper. Each browser acquisition compiles into the same immutable Detection Strategy plan, executes as one ordered kernel attempt, and yields either a browser-rendered typed input, an ordinary typed browser failure, or the shared budget/Cancellation terminal. Runtime never interprets raw authored browser JSON.

Replace the high-level `ProfileBrowserClient::render` shape directly with one crate-internal, domain-owned lifecycle seam implemented by managed Chrome and a deterministic scripted browser. Exact names follow landed code; the responsibility is:

```rust
trait BrowserLifecycle: Send + Sync {
    fn execute<'a>(
        &'a self,
        plan: &'a CompiledBrowserAcquisition,
        control: BrowserLifecycleControl<'a>,
    ) -> BoxFuture<'a, BrowserLifecycleEnvelope>;
}

struct BrowserLifecycleEnvelope {
    primary_outcome: BrowserLifecycleOutcome,
    teardown_report: BrowserTeardownReport,
}

enum BrowserLifecycleOutcome {
    Rendered(BoundedRenderedDocument),
    Failed(BrowserFailure),
    Terminal(BrowserLifecycleTerminal),
}
```

`BrowserLifecycleControl` is concrete shared in-process control over the landed ledger, monotonic work/hard deadlines, and typed Cancellation. Both implementations invoke it at real launch, navigation, logical-wait, selector-polling, action, `waitAfterMs`, content-read/byte, and teardown points. The scripted implementation may gate or fail stages and supply rendered bytes, but may not synthesize budget or Cancellation results; those must originate from the same control used in production. No separate trait is added for arithmetic, clocks, deadlines, teardown, filesystem cleanup, reducers, or terminals.

Every invocation returns exactly one fixed primary outcome and one complete private teardown report. A second primary terminal is unrepresentable. The phase adapter consumes the envelope once:

| Primary outcome | Clean teardown | Classified residue |
|---|---|---|
| Rendered | Continue parse/select/value/acceptance | Continue with the same document and append exactly one bounded warning after acquisition Diagnostics and before parse Diagnostics; acceptance is unchanged |
| Failed | Preserve the existing failure projection | Add a safe teardown summary to that projection; no second failure, terminal, or warning |
| Budget terminal | Project the shared terminal once | Add the safe summary to its one Diagnostic/details projection; terminal identity and policy transition are unchanged |
| Cancellation terminal | Project typed Cancellation once | Add the safe summary to the single phase Cancellation projection; no browser failure, warning, or second terminal |

Residue means unconfirmed process termination/reap, handler abort, or quarantined/residual session state. Caller-visible summaries are constant-volume enums/booleans only—never HTML, URL/query data, cookies, paths, PIDs, browser output, raw errors, or lists. At most one residue warning exists per rendered attempt and remains visible if later parsing rejects the attempt. It uses the landed runtime/Detection category convention, Strategy key, and canonical Strategy path. Residue augmentation cannot change the selected code, severity, terminal kind, policy transition, attempt order, or Cancellation precedence.

### Approved immutable limits

| Dimension | Immutable backend ceiling |
|---|---:|
| Target navigations | 1 per browser Strategy; 2 per profile Strategy Set; 8 per Detection operation |
| Authored action `maxCount` | 5 per action |
| Executed actions | 10 per profile Strategy Set; 32 per Detection operation |
| Logical waits | 4 per navigation; 8 per profile Strategy Set; 32 per Detection operation |
| Per-wait timeout | 5,000 ms |
| Browser duration, including teardown | 20,000 ms per Strategy; 30,000 ms per profile Strategy Set; 60,000 ms per Detection operation |
| Rendered UTF-8 document | 2 MiB per navigation; 4 MiB per profile Strategy Set; 16 MiB per Detection operation |

For each dimension, the effective limit is the strictest applicable immutable Strategy/Strategy-Set/Detection-operation ceiling, valid authored local bound, and caller-provided tighter bound. Authored values above immutable ceilings, and authored browser durations below `2,000 ms`, are compiler errors with no plan or browser call; values are never clamped. Detection remains profile-owned and is not Source-specializable.

Reservations/debits are checked and committed atomically across every applicable scope before the side effect. Failure at any scope debits none and prevents work. Counters neither wrap nor silently saturate. Deterministic profile/Strategy order governs cumulative exhaustion.

- Reserve one target navigation before launch; redirects are not additional target navigations, and secondary/recursive navigation is prohibited.
- Authored action `maxCount` is required and at most five. Debit an action immediately before each attempted click execution. A selector lookup proving an optional target absent does not debit an action.
- Debit one logical wait before each authored wait, each executed nonzero `waitAfterMs`, and each backend implicit settle. Zero-duration waits debit none; selector polling sleeps remain inside one logical wait. Hidden settling is prohibited, and every effective wait timeout is at most `5,000 ms`.
- Charge monotonic elapsed launch, navigation, waits, actions, content read, close, kill/reap, handler finalization, and filesystem finalization to all applicable duration scopes.
- Check rendered UTF-8 bytes before exposing the body to Detection or parse/select/value/reducer work. Exact-limit input may proceed. Oversize returns no body, truncation, parsed value, or contribution; it atomically consumes the remaining applicable byte allowance and reports only a safe observed size. Document the unavoidable temporary-allocation risk if `page.content()` must first materialize the full string. Chrome subresource bytes are not claimed as accounted, and browser-rendered data is not fabricated as HTTP input.

Transport/render success does not accept a Strategy; canonical parse/select/value/acceptance owns that decision. Accepted contributions enter T14b's reducer in Strategy/output order. Equal responsibilities merge origins, conflicts follow T14b, and browser code never mutates final captures, Source Config, recommendation, evidence, or Source Proposal. Diagnostics remain ordered as prior attempts, the current browser attempt, then the one terminal phase outcome. A failed profile emits no partial proposal; other profiles retain the landed deterministic aggregation behavior.

### Duration, teardown, and Cancellation

Every applicable duration ceiling reserves its final `2,000 ms` for teardown, inside rather than beyond the ceiling. Foreground work uses the earlier work deadline; teardown uses the immutable hard deadline. A Strategy cannot launch unless all scopes preserve the reserve. Work-deadline exhaustion returns the shared duration budget terminal, seals lifecycle work, and enters teardown.

Teardown runs exactly once after rendered success, ordinary failure, budget exhaustion, or Cancellation:

1. seal all foreground work and preserve the primary outcome;
2. graceful `Browser::close` for at most `500 ms`;
3. if process exit is unconfirmed, forced `Browser::kill` and reap for at most `1,000 ms`, retaining process ownership and kill-on-drop fallback;
4. await handler completion for at most `250 ms`; on timeout abort it and observe the cancelled `JoinHandle` before return;
5. release the active-session guard and remove, safely quarantine under managed `.tmp`, or classify session residue within `250 ms`;
6. return the envelope without extending the hard deadline.

Unused slice time carries forward; no phase borrows beyond the hard deadline. If an external operation consumes the reserve, the emergency path requests synchronous kill-on-drop/OS termination, aborts and observes handler completion within its reserved slice, releases owned handles/guard, classifies process/filesystem residue, and performs no further external await. Pre-return state must prove lifecycle sealed; handler completion observed; process reaped or `termination_unconfirmed`; guard released; and session removed, quarantined, or explicitly residual. Teardown residue never changes the primary outcome or authorizes later work.

Cancellation is typed control flow from lifecycle control, not a browser error string, Diagnostic-code inspection, or Resolution Partial Completion. Check before reservation/launch/navigation/every wait/every action/content read/parse/reduction/later Strategy, and race it against external awaits wherever cancellation-safe interruption is possible. A Cancellation win suppresses ordinary failure/exhaustion translation, later work, proposal construction, and accumulated operation proposals according to the landed Detection contract, while still running bounded teardown. Exactly one phase Cancellation outcome remains visible.

## Dependency and deletion decision

Compiled plans, limit selection, ledger use, lifecycle control, projection, parse/select/value, acceptance, reduction, provenance, and Diagnostics remain concrete in-process logic. Registry order is immutable input data. HTTP reuses the landed external seam. SQLite is not involved.

Managed Chrome/process/filesystem work is local-substitutable external behavior and justifies the single `BrowserLifecycle` seam: production uses the pinned managed runtime; deterministic tests use a scripted lifecycle with the same stage vocabulary and concrete control. `chromiumoxide` DTOs and errors remain at the managed edge.

**Deletion test:** Without browser support inside the shared Detection adapter, Strategy ordering, multi-scope limits, typed Cancellation, lifecycle translation, contribution creation, and Diagnostic ordering would spread into the Source-setup command, browser runtime, reducer bridge, and tests. A forwarding browser module fails this test.

## Examples

1. **Accepted browser evidence:** one navigation and selector wait produce in-limit HTML; canonical parsing and acceptance create a typed contribution, and T14b alone reduces it into proposal provenance.
2. **Eleventh profile action:** ten clicks were committed across prior attempts; an optional target for the next click is present, but the atomic reservation fails. The click never starts, no scope is partially debited, the shared budget terminal is returned, and no partial proposal is built.
3. **`2 MiB + 1` document:** the body is rejected before parser exposure, remaining applicable byte capacity is consumed, only a safe observed size is reported, and no truncated value or contribution escapes.
4. **Cancellation during navigation:** lifecycle control fixes the shared Cancellation terminal, seals work, performs bounded teardown, and prevents later waits, Strategies, and proposal construction.
5. **Rendered result with quarantined session:** the same document proceeds to parsing; exactly one ordered bounded warning describes residue. Multiple residue states still produce one warning and cannot change acceptance.
6. **Blocked close/kill:** close escalates after `500 ms`; kill/reap is bounded to `1,000 ms`; an unconfirmed process is safely classified while handler/guard/session finalization completes by the hard deadline without replacing the original outcome.

## Scope

- Compile browser acquisition into the blocker-landed immutable Detection Strategy plan with context/capability and ceiling validation.
- Extend the shared cumulative ledger only for the approved navigation, action, wait, browser-duration, and rendered-byte dimensions.
- Replace the high-level render seam with managed and scripted `BrowserLifecycle` implementations using one concrete control and shared terminals.
- Implement all stage Cancellation/deadline races, the work/teardown split, forced termination, handler observation, managed `.tmp` finalization, private envelope/report, and exhaustive adapter projection.
- Document the unavoidable `page.content()` temporary-allocation risk and any residual Chrome process/subprocess termination risk; justify the pinned library's kill-on-drop plus kill/reap path or identify a focused hardening follow-up.
- Pass in-limit rendered HTML to canonical parse/select/value/acceptance and accepted output to T14b's reducer/provenance/sole proposal constructor.
- Migrate Source setup, deterministic browser Detection fixtures, built-in profiles, schema/Serde fixtures, and frontend authoring types directly to the landed Strategy model where affected.
- Delete `DetectionBrowserProbe`, `DetectionBrowserInteraction`, `detect.browserProbes`, `evaluate_browser_probes`, browser-probe request/error/Cancellation translation, any transitional T14b browser bridge, the superseded high-level render wrappers, compatibility Diagnostics, duplicate ledger/terminal code, and replaced tests.
- Update active canonical Detection/browser-limit documentation. T14d retains only post-migration convergence residue not replaced by this slice.

## Adjacent non-goals

- New Detection policies, T14b reducer/provenance redesign, Source specialization of Detection, or changing the approved limits/accounting.
- Browser installation/update UI or managed-runtime manifest changes; Chrome subresource-byte interception/accounting.
- HTTP conversion, duplicate parsers, arbitrary JavaScript/eval/DOM mutation, login/credentials/CAPTCHA, secondary or recursive navigation, or unrestricted automation.
- Parallel browser Strategies/reservations, resumability, Search Run/Candidate Resolution/persistence/Source Live Check/status work, or T14d's later convergence-only audit.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| Accepted browser Strategy | One proposal with T14b-ordered contribution provenance | Public `source_profile_detection` integration test |
| Local tightening / attempted raise | Lower valid bounds apply exactly; raise is compiler rejection with no browser call | Compiler and Detection boundary table |
| Duration below reserve / insufficient runtime reserve | Compile rejection below `2,000 ms`; otherwise shared budget terminal before launch with no partial debit | Compiler plus paused-time runtime test |
| Navigation exact/one-over | Exact boundary starts; one-over does not navigate | Multi-Strategy scripted call/debit log |
| Action exact/one-over / absent optional target | Extra visible click does not start; absent target consumes no action | Scripted interaction table |
| Authored wait, nonzero/zero `waitAfterMs`, polling, implicit settle | Exact logical-wait semantics and duration charging; no hidden waits | Scripted paused-time wait table |
| Work deadline | Shared duration terminal, sealed foreground, teardown starts with reserve | Paused-time lifecycle test |
| Exact/one-over/cumulative rendered bytes | Exact body may parse; oversize/later cumulative excess never reaches parser/reducer and reports safe size only | Deterministic body fixtures |
| Rendered with clean/residual teardown | No warning when clean; exactly one ordered safe warning for one or many residues; acceptance unchanged | Public Detection plus serialization table |
| Rendered then parser rejection | Residue warning precedes parse Diagnostic; parser owns rejection | Public Diagnostic-order test |
| Ordinary browser failure with residue | Existing failure/code/severity retained; safe details augmented; no second outcome/warning | Public error-kind table |
| Budget/Cancellation with residue | Shared terminal translated once; safe details augmented; no warning or second terminal | Public lifecycle projection table |
| Blocked close / kill-reap / handler / filesystem | Fixed slice escalation, observed handler completion, guard release, classified residue, return by hard deadline | Scripted lifecycle teardown table |
| Hard-deadline emergency path | No later external await or Strategy work; report remains complete/classified | Paused-time invariant test |
| Pre- and mid-stage Cancellation | No partial debit/proposal or later work; one typed Cancellation plus bounded teardown | Public Cancellation tests with stage gates |
| Browser rejection and recovered fallback | Acceptance owns rejection; earlier failed attempt remains visible without failing later accepted fallback | Public policy/attempt test |
| Reducer conflict | T14b conflict behavior; no last-write-wins proposal | Public Detection reducer test |
| Multi-profile operation ceiling | Later browser work is denied in deterministic registry order | Registry-order budget test |
| Data minimization | Secret sentinels from HTML/query/cookies/paths/raw errors are absent from Diagnostics/provenance/logs | Serialization/sanitization test and review |
| Managed runtime unavailable | One landed typed external failure projection; no probe compatibility path | Public Detection test |
| Managed/scripted parity | Same stage vocabulary, concrete control, envelope, and shared terminal types | Lifecycle contract test |
| Managed adapter placement | Real managed launch/navigation/wait/action/content/teardown stages invoke shared control; `chromiumoxide` errors/process/handler state translate at the edge; bytes are checked before parser handoff; teardown cannot replace the primary result | Focused browser-runtime adapter tests and pinned-library API review |
| Profile regressions | Greenhouse, Workday, and SuccessFactors remain generic/data-driven | Existing deterministic fixture targets |
| Migration deletion | No active probe authoring/evaluator/bridge/forwarder or duplicate browser Detection runtime remains | Reviewed searches and call graph |

Public tests cross the single typed Detection operation representing `detect_source_proposal_from_url`; they use real compiler/kernel/ledger/Primitives/reducer/proposal code and deterministic browser/HTTP implementations. Focused browser-runtime adapter tests must verify actual managed lifecycle-stage placement, `chromiumoxide` error/process/handler translation, rendered-byte checking before parser handoff, and teardown precedence; they may inspect private process/handler/filesystem state that cannot be observed economically through a Source Proposal. Default CI uses scripts and saved HTML, never an installed browser or network.

### Focused commands

Re-baseline exact target names after #206 lands:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation
cargo test --manifest-path src-tauri/Cargo.toml --test source_profile_registry
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_set_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_policy_all_required
cargo test --manifest-path src-tauri/Cargo.toml --test source_profile_detection
cargo test --manifest-path src-tauri/Cargo.toml --test browser_lifecycle
cargo test --manifest-path src-tauri/Cargo.toml browser_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
npm run build
rg -n 'browserProbes|DetectionBrowserProbe|DetectionBrowserInteraction|evaluate_browser_probes|browser_probe_|render_with_context|ProfileBrowserClient' src-tauri src
rg -n 'BrowserLifecycle|BrowserLifecycleControl|BrowserLifecycleEnvelope|primary_outcome|teardown_report|BudgetTerminal|CancellationTerminal|BrowserTeardownReport' src-tauri/src src-tauri/tests
rg -n 'legacy|compat|forward|adapterKey|greenhouse|workday|successfactors|profile_key|source_key|host|company' src-tauri/src src-tauri/tests
git diff --check
```

## Ticket-specific migration items

- [ ] Re-baseline #206's landed Detection operation, compiled plans, kernel/ledger, terminals, contribution/reducer/proposal flow, browser bridge, callers, and tests before implementation.
- [ ] Move browser Detection authoring and fixtures to the landed Strategy model; keep runtime input typed and immutable.
- [ ] Replace `ProfileBrowserClient::render`/`render_with_context` and every managed/deterministic caller directly with the selected lifecycle seam; leave no forwarding wrapper.
- [ ] Add exact compiler/runtime boundary coverage for every approved ceiling, atomic multi-scope debit, local tightening, and the `2,000 ms` reserve.
- [ ] Add deterministic blocked-stage and exhaustive outcome/residue projection tests using the real lifecycle control.
- [ ] Delete old probe schema/document/frontend types, imperative evaluator/error mapping, transitional reducer bridge, browser-specific budget/Cancellation translation, duplicate runtime pieces, compatibility codes, and superseded tests.
- [ ] Confirm `proposal.rs` remains the sole Source Proposal constructor and browser-rendered input is neither exposed early nor converted to HTTP.
- [ ] Classify every remaining hit from the focused searches; only shared non-Detection browser consumers or landed final lifecycle names may remain.

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
