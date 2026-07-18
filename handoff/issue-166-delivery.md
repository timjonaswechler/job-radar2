# Issue #166 — Shared delivery contract

Status: **local review contract — not a GitHub issue body**

This document contains the delivery rules shared by every implementation ticket under #166. Ticket bodies should reference this document instead of copying these rules. Product and architecture semantics remain canonical in #166 and `docs/prd/declarative-profile-strategy-algebra.md`.

## 1. Authority and readiness

- Live GitHub parent links, native dependencies, labels, and issue state are authoritative.
- `handoff/issue-166-ticket-index.md` is a local navigation aid, not a tracker replacement.
- A blocked ticket must not carry `ready-for-agent`.
- Before assignment, re-check all direct blockers and re-baseline the ticket's Current Gap against the landed repository.
- Future type/path sketches may adapt to landed names. Responsibility, invariants, and observable behavior may not drift silently.
- Stop and return the ticket for review if implementation exposes an unresolved product decision, immutable production limit, security policy, persistence policy, or architecture boundary.

A ticket is ready only when:

1. direct blockers are complete;
2. the current code and tests have been re-inspected;
3. no unresolved decision remains inside the ticket scope;
4. the Target Delta is still coherent against landed blockers;
5. GitHub readiness metadata has been reviewed.

## 2. Inherited architecture constraints

All tickets inherit the accepted #166/PRD contracts. In particular:

- no ATS-, provider-, host-, company-, Source-key-, or Profile-key-specific Rust execution branch;
- Search Request criteria remain outside Source Config, Source Profiles, Access Paths, and direct Source specialization;
- runtime executes immutable typed plans, not raw authored JSON;
- all network, browser, pagination, retry, item, byte, duration, action, and fan-out behavior is bounded at its owning layer;
- Sources/Profiles may tighten accepted limits but may not raise immutable backend ceilings;
- Cancellation is typed control flow, not inferred from Diagnostic text or codes;
- Cancellation is not persistable `ResolutionCompletion::Partial` and does not automatically release finalized candidates after abort;
- no unauthorized `SourceStatus`, `SourceRunStatus`, or `SearchRunStatus` variant;
- pure merge, policy, reducer, normalization, field, count, sampling, and projection logic remains in-process unless a real varying dependency justifies a seam;
- Greenhouse, Workday, and SuccessFactors are acceptance profiles, never runtime dispatch keys;
- no network-dependent test enters default CI.

Ticket bodies retain only the concrete effect of these constraints on their own interface and acceptance cases.

## 3. Hard-cut and migration rule

A moved slice must reach its final responsibility and remove what it replaces in the same ticket.

Required behavior:

1. add the target implementation/interface;
2. move every production caller directly;
3. move behavior tests to the target interface;
4. delete replaced files, functions, types, exports, aliases, wrappers, duplicate runtimes, migration-only names, and superseded implementation-detail tests;
5. run ticket-specific repository searches and classify every remaining hit;
6. confirm that runtime boundaries receive typed compiled data only.

Do not introduce a committed compatibility layer merely to bridge adjacent tickets. A temporary private helper is acceptable only when it hides meaningful current complexity and has a final responsibility—not when it forwards an old API.

## 4. Dependency categories and seams

Use the repository architecture-language criteria.

- **In-process:** concrete code behind the module interface; test with the real implementation.
- **Local-substitutable:** keep a small domain-owned boundary and use a local stand-in such as temporary SQLite or a temporary filesystem.
- **Remote but owned / true external:** use a domain-owned interface with production and deterministic test implementations.
- Immutable documents and snapshots are input data, not ports.

A new trait/port requires:

1. behavior that actually varies;
2. a named current production implementation;
3. a deterministic test implementation;
4. callers depending on the domain-owned interface rather than implementation/vendor types.

Each ticket keeps one ticket-specific deletion test: if the proposed module were removed, which meaningful complexity would spread into which callers? A forwarding wrapper or naming-only module fails this test.

## 5. Testing contract

### Preferred test surface

- Prefer external Rust integration tests under `src-tauri/tests/` for behavior visible through the crate/module API.
- Use in-module tests only for private parser/state-machine edges that cannot be observed economically through the public interface.
- Test through the highest practical application/module operation rather than private construction steps.
- Persistence tests use the real migrated temporary SQLite database, not a repository mock.
- HTTP/browser/external execution uses production and deterministic adapters behind the same domain-owned interface.
- Deterministic profile fixtures prove generic behavior, not current live operability.

### Required evidence per behavior group

A ticket states only:

- interface crossed by the test;
- production caller represented;
- real implementation versus deterministic adapter/stand-in;
- observable result asserted;
- why the test remains valid after internal refactoring.

### Commands

Every ticket supplies exact focused commands for its changed behavior. Before completion, also run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Run `npm run build` when TypeScript, schemas consumed by the frontend, commands, or serialized frontend-facing contracts change.

Run Greenhouse, Workday, and SuccessFactors regression targets only when the changed generic behavior can affect those profiles, or when the ticket explicitly uses them as acceptance evidence. Do not copy all profile commands into unrelated tickets.

Live/network smoke checks remain manual and separately documented.

## 6. Diagnostics, data, and security evidence

Where applicable, tests and review must show:

- deterministic Diagnostic category, stable code, severity, path, ordering, and bounded volume;
- control flow does not depend on Diagnostic strings;
- secrets, credentials, raw response bodies, provider payloads, or unnecessary resolved values are not retained in Diagnostics, provenance, reports, fingerprints, or persistence;
- conflicts are not silently resolved through last-write-wins;
- rejected/failed/cancelled results expose no forbidden partial typed output;
- exact-boundary success and one-over-limit rejection for new bounds.

Use the distinct provenance model owned by the ticket; do not conflate Effective Profile origins, runtime Strategy attempts, Detection proposal evidence, or phase-output contributions.

## 7. Shared migration checklist

Every implementing PR confirms the applicable items:

- [ ] Direct blockers and current code were re-inspected before implementation.
- [ ] Target interface/behavior is used by every intended production caller.
- [ ] Replaced callers and tests moved directly to the target boundary.
- [ ] Replaced implementation, wrappers, aliases, exports, duplicate fakes, and superseded tests were deleted.
- [ ] Ticket-specific searches were run and every remaining hit classified.
- [ ] No raw authored JSON crosses a typed runtime boundary.
- [ ] No provider-specific Rust dispatch was introduced.
- [ ] No speculative trait/port or duplicate runtime was introduced.
- [ ] Bounds, Cancellation, Diagnostics, and data minimization were reviewed where applicable.
- [ ] Focused tests and required regressions passed.
- [ ] No network-dependent default-CI test was added.
- [ ] Active domain/ADR/smoke documentation touched by the landed behavior was updated in the same slice.

Tickets add only checklist items unique to their migration.

## 8. Definition of done

A ticket is complete when:

1. its Target Delta is observable through the intended caller-facing interface;
2. every ticket-specific acceptance row passes through its named test or static/manual check;
3. the shared migration checklist is satisfied;
4. exact focused commands and required regression commands pass;
5. the ticket-specific deletion test passes;
6. GitHub dependency/readiness state is updated through a separately approved tracker workflow;
7. no unresolved architecture, product, security, persistence, or limit decision was invented during implementation.

## 9. PR evidence

The PR description records:

- implemented issue and Target Delta;
- landed public/internal interface names and paths where relevant;
- production callers moved;
- old paths/types/functions/tests deleted;
- exact commands run and results;
- ticket-specific repository-search/deletion evidence;
- applicable bounds, Cancellation, Diagnostic, persistence, and data-minimization evidence;
- dependency/readiness links reviewed;
- residual risk or `none`;
- confirmation that no provider-specific branch, compatibility runtime, duplicate implementation, unjustified seam, unauthorized status, or network-dependent default-CI test was added.

Ticket bodies should not repeat this list as a separate Required PR Attestation section.

## 10. Re-baseline note for lean drafts

Lean ticket drafts may describe the current gap at drafting time, but that section is explicitly provisional while blockers remain open. At readiness review:

- replace stale paths and symbols with the landed baseline;
- remove authoring-time commit hashes, dirty-tree inventories, publication history, and placeholder statements;
- preserve the accepted responsibility and observable contract;
- update only ticket-specific tests, searches, examples, and migration targets.
