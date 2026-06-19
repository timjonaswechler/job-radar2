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
- Custom source profiles: app data directory + `source-profiles/*.json`
- Custom sources: app data directory + `sources/*.json`
- Built-in source profiles and sources: versioned under `source-profiles/builtin/*.json` and `sources/builtin/*.json`, then embedded into the Rust binary with `include_str!`
- Current tables: `app_metadata`, `app_settings`, `search_requests`
- Migrations: `src-tauri/migrations/`
- Backend access: `src-tauri/src/db/` and `src-tauri/src/app/commands.rs`
- Dev reset: `npm run tauri:dev:reset-db` starts Tauri with `JOB_RADAR_RESET_DEV_DB=1` and deletes only the local SQLite file family before migration/seeding. This is debug-build only.
- Local app-data DB reset: `just db-clear` deletes the installed/dev app-data SQLite file family (`job_radar.db`, `-wal`, `-shm`, `-journal`) without deleting custom source/profile JSON documents.
- Migration squash: `just migrations-squash` rebuilds a single current-schema SQLx migration from all files in `src-tauri/migrations/`; run `just db-reset-after-squash` if you want to squash and then clear the app-data DB so checksum conflicts cannot occur.
- Data-preserving migration squash: `just db-preserve-after-squash` squashes migrations and rewrites SQLx bookkeeping on the existing DB, but only if the DB schema already matches the squashed schema; it creates a backup under `backups/db/` first.

## Useful scripts

- `npm run dev` — run the Vite frontend only
- `npm run build` — type-check and build the frontend
- `npm run tauri -- dev` — run the Tauri desktop app in development
- `npm run tauri:dev:reset-db` — explicitly reset the local development SQLite DB and start Tauri
- `npm run smoke:search-run` — run the manual network-dependent SCHOTT + StepStone backend smoke path (see `docs/dev-search-run-smoke.md`)
- `npm run tauri -- build` — build a distributable desktop app
