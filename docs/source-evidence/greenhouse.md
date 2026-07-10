# Greenhouse Source Evidence

Source Profile: `greenhouse`

Audit parent: GitHub issue #33

Current evidence is sufficient to document the public API shape and deterministic parser behavior. It is not yet sufficient to claim current operability for a concrete Greenhouse Source or representative real-world location coverage.

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

## Concrete Source observations

None recorded yet.

No Built-in Source exists under `src-tauri/resources/sources/`, no temporary audit Source has been documented, and no Greenhouse Source Live Check Report is available. Current Source Live Check state is therefore `unknown`.

## Evidence still needed

Find two or three stable public Greenhouse boards and record, without adding them as Built-in Sources by default:

- public board URL and `boardSlug`;
- dated raw `location.name` observations from the public endpoint;
- current normalized `locations` output;
- city/country, region, remote, multi-location, and delimiter variants where available;
- one bounded Source Live Check per selected concrete Source;
- persisted Check Report result and derived freshness state;
- whether lazy detail succeeded for the checked candidate.

A direct Boards API request may provide raw endpoint evidence, but it must not be described as a Source Live Check.

## Related follow-up

GitHub issue #164 covers deterministic detection regressions for all URL variants declared by the Greenhouse Source Profile. It does not add live or location evidence.
