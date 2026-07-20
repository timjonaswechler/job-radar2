# SAP SuccessFactors Source Evidence

> Historical evidence snapshot: schema-v2 and old phase references below describe the recorded pre-#242 check and are not current authoring guidance.

Source Profile: `successfactors`

Audit parent: GitHub issue #33

Current evidence covers official SAP guidance about Recruiting Marketing (RMK) sitemaps and job URL construction, deterministic repository fixtures, four dated public endpoint observations, and four isolated Source Live Checks. SCHOTT and KraussMaffei passed. SAP and DACHSER failed for different observed variants. These concrete results do not create Built-in Sources and do not by themselves determine the reusable profile's Support Level.

## Official vendor evidence

### Recruiting Marketing sitemap guidance

- Evidence type: official SAP Knowledge Base documentation
- References:
  - [SAP KBA 2887940: How to proceed with Sitemap Submissions - Recruiting Marketing](https://userapps.support.sap.com/sap/support/knowledge/en/2887940)
  - [SAP KBA 2757876: Site Map in Career Site Builder - Recruiting Marketing](https://userapps.support.sap.com/sap/support/knowledge/public/E/2757876)
- Relevance: SAP documents sitemap submission for public Recruiting Marketing career sites and shows that sitemap filenames are configurable rather than one universal provider endpoint.
- Limitation: parts of the SAP Knowledge Base content require authenticated access. The references establish the product behavior but are not a complete public XML contract.

### RMK job URL construction

- Evidence type: official SAP Knowledge Base documentation
- Reference: [SAP KBA 2845557: How are RMK Jobs URLs generated](https://userapps.support.sap.com/sap/support/knowledge/en/2845557)
- Relevance: SAP describes classic RMK job URLs as a combination of requisition title text, primary-location elements, and a generated RMK page ID. Unsafe characters are URL-encoded.
- Consequence: a sitemap URL alone does not expose a guaranteed delimiter between a multi-token location and the title. The current profile's URL capture is therefore a heuristic, not a lossless provider field mapping.

### Branded career-site hosts

- Evidence type: official SAP Help documentation
- Reference: [Career Site Builder](https://help.sap.com/docs/successfactors-recruiting/setting-up-and-maintaining-sap-successfactors-recruiting/career-site-builder)
- Relevance: SuccessFactors Recruiting Marketing supports branded public career sites. A reusable Source Profile cannot identify the system from a `successfactors.com` hostname alone.

## Deterministic repository evidence

The synthetic fixtures under `src-tauri/tests/fixtures/successfactors/` prove current compiler and runtime behavior without making a live-operability claim.

Covered behavior:

- schema-v2 Source Profile deserialization and compilation;
- named `successFactorsHost` detection capture;
- one bounded sitemap/XML discovery request;
- filtering a non-job `/content/` URL;
- classic `-<numeric-id>` and `/<numeric-id>/` job URL forms;
- normalized title, location, URL, `postingMeta.jobId`, and `postingMeta.externalPath` output;
- primary HTML description extraction;
- ordered generic and SCHOTT-style HTML fallback selectors;
- Structured Diagnostic output when the primary selector is empty;
- positive sitemap HTTP evidence and rejection of a non-RMK sitemap body during detection.

Focused commands run on 2026-07-11:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test source_profile_detection successfactors
```

Results:

- `successfactors_profile_dsl`: `1 passed; 0 failed`
- focused SuccessFactors detection: `2 passed; 0 failed`

Missing deterministic slices include multi-token locations, prefixed `/job/` paths, RSS feeds, sitemap indexes, non-root sitemap filenames, malformed XML, request failures, item-bound diagnostics, both detail strategies failing, and detection coverage for every declared input path family.

## Public endpoint observations

The following endpoints were observed on 2026-07-11. These are direct public endpoint observations, not Source Live Checks.

| Source | Public endpoint | Observed shape | Representative raw values or paths | Assessment |
|---|---|---|---|---|
| SCHOTT | <https://join.schott.com/sitemap.xml> | standard XML `urlset` | `Mainz-.../1388186833/`; `PeraiPenang-.../1037615501/`; `Ilmenau-.../1377312533/` | Matches the profile's currently covered root `/job/` family. |
| KraussMaffei | <https://jobs.kraussmaffei.com/sitemap.xml> | standard XML `urlset` | `Parsdorf-bei-M%C3%BCnchen-.../1270636901/`; detail HTML exposes `streetAddress="Parsdorf bei München, DE"` | Demonstrates that URL-derived location/title boundaries are ambiguous. |
| SAP | <https://jobs.sap.com/sitemap.xml> | RSS 2.0 job feed, not a sitemap `urlset` | feed titles include title and location, for example `(... Bonn, DE, 53113)`; feed entries expose job links and `g:location` | The current sitemap-only assumption does not describe SAP's own observed feed shape. |
| DACHSER | <https://careers.dachser.com/sitemap.xml> | standard XML `urlset` with mixed paths | root `/job/...` plus prefixed `/dachser_europe/job/...` and `/dachser_apac/job/...` | Prefixed paths do not match the current `externalPath` capture. |

Counts and postings are volatile. A public endpoint response does not establish that the app's compiler/runtime and detail extraction currently succeed.

## Isolated Source Live Checks

Four temporary active Sources were created in an isolated app-data directory under `/tmp`. No existing app data was read or modified, and no Source document was added under `src-tauri/resources/sources/`.

Each `check_source` call used `source-live-check/v2`, persisted a Check Report, and was followed immediately by `source_live_check_report_status` against unchanged Source/Profile documents. All four reports were therefore `fresh` at evaluation time.

| Temporary Source | Source Config | Checked at | Persisted result | Report state | Candidates | Detail | Structured Diagnostics |
|---|---|---|---|---|---:|---|---|
| `successfactors_schott` | `baseUrl: https://join.schott.com`; `sitemapUrl: https://join.schott.com/sitemap.xml` | `2026-07-11T08:32:11Z` | `passed` | `fresh` | 171 | checked and passed | none |
| `successfactors_kraussmaffei` | `baseUrl: https://jobs.kraussmaffei.com`; `sitemapUrl: https://jobs.kraussmaffei.com/sitemap.xml` | `2026-07-11T08:32:13Z` | `passed` | `fresh` | 14 | checked and passed | none |
| `successfactors_sap` | `baseUrl: https://jobs.sap.com`; `sitemapUrl: https://jobs.sap.com/sitemap.xml` | `2026-07-11T08:32:23Z` | `failed` | `fresh` | 0 | not checked | `fetch_failed` (`error decoding response body`), `fallback_exhausted`, `source_live_check.no_candidates` |
| `successfactors_dachser` | `baseUrl: https://careers.dachser.com`; `sitemapUrl: https://careers.dachser.com/sitemap.xml` | `2026-07-11T08:32:38Z` | `failed` | `fresh` | 200 | checked and passed | 171 error `capture_not_matched` for `externalPath`; warning `pagination_max_items_reached` |

The SAP report proves the current app client/runtime failed against that concrete Source at the recorded time. The separate endpoint observation establishes that the response had an RSS shape; it does not by itself prove whether the runtime failure originated in HTTP body decoding, XML parsing compatibility, or both. That distinction requires a deterministic response fixture.

The DACHSER check discovered 200 acceptable candidates and passed one detail check, but the report correctly failed because error diagnostics were emitted for prefixed paths. A partially populated candidate list must not be presented as a passing live result.

The temporary reports and a separate immediate discovery sample remain in `/tmp/job-radar-successfactors-audit-20260711/` as local audit artifacts. They are not repository fixtures or durable product data.

## Location and title evidence for #57

The current profile derives both title and location from each job URL. It does not read the detail page's structured `jobLocation` during `postingDiscovery`.

| Source | Provider value or URL | Current normalized output | Provenance | Implication |
|---|---|---|---|---|
| SCHOTT | URL prefix `Mainz-...` | location `"Mainz"` | fresh Source Live Check | Correct for a single-token location. |
| SCHOTT | URL prefix `PeraiPenang-...` | location `"PeraiPenang"` | fresh Source Live Check | Provider spelling is preserved without separation. |
| KraussMaffei | URL `Parsdorf-bei-München-Auszubildenden-...`; HTML `streetAddress="Parsdorf bei München, DE"` | location `"Parsdorf"`; title starts `"Bei München Auszubildenden ..."` | fresh Source Live Check plus public detail-page observation | The passing check proves transport/extraction, but the normalized semantics are incorrect. |
| DACHSER | URL `Amt-Wachsenburg-Disponent-...` | location `"Amt"`; title starts `"Wachsenburg Disponent ..."` | fresh failed Source Live Check | Another multi-token location is split incorrectly. |
| SAP | RSS `g:location` and title location suffix such as `Bonn, DE, 53113` | no candidate output | public endpoint observation plus fresh failed Source Live Check | Structured feed location exists but is not consumed by the current Access Path. |

These observations are evidence for #57, not a design for location normalization. The immediate SuccessFactors problem is earlier: discovery does not always preserve the provider's title/location boundary.

## Audit assessment

- **Support metadata: needs revision.** Official URL-construction evidence and current candidate output show that the profile cannot generally recover title/location boundaries for the broad RMK family claimed by its summary. Support metadata should describe the actually supported classic RMK subset and known variants without implying live operability. Concrete Source pass/fail results remain separate operational evidence.
- **Detection: needs hardening.** It uses a useful bounded sitemap HTTP check, but assumes a root sitemap and immediate root `/job/` path forms.
- **`postingDiscovery`: needs HTTP/profile fix.** Standard root `urlset` Sources execute, while RSS and prefixed-path variants fail or emit errors. URL-only title/location extraction is not reliable for multi-token locations.
- **`postingDetail`: works for the checked standard variants.** SCHOTT, KraussMaffei, and the accepted DACHSER candidate passed lazy detail extraction. Deterministic primary and fallback coverage exists.
- **Source Config schema: too broad/misleading.** `maxUrls` appears executable but the profile always uses authored `maxItems: 200`; `baseUrl` is required but not used by this Access Path's execution.
- **Regression coverage: partial.** Compiler, standard discovery, detail fallbacks, and limited detection are covered; observed live variants are not yet fixtures.
- **Live-check evidence: mixed fresh results.** SCHOTT and KraussMaffei passed; SAP and DACHSER failed.
- **Location evidence: captured.** Single-token, concatenated, multi-token, city/country/postcode, and prefixed-path examples are recorded, including current incorrect output.

## Recommended next action

Issue [#166](https://github.com/timjonaswechler/job-radar2/issues/166) is the generic architecture consequence of this audit. The observed RSS/sitemap variants, ambiguous multi-token locations, prefixed job paths, and Structured Diagnostics are acceptance cases for the shared schema-v3 Strategy-/Primitive-Algebra and Search Run finalization. They must not be implemented as SuccessFactors-specific regex or Rust behavior. Issue #165 is superseded by #166; concrete implementation belongs in the approved tracer-bullet follow-ups from #166. Source Config cleanup remains deferred until the relevant generic contract is scheduled.
