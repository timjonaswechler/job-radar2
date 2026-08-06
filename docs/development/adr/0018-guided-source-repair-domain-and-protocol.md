---
status: accepted
---

# Guided Source Repair uses a staged, deterministic Source-only protocol

Guided Source Repair is a reviewable authoring workflow for repairing one concrete Source from bounded user evidence. It deliberately separates the user-driven Interactive Source Session from productive Source execution: Desktop/Tauri owns the visible WebView host and its native lifecycle, the `sources` crate owns Repair Drafts, Repair Steps, staged proposals, candidate validation, and Source application, and `source-engine` remains the sole owner of declarative compilation and execution. The removed `SourceOnboarding` facade is not reintroduced; this decision follows the ownership completed by [ADR 0013](0013-source-engine-and-sources.md) and the refactoring map in issue [#305](https://github.com/timjonaswechler/job-radar2/issues/305).

## Contract

### Ownership and boundaries

- The Interactive Source Session is a user-driven authoring surface. It is not a Browser Acquisition request and cannot be called by Detection, Discovery, Detail, Source Live Check, or Search Run execution.
- Desktop/Tauri owns WebView creation, navigation and download gates, guest-script installation, child cleanup, and transport adaptation. The domain protocol does not expose Tauri handles, page objects, CDP targets, browser processes, cookies, or storage.
- `sources` owns the Source-repair application boundary: Repair Draft persistence, ordered Repair Steps, Diagnostic-to-step projection, typed picker evidence validation, proposal construction, candidate Source Live Check orchestration, review/confirmation admission, and generation-checked Source replacement.
- `source-engine` remains authoritative for Source Behavior Language validation, Profile compilation, immutable Execution Plans, and productive runtime semantics. Repair must use generic declarative capabilities and may not add profile-specific Rust behavior.
- No repair operation mutates a reusable Source Profile. A profile-selected Source may receive Direct Source Specialization; a Source without a reusable fit may receive one explicit Source-owned Access Path. These authored forms remain mutually exclusive according to the schema-v3 Source contract.

### Identities and state

The protocol uses opaque typed identities:

- `InteractiveSourceSessionId` identifies one ephemeral authoring session;
- `RepairDraftId` identifies one resumable draft;
- `RepairStepId` identifies one ordered question;
- `PickerOperationId` identifies one picker attempt;
- the existing exact installed `SourceGeneration` binds a draft and candidate to the Source/Profile material they started from;
- `SessionGeneration` identifies the current top-level document within a session;
- a proposal fingerprint identifies the exact reviewed candidate.

A `PageIdentity` is the session identity, current session generation, and bounded top-level URL/origin. Every picker request and result echoes the session, generation, step, and operation identities. Evidence is invalid after any accepted top-level navigation, not only after an origin change. Origin changes are additionally visible session events.

The protocol keeps four related state machines separate:

```text
Session:    opening -> ready -> closing -> closed
Navigation: loading -> ready -> invalidated
Picker:     idle -> active -> terminal
Draft:      editable -> staged -> stale | applied | discarded
```

A session is bound to one persisted concrete Source or an explicit custom draft created by Built-in copy. An unowned URL-only repair session is not supported. There is at most one active repair session for a Repair Draft and one active picker per session.

One owner serializes session events. Teardown wins over pending work. Navigation invalidates the current page before cancelling its picker. A new picker cancels the previous picker. After one terminal outcome, late guest or child messages are ignored and cannot produce a second terminal.

### Session commands and outcomes

The transport exposes typed intent and outcomes rather than WebView mechanics. The initial command family is `open`, `navigate`, `beginPicker`, `cancelPicker`, `close`, and `discardDraft`; the event family reports opening, navigation, picker terminal outcomes, closure, and structured Diagnostics.

Picker outcomes are a closed set:

- `Selected(ElementEvidence)`;
- `NotPresent`;
- `CannotDetermine`;
- `Cancelled(reason)` for user cancellation, navigation, teardown, Source change, shutdown, or child closure;
- `Failed(kind)` for timeout, malformed or oversized payload, host failure, or other operational failure.

A result from an old generation is ignored after the active picker has reached its terminal outcome. WebView cleanup failures become structured infrastructure Diagnostics and retain the Repair Draft; they prevent further use of that session but do not silently discard valid authored work.

The protocol does not choose between backend-directed `eval_with_callback` and one narrowly scoped guest result command. That is an implementation prototype decision. Either transport must enforce the same typed identity tuple, payload limits, origin policy, and Rust-side validation.

### Repair Draft and Repair Steps

A Repair Draft is a versioned, bounded, resumable record owned by the Source-repair boundary. It contains:

- its identity, concrete Source key, and exact base `SourceGeneration`;
- ordered Repair Step answers and their dependency state;
- safe, bounded Element Evidence;
- typed authored Source intent and proposed Source changes;
- bounded Diagnostics and the current draft state.

It never contains browser state, HTML, screenshots, cookies, credentials, page objects, Effective Source Profiles, compiler plans, or raw provider payloads. One active draft exists per concrete Source. A Source/Profile generation change marks the draft stale; it is not automatically rebased. Explicit discard is required to delete it. Session close, cancellation, child crash, and application shutdown preserve the last persisted draft.

Initial protocol ceilings are eight selector candidates, sixteen safe attributes, 512 Unicode characters per text preview, 64 Repair Steps, and 256 KiB per serialized draft. Diagnostics and aggregate persistence remain subject to the existing `sources` limits.

A Repair Step identifies one phase (`discovery` or `detail`), strategy and schema locations where available, a stable Diagnostic/step identity, an allowed answer shape, prerequisites, and completion state. Steps are ordered by phase, strategy, and schema/Diagnostic order. Resolving an earlier answer invalidates dependent answers and evidence. Retry replaces only unconfirmed evidence for the current step and does not introduce Source Behavior Language retry semantics. `CannotDetermine` leaves a step unresolved. `NotPresent` is accepted only where the target field's compiled contract permits absence.

Only stable generic Diagnostic metadata—phase, strategy key, schema path, stable code, and typed details—may create a Repair Step. Message text and profile-specific mappings do not drive repairability. Diagnostics without a supported repair mapping remain Diagnostics.

### Element Evidence

`ElementEvidence` is advisory, bounded evidence for one current page and step. It may contain the document URL, element tag, ranked selector candidates, match counts and stability flags, an allowlisted set of safe attributes, a normalized text preview, bounded geometry, frame kind, and Diagnostics. It does not contain raw DOM or unrestricted computed styles.

Picker output is CSS-only and must be accepted by the canonical Source compiler selector parser. Candidate ranking is deterministic: stable semantic attributes and anchors precede stable IDs/classes, while generated-looking and structural selectors carry warnings. Ties are resolved lexicographically after canonical normalization. A candidate must be rechecked in the current document and be unique unless the target field explicitly accepts a collection. The first version accepts only the top-level document; cross-origin frames, closed shadow roots, and richer frame/shadow paths produce explicit unsupported Diagnostics.

Picker evidence is never runtime proof. The final Source Live Check runs the candidate through managed Chrome, where engine differences are expected and a mismatch returns the user to the affected Repair Step rather than weakening acceptance.

### Source intent, proposal, and application

Repair produces typed Source-authored intent, not a generic JSON patch. It may fill supported Source Config, `css_text`, `css_attribute`, Direct Source Specialization, or a complete explicitly supported Source-owned Access Path. A selector alone cannot invent missing fetch, parse, strategy, or phase structure; insufficient structure prevents staging.

A staged Repair Proposal is immutable and contains the draft revision, base Source/Profile generation, candidate concrete Source document, compiler result and Diagnostics, complete candidate Source Live Check report, proposal fingerprint, and review summary. Candidate checking compiles and fully checks the unpersisted candidate through the existing engine and managed runtime. It does not write a temporary Source document.

Review displays the authored Source difference, Source Config, selected Access Path, Direct Source Specialization or Source-owned behavior, Diagnostics, complete Live Check result, generation, and status impact. Confirmation is single-use and bound to the exact proposal fingerprint. A changed Source/Profile generation, changed draft revision, or replayed confirmation rejects application without a write.

Application atomically replaces exactly one concrete Source document under the `sources` mutation coordinator. It does not claim a transaction across Source, Repair Draft, and Check Report. Existing Source Status is preserved: drafts remain drafts, disabled Sources remain disabled, and active custom Sources remain active. Repair never auto-activates; explicit activation continues through the existing fresh-success `check_and_activate` path.

The derived Check Report is persisted separately after the Source replacement. A Source write failure produces no report write and retains the proposal. A report persistence failure after a successful Source replacement returns a typed `AppliedReportPending` outcome and retains the proposal for retry. Existing activation ordering remains unchanged: the report is persisted before a lifecycle status change.

Repeated cancel, close, and confirmation commands are idempotent. A repeated confirmation returns the existing applied/stale result and cannot replace the Source twice.

### Built-in copy

Built-in Sources cannot be mutated. Repair begins with an explicit copy-to-draft operation that clones the concrete authored Source document into a custom Source draft, assigns a deterministic collision-safe key and a generated copy name, sets status to `draft`, and discards Built-in lifecycle and Check Report history. The copied behavior remains subject to normal compiler validation, Source Live Check, review, and confirmation.

### Determinism and Agent Assistance

The complete repair flow is deterministic and does not require an AI Provider. Agent Assistance may optionally explain Diagnostics or suggest an answer, but it receives no privileged browser state and cannot bypass typed validation, review, generation checks, or application. It is not a Source-repair owner or workflow authority.

## Consequences

This decision adds a deliberate Source-repair application seam without recreating the superseded onboarding facade or creating a new speculative crate. It keeps interactive authoring and productive browser execution separate, makes stale and partial outcomes visible, and permits restart-resumable repair without storing sensitive browser state.

The first implementation must provide a Tauri-free `sources` protocol seam, a Desktop WebView Adapter, candidate-check support for unpersisted Source intent, versioned bounded draft persistence, and Interface tests for state transitions, race ordering, stale generations, limits, report-pending application, Built-in copying, and selector revalidation. The guest transport fork and cross-platform WebView behavior remain prototype concerns, not domain-contract variants.

Rejected alternatives include reintroducing `SourceOnboarding`, mutating Source Profiles, sharing the visible WebView with managed Browser Acquisition, persisting browser state, using generic JSON patches, writing temporary Source documents for candidate checks, and claiming a multi-file Source/Report/Draft transaction.
