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
- `docs/development/prd/source-behavior-language.md` — Zielbild der deklarativen Source Behavior Language und Source-/Source-Profile-Dokumentmodell.
- `docs/development/adr/0001-source-config-as-json-schema.md` und `docs/development/adr/0013-source-engine-and-sources.md` — zentrale Source/Profile-Architekturentscheidungen.
- `docs/development/adr/` — weitere Architekturentscheidungen.
- `docs/index.md` — Dokumentationsübersicht und Ablageregeln.
- `docs/development/search-run-smoke.md` — manueller Live-Smoke für Suchläufe.
- `handoff/` — ausschließlich temporäre, aktive Übergaben.

## Agent skills

### Issue tracker

Issues werden als GitHub Issues in diesem Repository verwaltet; externe Pull Requests sind keine Triage-Request-Surface. Siehe `docs/development/agents/issue-tracker.md`.

### Triage labels

Die fünf kanonischen Triage-Rollen verwenden `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human` und `wontfix`. Siehe `docs/development/agents/triage-labels.md`.

### Domain docs

Das Repository verwendet ein Single-Context-Layout mit `CONTEXT.md` und `docs/development/adr/` im Repository-Root. Siehe `docs/development/agents/domain.md`.

## Befehle

Im Entwicklungsloop gilt `quick → focused → full`; das Full Gate ist grundsätzlich erst vor der Übergabe erforderlich. Quick- und Focused-Befehle ersetzen es nicht. Details und Filterbeispiele stehen in [`docs/development/validation.md`](docs/development/validation.md).

```bash
just quick                                    # schnelle Typechecks, Frontendtests, cargo check --tests
just frontend-test settings                   # fokussierter Vitest-Lauf
just rust-test bundled_source_profiles workday:: # fokussiertes sichtbares Cargo-Testtarget
just verify                                   # vollständiges lokales Übergabe-Gate
just package                                  # Tauri-Paket für die aktuelle Plattform
npm run tauri -- dev                          # App starten
npm run smoke:search-run                      # manueller, netzwerkabhängiger Smoke
```

## Arbeitsregeln
- replace, don't layer.
- Domain-Begriffe aus `CONTEXT.md` verwenden: z. B. Source, Source Profile, Access Path, Search Request, Search Run.
- Bei neuen oder wesentlich umgebauten Modulen die Konventionen aus [`docs/development/architecture/module-design.md`](docs/development/architecture/module-design.md) anwenden: Der Modulpfad trägt den Kontext, öffentliche Namen wiederholen ihn nicht, und das Interface bleibt klein.
- Bei neuen oder wesentlich veränderten Rust-Modulen, öffentlichen Rust-Interfaces, Fehlertypen oder Nebenwirkungs-Seams vor dem Entwurf [`docs/development/architecture/rust.md`](docs/development/architecture/rust.md) lesen und dessen Completion Criteria auf jeden berührten öffentlichen Seam anwenden.
- Bei neuen oder wesentlich veränderten TypeScript-/React-Feature-Seams, Tauri-Transportverträgen, Zustandsmodellen oder Async-Lebenszyklen vor dem Entwurf [`docs/development/architecture/typescript-react.md`](docs/development/architecture/typescript-react.md) lesen und dessen Completion Criteria auf jeden berührten Feature-Seam anwenden.
- Suchkriterien gehören zur Search Request, nicht in Source Config oder Source Profile.
- Die Source Behavior Language bleibt deklarative Konfiguration; keine profile-spezifischen Rust-Sonderfälle einbauen.
- Source Profiles beschreiben wiederverwendbare Verhaltensfamilien, nicht bloß Host- oder Linkstrukturen. URL-Muster sind Detection-Hinweise; belastbare Detection kombiniert sie bei Bedarf mit begrenzter HTTP-, API-, HTML- oder Browser-Evidenz.
- Ziel der Source-Einrichtung ist: Der User gibt einen Einstiegspunkt an, Profile Detection wählt Profile und Access Path, füllt die Source Config möglichst vollständig aus und prüft die konkrete Source per Source Live Check. Quellenspezifische Konfiguration darf variieren, soll aber nicht zum erforderlichen Integrationswissen des Users werden.
- Bei der Weiterentwicklung von Search Runs günstige Discovery-Hinweise von kanonischen Posting-Daten unterscheiden: nur plausible Kandidaten detailliert laden, Titel und Locations vor dem finalen Matching normalisieren und nur final geprüfte Matches persistieren. Das als generische Source Behavior Language-/Pipeline-Fähigkeit lösen, nicht als ATS-Sonderfall.
- Strategien sollen begrenzt sein und strukturierte Diagnostics liefern.
- Diese Datei kurz halten: Details verlinken statt duplizieren.

## Rust-Tests

Für Logik, die über die öffentliche Crate-API sichtbar ist, bevorzugt Integration Tests als externe Tests unter `src-tauri/tests/*.rs` schreiben. In-Modul-Tests mit `#[cfg(test)]` nur für private Helper, enge Edge-Cases oder wenn bewusst Implementierungsdetails getestet werden.
