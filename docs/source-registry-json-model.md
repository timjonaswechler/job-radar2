# Source registry JSON model

This document turns the profile/source model from issue #34 and ADRs 0006/0007 into an implementation target.

## Scope

Job Radar treats sources and source profiles as authoritative JSON documents. SQLite stores search requests, search runs, results, caches, and diagnostics, but it does not own source/profile domain records.

## Files

Repository / app bundle:

```txt
source-profiles/builtin/*.json
sources/builtin/*.json
```

App data directory:

```txt
source-profiles/*.json
sources/*.json
```

Rules:

- File name must be `<key>.json` and match the document `key`.
- Keys use `^[a-z0-9_]+$`.
- Built-in/custom origin is derived from location, not from JSON fields.
- Custom documents do not override built-ins. A custom document with a built-in key is a duplicate-key diagnostic and is ignored.
- Invalid JSON documents are reported as diagnostics and are not silently repaired.
- Registry reads freshly from bundled/app-data files per API call. A search run builds one immutable in-memory execution snapshot at start and uses it until the run ends.

## Schemas

Authoring schemas live in:

```txt
docs/schemas/source-profile.schema.json
docs/schemas/source.schema.json
```

They are intentionally stricter than the previous DB-backed model:

- use `schemaVersion`, not `definitionSchemaVersion`
- no database IDs
- no timestamps
- no persisted `validationError`
- no `builtIn`/`custom` flags
- no first-pass i18n fields
- source status remains in source JSON
- profile `kind` is required and is only classification/UI metadata

## Source profile document

Minimal shape:

```json
{
  "schemaVersion": 1,
  "key": "greenhouse",
  "name": "Greenhouse",
  "kind": "recruiting_system",
  "detect": {
    "phases": ["http", "browser"],
    "required": []
  },
  "identity": {
    "keyCandidates": [
      "{{capture:companyWebsite|domainKey}}",
      "{{capture:boardSlug|technicalKey}}"
    ],
    "nameCandidates": [
      "{{capture:companyWebsite|domainTitle}}",
      "{{capture:organizationName}}",
      "{{capture:boardSlug|titleCase}}"
    ]
  },
  "sourceConfigSchema": {
    "type": "object"
  },
  "accessPaths": [
    {
      "key": "endpoint_inventory",
      "adapterKey": "declarative_endpoint_inventory",
      "availability": {
        "requiredCaptures": ["boardSlug"],
        "checks": [],
        "sourceConfig": {
          "boardSlug": "{{capture:boardSlug}}",
          "startUrl": "{{inputUrl}}"
        }
      },
      "sourceConfigSchema": {
        "type": "object",
        "required": ["boardSlug"],
        "additionalProperties": false,
        "properties": {
          "boardSlug": {
            "type": "string",
            "pattern": "^[A-Za-z0-9][A-Za-z0-9_-]*$"
          },
          "startUrl": { "type": "string", "format": "uri" }
        }
      },
      "inventory": {}
    }
  ]
}
```

Semantics:

- A source profile is reusable and must not be tailored to exactly one source.
- `detect` identifies the reusable profile.
- `accessPaths[]` defines allowed technical access paths.
- `availability` belongs only to profile access paths and decides whether that path is usable for a concrete submitted entry point.
- `availability.sourceConfig` may contain templates and static values, but not search criteria.
- Profile-level `sourceConfigSchema` contains fields common to all access paths.
- Access-path `sourceConfigSchema` contains path-specific fields.
- Effective validation for a profiled source is profile schema + selected access-path schema.

### Reusable source config field names

Use shared field names when different recruiting systems expose the same concept. This keeps source documents and generated proposals predictable across profiles.

- `boardSlug` is the preferred field/capture name for a vendor-local job-board token, tenant slug, board name, or postings API path segment. Examples include Ashby job board names, Greenhouse board tokens, and Lever site/postings slugs when the profile can resolve inventory from that value.
- `boardSlug` is scoped by the selected profile; it is not globally unique and should not be used alone as a source key unless the key candidate has no better company/domain input.
- `startUrl` is optional context for the submitted page or canonical career URL. Do not require it when inventory can be derived solely from `boardSlug`.
- Provider-specific names are still allowed when the concept is genuinely different, but avoid introducing aliases such as `boardToken`, `companySlug`, or `postingSlug` for the same reusable board identifier.

### Declarative inventory location fields

`inventory.fields.locations` is an array of location expressions. Each expression may produce zero, one, or many locations:

- a string result is one location by default;
- an array result contributes each scalar array item as one location;
- `null`, missing values, and empty strings are ignored;
- duplicate locations are removed while preserving first-seen order;
- optional `"split": "<delimiter>"` on a location expression splits string results by that delimiter after trimming parts.

Do not split by punctuation implicitly: values such as `Berlin, Germany` are single locations unless the profile explicitly declares `split`.

## Source document

Profile-backed source:

```json
{
  "schemaVersion": 1,
  "key": "helsing",
  "name": "Helsing",
  "status": "draft",
  "sourceConfig": {
    "boardSlug": "helsing",
    "startUrl": "https://helsing.ai/de/jobs"
  },
  "selectedAccessPath": {
    "type": "profile",
    "profileKey": "greenhouse",
    "pathKey": "endpoint_inventory"
  }
}
```

Source-specific fallback:

```json
{
  "schemaVersion": 1,
  "key": "example_company",
  "name": "Example Company",
  "status": "draft",
  "sourceConfig": {
    "startUrl": "https://example.com/jobs"
  },
  "selectedAccessPath": {
    "type": "source_specific",
    "adapterKey": "declarative_browser_inventory",
    "sourceConfigSchema": {
      "type": "object",
      "required": ["startUrl"],
      "properties": {
        "startUrl": { "type": "string", "format": "uri" }
      }
    },
    "interactions": [
      { "type": "waitFor", "selector": ".job-card", "timeoutMs": 15000 }
    ],
    "inventory": {
      "items": { "select": ".job-card" },
      "fields": {
        "title": { "selectorText": ".job-title" },
        "company": { "template": "{{sourceName}}" },
        "url": {
          "selectorAttribute": { "selector": "a", "attribute": "href" }
        },
        "locations": []
      }
    }
  }
}
```

Semantics:

- A source has exactly one `selectedAccessPath`.
- `type: "profile"` references one profile-defined access path by stable keys.
- `type: "source_specific"` embeds one access-path definition directly in the source.
- Source-specific access paths have no `availability`; they are already selected.
- Source-specific access paths are authored from one concrete page and are not implicit profile candidates.
- Source-specific browser access may use bounded declarative interactions such as `waitFor`, `clickIfVisible`, and `clickUpToN`; arbitrary scripting is not part of the model.

## Profile detection

Profilerkennung returns a source proposal, not just a profile match:

```txt
profileKey
selectedAccessPath.pathKey
sourceConfig
key/name candidates
evidence
```

Flow:

1. Direct HTTP checks against source profiles.
2. If blocked or unsupported, browser-assisted analysis may render the page, inspect DOM/HTML/embedded JSON/network requests, and retry profile/access-path matching.
3. Browser use during detection does not imply that the resulting source uses a browser access path.
4. If no reusable profile/access path is available, UI may offer source-specific extraction via element selection.

Named captures from detection, for example `boardSlug`, are reusable evidence fields. They may feed identity candidates and `availability.sourceConfig` templates, and should use the shared field-name guidance above when the same concept appears across profiles.

Detection `required` checks are conjunctive: every required check must pass. Optional `detect.anyOf` adds ordered OR-style alternatives after `required`: each alternative is an array of detection checks that must all pass, and the first passing alternative wins. If `anyOf` exists and no alternative passes, the profile does not match. Captures from `required` plus the selected alternative are available to identity templates, access-path availability, and `availability.sourceConfig`; captures from failed alternatives are discarded.

When a regex check has `captureAs`, the detector stores the first non-empty capture group from the first regex match, so an alternative can normalize one URL shape into a reusable field such as `boardSlug`.

A profile access path may be recommended only when:

- the profile itself has required evidence,
- the access path has required captures/config values,
- the access path availability checks pass or are otherwise plausibly executable.

## Registry API target

Initial Rust module target:

```txt
source_registry::load_snapshot(app_data_dir) -> SourceRegistrySnapshot
SourceRegistrySnapshot.valid_profiles
SourceRegistrySnapshot.valid_sources
SourceRegistrySnapshot.diagnostics
SourceRegistrySnapshot.resolve_source(key) -> ResolvedSourceExecutionPlan
```

Diagnostics should include document path/origin and error. Invalid documents are not repaired by the registry.

Search requests store source keys:

```txt
source_keys_json
```

Search runs/source runs store `source_key`; they do not need persistent source/profile snapshots. Consistency is guaranteed only inside a running search run by using the immutable registry snapshot loaded at run start.

## Implementation notes

The JSON Source Registry cut is implemented directly:

1. JSON schemas and Rust document structs cover source profiles, sources, selected access paths, access path definitions, and registry diagnostics.
2. `source_registry` loads bundled built-ins plus app-data custom documents fresh, validates document shape, checks key/file-name consistency, applies duplicate-key rules, and returns valid documents plus diagnostics.
3. Built-in source-profile and source documents live under `source-profiles/builtin/` and `sources/builtin/`.
4. The current development SQLite schema has no source/profile domain tables; search requests store `source_keys_json`.
5. Detection uses source-registry source profiles and profile access-path availability.
6. Search execution resolves source keys into one registry execution-plan snapshot at run start.
7. Tauri commands/UI list sources, profiles, and diagnostics from the registry instead of database-owned source/profile tables.
8. Tests cover schema validation, duplicate-key diagnostics, missing profile/path diagnostics, registry snapshot consistency, and search execution via profile and source-specific access paths.

The legacy `system-profiles/`, `browser-profiles/`, `system_profiles`, `browser_profiles`, and database-owned `sources` model is historical only; use this document and `docs/schemas/` for current authoring.
