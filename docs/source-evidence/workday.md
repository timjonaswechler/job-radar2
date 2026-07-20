# Workday Source Evidence

> Historical evidence snapshot: old fingerprint or phase wording below describes the recorded pre-#242 check and is not current authoring guidance.

Source Profile: `workday`

Audit parent: GitHub issue #33

Current evidence covers synthetic deterministic fixtures, five dated public Workday CXS endpoint observations, pagination-size probes against three Sources, historical audit checks, and three fresh Source Live Checks using the dedicated bounded-smoke execution budget. No public vendor API documentation for this CXS shape was established during this audit; the endpoint observations must not be presented as official documentation. Passing live checks prove current bounded operability, not complete discovery of every posting.

## Deterministic repository fixtures

### Synthetic Workday DSL regression fixture

- Source Profile: `workday`
- Evidence class: deterministic repository fixture
- Entry URL: not applicable
- Source Config: synthetic `workdayHost: "acme.wd3.myworkdayjobs.com"`, `tenant: "acme"`, `site: "External"`
- Source Live Check result: not applicable
- Source Live Check report state: not applicable
- Detail checked: yes, offline through the shared DSL runtime
- Evidence references:
  - [`posting-discovery-page-0-response.json`](../../src-tauri/tests/fixtures/workday/posting-discovery-page-0-response.json)
  - [`posting-discovery-page-20-response.json`](../../src-tauri/tests/fixtures/workday/posting-discovery-page-20-response.json)
  - [`posting-discovery-expected-candidates.json`](../../src-tauri/tests/fixtures/workday/posting-discovery-expected-candidates.json)
  - [`posting-detail-jr-1001-response.json`](../../src-tauri/tests/fixtures/workday/posting-detail-jr-1001-response.json)
  - [`workday_profile_dsl.rs`](../../src-tauri/tests/workday_profile_dsl.rs)

The `Acme Robotics` tenant and posting data are synthetic test data. They prove detection, compilation, JSON-body offset pagination with `limit: 20`, discovery extraction, `postingMeta.externalPath`, and lazy detail behavior only. A separate inline regression temporarily lowers the compiled profile bound to two requests and proves that an initial `total: 373` remains authoritative when a successful follow-up page contains items but reports `total: 0`; the bounded run returns four synthetic candidates and emits `pagination_max_requests_reached`.

| Raw fixture value | Tested normalized output | Provenance | Notes |
|---|---|---|---|
| `"Berlin, Germany"` | `["Berlin, Germany"]` | synthetic repository fixture | One string remains one location |
| `"Remote - Germany"` | `["Remote - Germany"]` | synthetic repository fixture | Parser behavior, not a provider observation |
| `"Munich, Germany"` | `["Munich, Germany"]` | synthetic repository fixture | Parser behavior, not a provider observation |

Focused deterministic validation:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
```

Post-fix result on 2026-07-10: `3 passed; 0 failed`.

## Public endpoint observations

The following public Workday-hosted CXS endpoints were observed at `2026-07-10T13:50:02Z`. The request used `POST .../jobs` with a JSON body containing `appliedFacets`, `limit`, and `offset`. These are public endpoint observations, not official vendor documentation and not Source Live Checks. Counts and public job paths are volatile.

| Source | Public career URL | Source Config (`workdayHost`; `tenant`; `site`) | Jobs | Representative exact `locationsText` values | Detail observation |
|---|---|---|---:|---|---|
| Workday | <https://workday.wd5.myworkdayjobs.com/en-US/Workday> | `workday.wd5.myworkdayjobs.com`; `workday`; `Workday` | 373 | `USA, NY, New York City`; `Costa Rica`; `2 Locations`; `5 Locations` | CXS detail returned HTTP 200; list `2 Locations` expanded to primary `USA, IL, Chicago` plus `USA, GA, Atlanta` |
| NVIDIA | <https://nvidia.wd5.myworkdayjobs.com/en-US/NVIDIAExternalCareerSite> | `nvidia.wd5.myworkdayjobs.com`; `nvidia`; `NVIDIAExternalCareerSite` | 2000 | `4 Locations`; `Israel, Yokneam`; `Hong Kong, STP`; `China, Shenzhen` | CXS detail returned HTTP 200; list `4 Locations` expanded to `US, CA, Santa Clara`, `US, TX, Austin`, `US, TX, Remote`, and `US, CA, Remote` |
| Deutsche Bank | <https://db.wd3.myworkdayjobs.com/de-DE/DBWebsite> | `db.wd3.myworkdayjobs.com`; `db`; `DBWebsite` | 1083 | `Frankfurt Taunusanlage 12`; `3 Locations`; `2 Locations` | German-market query observation; CXS detail returned HTTP 200 and expanded `3 Locations` to `Frankfurt Theodor-H-A IBC`, `Bonn, Bundeskanzlerplatz 6`, and `Berlin Otto-Suhr-Allee 16` |
| HP | <https://hp.wd5.myworkdayjobs.com/en-US/ExternalCareerSite> | `hp.wd5.myworkdayjobs.com`; `hp`; `ExternalCareerSite` | 733 | `Fort Collins, Colorado, United States of America`; `All Cities, California, United States of America`; `2 Locations` | CXS detail returned HTTP 200; list `2 Locations` expanded to locations in Spain and the United States |
| Autodesk | <https://autodesk.wd1.myworkdayjobs.com/en-US/Ext> | `autodesk.wd1.myworkdayjobs.com`; `autodesk`; `Ext` | 575 | `Germany - Remote`; `EMEA - Germany - Offsite/Home`; `2 Locations`; `7 Locations` | Germany-query observation; CXS detail returned HTTP 200 and expanded `2 Locations` to `EMEA - Germany - Munich - Balanstrasse` and `Germany - Remote` |

The public list field is tenant- and posting-dependent. It may contain one place, a remote/work-mode phrase, or only an opaque count such as `4 Locations`. The detail response exposes a primary `location` and may expose `additionalLocations`; the current `postingDetail` strategy intentionally extracts only `descriptionText`, so those detail locations do not replace discovery locations today.

A follow-up pagination probe at `2026-07-10T21:46:13Z` clarified the observed CXS behavior. Workday, NVIDIA, and Deutsche Bank each accepted `limit: 20`: the initial `offset: 0` response contained 20 postings and reported a positive source-wide `total`, while the successful `offset: 20` response contained another 20 postings and reported `total: 0`. All three rejected `limit: 50` and `limit: 100` with HTTP 400. For these observed Sources, the positive initial-page total is therefore the useful run-level count; `0` on a non-empty follow-up page must not replace it.

| Source | `offset: 0`, `limit: 20` | `offset: 20`, `limit: 20` | Larger limits |
|---|---|---|---|
| Workday | 20 items; `total: 372` | 20 items; `total: 0` | 50/100: HTTP 400 |
| NVIDIA | 20 items; `total: 2000` | 20 items; `total: 0` | 50/100: HTTP 400 |
| Deutsche Bank | 20 items; `total: 1080` | 20 items; `total: 0` | 50/100: HTTP 400 |

At initial audit time, fixture-sized production settings (`limit: 2`, `maxRequests: 2`) returned only four candidates. The generic runtime now retains the highest observed total during one `offset_limit` run. The built-in profile uses the observed page size `limit: 20` and a hard safety ceiling of `maxRequests: 100`; ordinary Search Runs stop earlier when the retained initial total is exhausted.

## Isolated Source Live Checks

Three schema-valid draft Sources were checked under an isolated temporary app-data directory. No existing app data or Built-in Source was read or modified. Each `check_source` call persisted a Check Report; `source_live_check_report_status` was called immediately afterward against the unchanged Source and profile.

| Temporary Source | Public entry URL | Checked at | Persisted result | Derived state | Candidates | Detail | Structured Diagnostics |
|---|---|---|---|---|---:|---|---:|
| `workday_vendor` | <https://workday.wd5.myworkdayjobs.com/en-US/Workday> | `2026-07-10T13:48:34Z` | `passed` | `fresh` | 4 | checked and passed | 0 |
| `workday_deutsche_bank` | <https://db.wd3.myworkdayjobs.com/de-DE/DBWebsite> | `2026-07-10T13:48:41Z` | `passed` | `fresh` | 4 | checked and passed | 0 |
| `workday_nvidia` | <https://nvidia.wd5.myworkdayjobs.com/en-US/NVIDIAExternalCareerSite> | `2026-07-10T13:48:47Z` | `passed` | `fresh` | 4 | checked and passed | 0 |

Separate immediate discovery through each unchanged compiled Execution Plan produced the same candidate count and zero diagnostics. Representative current normalized outputs were:

| Source | Raw public list value | Current normalized output | Limitation |
|---|---|---|---|
| Workday | `USA, NY, New York City` | `["USA, NY, New York City"]` | City/region/country remains one unstructured string |
| Deutsche Bank | `2 Locations` | `["2 Locations"]` | Count placeholder is treated as a location; actual places are available only from detail |
| NVIDIA | `4 Locations` | `["4 Locations"]` | Count placeholder is treated as a location; primary/additional places and Remote semantics are available only from detail |

The passing results establish bounded compilation, POST discovery, and one-candidate lazy detail behavior for these concrete Sources at the historical audit time. They do not establish complete source-wide discovery; at that time the profile still used fixture-sized pagination settings.

## Intermediate post-total-fix Source Live Checks

A new isolated temporary app-data directory was populated only with copies of the three temporary draft Source documents and checked against the fixed worktree on 2026-07-10. No existing app data or Built-in Source was used. Each persisted Check Report was read back immediately and had derived Freshness `fresh`.

| Temporary Source | Checked at | Persisted result | Derived Freshness | Persisted discovery/detail | Persisted Structured Diagnostics |
|---|---|---|---|---|---|
| `workday_vendor` | `2026-07-10T14:33:34Z` | `passed` | `fresh` | 4 candidates; detail checked and passed | warning `pagination_max_requests_reached` |
| `workday_deutsche_bank` | `2026-07-10T14:33:47Z` | `passed` | `fresh` | 4 candidates; detail checked and passed | warning `pagination_max_requests_reached` |
| `workday_nvidia` | `2026-07-10T14:33:52Z` | `passed` | `fresh` | 4 candidates; detail checked and passed | warning `pagination_max_requests_reached` |

The harness also performed a separate immediate discovery after each persisted check. Deutsche Bank and NVIDIA again returned four candidates with `pagination_max_requests_reached`. The separate Workday-vendor discovery encountered `fetch_failed` (`error decoding response body`) followed by `fallback_exhausted`; this does not change its already persisted passing Check Report or the separately derived `fresh` state. These checks are historical intermediate evidence from before the dedicated Source Live Check budget and production pagination settings.

## Final bounded-smoke Source Live Checks

The final checks used a new isolated temporary app-data directory and `source-live-check/v2`. The generic Source Live Check execution budget allowed one pagination request per strategy without changing the compiled profile's Search Run bounds. Each check requested `offset: 0`, `limit: 20`, checked at most one detail candidate, persisted its report, and was read back immediately for Freshness. The expected `posting_discovery_request_budget_reached` diagnostic has `info` severity and describes the intentional smoke bound rather than profile truncation.

| Temporary Source | Checked at | Persisted result | Derived Freshness | Candidates | Detail | Diagnostic |
|---|---|---|---|---:|---|---|
| `workday_vendor` | `2026-07-10T22:15:29Z` | `passed` | `fresh` | 20 | checked and passed | info `posting_discovery_request_budget_reached` |
| `workday_deutsche_bank` | `2026-07-10T22:15:31Z` | `passed` | `fresh` | 20 | checked and passed | info `posting_discovery_request_budget_reached` |
| `workday_nvidia` | `2026-07-10T22:15:35Z` | `passed` | `fresh` | 20 | checked and passed | info `posting_discovery_request_budget_reached` |

These results are the current operational evidence. They intentionally do not claim complete Search Run inventory coverage.

## Evidence still needed

- official vendor documentation for the public CXS contract, if Workday publishes it;
- non-Latin original location values;
- missing, empty, and `null` `locationsText` behavior from real Sources;
- pipe/newline and commuting-distance formats;
- a product decision under #57 about preserving list placeholders while optionally using structured detail locations;
- an isolated full Search Run observation after integration to assess request duration and retained-total completion at larger inventories.
