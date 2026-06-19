# System profiles (superseded)

System profiles and browser profiles are no longer active Job Radar authoring or runtime concepts.

Use the JSON Source Registry instead:

- model: [`docs/source-registry-json-model.md`](source-registry-json-model.md)
- source profile schema: [`docs/schemas/source-profile.schema.json`](schemas/source-profile.schema.json)
- source schema: [`docs/schemas/source.schema.json`](schemas/source.schema.json)

Current source knowledge lives in:

```txt
source-profiles/builtin/*.json
sources/builtin/*.json
```

Custom runtime/user documents live in the OS app data directory:

```txt
source-profiles/*.json
sources/*.json
```

Historical notes:

- ADR-0004 introduced declarative system profiles and has been superseded by ADR-0006 and ADR-0007.
- ADR-0002 introduced declarative browser profiles and has been superseded by ADR-0006 and ADR-0007.
- Legacy `system_profiles`, `browser_profiles`, and `sources` domain tables are not part of the current schema.
- Legacy `system-profiles/` and `browser-profiles/` registry directories are not active sources of truth.
