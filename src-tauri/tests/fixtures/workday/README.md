# Workday fixtures

These synthetic fixtures exercise the built-in `workday` Source Profile and its public CXS-style request/response contract without contacting a live tenant.

## Coverage

- JSON POST discovery requests;
- offset pagination with a page size of 20;
- preservation of the positive initial `total` when a non-empty later page reports `total: 0`;
- normalized title, company, URL, and location extraction;
- `postingMeta.externalPath` capture;
- lazy detail acquisition and description extraction;
- deterministic expected Candidate output.

The `Acme Robotics` tenant, host, site, requisitions, and posting data are repository-owned test values.

## Files

- [`posting-discovery-page-0-response.json`](posting-discovery-page-0-response.json) — synthetic initial page.
- [`posting-discovery-page-20-response.json`](posting-discovery-page-20-response.json) — synthetic follow-up page.
- [`posting-discovery-expected-candidates.json`](posting-discovery-expected-candidates.json) — expected normalized Candidates.
- [`posting-detail-jr-1001-response.json`](posting-detail-jr-1001-response.json) — synthetic detail response.
- [`posting-detail-jr-1001-expected.json`](posting-detail-jr-1001-expected.json) — expected lazy Detail result.

The corresponding integration tests are in [`bundled_source_profiles/workday.rs`](../../bundled_source_profiles/workday.rs).

## Provenance and limits

The request shape is based on observed public Workday-hosted CXS endpoints; no stable public vendor specification for this endpoint is asserted here. Real tenants can return one place, a remote-work phrase, or an opaque count such as `2 Locations`. These fixtures prove the checked-in compiler/runtime behavior, not current reachability or complete normalization for every tenant.

Operational confidence for a concrete Source comes from its Source Live Check.

## Focused validation

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test bundled_source_profiles workday::
```
