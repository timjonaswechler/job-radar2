# Primitive completeness fixtures

These files are frozen inputs for the Global Primitive Completeness Gate. They are test fixtures, not runtime registries or documentation.

| File | Purpose |
|---|---|
| `primitive-positive-catalogue.txt` | Exact normalized Primitive occurrences discovered from the checked-in JSON Schemas. |
| `primitive-schema-owner-catalogue.txt` | Independent expected Schema owner, canonical file, shape, and compiled identity metadata. |
| `primitive-serde-owner-catalogue.txt` | Independent expected Serde owner, canonical file, shape, and compiled identity metadata. |
| `primitive-compiled-catalogue.txt` | Exact typed compiled registrations and callable identities. |
| `primitive-residue-classification.txt` | Reviewed classification of every repository hit for removed or restricted Primitive vocabulary. |

The independent catalogues prevent the gate from deriving both actual and expected values from one implementation. A change is therefore reviewed as an explicit contract change instead of silently regenerating its own expectation.

Run the focused checks from the repository root:

```bash
just rust-crate-test source-engine primitives schema_inventory::
just rust-crate-test source-engine primitives serde_inventory::
just rust-crate-test source-engine primitives compiled_registration::
just rust-crate-test source-engine primitives registry_structure::
just primitive-residue
```

To review residue changes, first generate the current evidence set:

```bash
just primitive-residue-emit
```

Classify every added, removed, or relocated hit deliberately. The normal check also verifies the reviewed SHA-256 embedded in `scripts/checks/primitive-residue.sh`.
