# Built-in Source Profiles

This directory contains the Source Profile JSON documents shipped with Job Radar. The registry loads the profile documents as product resources; do not place test data or additional JSON files here.

Each profile must remain declarative and reusable across a behavior family. Search criteria belong to Search Requests, not Source Config or Source Profiles. Concrete operational confidence comes from a Source Live Check, not from profile support metadata alone.

## Executable evidence

Deterministic response and expected-output fixtures live with the Rust integration tests:

- [`greenhouse`](../../../../tests/fixtures/greenhouse/)
- [`workday`](../../../../tests/fixtures/workday/)
- [`successfactors`](../../../../tests/fixtures/successfactors/)

The fixture README files distinguish synthetic regression data, vendor documentation, dated public observations, and historical live checks. Dated observations do not guarantee current provider behavior.

Run the built-in profile tests with:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test bundled_source_profiles
```
