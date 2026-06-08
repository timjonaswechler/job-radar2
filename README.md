# Job Radar

A desktop app shell built with [Tauri](https://tauri.app/), React, TypeScript, Vite, and SQLite.

## Development

```bash
npm install
npm run tauri dev
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
- Current tables: `app_metadata`, `app_settings`
- Migrations: `src-tauri/migrations/`
- Backend access: `src-tauri/src/db.rs` and `src-tauri/src/commands.rs`

## Useful scripts

- `npm run dev` — run the Vite frontend only
- `npm run build` — type-check and build the frontend
- `npm run tauri dev` — run the Tauri desktop app in development
- `npm run tauri build` — build a distributable desktop app
