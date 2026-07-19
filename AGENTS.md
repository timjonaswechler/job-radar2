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
- `docs/prd/declarative-source-profile-dsl.md` — Zielbild der deklarativen Profile-DSL und Source-/Source-Profile-Dokumentmodell.
- `docs/adr/0001-source-config-as-json-schema.md` und `docs/adr/0009-declarative-source-profile-dsl.md` — zentrale Source/Profile-Architekturentscheidungen.
- `docs/adr/` — weitere Architekturentscheidungen.
- `docs/dev-search-run-smoke.md` — manueller Live-Smoke für Suchläufe.
- `handoff/` — laufende oder frühere Übergabepläne.

## Agent skills

### Issue tracker

Issues werden als GitHub Issues in diesem Repository verwaltet; externe Pull Requests sind keine Triage-Request-Surface. Siehe `docs/agents/issue-tracker.md`.

### Triage labels

Die fünf kanonischen Triage-Rollen verwenden `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human` und `wontfix`. Siehe `docs/agents/triage-labels.md`.

### Domain docs

Das Repository verwendet ein Single-Context-Layout mit `CONTEXT.md` und `docs/adr/` im Repository-Root. Siehe `docs/agents/domain.md`.

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
- Source Profiles beschreiben wiederverwendbare Verhaltensfamilien, nicht bloß Host- oder Linkstrukturen. URL-Muster sind Detection-Hinweise; belastbare Detection kombiniert sie bei Bedarf mit begrenzter HTTP-, API-, HTML- oder Browser-Evidenz.
- Ziel der Source-Einrichtung ist: Der User gibt einen Einstiegspunkt an, Profile Detection wählt Profile und Access Path, füllt die Source Config möglichst vollständig aus und prüft die konkrete Source per Source Live Check. Quellenspezifische Konfiguration darf variieren, soll aber nicht zum erforderlichen Integrationswissen des Users werden.
- Bei der Weiterentwicklung von Search Runs günstige Discovery-Hinweise von kanonischen Posting-Daten unterscheiden: nur plausible Kandidaten detailliert laden, Titel und Locations vor dem finalen Matching normalisieren und nur final geprüfte Matches persistieren. Das als generische DSL-/Pipeline-Fähigkeit lösen, nicht als ATS-Sonderfall.
- Strategien sollen begrenzt sein und strukturierte Diagnostics liefern.
- Diese Datei kurz halten: Details verlinken statt duplizieren.

## Rust-Tests

Für Logik, die über die öffentliche Crate-API sichtbar ist, bevorzugt Integration Tests als externe Tests unter `src-tauri/tests/*.rs` schreiben. In-Modul-Tests mit `#[cfg(test)]` nur für private Helper, enge Edge-Cases oder wenn bewusst Implementierungsdetails getestet werden.
