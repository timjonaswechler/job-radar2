# Issue #166 — Phase-2 preparation handoff

Status: **local handoff for a fresh session — no GitHub changes approved**  
Date: 2026-07-17

## Goal of the next session

Prepare the #166 ticket series for later content and dependency restructuring without implementing product code or editing GitHub issues.

The next session must compare every current GitHub implementation issue with its local lean draft, prove where each contract went, discover remaining contradictions, and produce a reviewed reconciliation matrix plus a conflict register. Ticket merges, splits, moves, deferrals, drops, and dependency changes come only after that preparation has been reviewed.

## Authority and current sources

Use this hierarchy:

1. Live GitHub issues and native dependencies are the authoritative original ticket bodies and tracker state.
2. `CONTEXT.md` is authoritative for domain vocabulary.
3. `docs/prd/declarative-profile-strategy-algebra.md`, #166, and applicable ADRs contain accepted product and architecture decisions.
4. `handoff/issue-166-lean-tickets/` contains the unpublished lean replacements for the 27 implementation tickets.
5. `handoff/issue-166-delivery.md` contains the shared delivery/test/migration/PR-evidence rules removed from individual tickets.
6. `handoff/issue-166-content-deduplication-matrix.md` is the existing Phase-1 deduplication inventory and initial conflict list.
7. `handoff/issue-166-ticket-index.md` is navigation only; live GitHub remains authoritative.

The old local published-ticket snapshots and archive were intentionally deleted because GitHub is the original source. These paths must not be expected or restored:

- `handoff/issue-166-final-tickets/`
- `handoff/archive/`
- `handoff/issue-166-lean-ticket-worker-handoff.md`

`handoff/issue-166-ticket-template.md` was deleted accidentally. Do not restore the old verbose template. Derive a new short template only after reconciliation and restructuring show which lean sections are actually needed.

## Work already completed

- #166 and all 27 implementation issues were reviewed in full.
- Repetition and shared contracts were inventoried in `handoff/issue-166-content-deduplication-matrix.md`.
- Shared delivery rules were extracted into `handoff/issue-166-delivery.md`.
- All 27 unpublished lean ticket bodies exist under `handoff/issue-166-lean-tickets/`.
- Lean drafts use one common structure and total roughly 74,866 words, down from roughly 175,000 words in the published originals.
- No lean ticket has been published and no GitHub issue is approved for modification in this workflow.

## Non-negotiable constraints

- Do not implement product code.
- Do not edit, close, relabel, create, or change dependencies of GitHub issues.
- Do not treat current ticket numbers, boundaries, or ordering as accepted architecture.
- Provider-/ATS-/host-/company-/Source-key-/Profile-key-specific Rust execution branches remain excluded.
- Search Request criteria remain outside Source Config, Source Profiles, Access Paths, and Source specialization.
- Runtime consumes immutable typed plans, not raw authored profile/source JSON.
- Cancellation is typed control flow, is not persistable Resolution Partial Completion, and does not automatically release finalized values after abort.
- Source-local Posting Occurrence identity remains distinct from cross-Source Job Posting deduplication.
- Preserve provider values separately from hints; hints cannot become canonical persisted values.
- Use architecture status language accurately: proposed modules remain deepening candidates until implementation evidence satisfies the acceptance criteria.

## Required first reads

Read these files completely before analysis:

- `AGENTS.md`
- `CONTEXT.md`
- `handoff/README.md`
- `handoff/issue-166-content-deduplication-matrix.md`
- `handoff/issue-166-delivery.md`
- `handoff/issue-166-ticket-index.md`
- every file under `handoff/issue-166-lean-tickets/`
- `docs/prd/declarative-profile-strategy-algebra.md`
- `docs/adr/0001-source-config-as-json-schema.md`
- `docs/adr/0008-persist-job-postings-as-work-items.md`
- `docs/adr/0009-declarative-source-profile-dsl.md`
- `docs/adr/0010-source-live-checks-as-operational-confidence.md`

Then load #166 and all 27 implementation issues from GitHub in full. GitHub issue numbers are listed in `handoff/issue-166-ticket-index.md`.

## Phase-2 preparation deliverables

Create two separate local review artifacts. Do not overload the existing deduplication matrix.

### 1. Original-to-lean reconciliation matrix

Suggested path:

`handoff/issue-166-original-to-lean-reconciliation.md`

For every ticket record:

- observable outcome in the GitHub original;
- observable outcome in the lean draft;
- preserved ticket-specific contracts;
- contracts moved to the PRD, ADR, `CONTEXT.md`, or shared delivery document;
- intentionally removed historical/editorial material;
- accidentally lost contract candidates;
- semantic changes introduced during shortening;
- stale current-state/path/test assumptions requiring readiness re-baselining;
- verdict: equivalent, equivalent after reference fix, decision required, or contract loss.

Do not use line-count or textual similarity as proof. Compare responsibility, caller-visible behavior, invariants, error/control flow, bounds, Diagnostics, tests, migration/deletion ownership, and non-goals.

### 2. Conflict register

Suggested path:

`handoff/issue-166-conflict-register.md`

For each contradiction or unresolved transition record:

- stable conflict ID;
- tickets/documents involved;
- exact conflicting claims;
- behavioral consequence;
- affected callers and downstream tickets;
- viable resolution options;
- recommendation;
- proposed canonical owner;
- latest point at which it must be resolved;
- likely effect on ticket boundaries/dependencies;
- status: open, resolved by existing accepted decision, or documentation-only cleanup.

Search for contradictions beyond those already listed in the deduplication matrix.

## Known conflicts and questions to verify

The existing matrix already identifies these material items; verify rather than blindly copy them:

1. Direct Source specialization versus the still-active old `sourceOverrides` path between T1 and the T7 hard cut.
2. T14b may receive current browser output only after same-key capture overwrite, while it also needs reconciled pre-browser Source Config for browser templates.
3. T14c replaces or changes the shared `ProfileBrowserClient` seam while Discovery, Detail, Source Live Check, and Search Run callers are partly declared out of scope.
4. T14c/T14d mention recovered fallback although T14a/T14b Detection uses `all_required` fail-fast. Identify an explicitly owned inner fallback or remove the claim later.
5. T12b exposes no reduced patch on Strategy Set budget exhaustion, while T15 wants per-field `Unavailable` classification and the exact phase result. Define one result algebra without inventing field evidence.
6. T13a/T13b/T13c are tracker-independent siblings but all may become the first owner of the same `Accepted`/`PolicyUnsatisfied` result migration. A stable owner or serial order is needed.
7. `postingDiscovery`/`postingDetail`, complete Discovery values, and description-only Detail remain in current domain/implementation documentation despite the accepted schema-v3 target.
8. SCHOTT smoke expectations still rely on URL-derived canonical title/location and the old unbounded Candidate/artifact flow.
9. ADR 0008 rejects Search Run history while T17 requires durable `search_runs` and `matches`; T17's accepted persistence decision supersedes it.
10. T4a claims provenance coverage for Policies before an authored Policy exists in the current dependency route. Decide whether compiler-derived policy is excluded, provenance moves later, or the dependency changes.
11. T16 includes retry limits/usage although T9/T10 explicitly defer retry accounting until an executable retry capability exists. Remove, define as structurally zero with justification, or identify a real owner.
12. T4b's closed fingerprint/global inventory must remain coherent when T9/T10 add new immutable limits and runtime behavior. Determine which canonical plan material or behavior-version partition owns those changes.
13. Later lean tickets sometimes retain pre-T6/T7 symbol and test names. Most are marked provisional, but classify stale readiness-rebaseline material separately from real contract dependencies.

## Handoff metadata cleanup to include in the review

`handoff/README.md` is stale and currently references intentionally deleted paths and a worker handoff that no longer exists. The deduplication matrix also references the accidentally deleted old ticket template.

Do not make these cleanup edits before establishing the fresh-session baseline unless the user explicitly approves writing them. Record the required changes:

- state that GitHub is the sole original-ticket source;
- remove deleted snapshot/archive/worker-handoff references;
- state that all lean drafts already exist;
- describe the current step as Phase-2 preparation: reconciliation and conflict discovery;
- remove the missing old-template reference;
- reserve creation of a new lean template until after restructuring.

## Recommended workflow

1. Verify repository and GitHub state read-only.
2. Read all shared files and lean tickets completely.
3. Fetch #166 and all 27 original issues from GitHub, including comments and native dependency metadata.
4. Compare tickets in logical groups, but maintain one complete cross-series contract map.
5. Draft the reconciliation matrix.
6. Independently derive the conflict register from the actual contracts, then compare it with the existing matrix.
7. Run an adversarial consistency pass across compiler, runtime, Detection, Detail, Candidate Resolution, and persistence boundaries.
8. Report findings and unresolved decisions to the user before proposing ticket merges/splits/dependency changes.
9. Only after user approval proceed to the later restructuring phase.

## Completion criteria for the next session

Phase-2 preparation is complete only when:

- all 27 GitHub originals and lean drafts have a reconciliation verdict;
- every removed contract has a canonical destination or an explicit decision;
- semantic changes introduced by shortening are visible;
- the conflict register includes existing and newly discovered contradictions;
- each conflict has a proposed owner and decision deadline;
- no product code or GitHub state changed;
- residual uncertainty is explicit;
- the user receives a concise recommendation for starting ticket/dependency restructuring.

## Fresh-session starter prompt

Use this prompt in the next session:

> Lies `handoff/issue-166-phase-2-preparation-handoff.md` vollständig und führe die dort beschriebene Phase-2-Vorbereitung aus. Arbeite zunächst rein lesend. Vergleiche alle 27 veröffentlichten GitHub-Originaltickets mit den Lean-Entwürfen, erstelle die Original-to-Lean-Reconciliation-Matrix und das Conflict Register lokal. Implementiere nichts und ändere keine GitHub-Issues. Stoppe vor Ticket-Splits, Merges oder Dependency-Änderungen und lege mir zuerst die Review-Ergebnisse und offenen Entscheidungen vor.
