# Greenhouse Source Evidence

> Historical evidence snapshot: old fingerprint or phase wording below describes the recorded pre-#242 check and is not current authoring guidance.

Source Profile: `greenhouse`

Audit parent: GitHub issue #33

Current evidence covers the documented API shape, deterministic parser behavior, five dated public endpoint observations, and fresh passing Source Live Checks for three isolated temporary Sources. The observations do not make those Sources built-in or guarantee future operability.

## Official vendor documentation

### Greenhouse Job Board API documentation

- Source Profile: `greenhouse`
- Evidence class: official vendor documentation
- Entry URL: <https://developers.greenhouse.io/job-board.html>
- Source Config: documentation describes a public `board_token`, represented by Job Radar as `sourceConfig.boardSlug`
- Checked at: 2026-07-10
- Source Live Check result: not applicable
- Source Live Check report state: not applicable
- Detail checked: not applicable
- Evidence reference: Greenhouse Job Board API list and retrieve-job examples

The documentation describes public unauthenticated GET endpoints for listing a board's jobs and retrieving one job. Its list-response example exposes the raw location through `location.name`.

| Raw provider value | Current normalized output | Provenance | Notes |
|---|---|---|---|
| `"NYC"` | `["NYC"]` expected by static DSL derivation | official documentation | Documented vendor example, not a runtime execution or current public Source observation |

## Deterministic repository fixtures

### Synthetic Greenhouse DSL regression fixture

- Source Profile: `greenhouse`
- Evidence class: deterministic repository fixture
- Entry URL: not applicable
- Source Config: synthetic `boardSlug: "acmejobs"`
- Added at: commit `4f9dcb9` for GitHub issue #105
- Source Live Check result: not applicable
- Source Live Check report state: not applicable
- Detail checked: yes, offline through the shared DSL runtime
- Evidence references:
  - [`posting-discovery-response.json`](../../src-tauri/tests/fixtures/greenhouse/posting-discovery-response.json)
  - [`posting-discovery-expected-candidates.json`](../../src-tauri/tests/fixtures/greenhouse/posting-discovery-expected-candidates.json)
  - [`posting-detail-9001-response.json`](../../src-tauri/tests/fixtures/greenhouse/posting-detail-9001-response.json)
  - [`greenhouse_profile_dsl.rs`](../../src-tauri/tests/greenhouse_profile_dsl.rs)

Issue #105 explicitly required local synthetic or anonymized fixtures. `Acme Robotics`, `acmejobs`, job IDs `9001` and `9002`, and their job data are test data created in this repository. These values prove current parser/runtime behavior only; they are not evidence that a real Greenhouse board emitted the same formats.

| Raw fixture value | Tested normalized output | Provenance | Notes |
|---|---|---|---|
| `"Berlin, Germany"` | `["Berlin, Germany"]` | synthetic repository fixture | Proves the current DSL preserves one location string |
| `"Remote"` | `["Remote"]` | synthetic repository fixture | Proves parser behavior, not a provider-observed remote format |

Focused deterministic validation:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
```

Last audit result on 2026-07-10: `1 passed; 0 failed`.

## Public endpoint observations

The following Boards API list endpoints were observed at `2026-07-10T11:52:24Z`. These are public endpoint observations, not Source Live Checks.

| Board | `boardSlug` | Jobs | Representative raw `location.name` values | Assessment |
|---|---|---:|---|---|
| Greenhouse Software | `greenhouse` | 22 | `Anywhere in the United States`; `Argentina`; `British Columbia`; `London, United Kingdom`; `Ontario` | Vendor-owned control Source |
| Karbon | `karbon` | 29 | `Remote, United States`; `Sydney, NSW, Australia`; long semicolon-separated city lists | Strong real multi-location evidence |
| Prophecy | `prophecysimpledatalabs` | 6 | `San Francisco, CA (Remote)`; `Leeds/Sheffield/Manchester, UK (Hybrid)`; `Bengaluru, Karnataka, India` | Slash-delimited and Remote/Hybrid edge cases; small volatile board |
| Example Corp Sandbox | `examplecorpsandbox` | 224 | missing/`null`; `Anywhere`; `Amsterdam Area (Hybrid) `; `Austin, TX, Reston, VA, Boston, MA` | Vendor sandbox, not a real employment Source |
| Cloudflare | `cloudflare` | 245 | `Distributed`; `Distributed; Hybrid`; `Hybrid or Remote`; `Remote India`; `Tokyo, Japan` | `location.name` often represents work mode rather than a concrete place |

Each list endpoint used the documented public shape:

```text
https://boards-api.greenhouse.io/v1/boards/<boardSlug>/jobs
```

The reported counts and values are dated observations and may change as postings open or close.

## Isolated Source Live Checks

Three temporary draft Sources were created under an isolated temporary app-data directory. No existing app data was read or modified, and the Sources were not added under `src-tauri/resources/sources/`.

| Temporary Source | `boardSlug` | Checked at | Report result | Report state | Candidates | Detail |
|---|---|---|---|---|---:|---|
| `greenhouse_karbon` | `karbon` | `2026-07-10T12:00:06Z` | `passed` | `fresh` | 29 | checked and passed |
| `greenhouse_prophecy` | `prophecysimpledatalabs` | `2026-07-10T12:00:08Z` | `passed` | `fresh` | 6 | checked and passed |
| `greenhouse_vendor_careers` | `greenhouse` | `2026-07-10T12:00:09Z` | `passed` | `fresh` | 22 | checked and passed |

All three reports had zero Structured Diagnostics. Freshness was evaluated immediately against the unchanged temporary Source documents, Source Config, built-in Greenhouse profile, and live-check logic.

A separate immediate live discovery through the same compiled profile/runtime reproduced the candidate counts with zero diagnostics and exposed the current normalized location output. The profile currently preserves one raw `location.name` string as one array item; it does not split embedded delimiters:

| Source | Raw endpoint value | Current normalized output | Location implication |
|---|---|---|---|
| Karbon | `Canberra, ACT, Australia; Melbourne, VIC, Australia; Sydney, NSW, Australia` | `["Canberra, ACT, Australia; Melbourne, VIC, Australia; Sydney, NSW, Australia"]` | Semicolon-separated places remain one location |
| Prophecy | `Leeds/Sheffield/Manchester, UK (Hybrid)` | `["Leeds/Sheffield/Manchester, UK (Hybrid)"]` | Slash-separated places and work mode remain one location |
| Greenhouse Software | `Anywhere in the United States` | `["Anywhere in the United States"]` | Remote/region semantics remain unstructured |

These concrete observations are input for #57. They document current behavior but do not define the future normalized location model.

## Evidence still needed

- non-Latin original location strings;
- verified pipe- or newline-delimited multi-location values;
- explicit timezone or commuting-distance formats;
- repeated checks after Source/Profile changes to establish longer-term stability;
- a decision on whether provider metadata such as Cloudflare's concrete-location fields should be modeled generically.

A direct Boards API request remains public endpoint evidence, not a Source Live Check.

## Related follow-up

GitHub issue #164 covers deterministic detection regressions for all URL variants declared by the Greenhouse Source Profile. It does not add live or location evidence.
