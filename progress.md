# Adapter MVP worker progress

- Read `context.md` and `handoff/adapter-mvp-context.md`.
- `plan.md` was requested but is not present in the worktree.
- Inspected initial dirty worktree with `git status --short`; preserving unrelated existing changes.
- Implemented built-in adapter metadata registry for `sitemap_xml`, `http_json`, and `headless_browser`.
- Exposed `list_adapters` through Tauri and frontend API/types.
- Added source validation against registered adapters plus MVP JSON Schema subset; `headless_browser` requires `browserProfileId`, non-browser adapters reject it.
- Updated source inventory/form UI to load adapters, use an adapter select, require browser profiles only for browser-based adapter metadata, and render dynamic schema fields with JSON fallback.
- Validation completed:
  - `cd src-tauri && cargo test` passed.
  - `cd src-tauri && cargo fmt -- --check` passed.
  - `npm run build` passed.
