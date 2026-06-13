# Job Radar

A desktop app shell built with [Tauri](https://tauri.app/), React, TypeScript, Vite, and SQLite.

## Development

```bash
npm install
npm run tauri -- dev
```

## Frontend structure

`src/` is organized as a shadcn/ReUI-style React app:

```txt
src/
  app/                 # App composition and page switching
  components/
    ui/                # shadcn-compatible primitives
    reui/              # composed ReUI-style building blocks
    layout/            # shell/navigation layout
  features/            # vertical slices with feature-owned components (currently only home)
  pages/               # route/page adapters
  hooks/               # shared React hooks
  lib/                 # utilities, navigation config, API clients
  styles/              # Tailwind v4 theme and globals
```

UI foundation:

- Tailwind CSS v4 via `@tailwindcss/vite`
- shadcn-compatible `components.json` and `@/*` alias
- Base UI via `@base-ui/react` for headless primitives
- local copy-and-own components in `src/components/ui` and `src/components/reui`

## SQLite

The Rust backend creates and migrates a local SQLite database on startup. The schema is intentionally domain-neutral for now.

- Database file: app data directory + `job_radar.db`
- Custom system profiles: app data directory + `system-profiles/*.json`
- Built-in system profiles: versioned under `system-profiles/builtin/*.json` and embedded into the Rust binary with `include_str!`
- Current tables: `app_metadata`, `app_settings`, `browser_profiles`, `system_profiles`, `sources`
- Migrations: `src-tauri/migrations/`
- Backend access: `src-tauri/src/db.rs` and `src-tauri/src/commands.rs`
- Dev reset: `npm run tauri:dev:reset-db` starts Tauri with `JOB_RADAR_RESET_DEV_DB=1` and deletes only the local SQLite file family before migration/seeding. This is debug-build only.

## Useful scripts

- `npm run dev` — run the Vite frontend only
- `npm run build` — type-check and build the frontend
- `npm run tauri -- dev` — run the Tauri desktop app in development
- `npm run tauri:dev:reset-db` — explicitly reset the local development SQLite DB and start Tauri
- `npm run smoke:search-run` — run the manual network-dependent SCHOTT + StepStone backend smoke path (see `docs/dev-search-run-smoke.md`)
- `npm run tauri -- build` — build a distributable desktop app
