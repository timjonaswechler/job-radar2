# T12b — Define requested Detail patches and conflict-safe phase reducers

## Result

The public typed Discovery and candidate-scoped Detail operations return explicit phase envelopes containing the single T12a `PostingOccurrence` payload or a requested-only `DetailPatch`, plus bounded contribution provenance, typed conflicts/rejections, the complete landed `StrategySetBudgetReport`, and Structured Diagnostics. Crate-private reducers deterministically merge equal contributions and quarantine conflicting responsibilities instead of choosing first/last values.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#193/T12a](https://github.com/timjonaswechler/job-radar2/issues/193).
- Blocking: [#202/T13a](https://github.com/timjonaswechler/job-radar2/issues/202), [#203/T13b](https://github.com/timjonaswechler/job-radar2/issues/203), [#204/T13c](https://github.com/timjonaswechler/job-radar2/issues/204), and [#219/T15](https://github.com/timjonaswechler/job-radar2/issues/219).
- Readiness: **Blocked — not ready for agent execution.** Re-baseline the provisional paths and names below after T12a and its transitive blockers land; the ticket must not receive `ready-for-agent` while blocked.
- Open decision: none. The four Detail fields and field-local quarantine policy are selected.

## Consumed contracts

- #166 / [PRD Implementation Decisions 9–10, 24–25, and 31–32](../../docs/prd/declarative-profile-strategy-algebra.md#implementation-decisions): one Source-local occurrence identity, explicit provider values versus hints, requested-only Detail, and deterministic non-last-write reducers.
- #166 / [PRD “Strategy Set Runtime” module decision](../../docs/prd/declarative-profile-strategy-algebra.md#module-and-interface-decisions): typed public phase operations surround a crate-private kernel; T8 Attempt History, outcomes, diagnostic control state, and transport data remain private.
- T12a provides one shared `PostingOccurrence`, disjoint `reference`, `providerValues`, `hints`, and `postingMeta`, provider-ID-first identity with normalized-absolute-URL fallback, typed compiled value outputs, and no hint-to-canonical conversion.
- Preserve T12a identity normalization and mixed-kind non-correlation. The sole refinement here is that a **reduced URL-fallback occurrence** exposes its normalized identity URL as `provider_url` and exposes no original URL spelling in the serialized phase result.
- `handoff/issue-166-delivery.md` owns shared readiness, hard-cut, testing, migration, deletion, and PR-evidence rules.

If landed T12a cannot support this contract without a second occurrence/value model, raw authored JSON at runtime, a compatibility wrapper, provider-specific branching, or moving T13/T15/T16 work here, stop and perform Design It Twice.

## Current gap

This section is provisional because T12a is open. The current tree still uses pre-schema-v3 paths:

- `profile_dsl/documents/posting_discovery.rs`, `execution_plan/posting_discovery.rs`, and `runtime/posting_discovery.rs` require complete title/company/URL candidates; `runtime/posting_discovery/pagination.rs` accumulates candidates without Source-local conflict-safe reduction.
- `profile_dsl/documents/posting_detail.rs` admits only `descriptionText`; `execution_plan/posting_detail.rs` clones authored `FieldExpression` into runtime plans.
- `runtime/posting_detail.rs` owns a separate `PostingDetailPostingOccurrence` and returns `PostingDetailExecutionResult { description_text, diagnostics }`; no typed requested set, patch, provenance, conflict marker, rejection, or phase reducer exists.
- `posting_discovery_runtime`, `posting_detail_runtime`, `source_live_check`, and the Greenhouse/Workday/SuccessFactors fixtures assert the old shapes; Search Run, Source Live Check, and posting-service callers consume them.

At readiness review, inspect T12a’s exact occurrence/reference/value/provenance types, accepted-attempt and phase-operation shapes, Diagnostics, exports, tests, and production callers, then replace these provisional names without changing this ticket’s responsibilities.

## Target delta

### Public data and private reducers

Exact names may adapt to landed T12a, but the public contract has these responsibilities:

```rust
pub enum CanonicalPostingField { Title, Company, Locations, DescriptionText }
pub struct NonEmptyRequestedFields(/* finite typed set; non-empty */);

pub struct DetailPatchValues {
    pub title: Option<String>,
    pub company: Option<String>,
    pub locations: Option<Vec<String>>,
    pub description_text: Option<String>,
}
pub struct DetailPatch {
    pub occurrence_identity: PostingOccurrenceIdentity,
    pub values: DetailPatchValues,
}

pub struct ContributionOrigin {
    pub strategy_key: StrategyKey,
    pub attempt_index: u32,
    pub provider_item_index: Option<u32>,
}
pub struct NonEmptyContributors(/* ordered, deduplicated, length >= 1 */);

pub enum OutputResponsibility {
    ProviderField(CanonicalPostingField), HintKey(HintKey),
    PostingMetaKey(PostingMetaKey), DetailField(CanonicalPostingField),
    RequiredProviderUrl,
}
pub struct ContributionProvenance {
    pub occurrence_identity: PostingOccurrenceIdentity,
    pub responsibility: OutputResponsibility,
    pub contributors: NonEmptyContributors,
}
pub struct ConflictMarker {
    pub occurrence_identity: PostingOccurrenceIdentity,
    pub responsibility: OutputResponsibility,
    pub contributors: NonEmptyContributors,
}

pub enum OccurrenceRejectionReason { RequiredProviderUrlConflict }
pub enum ContributionRejectionReason {
    UnrequestedDetailField { field: CanonicalPostingField },
    OccurrenceIdentityMismatch,
}
pub enum PhaseRejection {
    Occurrence {
        occurrence_identity: PostingOccurrenceIdentity,
        reason: OccurrenceRejectionReason,
        contributors: NonEmptyContributors,
    },
    Contribution {
        // Always the expected/requested identity supplied to Detail.
        occurrence_identity: PostingOccurrenceIdentity,
        reason: ContributionRejectionReason,
        origin: ContributionOrigin,
    },
}

pub struct DiscoveryPhaseResult {
    pub occurrences: Vec<PostingOccurrence>,
    pub provenance: Vec<ContributionProvenance>,
    pub conflicts: Vec<ConflictMarker>,
    pub rejections: Vec<PhaseRejection>,
    pub budget_report: StrategySetBudgetReport,
    pub diagnostics: Vec<StructuredDiagnostic>,
}
pub struct DetailPhaseResult {
    pub patch: DetailPatch,
    pub provenance: Vec<ContributionProvenance>,
    pub conflicts: Vec<ConflictMarker>,
    pub rejections: Vec<PhaseRejection>,
    pub budget_report: StrategySetBudgetReport,
    pub diagnostics: Vec<StructuredDiagnostic>,
}

pub async fn execute_discovery(/* landed typed inputs */)
    -> Result<DiscoveryPhaseResult, PhaseCancelled>;
pub async fn execute_detail(
    occurrence: &PostingOccurrence,
    requested: &NonEmptyRequestedFields,
    /* landed typed execution inputs */
) -> Result<DetailPhaseResult, PhaseCancelled>;

pub(crate) fn reduce_discovery_occurrences(
    accepted_attempts: &[AcceptedDiscoveryAttempt],
) -> ReducedDiscovery;
pub(crate) fn reduce_detail_patches(
    occurrence: &PostingOccurrence,
    requested: &NonEmptyRequestedFields,
    accepted_attempts: &[AcceptedDetailAttempt],
) -> ReducedDetail;
```

The reducers’ accumulator outputs are private. Reuse the landed `StrategySetBudgetReport`—typed completion plus exact usage—Cancellation, identity, keys, attempts, and typed value plans rather than duplicating them. T12b preserves the established budget-exhaustion terminal and never reconstructs, narrows, flattens, or splits the report.

`PostingOccurrence` remains the only Discovery occurrence payload. Do not add a reduced occurrence DTO, provenance-bearing occurrence wrapper, conversion snapshot, public reducer, reducer trait, or generic contribution bag. Public Attempt-derived observability is limited to `ContributionOrigin`; indexes are coordinates, not handles into Attempt History.

### Detail authoring and requests

- The finite Detail patch field set is exactly title, company, raw provider locations, and description text. URL remains the required reference, not a patch field.
- Schema-v3 Detail output admits optional expressions for exactly those fields; a complete Strategy declares at least one. Source specialization uses the same nested schema-v3 Detail vocabulary and inherited merge rules—no Source-only patch language. Unknown/dynamic fields, `null`, URL replacement, hints, postingMeta mutation, Search Request fields, and persistence fields fail schema, direct Serde, and compiler parity checks.
- Each expression compiles through the landed typed value-plan/context rules. No authored `FieldExpression`, authored JSON, dynamic field string, or second evaluator crosses the runtime boundary.
- `NonEmptyRequestedFields` is typed runtime input, not authored JSON. Duplicate fields collapse deterministically; empty construction fails before execution.
- Each accepted Detail attempt targets exactly one occurrence and contributes only requested, non-empty available fields. One bounded response may contribute several requested fields.

### Provenance and retention

Origins contain only a validated compiled Strategy key, the zero-based private deterministic Attempt position, and an optional zero-based provider-item position (`None` for one-per-attempt Detail). They contain no values, bodies, descriptions, Source Config, URLs, arbitrary JSON, paths, error strings, or secrets.

Origins are ordered by `attempt_index`, then `provider_item_index`; exact duplicates appear once. For every retained or conflicted responsibility, contributors are complete across accepted attempts/items. `ContributionProvenance` exists only for retained responsibilities; conflicts and rejections own their contributors instead of duplicating them into generic provenance. Entry counts and numeric indexes inherit landed Strategy/item/cumulative-budget limits. Identifier strings are validated, but this ticket claims no byte/character maximum without landed evidence.

For URL-fallback reduction, the retained reference is the normalized identity URL. Original spellings must be absent from the complete serialized public result, including payload, provenance, conflicts, rejections, budget report, and Diagnostics. Provider-ID occurrences retain their one non-conflicting validated URL.

### Discovery reduction

1. Group accepted typed output only by exact T12a occurrence identity. Never correlate mixed identity kinds or deduplicate across Sources.
2. Preserve first-seen provider order. Map/hash insertion order must not affect any envelope field or Diagnostic.
3. Retain equal non-empty values once with complete ordered contributors. Missing plus present is not a conflict.
4. Conflicting optional title, company, description, or locations quarantine only that provider field; other responsibilities survive.
5. Treat each raw location vector atomically: equality includes length, bytes, order, whitespace, and duplicates. Never normalize, concatenate, union, sort, split, trim, or deduplicate.
6. Reduce hints and postingMeta independently by stable key. Equal complete typed values merge; conflict quarantines only that key. Hint structure cannot become canonical data.
7. Equal provider-ID identities with distinct required provider URLs reject the entire group. Return no occurrence or optional field from it, and select no URL.
8. Equal URL-fallback identities use the normalized identity URL and discard original spellings.
9. A conflict never heals after later equal input. Its marker includes every contributor in deterministic order.

### Detail reduction

Fields merge independently using the same exact scalar/vector rules. A conflict removes only that field from `DetailPatchValues`, emits its marker, and must not be reported available by any helper. No contribution means absent without conflict.

Before T13, `first_accepted` permits at most one accepted Detail Attempt through the public operation. Define the multi-Attempt reducer now, but test complementary/equal/conflicting snapshots only in narrow in-module tests; do not invent a Policy, fake public executor, alternate phase operation, or public reducer.

Defensively reject unrequested fields and identity mismatches. `UnrequestedDetailField` carries the exact typed field. Every contribution rejection exposes the **expected/requested** identity passed to Detail, never a contributed foreign identity. A mismatched identity and all its components or derivations are discarded before envelope construction and forbidden from rejection, provenance, conflicts, budget report, Diagnostic path/message/details, and all serialization.

### Diagnostics, ordering, and cancellation

Reducer Diagnostics have category `runtime`, severity `error`, and exactly these stable codes:

| Code | Responsibility |
|---|---|
| `discovery_provider_field_conflict` | canonical provider field conflict |
| `discovery_hint_conflict` | one hint key conflict |
| `discovery_posting_meta_conflict` | one postingMeta key conflict |
| `discovery_required_provider_url_conflict` | whole-group required URL rejection |
| `detail_field_conflict` | requested Detail field conflict |
| `detail_unrequested_field` | typed unrequested field rejection |
| `detail_occurrence_identity_mismatch` | sanitized mismatch rejection |

Use RFC 6901 logical reduction paths, not compacted payload or authored-document paths:

- `/discovery/reduction/groups/{groupIndex}/providerValues/{fieldName}`;
- `/discovery/reduction/groups/{groupIndex}/hints/{escapedHintKey}`;
- `/discovery/reduction/groups/{groupIndex}/postingMeta/{escapedPostingMetaKey}`;
- `/discovery/reduction/groups/{groupIndex}/rejection/reference/url`;
- `/detail/reduction/patch/{fieldName}`;
- `/detail/reduction/patch/occurrenceIdentity`.

`groupIndex` is the zero-based first-seen logical group before quarantine/rejection, so compaction cannot retarget a Diagnostic. Escape `~` as `~0` and `/` as `~1`. Field names are `title`, `company`, `locations`, and `descriptionText`. Multi-contributor Diagnostics leave optional `strategy_key` unset.

Diagnostic `details` have exactly these camelCase members: Discovery `groupIndex` when applicable; expected/retained validated `sourceKey`; `identityKind` (`provider_posting_id` or `normalized_url`); `responsibility`; and `contributors`. Each contributor contains exactly `strategyKey`, `attemptIndex`, and `providerItemIndex` when non-`None`. `responsibility.kind` is exactly `provider_field`, `hint_key`, `posting_meta_key`, `detail_field`, `required_provider_url`, `detail_unrequested_field`, or `detail_occurrence_identity_mismatch`. `responsibility.name` is required for the first four kinds and `detail_unrequested_field` (canonical field name or validated hint/postingMeta key), and omitted for required URL and identity mismatch. No identifier-length maximum is claimed without landed evidence.

Mismatch details contain exactly `sourceKey`, `identityKind`, `responsibility: { "kind": "detail_occurrence_identity_mismatch" }`, and `contributors`; they describe only the expected identity and omit `groupIndex`, responsibility name, provider item index when `None`, and every foreign identity component. Unrequested-field details use the same Detail shape with kind `detail_unrequested_field` and the canonical field name. No other member—including posting ID, URL, conflicting value, response data, message, or arbitrary JSON—is permitted.

Messages are responsibility-only and may name only phase, responsibility, canonical field, or validated hint/postingMeta key. Whole-result serialization tests seed unique sentinels only into quarantined/rejected values, original URL spellings, postingMeta contents, response data, and foreign identity components, then prove no sentinel appears anywhere. Expected identity and retained fields must not contain those sentinels.

Append reducer Diagnostics after contributing accepted-attempt Diagnostics and before the phase terminal summary. Discovery responsibility order is required URL, title, company, locations, descriptionText, bytewise hint keys, then bytewise postingMeta keys. Detail uses canonical field enum order.

Reducers perform no I/O or cancellation polling. Existing cumulative limits bound their finite input. Cancellation before envelope commit releases no reduced payload and remains typed cancellation, never persistable Resolution Partial Completion.

## Dependency and deletion decision

Occurrence, field, patch, provenance, conflict, rejection, and reducer logic are concrete in-process data/logic. T12a identity/value plans are reused directly. Existing HTTP/browser production and deterministic adapters remain behind typed phase operations; this ticket adds no external seam. Search normalization, Candidate Resolution, and SQLite remain outside.

**Deletion test:** Without the crate-private reducers and public envelope responsibilities, Discovery/Detail adapters, later T13 policies, T15 Source execution and Source Live Check, and T16 Candidate Resolution would each repeat identity grouping, exact equality/location atomicity, requested-only filtering, contributor association, quarantine, required-URL rejection, and Diagnostic ordering. A forwarding wrapper does not pass.

## Examples

1. Two equal-identity Discovery contributions with equal title/location values and complementary company/postingMeta become one T12a occurrence; every retained responsibility lists all ordered origins.
2. Titles `Engineer` and `Senior Engineer` quarantine title while equal `locations = ["Mainz", "Remote"]` survives. Reversing the locations or changing whitespace conflicts atomically.
3. Equal provider-ID identities with different URLs reject the whole group; equivalent URL-fallback spellings produce the normalized identity URL with no original spelling serialized.
4. A Detail response may return requested title, locations, and description while unrequested company is omitted. Private multi-Attempt snapshots prove equal merge and field-local conflict behavior before T13.
5. A foreign Detail identity containing sentinel ID/URL data yields only the expected identity and sanitized mismatch responsibility publicly.
6. A normal completed policy outcome may reduce only its completed accepted inputs. Cumulative budget exhaustion preserves the landed typed budget outcome/report and exposes no reduced phase payload; Cancellation before envelope commitment returns typed Cancellation and no envelope.

## Scope

- Re-baseline against landed T12a, then add the four typed Detail fields, non-empty requested set, requested-only patch, schema/Serde/Source-fragment/compiler parity, and immutable typed value plans.
- Add explicit Discovery/Detail envelopes and crate-private reducers with the exact merge, order, provenance, quarantine, rejection, Diagnostic, and retention behavior above.
- Integrate behavior reachable under `first_accepted`; use private tests only for pre-T13 multi-Attempt Detail semantics and unreachable protocol violations.
- Migrate direct typed phase callers/tests and delete the description-only Detail result/DTO, separate occurrence DTO, authored runtime-plan expressions, duplicate accumulators/merges, aliases, wrappers, conversion snapshots, and superseded tests.
- Update active canonical domain/DSL documentation and preserve generic Greenhouse, Workday, and SuccessFactors behavior.

## Adjacent non-goals

- T13/#202–#204 owns additional Policies and public multi-Attempt integration.
- T15/#219 owns the public Source field-request seam, capability/disposition routing, prior-value reuse, deterministic Source execution fake, and caller migration to that seam.
- T16/T17 own Candidate Resolution, Search Request evaluation, normalization, batching, budgets, completion/counts, diagnostic sampling, finalization, and persistence.
- Detection reduction, cross-Source deduplication, structured Location semantics (#57), persisted markers, new statuses, parallelism, resumability, dynamic fields, URL patches, hint-to-canonical conversion, and provider-specific behavior remain outside.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| Complementary/equal Discovery | One T12a occurrence; equal values once; complete ordered deduplicated origins | Public typed Discovery operation |
| Missing plus present | Present optional value survives without conflict | Public Discovery table case |
| Provider scalar/location conflict | Only responsibility is absent; raw vectors compare byte/order/duplicate-exactly | Public Discovery table cases |
| Hint/postingMeta equal/conflict | Equal keyed value merges; only conflicting key quarantines; escaped logical path | Public Discovery operation |
| Provider-ID URL conflict | Whole group rejected with exact typed rejection/code/path; permutations cannot choose first/last | Public operation plus permutation tests |
| URL fallback | Normalized identity URL retained; original spellings absent from whole serialization | Public operation plus sentinel test |
| Determinism | Repeated/permuted-map execution yields equal payload, metadata, paths, and Diagnostics | Public rerun test |
| Provenance/privacy | Only bounded validated-key/index origins; complete and ordered; no Attempt History or values | Public API/serialization/static review |
| One occurrence type/private reducers | T12a payload only; no wrapper/alias/public reducer or indirect export | Compile assertion and structural/manual review |
| Requested set | Duplicates collapse; empty construction fails before execution | Public Detail input test |
| Requested multi-field Detail | One response contributes all requested available fields and no unrequested field | Public Detail operation |
| Missing requested field | Absent with no conflict | Public Detail operation |
| Multi-Attempt Detail equal/conflict | Equal values merge; conflicting field quarantines while others survive | Narrow private reducer tests only |
| Unrequested contribution | Typed field retained in reason; rejection carries expected identity and exact sanitized details | Private reducer plus envelope serialization test |
| Identity mismatch | Expected identity only; foreign identity/ID/URL/derivations absent everywhere | Private reducer plus whole-result sentinel test |
| Diagnostics | Seven exact codes, severity/category, paths, detail vocabulary, escaping, and order | Public operations plus private Detail cases |
| Normal completed reduction | Only completed accepted outputs reduce; the single complete `StrategySetBudgetReport` and ordered Diagnostics are preserved | Public phase regressions |
| Budget exhaustion | Landed typed budget outcome/report remains intact; no reduced occurrence/patch/provenance/conflict/rejection payload is exposed | Public Strategy Set budget regressions |
| Cancellation | Typed cancellation before commit; no envelope/Partial Completion | Public Discovery and Detail regressions |
| Schema/Serde/compiler | Exactly four optional fields; forbidden dynamic/null/mutation shapes rejected consistently | External compiler/schema parity tests |
| Immutable runtime | No authored expression/JSON reaches runtime plan | Compiler/primitive tests and import search |
| Acceptance profiles | Greenhouse, Workday, and SuccessFactors-shaped contributions use generic envelope behavior without provider branches | Three deterministic profile integrations |
| Source Live Check conflict | A reachable Discovery conflict is consumed as trustworthy payload plus marker/Diagnostic without inventing a value | Source Live Check integration test |
| Cross-Source boundary | Source-local reduction absent from Job Posting deduplication/persistence | Search Run regression and call-graph review |
| Deletion | No description-only DTO, second occurrence/wrapper, aliases, public reducer, raw runtime expression, first/last merge, or location normalization | Structural checks, searches, and mandatory Public-API/Serialization/Call-Graph review |

### Focused commands

Inspect landed T12a target names and adapt names, not responsibilities:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_resolution
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_semantic_validation
cargo test --manifest-path src-tauri/Cargo.toml --test primitive_registry
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_set_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test discovery_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test detail_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test source_live_check
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml search::run
cargo test --manifest-path src-tauri/Cargo.toml search::posting
```

If T12a retains current target names, also run `posting_discovery_runtime` and `posting_detail_runtime`. Shared full-suite/frontend requirements follow the delivery contract.

## Ticket-specific migration items

- [ ] Inventory landed T12a occurrence/reference/value types, accepted attempts, phase operations, exports, Diagnostics, tests, and direct callers before implementation.
- [ ] Replace description-only authored/plan/result shapes with exactly four compiled typed fields and requested-only patches.
- [ ] Delete `PostingDetailPostingOccurrence`, `PostingDetailExecutionResult`, authored runtime-plan `FieldExpression`, local first/last merges, duplicate accumulators, occurrence/Attempt/envelope aliases or wrappers, conversion snapshots, and superseded tests after callers move.
- [ ] Confirm reducers are crate-private, T8 state remains private, and no T13 Policy/test executor or T15/T16 path was introduced.
- [ ] Run and classify every hit from:

```bash
rg -n '\b(PostingOccurrence|DiscoveryPhaseResult|DetailPhaseResult|DetailPatch|ContributionOrigin|ConflictMarker|PhaseRejection|StrategyAttemptHistory)\b' src-tauri/src src-tauri/tests --glob '*.rs'
rg -n '\b(discovery_provider_field_conflict|discovery_hint_conflict|discovery_posting_meta_conflict|discovery_required_provider_url_conflict|detail_field_conflict|detail_unrequested_field|detail_occurrence_identity_mismatch)\b' src-tauri/src src-tauri/tests --glob '*.rs'
rg -n '\b(PostingDetailPostingOccurrence|PostingDetailExecutionResult)\b' src-tauri/src src-tauri/tests --glob '*.rs'
rg -U -n '^\s*pub\s+(mod|use)\b[^;]*;' src-tauri/src/lib.rs src-tauri/src/profile_dsl --glob '*.rs'
rg -n '\.(first|last)\(\)|\[[[:space:]]*0[[:space:]]*\]|\.next\(\)|or_insert' $(rg -l '\b(reduce_discovery_occurrences|reduce_detail_patches)\b' src-tauri/src --glob '*.rs')
```

- [ ] Run a name-independent structural check and manual Public-API, Serialization, and Call-Graph review for second occurrence/description DTOs, aliases/wrappers, `serde_json::Value` in envelope data, public reducers or indirect exports, private Attempt state exposure, authored runtime expressions, first/last selection, and reducer-level location normalization.

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
