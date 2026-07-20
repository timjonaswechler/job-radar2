# Canonical Source behavior fingerprints

Schema-v3 Source Live Check uses this C08 preparation as its sole freshness identity path. Raw whole-document and live-check-logic fingerprints do not exist. Check, status, activate, and reactivate load one immutable registry snapshot, reuse its exact authoritative `compile_source` outcome, and prepare once. Activation persists the already checked set unchanged without reload or re-preparation.

## Preparation boundary

`prepare_source_behavior_fingerprints` receives the authoritative typed Source, the optional resolved Base Source Profile, and the exact `compile_source` outcome. Exact operation-local pairing is a caller precondition: preparation checks structural coherence but does not replay compilation or reconstruct the merge. The registry compiles each authoritative Source once while constructing the immutable snapshot; the operation passes that same Source/Base/outcome tuple directly into its one preparation call. The operation returns the complete ordered `Vec<CheckFingerprint>` or one value-free error. It does not persist a partial set, projection material, Source Config values, or version tokens.

All behavior rows use strict `(kind, reference)` identities and independently serialize and SHA-256 hash a closed projection. Dynamic JSON objects retain recursively sorted map keys through the typed `serde_json` representation; semantic array order is preserved. Source Config Schema `title` annotations and other non-executable metadata are excluded.

## Branch order and counts

Profile success:

1. `base_source_profile`
2. optional `direct_source_specialization`
3. `effective_source_profile`
4. `compiler_provenance`
5. `source_config`
6. `selected_access_path`
7. optional `source_runtime_bindings`
8. fixed six-row tail

This produces 11, 12, or 13 rows. Source-owned success starts with `source_owned_access_path`, then provenance, config, selector, optional bindings, and the tail, producing 10 or 11 rows.

Rejected compilation contains only independently available authored material. A resolved Profile rejection produces 9 or 10 rows; an unresolved Base produces 8 or 9 rows; a Source-owned rejection produces 9 rows. Effective, provenance, runtime-binding, and fabricated effective behavior rows are never emitted for rejection.

A direct fragment containing no execution behavior is absent. Any explicit execution terminal remains represented even when it replaces a Base value with the same value.

## Runtime bindings

Template validation emits compiler-owned `SourceRuntimeBindingDependencies` during the same validation and plan-compilation walk. Dependencies are unique in canonical enum order and only the selected Access Path's dependencies reach a successful `CompiledSource`. Checks code never scans templates or serialized plans.

The currently closed binding inventory is `SourceRuntimeBinding::Name`. A `source_runtime_bindings` row is present only when `source:name` is referenced and hashes the typed `{ name }` value. Otherwise Source name is excluded.

## Version and immutable-global tail

The tail always contains exactly these rows in order:

1. `behavior_version/profile_compiler`
2. `behavior_version/profile_runtime`
3. `behavior_version/immutable_globals`
4. `immutable_global_behavior/source_live_check_pagination_smoke_budget`
5. `immutable_global_behavior/compiler_max_fallback_strategies`
6. `immutable_global_behavior/security_forbidden_request_key_behavior`

Compiler, plan, provenance, or request-key security semantics update canonical material or bump `profile_compiler`. Runtime interpretation changes bump `profile_runtime`. Approved immutable-global inventory or material follows `immutable_globals`. Adding a fourth partition requires a new architecture decision.
