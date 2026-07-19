# !! outdated !! update after #166 is done

# Standalone Source Profile implementation prompt for production agents

Use this file as the self-contained handoff for an agent that runs in a production Job Radar installation and needs to add support for one recruiting system, job board, career-site family, or concrete public career source that is not already supported.

The simplified product rule is:

> **Source Profiles describe reusable access behavior. Concrete Sources are live checked.**

A production agent should create configuration, not code. In the normal path, adding support requires at most:

```text
<app-data-dir>/source-profiles/<profile-key>.json
<app-data-dir>/sources/<source-key>.json
```

Do not create captured response bundles, Rust code, TypeScript code, plugins, or arbitrary scraping scripts.

## Required inputs

Before writing any file, collect:

```text
Target: <recruiting system / profile family / career URL>
App data directory or Source Profiles directory: <path>
Implementation requested now: yes/no
Candidate URLs: <career page, API/feed/sitemap URL, vendor docs URL if known>
Desired Source Profile key: <optional>
Desired Source key: <optional, only if creating a Source too>
```

If the app data directory is missing, ask the user to provide it or use Job Radar's diagnostic surface if available. In Job Radar, the app data directory contains `source-profiles/` and `sources/` directories. Do not guess OS-specific paths.

If a registry command or UI list is available, check existing Source Profile keys first. A custom profile key must not collide with a built-in or existing custom profile key.

If `Implementation requested now` is `no`, do not write files. Return the evidence found, recommended profile key, safest Access Path, and missing inputs needed for a later implementation pass.

## Canonical vocabulary

Use these terms exactly:

- **Source Profile**: reusable declarative knowledge for a recruiting system, job board, career-site family, or website family.
- **Source**: one saved concrete endpoint/entry point that selects one Source Profile Access Path or owns one inline Access Path.
- **Access Path**: one executable variant inside a Source Profile. It owns `postingDiscovery` and optionally `postingDetail`.
- **Source Config**: stable access configuration such as `boardSlug`, `host`, `tenant`, `site`, `baseUrl`, `sitemapUrl`, or `startUrl`. It must not contain search criteria.
- **postingDiscovery**: bulk discovery of posting candidates. It returns normalized candidates with at least `title`, `company`, and `url`.
- **postingDetail**: lazy loading for one posting's `descriptionText`, using the posting URL, Source Config, and optional `postingMeta`.
- **postingMeta**: hidden technical metadata captured during discovery for later detail loading, such as `jobId`, `externalPath`, or `requisitionId`.
- **Source Live Check**: bounded live check for one concrete Source and its current Source Config/selected Access Path.
- **Source Live Check Report**: the latest derived Check Report for a concrete Source. It is the user-facing operational signal.
- **Structured Diagnostics**: machine-readable validation/runtime issues surfaced by Job Radar.

## Non-goals and hard rules

- Do not add search keywords, roles, locations, radius, countries, include rules, or exclude rules to Source Config. Search criteria belong to Search Requests.
- Do not log in, bypass auth, solve captchas, bypass WAF/bot protections, scrape private data, or use credentials/cookies.
- Do not add secrets, auth headers, cookies, bearer tokens, API keys, or private headers.
- Do not run high-volume live fetches. Use small samples only.
- Do not create multiple profiles in one pass unless the user explicitly asks.
- Do not implement a whole catalog.
- Do not use arbitrary JavaScript. Browser fetch may use bounded waits/interactions only if the DSL and app support them.
- Do not describe Source Profile support metadata as operational confidence. Production confidence comes from the concrete Source's latest Source Live Check.
- Do not create files outside `source-profiles/` and `sources/` as part of the production custom profile workflow.

## Evidence and classification workflow

### 1. Identify the recruiting platform and profile scope

Inspect the submitted URLs, public page HTML, linked scripts, JSON-LD, public API paths, sitemap URLs, robots.txt, response headers, and vendor docs when available.

Look for stable markers such as:

- known hosts or paths, for example Greenhouse, Workday, SAP SuccessFactors, SmartRecruiters, Lever, Ashby, Personio, Phenom, or another recruiting platform;
- public API names, board slugs, tenant IDs, requisition IDs, `JobPosting` JSON-LD, sitemap conventions, script bundle names, or meta tags;
- vendor documentation or demo boards that prove the same pattern applies beyond one company.

Classify the target as one of:

- `existing_profile_candidate`: Job Radar may already support this platform;
- `reusable_platform_candidate`: evidence supports a reusable Source Profile for a platform or website family;
- `company_specific_career_site`: evidence is stable for this concrete company source, but not enough to generalize;
- `unknown`: no stable platform marker was found.

Do not infer a broad platform profile from one customer page alone. If evidence only proves one company source, create a company-specific `career_site` or `generic` profile instead of a broad platform profile.

### 2. Determine whether a custom Source Profile is needed

Before creating a new profile, check whether Job Radar already supports the identified profile family if you can inspect the registry. If an existing profile fits, do not create a duplicate; create or propose a Source selecting the existing profile instead.

If no matching profile exists, continue.

### 3. Identify the safest Access Path

Prefer access paths in this order:

1. Official unauthenticated JSON API.
2. Official XML/RSS feed or sitemap.
3. Stable public HTML with server-rendered posting data.
4. Browser-backed page only when HTTP/API/feed is unavailable and behavior can be bounded.
5. Manual investigation only when none of the above is stable.

Use browser-backed discovery only when necessary and be explicit that it may require Job Radar's managed Browser Runtime.

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

Use the simplified support levels:

- `stable`: maintained support expected to be broadly robust for this profile family. Prefer this only for intentionally maintained built-in or well-proven reusable profiles.
- `best_effort`: evidence is real and the profile likely works, but live source variation is expected. This is the default for production custom profiles.
- `experimental`: shape is plausible but evidence is limited.
- `unsupported`: detection knowledge only; no executable Access Path should be relied on.

Profile support is not a production guarantee.

### 6. Choose support evidence kinds

Support evidence documents why the declared support level is plausible. Prefer:

- `manual_review`: human/agent review of public docs, HTML, API responses, selectors, or platform markers.
- `smoke`: a bounded live/manual smoke result.
- `schema_check`: evidence that the profile shape loaded or validated through available Job Radar diagnostics.

Example:

```json
"support": {
  "level": "best_effort",
  "summary": "Manual review and a live smoke suggest the API path works, but live availability depends on the concrete Source.",
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
    }
  ]
}
```

`detect.evidence.kind = "url"` remains valid detection evidence. Do not put `url` inside `support.evidence`.

## Source Live Check expectations

A Source Live Check runs against one concrete Source and the real public source endpoint. It should be the main confidence signal after creating or editing a Source.

A Source Live Check should:

- compile the concrete Source into an Execution Plan;
- run bounded live `postingDiscovery`;
- expect at least one candidate with `title`, `company`, and `url`;
- if `postingDetail` exists, check detail extraction for at most one candidate;
- avoid Search Request criteria and Match Rules;
- avoid running a full Search Run;
- persist the latest report;
- show `passed`, `failed`, `stale`, or `unknown` for the concrete Source.

A stale Source Live Check Report means the Source, selected profile, Source Config, Source Overrides, or live-check logic changed after the report was written. Stale does not automatically disable or mutate the Source.

## Implementation workflow

1. Confirm app data paths and existing profile/source keys.
2. Identify the platform/scope and safest Access Path.
3. Draft one Source Profile JSON if no existing profile fits.
4. Optionally draft one Source JSON selecting that profile.
5. Save files only under the app data directory.
6. Reload/inspect registry diagnostics if available.
7. Run a Source Live Check for the concrete Source if the app exposes the command/UI.
8. Report the resulting Source Live Check state and diagnostics to the user.

## Source Profile JSON example

<!-- schema-test:source-profile -->
```json
{
  "schemaVersion": 2,
  "key": "example_jobs_api",
  "name": "Example Jobs API",
  "kind": "career_site",
  "description": "Declarative profile for Example's public jobs API.",
  "support": {
    "level": "best_effort",
    "summary": "Manual review found a public JSON jobs endpoint. Live reliability should be judged through concrete Source Live Checks.",
    "evidence": [
      {
        "kind": "manual_review",
        "reference": "https://jobs.example.com/api/jobs",
        "summary": "Public endpoint exposed title, company, location, URL, and job ID fields."
      }
    ]
  },
  "sourceConfigSchema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["apiBaseUrl"],
    "properties": {
      "apiBaseUrl": {
        "type": "string",
        "format": "uri",
        "title": "API Base URL"
      }
    }
  },
  "detect": {
    "inputUrlPatterns": [
      {
        "pattern": "^https://jobs\\.example\\.com(?:/.*)?$"
      }
    ],
    "recommendedAccessPathKey": "api",
    "sourceConfig": {
      "apiBaseUrl": "https://jobs.example.com/api"
    },
    "evidence": [
      {
        "kind": "url",
        "message": "Example jobs URLs map to the public jobs API."
      }
    ]
  },
  "accessPaths": [
    {
      "key": "api",
      "name": "Public jobs API",
      "postingDiscovery": {
        "strategies": [
          {
            "key": "jobs_api",
            "fetch": {
              "mode": "http",
              "method": "GET",
              "url": "{{sourceConfig:apiBaseUrl}}/jobs",
              "timeoutMs": 10000
            },
            "parse": {
              "type": "json"
            },
            "select": {
              "type": "json_path",
              "jsonPath": "$.jobs"
            },
            "extract": {
              "fields": {
                "title": {
                  "type": "json_path",
                  "jsonPath": "$.title",
                  "cardinality": "one"
                },
                "company": {
                  "type": "json_path",
                  "jsonPath": "$.company",
                  "cardinality": "one"
                },
                "url": {
                  "type": "json_path",
                  "jsonPath": "$.url",
                  "cardinality": "one"
                },
                "postingMeta": {
                  "jobId": {
                    "type": "json_path",
                    "jsonPath": "$.id",
                    "cardinality": "one"
                  }
                }
              }
            }
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
              "timeoutMs": 10000
            },
            "parse": {
              "type": "json"
            },
            "select": {
              "type": "document"
            },
            "extract": {
              "fields": {
                "descriptionText": {
                  "type": "json_path",
                  "jsonPath": "$.descriptionText",
                  "cardinality": "one"
                }
              }
            }
          }
        ]
      }
    }
  ]
}
```

## Source JSON example

<!-- schema-test:source -->
```json
{
  "schemaVersion": 2,
  "key": "example_jobs",
  "name": "Example Jobs",
  "status": "draft",
  "selectedAccessPath": {
    "type": "profile_access_path",
    "profileKey": "example_jobs_api",
    "pathKey": "api"
  },
  "sourceConfig": {
    "apiBaseUrl": "https://jobs.example.com/api"
  }
}
```

## Final response format

When finished, report:

```text
Created/updated files:
- <path>

Profile:
- Key: <profile-key>
- Scope: <existing_profile_candidate | reusable_platform_candidate | company_specific_career_site | unknown>
- Support level: <stable | best_effort | experimental | unsupported>
- Access Path: <key and strategy shape>

Source, if created:
- Key: <source-key>
- Status: <draft | active | disabled>
- Source Live Check: <passed | failed | stale | unknown | not run>

Evidence:
- <brief public evidence and risks>

Diagnostics:
- <schema/registry/compiler/live-check diagnostics or “none observed”>

Next steps:
- Run Source Live Check if not run.
- Fix diagnostics if the Source Live Check failed.
```
