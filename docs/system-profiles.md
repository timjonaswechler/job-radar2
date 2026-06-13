# System profiles

A system profile is a declarative JSON definition of a recruiting system or career-system family. It is not an adapter. Adapters are technical runtimes such as `declarative_endpoint_inventory`, `declarative_sitemap_inventory`, or `declarative_browser_jobboard`.

## Storage model

```txt
Repository / app bundle:
  system-profiles/builtin/*.json
  -> embedded into the Rust binary with include_str!

OS app data directory:
  job_radar.db
  system-profiles/*.json
  -> local custom/user profiles
```

Built-in profiles are shipped app knowledge. The built app must not depend on a loose `system-profiles/builtin` folder existing beside the installed app.

Custom profiles are runtime user data. They are loaded from the same OS app data directory that contains `job_radar.db`. Custom profiles are loaded after built-ins and may not use a built-in key.

## JSON shape

```json
{
  "key": "muz_global_jobboard",
  "name": "Milch & Zucker Global Jobboard",
  "description": "Human-readable description.",
  "adapterKey": "declarative_endpoint_inventory",
  "definitionSchemaVersion": 1,
  "status": "active",
  "definition": {
    "detect": {
      "required": [
        { "htmlContains": "jobboard-widget" }
      ]
    },
    "sourceConfig": {
      "startUrl": "{{inputUrl}}"
    }
  },
  "sourceConfigSchema": {
    "type": "object",
    "required": ["startUrl"],
    "properties": {
      "startUrl": { "type": "string", "format": "uri" }
    }
  }
}
```

Rules:

- `key` and `adapterKey` use lowercase snake case with `a-z`, `0-9`, and `_`.
- `adapterKey` must reference a registered adapter that requires system profiles.
- Active profiles need at least one deterministic detection check.
- Domain-only mapping, company-specific adapters, confidence scores, or guesses are not valid evidence.

## Supported detection checks

Detection evaluates all entries in `definition.detect.required`. The profile matches only when every check passes.

```json
{ "htmlContains": "needle" }
```

Case-insensitive text check against the submitted page HTML.

```json
{ "htmlRegex": "pattern", "captureAs": "optionalCaptureName" }
```

Regex check against the submitted page HTML. If `captureAs` is present, the first capture group becomes available to templates as `{{capture:name}}`.

```json
{ "fetchText": { "url": "/path.txt", "contains": "needle" } }
```

Fetches a URL relative to the submitted URL and checks text content.

```json
{ "fetchText": { "url": "/script.js", "regex": "apiBase = \\\"([^\\\"]+)\\\"", "captureAs": "apiBaseUrl" } }
```

Fetches text and evaluates a regex.

```json
{ "fetchJson": { "url": "/config.json", "pathExists": "$.config.jobs" } }
```

Fetches JSON and checks that a simple `$.a.b.c` path exists.

```json
{ "fetchScript": { "srcRegex": "/webresources/js/.*script\\.js", "contains": "/.search?index=job" } }
```

Finds matching `<script src="...">` entries in the submitted HTML, fetches them, and checks their contents. `srcContains`, `srcRegex`, `contains`, `regex`, and `captureAs` are supported.

## Source config and identity templates

`definition.sourceConfig` may use:

- `{{inputUrl}}` — the submitted URL
- `{{origin}}` — scheme, host, and optional port of the submitted URL
- `{{capture:name}}` — a value captured by required detection checks or identity extraction checks

Templates also support filters:

- `technicalKey` — converts text to lowercase snake case suitable for source keys
- `titleCase` — converts hyphen/underscore separated text to a display title
- `domainKey` — extracts the company-like domain label from an HTTP(S) URL and converts it to a technical key
- `domainTitle` — extracts the company-like domain label from an HTTP(S) URL and converts it to a display title

Examples:

```txt
{{capture:boardSlug|technicalKey}}_careers
{{capture:companyWebsite|domainTitle}} Karriere
{{inputUrl|domainKey}}_careers
```

## Identity enrichment

A profile may include `definition.identity` to improve the auto-filled source key, source name, and optional source configuration metadata without hardcoding system-specific rules in Rust.

Identity enrichment is optional. Required detection checks still decide whether a profile matches. If identity extraction fails, detection can still succeed and falls back to the submitted URL or other configured candidates.

```json
"identity": {
  "extract": [
    {
      "htmlRegex": "\"publicWebsite\"\\s*:\\s*\"(https?://[^\"\\\\]+)\"",
      "captureAs": "companyWebsite"
    }
  ],
  "keyCandidates": [
    "{{capture:companyWebsite|domainKey}}_careers",
    "{{capture:boardSlug|technicalKey}}_careers"
  ],
  "nameCandidates": [
    "{{capture:companyWebsite|domainTitle}} Karriere",
    "{{capture:boardSlug|titleCase}} Karriere"
  ],
  "optionalSourceConfig": {
    "companyWebsite": "{{capture:companyWebsite}}"
  }
}
```

Fields:

- `extract` — optional detection checks whose captures enrich identity data. These checks do not decide whether the profile matches.
- `keyCandidates` — ordered source-key templates. The first candidate with all required captures wins.
- `nameCandidates` — ordered source-name templates. The first candidate with all required captures wins.
- `optionalSourceConfig` — object merged into `sourceConfig` only for fields whose templates can be rendered.

For example, Ashby hosted boards expose `organization.publicWebsite` in the page HTML. `https://jobs.ashbyhq.com/focused` can therefore auto-fill `Focused Energy Karriere` and store `companyWebsite: https://focused-energy.co` while still using Ashby's public posting API as `startUrl`.

## Declarative inventory DSL

`definition.inventory` describes how a Systemprofil turns one saved Quelle into a `SourceCandidate` inventory during a Suchlauf. It is a generic pipeline, not a system-specific Rust adapter:

```txt
fetch -> parse -> items.select -> items.where/captures -> fields -> SourceCandidate
```

The inventory block is optional while profiles are introduced gradually. Declarative HTTP/Sitemap Quellen without `definition.inventory` can still be detected and saved, but a Suchlauf for that source fails clearly until the inventory definition is added.

Supported MVP shape:

- `fetch.url` — template for an absolute HTTP(S) URL after rendering. Inventory templates support `{{sourceConfig:<key>}}`, `{{sourceName}}`, and `{{sourceKey}}` in `fetch.url`.
- `parse.as` — `"xml"` or `"json"`.
- `items.select` — `{"xmlText":"loc"}` for XML text elements, or `{"jsonPath":"$.jobs"}` for JSON arrays. JSONPath support is intentionally simple dot paths only, such as `$`, `$.jobs`, or `$.outer.jobs`.
- `items.where[]` — optional regex filters for selected text items.
- `items.captures[]` — optional regex captures for selected text items. Named captures are available in field templates as `{{capture:name}}`.
- `fields.title`, `fields.url`, `fields.company` — required field expressions.
- `fields.locations[]` — array of location field expressions; use an empty array when no location is available.
- Field expressions are JSON objects with exactly one supported form: `{ "template": "..." }` or `{ "jsonPath": "..." }`.

XML sitemap example:

```json
"inventory": {
  "fetch": { "url": "{{sourceConfig:url}}" },
  "parse": { "as": "xml" },
  "items": {
    "select": { "xmlText": "loc" },
    "where": [{ "regex": "(?i)/job/" }],
    "captures": [{
      "regex": "(?i)/job/(?P<location>[^/-]+)-(?P<title>.+?)(?:-\\d+)?/?$"
    }]
  },
  "fields": {
    "title": { "template": "{{capture:title|urlDecode|slugToTitle}}" },
    "url": { "template": "{{itemText}}" },
    "company": { "template": "{{sourceName|stripCareerSuffix}}" },
    "locations": [
      { "template": "{{capture:location|urlDecode|slugToTitle}}" }
    ]
  }
}
```

JSON API example:

```json
"inventory": {
  "fetch": { "url": "{{sourceConfig:startUrl}}" },
  "parse": { "as": "json" },
  "items": {
    "select": { "jsonPath": "$.jobs" }
  },
  "fields": {
    "title": { "jsonPath": "$.title" },
    "url": { "jsonPath": "$.jobUrl" },
    "company": { "template": "{{sourceName}}" },
    "locations": [
      { "jsonPath": "$.location" }
    ]
  }
}
```

Authoring checklist for inventory:

1. Keep platform detection in `definition.detect.required`; do not use inventory to guess a system.
2. Add only one inventory pipeline per Systemprofil: fetch URL, parser, item selector, optional filters/captures, and field mappings.
3. Validate regexes and JSONPath expressions with Rust tests before shipping a built-in profile or importing a custom profile.
4. Keep system-specific extraction knowledge in the profile JSON. Do not add SuccessFactors-, Ashby-, Greenhouse-, Lever-, or employer-specific extraction rules to Rust validation or execution code.
5. Built-in Job-Portale stay outside this system. StepStone and Indeed are query-parameterized portal adapters (`stepstone_search`, `indeed_search`), not Systemprofil inventory definitions.

## Agent checklist: extend a profile with better identity

Use this checklist when improving an existing built-in profile such as Greenhouse, Lever, Personio, Workday, Phenom, or SuccessFactors.

1. Start from a real company URL and inspect the submitted HTML plus any stable fetched scripts/config endpoints already used by detection.
2. Keep `definition.detect.required` for deterministic platform evidence only. Do not add company-specific checks or guesses.
3. Look for reusable identity fields exposed by the recruiting system, in this order:
   - canonical company/public website URL (`publicWebsite`, `companyUrl`, `organization.website`, canonical career page URL)
   - stable board slug or company token
   - stable organization/company display name
4. Put optional identity extraction in `definition.identity.extract`, not in `definition.detect.required`, unless the evidence is required to prove the platform itself.
5. Prefer `companyWebsite` when available:
   - derive source key with `{{capture:companyWebsite|domainKey}}_careers`
   - derive source name with `{{capture:companyWebsite|domainTitle}} Karriere`
   - store it via `identity.optionalSourceConfig.companyWebsite`
6. Add fallback candidates so detection still works when optional identity data is absent:
   - board slug fallback: `{{capture:boardSlug|technicalKey}}_careers`
   - submitted URL fallback is automatic if no candidate renders
7. If `optionalSourceConfig` adds fields such as `companyWebsite`, also add those fields to `sourceConfigSchema.properties`. Do not make optional identity fields required.
8. Add or update Rust tests in `src-tauri/src/source_detection.rs` with a fixture for the improved profile. Assert key, name, `startUrl`, and optional `companyWebsite` when available.
9. Run validation:

```bash
cd src-tauri && cargo fmt -- --check && cargo test
npm run build
```

Rules for agents:

- Do not hardcode recruiting-system identity logic in Rust. Rust owns the generic `identity` evaluator; profile JSON owns system-specific extraction rules.
- Do not create company-specific adapters or source profiles for one employer.
- Do not turn optional enrichment into required detection unless missing enrichment would make the platform evidence invalid.
- Existing built-in profile changes are upserted into the DB on app restart after rebuild. Existing saved sources are not automatically rewritten.

## Authoring workflow

1. For a bundled profile, add a JSON file under `system-profiles/builtin/` and add it to the embedded file list in `src-tauri/src/db.rs`.
2. For a local custom profile, place a JSON file under the OS app data directory: `system-profiles/*.json`.
3. Use deterministic evidence checks. Prefer multiple independent checks for broad platform profiles.
4. Run validation:

```bash
cd src-tauri && cargo fmt -- --check && cargo test
npm run build
```

If a schema/migration experiment breaks a local dev DB, explicitly reset it with:

```bash
npm run tauri:dev:reset-db
```
