# Repository scripts

Scripts are grouped by the workflow they own. Stable user-facing commands remain in `package.json` or the `Justfile`; callers should prefer those interfaces over invoking implementation files directly.

- [`checks/`](checks/) — repository invariants and security checks used by tests or CI.
- [`database/`](database/) — local SQLite and SQLx migration maintenance.
- [`development/`](development/) — optional source-maintenance utilities.
- [`geo/`](geo/) — generation of bundled geolocation resources.
- [`testing/`](testing/) — shared adapters for running frontend contract tests.

Add a new script to the narrowest owning directory. If developers or CI need to call it regularly, expose it through `package.json` or the `Justfile` and document that command instead of the implementation path.
