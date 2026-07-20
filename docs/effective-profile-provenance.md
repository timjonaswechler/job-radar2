# Effective Source Profile provenance

The dormant final `compile_source` boundary records typed provenance alongside each successful `CompiledSource`. Productive schema-v2 callers remain on `compile_source_execution_plan` until A01.

## Representation

`CompiledSourceProvenance` is a closed `profile` or `source_owned` value containing ordered `ProvenanceEntry` values. Each entry has only:

- a segmented `ProvenancePath` (`field`, `access_path`, `strategy`, or `map_key` segments); and
- one closed origin: `base_source_profile`, `direct_source_fragment`, or `source_owned_access_path`.

Paths do not use array indexes, JSON Pointers, compact aliases, or version wrappers. Values, concrete Source Config, secrets, diagnostics, runtime/check data, persistence identifiers, timestamps, and fingerprints are not retained.

## Covered terminal surface

The compiler records each applicable execution-relevant terminal exactly once:

- profile- and Access-Path-level Source Config schemas;
- Access Path keys and the names of complete Source-added or Source-owned paths;
- Discovery and Detail policy, Strategy keys, safe Strategy material, and acceptance;
- scalar and legal null terminals, whole non-keyed arrays, empty objects, and nested object leaves;
- dynamic schema properties, headers, bodies, captures, and posting metadata in lexical key order.

Profile identity/catalog metadata, Detection, descriptions, support/known issues, authored diagnostics, concrete Source Config, private validators, and compiler-derived plan-only fields are excluded.

## Origin and ordering rules

The private recorder is part of the keyed merge/materialization pipeline. Existing Access Path and Strategy key locators remain base; any explicit replacement, including an equal replacement, is direct. Complete additions are direct throughout, while an empty locator-only fragment changes no origin. Source-owned terminals are uniformly source-owned.

Static members use typed-document order, Access Paths and Strategies retain effective semantic order, and dynamic map keys are lexical. Arrays are atomic. This makes equivalent map insertion orders produce byte-identical serialized provenance.

Schema properties can mix origins leaf by leaf. New properties are direct throughout; `required` and `enum` are whole-array terminals; reusable-profile `title` remains base. Direct and Source-owned title authoring rejects before provenance is returned.

## Invariant and runtime boundary

The compiler validates unique, complete path coverage before exposing a result. Duplicate or missing coverage rejects with error code `compiler/compiled_provenance_invariant_violation`, an empty diagnostic document path, and `details.reason` plus the typed `details.provenancePath`. Rejections expose diagnostics only.

Runtime receives only the immutable Execution Plan. Provenance is neither persisted nor used for runtime branching.
