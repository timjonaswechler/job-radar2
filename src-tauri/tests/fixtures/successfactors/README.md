# SAP SuccessFactors fixtures

These synthetic fixtures exercise the built-in `successfactors` Source Profile for the supported SAP Recruiting Marketing sitemap and job-page shapes.

## Coverage

- sitemap/XML Discovery;
- filtering non-job URLs;
- classic numeric RMK job URL forms;
- title, location, URL, `postingMeta.jobId`, and `postingMeta.externalPath` extraction;
- primary HTML description extraction;
- ordered generic and browser-repair fallback selectors;
- deterministic Diagnostics when the primary selector is empty;
- bounded HTTP evidence used by Detection.

All posting data in this directory is repository-owned test data.

## Files

- `posting-discovery-sitemap.xml` — synthetic sitemap input.
- `posting-discovery-expected-candidates.json` — expected normalized Candidates.
- `posting-detail-*.html` — synthetic primary and fallback Detail inputs.
- `posting-detail-*-expected.json` — expected lazy Detail results.

The corresponding integration tests are in [`profile_dsl_profiles/successfactors.rs`](../../profile_dsl_profiles/successfactors.rs).

## Provenance and limits

Relevant SAP documentation includes:

- [Sitemap submissions for Recruiting Marketing](https://userapps.support.sap.com/sap/support/knowledge/en/2887940)
- [Site Map in Career Site Builder](https://userapps.support.sap.com/sap/support/knowledge/public/E/2757876)
- [RMK job URL generation](https://userapps.support.sap.com/sap/support/knowledge/en/2845557)
- [Career Site Builder](https://help.sap.com/docs/successfactors-recruiting/setting-up-and-maintaining-sap-successfactors-recruiting/career-site-builder)

SuccessFactors sites vary. Public installations may expose RSS rather than a `urlset`, use prefixed job paths, or encode multi-token locations ambiguously in URLs. The fixtures prove only the explicitly represented profile shapes. They do not establish support for every RMK installation or current operability of a concrete Source.

Operational confidence for a concrete Source comes from its Source Live Check. New provider variants should first become minimal sanitized regression fixtures and generic DSL/runtime behavior rather than Source-specific Rust branches.

## Focused validation

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test profile_dsl_profiles successfactors::
```
