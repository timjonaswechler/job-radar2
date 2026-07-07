# Standalone Source Profile implementation prompt for production agents

Use this file as the **only required handoff** for an agent that runs in a production Job Radar installation and needs to add support for one recruiting system, job board, career-site family, or concrete public career source that is not already supported.

The agent may not have access to the development repository, PRDs, ADRs, Rust tests, or built-in profile files. This prompt is therefore self-contained. If the production environment provides extra Job Radar commands or registry views, use them as validation helpers, but do not require developer-only files.

## Mission

Create at most **one custom Source Profile JSON document** in the Job Radar app data directory so that Job Radar can detect and execute one new source family through the declarative Profile DSL.

A custom Source Profile is configuration, not code. It lives at:

```text
<app-data-dir>/source-profiles/<profile-key>.json
```

Optionally, if the user also asks for a concrete Source, create one Source JSON document at:

```text
<app-data-dir>/sources/<source-key>.json
```

Do not edit bundled application files. Do not create Rust, TypeScript, or plugin code. Do not bypass the Profile DSL by scripting arbitrary scraping behavior.

## Required inputs

Before writing any file, you need these inputs:

```text
Target: <recruiting system / profile family / career URL>
App data directory or Source Profiles directory: <path>
Implementation requested now: yes/no
Candidate URLs: <career page, API/feed/sitemap URL, vendor docs URL if known>
Desired Source Profile key: <optional>
Desired Source key: <optional, only if creating a Source too>
```

If the app data directory is missing, ask the user to provide it or use the app's environment/diagnostic surface if available. In Job Radar, the app data directory contains subdirectories named `source-profiles/` and `sources/`. Do not guess OS-specific paths.

If the production environment exposes a command like `get_database_info`, use it to obtain:

- `appDataDir`
- `sourceProfilesDir`
- `sourcesDir`

If `Implementation requested now` is `no`, do not write files. Return the evidence found, the recommended profile key, the safest Access Path, and the missing inputs needed for a later implementation pass.

If a registry command or UI list is available, check existing Source Profile keys first. A custom profile key must not collide with a built-in or existing custom profile key.

## Canonical vocabulary

Use these terms exactly:

- **Source Profile**: reusable declarative knowledge for a recruiting system, job board, career-site family, or website family.
- **Source**: one saved concrete endpoint/entry point that selects one Source Profile Access Path or owns one inline Access Path.
- **Access Path**: one executable variant inside a Source Profile. It owns `postingDiscovery` and optionally `postingDetail`.
- **Source Config**: stable access configuration such as `boardSlug`, `host`, `tenant`, `site`, `baseUrl`, `sitemapUrl`, or `startUrl`. It must not contain search criteria.
- **postingDiscovery**: bulk discovery of posting candidates. It returns normalized candidates with at least `title`, `company`, and `url`.
- **postingDetail**: lazy loading for one posting's `descriptionText`, using the posting URL, Source Config, and optional `postingMeta`.
- **postingMeta**: hidden technical metadata captured during discovery for later detail loading, such as `jobId`, `externalPath`, or `requisitionId`.
- **Fixture Pack**: deterministic evidence bundle for one Source Profile. It lives outside the profile document and proves behavior against captured fixture inputs.
- **Fixture Manifest**: machine-readable file inside a Fixture Pack that names the profile/access path, fixture requests, response files, and expected outputs.
- **Profile Verification Check**: deterministic, offline, fixture-based check for one Source Profile. It produces a **Verification Report** and never performs live network requests.
- **Verification Report**: the Check Report produced by a Profile Verification Check. It is an overwriteable derived report, not the Source Profile document.
- **Effective Verification State**: derived state (`verified`, `failed`, `not_applicable`, or `unknown`) computed from declared support, report freshness, fixture checks, and diagnostics. It is not the same as `support.level`.
- **Source Live Check**: bounded live check for one concrete Source and its current Source Config/selected Access Path. It is separate from Profile Verification.
- **Source Live Check Report**: the Check Report produced by a Source Live Check. It does not verify a Source Profile.
- **Check Report**: derived JSON report with a shared envelope for Source Profile Verification and Source Live Checks.
- **Structured Diagnostics**: machine-readable validation/runtime issues surfaced by Job Radar.

Avoid legacy terms such as adapter, Systemprofil, Browserprofil, inventory, scraper plugin, or profile-specific runtime.

## Non-goals and hard rules

- Do not add search keywords, roles, locations, radius, countries, include rules, or exclude rules to Source Config. Search criteria belong to Search Requests.
- Do not log in, bypass auth, solve captchas, bypass WAF/bot protections, scrape private data, or use credentials/cookies.
- Do not add secrets, auth headers, cookies, bearer tokens, API keys, or private headers.
- Do not run high-volume live fetches. Use small samples only.
- Do not create multiple profiles in one pass.
- Do not implement a whole catalog.
- Do not use arbitrary JavaScript. Browser fetch may use bounded waits/interactions only if the DSL and the app support them.
- Do not claim a profile is `verified` unless `support.evidence` includes deterministic fixture evidence. Use `best_effort` for live-only validation.
- Do not use `support.evidence.kind = "url"`; `url` is valid only for `detect.evidence.kind`.
- Do not treat a live check or smoke result as fixture evidence. Use `support.evidence.kind = "smoke"` for live-only evidence.

## Evidence and classification workflow

### 1. Identify the recruiting platform and profile scope

Before choosing the profile shape, make a bounded platform-identification pass. Inspect the submitted URLs, public page HTML, linked scripts, JSON-LD, public API paths, sitemap URLs, robots.txt, response headers, and vendor docs when available.

Look for stable platform markers such as:

- known hosts or paths, for example Greenhouse, Workday, SAP SuccessFactors, SmartRecruiters, Lever, Ashby, Personio, Phenom, or another recruiting platform;
- public API names, board slugs, tenant IDs, requisition IDs, `JobPosting` JSON-LD, sitemap conventions, script bundle names, or meta tags;
- vendor documentation or demo boards that prove the same pattern applies beyond one company.

Classify the target as one of:

- `existing_profile_candidate`: Job Radar may already support this platform;
- `reusable_platform_candidate`: evidence supports a reusable Source Profile for a platform or website family;
- `company_specific_career_site`: evidence is stable for this concrete company source, but not enough to generalize;
- `unknown`: no stable platform marker was found.

Do not infer a generic platform profile from one customer page alone. Separate the **discovery surface** from the **apply backend**: an apply link to a known ATS, such as SAP SuccessFactors, proves the application handoff platform, but not necessarily the safest postingDiscovery Access Path. If a known ATS marker appears, make one bounded attempt to find an unauthenticated discovery/feed/API endpoint for that ATS. If the only stable public discovery surface is the company site, create a company-specific `career_site` or `generic` profile and record why the known ATS was not used as the Access Path.

If evidence only proves one company source, create a company-specific `career_site` or `generic` profile instead of a broad platform profile. Record the platform identification, its confidence, and the reason for the chosen scope in `support.summary`, `support.evidence`, and the final report even when the result is `unknown`.

### 2. Determine whether a custom Source Profile is needed

Before creating a new profile, check whether Job Radar already supports the identified profile family if you can inspect the registry. Common built-ins may include systems like Greenhouse, Workday, or SAP SuccessFactors. If an existing profile fits, do not create a duplicate; create or propose a Source selecting the existing profile instead.

If no matching profile exists, continue.

### 3. Identify the safest Access Path

Prefer access paths in this order:

1. Official unauthenticated JSON API.
2. Official XML/RSS feed or sitemap.
3. Stable public HTML with server-rendered posting data.
4. Browser-backed page only when HTTP/API/feed is unavailable and behavior can be bounded.
5. Manual investigation only when none of the above is stable.

Use browser-backed discovery only when necessary and be explicit that it may require the managed Browser Runtime.

### 4. Collect minimal public evidence

For each candidate URL, record:

- URL
- HTTP status if checked
- content type/shape
- stable markers that identify the system
- fields available for normalization
- risks such as JS-only rendering, empty board, volatile response, localization, bot blocking, pagination, or missing detail fields

Use vendor docs or vendor demo/sample boards when available. Real public company pages are acceptable if they are the only evidence, but record them explicitly.

### 5. Decide support level

Use one of:

- `verified`: deterministic fixture evidence proves discovery/detail shape. Job Radar compiler diagnostics require at least one `support.evidence` entry with `kind: "fixture"`.
- `best_effort`: evidence is real and profile likely works, but coverage is incomplete, site variants are expected, or validation is live-only.
- `experimental`: shape is plausible but evidence is limited.
- `unsupported`: detection knowledge only; no executable Access Path should be relied on.

For a production custom profile created from live evidence only, use `best_effort`.

### 6. Choose support evidence kinds correctly

`support.evidence` documents why the declared Support Level is plausible. The only supported values are:

- `fixture`: a deterministic Fixture Manifest reference, usually `fixture.json`, under the profile's Fixture Pack. Use this when captured offline fixtures can prove discovery/detail behavior.
- `smoke`: a bounded live/manual smoke result. Use this for live Source checks or one-off live validation; it is not deterministic fixture evidence.
- `manual_review`: human/agent review of public docs, HTML, API responses, selectors, or platform markers.
- `schema_check`: evidence that the profile shape was validated against schema/compiler checks, without runtime fixture proof.

Examples:

```json
"support": {
  "level": "best_effort",
  "summary": "Manual review and a live smoke suggest the API path works, but no Fixture Manifest has been captured.",
  "evidence": [
    {
      "kind": "manual_review",
      "reference": "https://jobs.example.com/api/jobs",
      "summary": "Public endpoint returned stable job fields."
    },
    {
      "kind": "smoke",
      "reference": "2026-07-08 live sample",
      "summary": "One bounded live check returned at least one candidate."
    },
    {
      "kind": "schema_check",
      "reference": "Job Radar registry diagnostics",
      "summary": "Profile JSON loaded without schema diagnostics."
    }
  ]
}
```

For fixture-backed verified support, reference a Fixture Manifest instead of raw response files:

```json
"support": {
  "level": "verified",
  "summary": "Fixture replay proves discovery and detail extraction for the API Access Path.",
  "evidence": [
    {
      "kind": "fixture",
      "reference": "fixture.json",
      "summary": "Offline Fixture Manifest covers postingDiscovery and postingDetail.descriptionText."
    }
  ]
}
```

Never set `kind` to `url` inside `support.evidence`. `url` remains valid only as detection evidence, for example:

```json
"detect": {
  "evidence": [
    {
      "kind": "url",
      "message": "Career URLs expose the host needed for Source Config."
    }
  ]
}
```

## Fixture Packs, Fixture Manifests, and Profile Verification

A Profile Verification Check is deterministic and offline. It replays captured fixture responses through the Profile DSL runtime and writes a derived Verification Report. It must not fetch the live internet while verifying fixtures.

### Fixture Pack directory convention

For a custom Source Profile with key `<profile-key>`, put fixture evidence under:

```text
<app-data-dir>/source-profile-fixtures/<profile-key>/
```

This directory is the **Fixture Pack**. The Source Profile JSON remains in:

```text
<app-data-dir>/source-profiles/<profile-key>.json
```

`support.evidence.kind = "fixture"` references a Fixture Manifest inside the Fixture Pack, not arbitrary raw fixture files. The default manifest name is:

```text
fixture.json
```

A non-default manifest may be referenced by setting `support.evidence[].reference`, but the reference must still be Fixture-Pack-root-relative.

### Fixture file path rules

Fixture Manifest references and response `bodyFile` references are resolved relative to the Fixture Pack root. Subdirectories such as `responses/jobs.json` are allowed.

Invalid references include:

- absolute paths;
- Windows absolute or UNC paths;
- `..` path traversal;
- `~` home-directory shortcuts;
- empty references;
- any normalized path that escapes the Fixture Pack root.

### Fixture Manifest v1 practical shape

A practical Fixture Manifest v1 contains:

- `schemaVersion: 1`;
- `profileKey`: must match the checked Source Profile key;
- `accessPathKey`: explicit Access Path key to verify;
- `sourceConfig`: config used to compile the selected Access Path during verification;
- `requests[]`: offline request mappings by normalized HTTP method and absolute HTTP(S) URL;
- `checks.postingDiscovery.expect`: discovery invariants;
- optional `checks.postingDetail.cases[]`: concrete detail cases and description expectations.

Example:

```json
{
  "schemaVersion": 1,
  "profileKey": "example_profile",
  "accessPathKey": "api",
  "sourceConfig": {
    "apiBaseUrl": "https://jobs.example.com/api"
  },
  "requests": [
    {
      "key": "discovery_jobs",
      "match": {
        "method": "GET",
        "url": "https://jobs.example.com/api/jobs"
      },
      "response": {
        "status": 200,
        "headers": {
          "content-type": "application/json"
        },
        "bodyFile": "responses/jobs.json"
      }
    },
    {
      "key": "detail_job_123",
      "match": {
        "method": "GET",
        "url": "https://jobs.example.com/api/jobs/123"
      },
      "response": {
        "status": 200,
        "headers": {
          "content-type": "application/json"
        },
        "bodyFile": "responses/job-123.json"
      }
    }
  ],
  "checks": {
    "postingDiscovery": {
      "expect": {
        "minCandidates": 1,
        "requiredFields": ["title", "company", "url"],
        "containsCandidates": [
          {
            "title": "Software Engineer",
            "company": "Example",
            "url": "https://jobs.example.com/jobs/123"
          }
        ]
      }
    },
    "postingDetail": {
      "cases": [
        {
          "key": "job_123_detail",
          "posting": {
            "title": "Software Engineer",
            "company": "Example",
            "url": "https://jobs.example.com/jobs/123",
            "postingMeta": {
              "jobId": "123"
            }
          },
          "expect": {
            "minDescriptionLength": 40,
            "descriptionContains": ["responsibilities"]
          }
        }
      ]
    }
  }
}
```

Rules for agents:

- Capture every request the runtime needs in `requests[]`; unmapped runtime fetches fail fixture verification.
- Use method + absolute URL matching. Query parameter order is insignificant; free-form URL regex matching is not part of verified fixture matching.
- Store fixture bodies under the Fixture Pack, for example `responses/jobs.json`.
- Prefer fixture coverage that proves both `postingDiscovery` and `postingDetail.descriptionText` for full verified support.
- Use `support.evidence.kind = "fixture"` only when the Fixture Manifest and referenced files exist.

### Declared Support Level vs Effective Verification State

`support.level` is authored metadata in the Source Profile. A Profile Verification Check must not mutate it.

Effective Verification State is derived when Job Radar reads or runs verification:

- `verified`: declared `support.level = "verified"`, fresh passed Verification Report, fixture evidence exists, and sufficient fixture coverage passed.
- `failed`: declared `verified`, but verification failed or fixture evidence/coverage is missing or broken.
- `not_applicable`: declared support is not `verified`; passing fixtures may be shown, but they do not automatically raise Support Level.
- `unknown`: no report is available, the report is stale, or the report cannot be used.

A button click or successful live Source check never grants verified support by itself.

### Check Report expectations for agents

Profile Verification and Source Live Check both write overwriteable derived **Check Reports**. A report is not an audit log and not an authored Source Profile or Source document.

Latest derived report conventions:

```text
<app-data-dir>/source-profile-verifications/<profile-key>.json
<app-data-dir>/source-live-checks/<source-key>.json
```

High-level Check Report envelope:

```json
{
  "schemaVersion": 1,
  "kind": "source_profile_verification",
  "subject": {
    "type": "source_profile",
    "key": "example_profile"
  },
  "checkedAt": "2026-07-08T12:00:00Z",
  "logicVersion": "profile-verification/v1",
  "result": "passed",
  "fingerprints": [],
  "diagnostics": [],
  "details": {}
}
```

Agent expectations:

- `kind` is `source_profile_verification` for Verification Reports and `source_live_check` for Source Live Check Reports.
- `subject.type` is `source_profile` for profile verification and `source` for source live checks.
- `result` is only `passed` or `failed`; stale/unknown states are derived when reading a report.
- `fingerprints` let Job Radar detect stale reports after profile, fixture, Source, Source Config, Source Overrides, or check-logic changes.
- Error Structured Diagnostics make a check fail. Warnings and info may still allow `result = "passed"`.
- Do not edit Check Reports manually to change profile support or Source status.

## Profile document shape

Write one JSON file:

```text
<app-data-dir>/source-profiles/<profile-key>.json
```

The top-level shape below is a complete schema-valid example. Replace the URL patterns, Source Config keys, selectors, and evidence with facts from the target source before writing a real profile.

<!-- schema-test:source-profile -->
```json
{
  "schemaVersion": 2,
  "key": "example_profile",
  "name": "Example Profile",
  "kind": "recruiting_system",
  "description": "Example profile for a public JSON jobs API.",
  "support": {
    "level": "best_effort",
    "summary": "Manual review found a public JSON jobs endpoint. No deterministic fixture has been captured, so this is not verified.",
    "evidence": [
      {
        "kind": "manual_review",
        "reference": "https://jobs.example.com/api/jobs",
        "summary": "Public endpoint returned a jobs array with title, URL, and location fields."
      }
    ]
  },
  "detect": {
    "recommendedAccessPathKey": "api",
    "sourceConfig": {
      "apiBaseUrl": "https://{{capture:host}}/api"
    },
    "keyCandidates": ["{{capture:host}}"],
    "nameCandidates": ["{{capture:host}}"],
    "inputUrlPatterns": [
      {
        "pattern": "(?i)^https?://(?<host>[A-Za-z0-9.-]+)/careers(?:[/?#].*)?$",
        "captures": ["host"]
      }
    ],
    "evidence": [
      {
        "kind": "url",
        "message": "Career URLs expose the host needed for Source Config."
      }
    ]
  },
  "sourceConfigSchema": {
    "type": "object",
    "required": ["apiBaseUrl"],
    "additionalProperties": false,
    "properties": {
      "apiBaseUrl": {
        "type": "string",
        "format": "uri",
        "title": "Jobs API base URL"
      }
    }
  },
  "accessPaths": [
    {
      "key": "api",
      "name": "Public jobs API",
      "description": "Discover jobs from a public JSON endpoint.",
      "postingDiscovery": {
        "strategies": [
          {
            "key": "jobs_api",
            "fetch": {
              "mode": "http",
              "method": "GET",
              "url": "{{sourceConfig:apiBaseUrl}}/jobs",
              "headers": { "accept": "application/json" },
              "timeoutMs": 10000
            },
            "parse": { "type": "json" },
            "select": { "type": "json_path", "jsonPath": "$.jobs" },
            "extract": {
              "fields": {
                "title": { "type": "json_path", "jsonPath": "$.title", "cardinality": "one", "transforms": [{ "type": "trim" }] },
                "company": { "type": "template", "template": "{{source:name}}", "cardinality": "one" },
                "url": { "type": "json_path", "jsonPath": "$.url", "cardinality": "one" },
                "locations": { "type": "json_path", "jsonPath": "$.location", "cardinality": "optional", "transforms": [{ "type": "trim" }] }
              }
            },
            "acceptWhen": { "requiredFields": ["title", "company", "url"], "minResults": 1 }
          }
        ]
      }
    }
  ]
}
```

Allowed `kind` values:

- `recruiting_system`
- `job_portal`
- `website_family`
- `career_site`
- `generic`

Technical keys should be lowercase snake-case-like strings using letters, numbers, and underscores, for example `smartrecruiters`, `company_jobs`, `api`, `sitemap_html`.

## Detection rules

Use `detect` when a submitted URL can be recognized and converted into Source Config.

A good detection rule:

- anchors the URL to a specific domain or stable path;
- captures only stable config values;
- avoids matching arbitrary generic career pages unless the profile truly handles them;
- sets `recommendedAccessPathKey` to an existing Access Path key;
- creates a usable `sourceConfig` proposal from captures.

Template variables available in detection strings:

- `{{inputUrl}}`
- `{{capture:name}}`

Example:

```json
"detect": {
  "recommendedAccessPathKey": "api",
  "sourceConfig": {
    "boardSlug": "{{capture:boardSlug}}"
  },
  "keyCandidates": ["{{capture:boardSlug}}"],
  "nameCandidates": ["{{capture:boardSlug}}"],
  "inputUrlPatterns": [
    {
      "pattern": "(?i)^https?://jobs\\.example\\.com/(?<boardSlug>[a-z0-9][a-z0-9_-]*)(?:[/?#].*)?$",
      "captures": ["boardSlug"]
    }
  ]
}
```

Use `httpChecks` only when URL regex alone is too broad and a small public fetch can prove the system:

```json
"httpChecks": [
  {
    "key": "feed_exists",
    "url": "https://{{capture:host}}/jobs.json",
    "timeoutMs": 10000,
    "expectStatus": 200,
    "regex": "(?is)\\\"jobs\\\"\\s*:"
  }
]
```

## Source Config schema rules

The Source Config schema must describe only stable access values. Use `additionalProperties: false` unless you have a strong reason not to.

Good fields:

- `boardSlug`
- `host`
- `tenant`
- `site`
- `baseUrl`
- `apiBaseUrl`
- `sitemapUrl`
- `language`
- `startUrl`

Bad fields:

- `keyword`
- `role`
- `preferredLocation`
- `country`
- `radius`
- `remoteOnly`
- `includeTerms`
- `excludeTerms`
- credentials, cookies, tokens, API keys

## Strategy basics

Each `postingDiscovery` or `postingDetail` has one or more named strategies. Strategies are tried in order as fallbacks.

A strategy usually contains:

```json
{
  "key": "strategy_key",
  "fetch": {},
  "parse": {},
  "select": {},
  "extract": { "fields": {} },
  "acceptWhen": {}
}
```

### Fetch

HTTP GET:

```json
"fetch": {
  "mode": "http",
  "method": "GET",
  "url": "{{sourceConfig:baseUrl}}/jobs.json",
  "headers": { "accept": "application/json" },
  "timeoutMs": 10000
}
```

HTTP POST JSON:

```json
"fetch": {
  "mode": "http",
  "method": "POST",
  "url": "https://{{sourceConfig:host}}/api/jobs",
  "headers": {
    "accept": "application/json",
    "content-type": "application/json"
  },
  "body": {
    "type": "json",
    "value": { "limit": 50, "offset": 0 }
  },
  "timeoutMs": 10000
}
```

Browser fetch, only when necessary and bounded:

```json
"fetch": {
  "mode": "browser",
  "url": "{{sourceConfig:startUrl}}",
  "timeoutMs": 30000,
  "waits": [
    { "type": "selector", "selector": ".job-card", "timeoutMs": 15000 }
  ]
}
```

Allowed public headers include `accept`, `accept-language`, `content-type`, `user-agent`, `x-requested-with`, and `referer`. Do not use auth/cookie/secret headers.

### Parse

```json
"parse": { "type": "json" }
```

```json
"parse": { "type": "xml" }
```

```json
"parse": { "type": "html" }
```

```json
"parse": { "type": "text" }
```

### Select

Use the whole document:

```json
"select": { "type": "document" }
```

Select a JSON array:

```json
"select": { "type": "json_path", "jsonPath": "$.jobs" }
```

Select HTML nodes:

```json
"select": { "type": "css", "selector": ".job-card" }
```

### Extract fields

Every `postingDiscovery.strategies[*].extract.fields` must produce at least:

- `title`
- `company`
- `url`

Prefer also:

- `locations`
- `postingMeta` with stable IDs needed for `postingDetail`

Common expressions:

JSON path:

```json
"title": {
  "type": "json_path",
  "jsonPath": "$.title",
  "cardinality": "one",
  "transforms": [{ "type": "trim" }]
}
```

Company from Source name:

```json
"company": {
  "type": "template",
  "template": "{{source:name}}",
  "cardinality": "one"
}
```

CSS text:

```json
"title": {
  "type": "css_text",
  "selector": ".job-title",
  "cardinality": "first",
  "transforms": [{ "type": "normalize_whitespace" }]
}
```

CSS attribute:

```json
"url": {
  "type": "css_attribute",
  "selector": "a.job-link",
  "attribute": "href",
  "cardinality": "first"
}
```

Source Config value:

```json
"base": {
  "type": "source_config",
  "key": "baseUrl",
  "cardinality": "one"
}
```

Combine fields into an absolute URL:

```json
"url": {
  "type": "combine",
  "join": "",
  "cardinality": "one",
  "parts": [
    { "value": { "type": "source_config", "key": "baseUrl", "cardinality": "one" } },
    { "value": { "type": "json_path", "jsonPath": "$.path", "cardinality": "one" } }
  ]
}
```

Posting metadata:

```json
"postingMeta": {
  "jobId": {
    "type": "json_path",
    "jsonPath": "$.id",
    "cardinality": "one",
    "transforms": [{ "type": "to_string" }]
  }
}
```

Useful transforms:

- `trim`
- `normalize_whitespace`
- `html_to_text`
- `url_decode`
- `slug_to_title`
- `to_string`
- `split`
- `dedupe`

### Acceptance checks

Use `acceptWhen` so failed strategies produce useful diagnostics:

```json
"acceptWhen": {
  "requiredFields": ["title", "company", "url"],
  "minResults": 1
}
```

For detail:

```json
"acceptWhen": {
  "minDescriptionLength": 40
}
```

## Common profile patterns

### Pattern A: JSON API discovery + JSON detail

Use this when a public API returns an array of jobs and a detail endpoint returns description HTML/text.

```json
{
  "key": "api",
  "name": "Public JSON API",
  "postingDiscovery": {
    "strategies": [
      {
        "key": "jobs_api",
        "fetch": {
          "mode": "http",
          "method": "GET",
          "url": "{{sourceConfig:apiBaseUrl}}/jobs",
          "headers": { "accept": "application/json" },
          "timeoutMs": 10000
        },
        "parse": { "type": "json" },
        "select": { "type": "json_path", "jsonPath": "$.jobs" },
        "extract": {
          "fields": {
            "title": { "type": "json_path", "jsonPath": "$.title", "cardinality": "one", "transforms": [{ "type": "trim" }] },
            "company": { "type": "template", "template": "{{source:name}}", "cardinality": "one" },
            "url": { "type": "json_path", "jsonPath": "$.url", "cardinality": "one" },
            "locations": { "type": "json_path", "jsonPath": "$.location", "cardinality": "optional", "transforms": [{ "type": "trim" }] },
            "postingMeta": {
              "jobId": { "type": "json_path", "jsonPath": "$.id", "cardinality": "one", "transforms": [{ "type": "to_string" }] }
            }
          }
        },
        "acceptWhen": { "requiredFields": ["title", "company", "url", "postingMeta.jobId"], "minResults": 1 }
      }
    ]
  },
  "postingDetail": {
    "strategies": [
      {
        "key": "detail_api",
        "fetch": {
          "mode": "http",
          "method": "GET",
          "url": "{{sourceConfig:apiBaseUrl}}/jobs/{{postingMeta:jobId}}",
          "headers": { "accept": "application/json" },
          "timeoutMs": 10000
        },
        "parse": { "type": "json" },
        "select": { "type": "document" },
        "extract": {
          "fields": {
            "descriptionText": {
              "type": "json_path",
              "jsonPath": "$.descriptionHtml",
              "cardinality": "one",
              "transforms": [{ "type": "html_to_text" }, { "type": "normalize_whitespace" }]
            }
          }
        },
        "acceptWhen": { "minDescriptionLength": 40 }
      }
    ]
  }
}
```

### Pattern B: XML sitemap discovery + HTML detail

Use this when the only stable listing is a sitemap of job detail URLs.

If the URL is a sitemap index that links to child sitemaps, include `childSitemapSelector` and a positive `limits.maxDepth`; otherwise Job Radar reads only the index file and will not reach job URLs in child sitemaps. For example:

```json
"pagination": {
  "type": "sitemap",
  "childSitemapSelector": { "type": "sitemap_urls" },
  "postingUrlSelector": { "type": "sitemap_urls", "urlPattern": "(?i)/job/" },
  "limits": { "maxRequests": 5, "maxItems": 1000, "maxDepth": 1 }
}
```

For a leaf sitemap that directly contains job URLs, omit `childSitemapSelector` and `maxDepth`.

```json
{
  "key": "sitemap_html",
  "name": "Sitemap and HTML detail pages",
  "postingDiscovery": {
    "strategies": [
      {
        "key": "sitemap_job_urls",
        "fetch": {
          "mode": "http",
          "method": "GET",
          "url": "{{sourceConfig:sitemapUrl}}",
          "headers": { "accept": "application/xml,text/xml" },
          "timeoutMs": 10000
        },
        "pagination": {
          "type": "sitemap",
          "postingUrlSelector": { "type": "sitemap_urls", "urlPattern": "(?i)/job/" },
          "limits": { "maxRequests": 1, "maxItems": 200 }
        },
        "parse": { "type": "xml" },
        "select": { "type": "document" },
        "captures": {
          "jobId": {
            "from": { "type": "item_field", "key": "value", "cardinality": "one" },
            "pattern": "(?i)(?:-|/)(?<value>[0-9]+)/?(?:[?#]|$)"
          },
          "title": {
            "from": { "type": "item_field", "key": "value", "cardinality": "one" },
            "pattern": "(?i)/job/[^/]*?(?<value>[A-Za-z0-9%_-]+)(?:-[0-9]+|/[0-9]+/?)(?:[?#]|$)"
          }
        },
        "extract": {
          "fields": {
            "title": { "type": "capture", "key": "title", "cardinality": "one", "transforms": [{ "type": "url_decode" }, { "type": "slug_to_title" }] },
            "company": { "type": "template", "template": "{{source:name}}", "cardinality": "one" },
            "url": { "type": "item_field", "key": "value", "cardinality": "one" },
            "postingMeta": {
              "jobId": { "type": "capture", "key": "jobId", "cardinality": "one" }
            }
          }
        },
        "acceptWhen": { "requiredFields": ["title", "company", "url"], "minResults": 1 }
      }
    ]
  },
  "postingDetail": {
    "strategies": [
      {
        "key": "html_description",
        "fetch": {
          "mode": "http",
          "method": "GET",
          "url": "{{posting:url}}",
          "headers": { "accept": "text/html" },
          "timeoutMs": 10000
        },
        "parse": { "type": "html" },
        "select": { "type": "document" },
        "extract": {
          "fields": {
            "descriptionText": {
              "type": "css_text",
              "selector": ".job-description, [data-automation-id='jobPostingDescription'], main [class*='description']",
              "cardinality": "first",
              "transforms": [{ "type": "normalize_whitespace" }]
            }
          }
        },
        "acceptWhen": { "minDescriptionLength": 40 }
      }
    ]
  }
}
```

### Pattern C: Server-rendered HTML listing only

Use this for simple public pages where job cards are in the initial HTML. This pattern does not provide `postingDetail`; add a bounded HTML detail strategy like Pattern B only when stable detail selectors are known.

```json
{
  "key": "html",
  "name": "HTML listing",
  "postingDiscovery": {
    "strategies": [
      {
        "key": "html_job_cards",
        "fetch": {
          "mode": "http",
          "method": "GET",
          "url": "{{sourceConfig:startUrl}}",
          "headers": { "accept": "text/html" },
          "timeoutMs": 10000
        },
        "parse": { "type": "html" },
        "select": { "type": "css", "selector": ".job-card" },
        "extract": {
          "fields": {
            "title": { "type": "css_text", "selector": ".job-title", "cardinality": "first", "transforms": [{ "type": "normalize_whitespace" }] },
            "company": { "type": "template", "template": "{{source:name}}", "cardinality": "one" },
            "url": { "type": "css_attribute", "selector": "a", "attribute": "href", "cardinality": "first" },
            "locations": { "type": "css_text", "selector": ".job-location", "cardinality": "optional", "transforms": [{ "type": "normalize_whitespace" }] }
          }
        },
        "acceptWhen": { "requiredFields": ["title", "company", "url"], "minResults": 1 }
      }
    ]
  }
}
```

## Optional Source document shape

Only create a Source if the user asks for a concrete Source too. Write it to:

```text
<app-data-dir>/sources/<source-key>.json
```

Example:

<!-- schema-test:source -->
```json
{
  "schemaVersion": 2,
  "key": "acme_jobs",
  "name": "Acme Jobs",
  "status": "draft",
  "sourceConfig": {
    "apiBaseUrl": "https://jobs.example.com/api"
  },
  "selectedAccessPath": {
    "type": "profile_access_path",
    "profileKey": "example_profile",
    "pathKey": "api"
  }
}
```

Allowed Source statuses are `draft`, `active`, and `disabled`. Use `draft` if the profile is uncertain or live validation was not possible.

## Source Live Check is separate from Profile Verification

A **Source Live Check** runs against one concrete Source and the real public source endpoint. It is useful after creating a Source, but it is not fixture evidence and does not verify the Source Profile.

Use this separation:

- Run or inspect a Profile Verification Check to evaluate deterministic Fixture Manifest evidence for a Source Profile.
- Run or inspect a Source Live Check to see whether one concrete Source currently returns candidates and, when available, one detail page.
- Record live-only evidence as `support.evidence.kind = "smoke"`, not `fixture`.
- Do not call a Source `verified`. Use live-check language such as `passed`, `failed`, `unknown`, or `stale`.
- Normal Source `Prüfen` is status-neutral. Explicit `Prüfen & Aktivieren` or `Prüfen & Reaktivieren` may change Source Status only after a complete passed Source Live Check.

## Validation after writing

After writing the JSON file:

1. Ensure it is valid JSON.
2. Ensure the top-level `key` matches the filename stem.
3. Ensure `detect.recommendedAccessPathKey` matches an existing Access Path key.
4. Ensure every `{{sourceConfig:name}}` variable is declared in `sourceConfigSchema.properties` and is supplied by a Source if creating one.
5. Ensure every `postingDiscovery.strategies[*].extract.fields` includes `title`, `company`, and `url`.
6. Ensure `postingDetail` uses only `descriptionText` unless the app explicitly supports more detail fields.
7. Reload or refresh Job Radar's registry view if available.
8. Check Structured Diagnostics. If any schema/compiler/source-validation diagnostic appears, fix the JSON before reporting success.
9. If fixture evidence was created and a Profile Verification Check is available, run it for the Source Profile and inspect the Verification Report result, Effective Verification State, Fixture Check Results, freshness, and diagnostics.
10. If live validation is available for a concrete Source, run one small Source Live Check against a public sample and record only broad invariants, not exact live counts.

If the production app does not expose validation or check commands, report that JSON was written but app validation must be checked in Job Radar's Source Profile registry/diagnostics UI.

## Reporting format

Return this report after the work:

```markdown
# Source Profile implementation: <profile-key>

## Result

- Action: <created profile | updated draft profile | no file written | needs manual investigation>
- Profile path: `<path>`
- Source path: `<path or not created>`
- Support level: `<verified | best_effort | experimental | unsupported>`

## Evidence used

| Evidence | URL/path | Observation | Risk |
|---|---|---|---|
| ... | ... | ... | ... |

## What the profile supports

- Platform identification: <identified platform/profile family/company-specific/unknown and confidence>
- Scope decision: <why this is existing-profile/generic-platform/company-specific and why any ATS apply backend was or was not used>
- Detection: <what URL patterns/captures are used>
- Access Path: <api/feed/sitemap/html/browser>
- postingDiscovery: <fields extracted>
- postingDetail: <supported/not supported and why>

## Validation

- JSON valid: <yes/no>
- Registry diagnostics checked: <yes/no/not available>
- Profile Verification Check: <not run / passed / failed / stale / unknown>
- Effective Verification State: <verified / failed / not_applicable / unknown / not checked>
- Source Live Check: <not run / passed / failed / stale / unknown>
- Remaining diagnostics: <none/list>

## Residual risks

- <risk or none>

## Next recommended action

<create Source / run Search Run smoke / improve selectors / gather official docs / defer>
```

## Stop conditions

Stop and ask the user instead of writing a profile if:

- you cannot determine the app data directory or Source Profiles directory;
- the source requires login, cookies, captcha, auth headers, or private API keys;
- the candidate appears private or sensitive;
- detection would be dangerously broad;
- the needed behavior requires a generic DSL capability not listed here;
- you cannot validate JSON syntax;
- a profile key collision exists and the user has not chosen a new key;
- the user asked for multiple systems at once.
