# Issue #315 migration ledger

## LOC disposition

Counts use physical Rust lines in the ownership slice. Explicit `test-support` and `#[cfg(test)]` code is counted as test, not production.

| Scope | Before | After | Delta |
| --- | ---: | ---: | ---: |
| Production: engine `src`, Desktop forwarding adapter/module, and removed Desktop export wall | 37,620 | 37,573 | -47 |
| Tests: engine integration tests, explicit engine test support, Desktop engine tests, native Reqwest adapter test | 21,639 | 21,630 | -9 |

Before components: engine production 37,312; Desktop forwarding 123; Desktop engine export wall 185. The test baseline includes 21,336 engine-test lines, 228 Desktop Source-engine test lines, and the 75-line Reqwest adapter contract. After excludes 82 lines of explicit engine `test_support.rs` and 67 lines of the private Reqwest adapter unit test from production. Test migration added the `schemas` target and compile-fail privacy contracts while deleting export-only/raw-plan Desktop coverage.

## Public export inventory

### Before

- Crate roots: `profile_dsl`, `source`, `source_profile`.
- Flat root exports: compiler products and stages, all authored documents, execution-plan nodes, every primitive family/evaluator/descriptor, templates, Detection internals, runtime reducers/outcomes/adapters, scripted HTTP/browser/detail implementations, transitional Source types.
- Desktop root duplicated the engine wall, including primitive completeness, raw plans, runtime, scripted adapters, Detection internals, and templates.

### After ordinary/default build

- Crate root exposes only `definition`, `detection`, and `execution`.
- `definition` exposes authored Source Behavior Language documents, Diagnostics, transitional Source documents/status, profile validation, compile outcome, and opaque `CompiledSource` behavior operations. Raw plan fields and primitive/compiler modules are private.
- `detection` exposes preparation/execution and typed proposal/outcome contracts; implementation files live directly under `detection`.
- `execution` exposes Discovery/Detail operations, Posting Occurrences, typed outcomes/usage, and HTTP/Browser/Cancellation ports. Scripted implementations are absent from the default interface.
- `test_support` exists only with the explicit `test-support` feature and owns scripted adapters plus private edge-test access.
- Desktop root has no Source engine re-export wall. `src/profile_dsl` retains only the private Reqwest adapter pending #320.

## Test disposition

| Before target/test | Disposition |
| --- | --- |
| `compiler` | Replaced by `definition`; compiler and semantic/security/provenance edges retained. |
| `primitives` | Folded into `definition`; exhaustive primitive/completeness edges retained privately through explicit test support. |
| `detection` | Retained as the deliberate Detection interface target. |
| `runtime` | Replaced by `execution`; Discovery, Detail, occurrence, allowance, HTTP and browser edges retained. |
| Desktop `source_profile_dsl/exports.rs` | Deleted; it tested only preservation of the superseded flat export wall. |
| Desktop raw execution-plan composition assertions | Deleted; Definition/Execution targets retain those private edges while productive callers use intentional operations. |
| Source document schema coverage | Existing Desktop cross-document tests updated to crate-root schemas; new engine `schemas` target verifies catalogue IDs and canonical terminology. |
| Reqwest adapter integration test | Moved beside the private Desktop adapter as a unit contract. |
| Frontend schema tests/support | Renamed to Source Behavior Language vocabulary and updated to the crate-root schema catalogue. |

## Validation notes

Focused Definition, Detection, Execution, schemas, Candidate Resolution, Desktop Source behavior, bundled profiles, frontend Sources, primitive residue, and `just quick` pass. Package/crate name `source-profile-dsl` / `source_profile_dsl`, transitional `src/source`, and Desktop `src/profile_dsl/http_reqwest.rs` are the intentional temporary technical exceptions until later #314 slices.
