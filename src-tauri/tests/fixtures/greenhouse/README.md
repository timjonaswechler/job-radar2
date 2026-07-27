# Greenhouse fixtures

These synthetic fixtures exercise the built-in `greenhouse` Source Profile without depending on a live provider.

## Coverage

- public Jobs Board API discovery response;
- normalized title, company, URL, and location extraction;
- `postingMeta.jobId` capture;
- lazy detail acquisition and description extraction;
- deterministic expected Candidate output.

`Acme Robotics`, the `acmejobs` board slug, job IDs, and posting data are repository-owned test values. They do not represent a real Greenhouse board.

## Files

- [`posting-discovery-response.json`](posting-discovery-response.json) — synthetic API response.
- [`posting-discovery-expected-candidates.json`](posting-discovery-expected-candidates.json) — expected normalized Candidates.
- [`posting-detail-9001-response.json`](posting-detail-9001-response.json) — synthetic detail response.
- [`posting-detail-9001-expected.json`](posting-detail-9001-expected.json) — expected lazy Detail result.

The corresponding integration tests are in [`profile_dsl_profiles/greenhouse.rs`](../../profile_dsl_profiles/greenhouse.rs).

## Provenance and limits

Greenhouse documents the unauthenticated Job Board API at <https://developers.greenhouse.io/job-board.html>. The fixture shape follows that public contract but intentionally uses fabricated content.

The fixture proves parser and runtime behavior only. It does not prove that a concrete board is currently reachable or that every provider-specific location format is normalized into separate places. Operational confidence for a concrete Source comes from its Source Live Check.

## Focused validation

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test profile_dsl_profiles greenhouse::
```
