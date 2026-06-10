# System profiles

A system profile is a declarative JSON definition of a recruiting system or career-system family. It is not an adapter. Adapters are technical runtimes such as `declarative_http_jobboard`, `declarative_sitemap_jobboard`, or `declarative_browser_jobboard`.

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
  "adapterKey": "declarative_http_jobboard",
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

## Source config templates

`definition.sourceConfig` may use:

- `{{inputUrl}}` — the submitted URL
- `{{origin}}` — scheme, host, and optional port of the submitted URL
- `{{capture:name}}` — a value captured by `htmlRegex`, `fetchText.regex`, or `fetchScript.regex`

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
