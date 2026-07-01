# AGENTS.md

## Projektüberblick

Job Radar ist eine lokale Tauri-2-Desktop-App für wiederholbare Jobsuchen: Quellen beschreiben, Suchanfragen speichern, Suchläufe ausführen und Job-Postings zusammenführen.

- Frontend: React/TypeScript/Vite in `src/`.
- Backend: Rust/Tauri/SQLite in `src-tauri/src/`.
- UI: shadcn/Base-UI-nahe Komponenten in `src/components/ui/` und `src/components/reui/`.
- Rust-Crate-Root: `src-tauri/`.

## Wichtige Einstiegspunkte

- `README.md` — Produktüberblick, lokale Befehle, Repo-Orientierung.
- `CONTEXT.md` — kanonische Domain-Sprache; vor Begriffsumbenennungen lesen.
- `docs/source-registry-json-model.md` — Source-/Source-Profile-Dokumentmodell.
- `docs/prd/declarative-source-profile-dsl.md` — Zielbild der deklarativen Profile-DSL.
- `docs/adr/` — Architekturentscheidungen.
- `docs/dev-search-run-smoke.md` — manueller Live-Smoke für Suchläufe.
- `handoff/` — laufende oder frühere Übergabepläne.

## Befehle

```bash
npm run tauri -- dev                         # App starten
npm run build                                # Frontend type-checken und bauen
cargo test --manifest-path src-tauri/Cargo.toml
npm run smoke:search-run                     # manueller, netzwerkabhängiger Smoke
```

## Arbeitsregeln

- Domain-Begriffe aus `CONTEXT.md` verwenden: z. B. Source, Source Profile, Access Path, Search Request, Search Run.
- Suchkriterien gehören zur Search Request, nicht in Source Config oder Source Profile.
- Die Profile DSL bleibt deklarative Konfiguration; keine profile-spezifischen Rust-Sonderfälle einbauen.
- Strategien sollen begrenzt sein und strukturierte Diagnostics liefern.
- Diese Datei kurz halten: Details verlinken statt duplizieren.

## Rust-Tests

Für Logik, die über die öffentliche Crate-API sichtbar ist, bevorzugt Integration Tests als externe Tests unter `src-tauri/tests/*.rs` schreiben. In-Modul-Tests mit `#[cfg(test)]` nur für private Helper, enge Edge-Cases oder wenn bewusst Implementierungsdetails getestet werden.
