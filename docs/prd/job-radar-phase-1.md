# PRD: Job Radar Phase 1 Desktop App

## Problem Statement

The user needs a personal desktop application to track Stellenanzeigen, Bewerbungen, reminders, and recurring job-search activity without running a multi-user hosted product. Existing manual tracking in filterable tables is useful, but the user wants Job Radar to automate the repetitive parts: configuring Suchanfragen, gradually checking selected Jobquellen, deduplicating findings into Stellenanzeigen, keeping irrelevant results out of the normal workflow, and turning promising Stellenanzeigen into editable Bewerbungen.

The system must remain local-first and single-user. It should avoid overbuilding web, mobile, account, or multi-profile features, while keeping enough structure that a later service mode could be added if needed.

## Solution

Build Job Radar as a Tauri v2 desktop application. The React interface lives in `apps/web` as the frontend bundle used by the desktop app. The Tauri shell and Rust backend live in `apps/desktop`. Phase 1 stores all data in local SQLite, accessed from Rust through SQLx, with automatic migrations at app startup and a backup before migrations.

Users configure Jobquellen and Suchanfragen. A Suchanfrage contains an optional Suchbegriff, optional location/radius criteria, selected Jobquellen, Ausschlussbegriffe, and title-focused Trefferregeln. A daily Erinnerung prompts the user to start a Suchlauf. The Suchlauf runs as a long-running job with progress, delay/limit behavior, and cancellation. It works through active, valid Suchanfragen and selected active Jobquellen, using each Quellsystem best-effort. Findings become Fundstellen and are deduplicated conservatively into Stellenanzeigen.

Stellenanzeigen enter a Stellenanzeigen-Inbox where the user can mark them as interesting, keep them for later, hide them, or turn them into Bewerbungen. A Bewerbung always belongs to exactly one Stellenanzeige, and each Stellenanzeige can have at most one Bewerbung. Bewerbungen remain editable and move through a defined status workflow.

## User Stories

1. As a desktop user, I want Job Radar to run locally, so that my Bewerbungen and job-search data stay on my machine.
2. As a desktop user, I want the app to work without accounts or profiles, so that it stays simple for one-person use.
3. As a desktop user, I want my data stored locally in SQLite, so that I do not need to operate a database server.
4. As a desktop user, I want stable IDs for all objects, so that deep links and backups remain reliable.
5. As a desktop user, I want JSON backup and restore, so that I can protect and move my local data.
6. As a desktop user, I want restored data to preserve IDs, so that existing `job-radar://...` links continue to work.
7. As a desktop user, I want technical deep links such as `job-radar://applications/:id`, so that I can link directly to Bewerbungen or Stellenanzeigen.
8. As a desktop user, I want the UI labels in German, so that the app uses my domain language.
9. As a desktop user, I want UI texts centralized in a simple German translation file, so that later i18n remains possible without making phase 1 complex.
10. As a user, I want to configure Jobquellen, so that Job Radar knows where to search.
11. As a user, I want each Jobquelle to belong to a Quellsystem, so that different source types can have different adapters and configuration needs.
12. As a user, I want a Jobquelle to store the configuration its Quellsystem needs, so that sources like feeds, APIs, and career pages can be handled appropriately.
13. As a user, I want Jobquellen to be activated or deactivated, so that I can keep configuration without searching every source.
14. As a user, I want Job Radar to skip inactive Jobquellen during Suchläufe, so that disabled sources are not accidentally searched.
15. As a user, I want skipped inactive Jobquellen to appear as warnings, so that I understand why a selected source was not used.
16. As a user, I want to create Suchanfragen, so that recurring searches can be reused.
17. As a user, I want a Suchanfrage to contain one optional Suchbegriff, so that portal-like Jobquellen can receive a simple search term.
18. As a user, I want a Suchanfrage to be allowed without a Suchbegriff, so that sources can be searched broadly and then filtered locally.
19. As a user, I want a Suchanfrage to select multiple concrete Jobquellen with a combo/multiselect control, so that I control where it runs.
20. As a user, I want a Suchanfrage without selected Jobquellen to be saved but not runnable, so that I can keep drafts.
21. As a user, I want to activate or deactivate Suchanfragen, so that only intentional searches run.
22. As a user, I want only active and valid Suchanfragen to run, so that broad unfinished searches do not accidentally execute.
23. As a user, I want a Suchanfrage to be valid only when it has at least one active Jobquelle and either a Suchbegriff or at least one Trefferregel, so that accidental “search everything” runs are avoided.
24. As a user, I want title-focused Trefferregeln on a Suchanfrage, so that local filtering can refine broad source results.
25. As a user, I want Trefferregeln to support “title contains” and “title does not contain”, so that rules stay simple.
26. As a user, I want all Trefferregeln on a Suchanfrage to match conjunctively, so that the matching behavior is predictable.
27. As a user, I want global Ausschlussbegriffe, so that terms like “Duales Studium” can be excluded across searches.
28. As a user, I want Suchanfrage-specific Ausschlussbegriffe, so that individual searches can exclude additional unwanted roles.
29. As a user, I want Ausschlussbegriffe to match only against titles, so that descriptions or company names do not create too many false exclusions.
30. As a user, I want matching Ausschlussbegriffe to keep results out of the Stellenanzeigen-Inbox, so that irrelevant results do not clutter my workflow.
31. As a user, I want excluded findings to be retained temporarily, so that I can debug why something was excluded.
32. As a user, I want excluded findings retained only for a limited review window, so that the database is not filled with unwanted data.
33. As a user, I want a daily Erinnerung to start or review a Suchlauf, so that search activity stays regular without requiring a background service.
34. As a user, I want to confirm starting a Suchlauf, so that searching is deliberate rather than fully automatic.
35. As a user, I want a Suchlauf to run slowly and respectfully, so that sources are not hit aggressively.
36. As a user, I want delay, limits, retry/backoff, and block-handling configured per Jobquelle, so that different sources can be treated appropriately.
37. As a user, I want a Suchlauf to run as a long-running job, so that the UI does not freeze while sources are checked.
38. As a user, I want to see Suchlauf progress, so that I understand what the app is doing.
39. As a user, I want progress to show current Suchanfrage, current Jobquelle, page/step, found Fundstellen, new Stellenanzeigen, excluded Treffer, errors, and waiting time, so that slow searching feels transparent.
40. As a user, I want to cancel a running Suchlauf, so that I can stop long searches.
41. As a user, I want already found results to remain when a Suchlauf is cancelled, so that useful work is not lost.
42. As a user, I want only one Suchlauf running at a time, so that the app remains simple and gentle on sources.
43. As a user, I want a Suchlauf to be marked as abgebrochen after app restart if it was running when the app closed, so that stale runs are not shown as active.
44. As a user, I want a Suchlauf to finish with errors when only some Jobquellen fail, so that successful results from other sources remain useful.
45. As a user, I want Suchlauf statuses `läuft`, `abgeschlossen`, `abgeschlossen_mit_fehlern`, `abgebrochen`, and `fehlgeschlagen`, so that search history is understandable.
46. As a user, I want a Suchlauf to find concrete Fundstellen, so that Job Radar can track where a job was discovered.
47. As a user, I want a Fundstelle to point to a concrete URL or source result, so that I can inspect the original place later.
48. As a user, I want multiple Fundstellen to point to the same Stellenanzeige, so that duplicates across sources are consolidated.
49. As a user, I want a Stellenanzeige to be deduplicated conservatively, so that distinct roles are not merged accidentally.
50. As a user, I want external ID or canonical URL matches to be treated as reliable identity when available, so that exact duplicates are recognized.
51. As a user, I want fallback deduplication to use company, exact normalized title, primary location/region, and Arbeitsmodell, so that duplicates can still be recognized when external IDs are missing.
52. As a user, I want title normalization to remove only cosmetic differences, so that “Frontend Developer (m/w/d)” and “Frontend Developer” can match without merging different roles.
53. As a user, I want similar but meaningfully different titles to remain separate Stellenanzeigen, so that “Frontend Engineer” and “Senior Frontend Engineer React” are not merged.
54. As a user, I want Arbeitsmodell separate from Ort, so that remote, hybrid, on-site, and unknown are represented cleanly.
55. As a user, I want a Stellenanzeige to store title, company text, primary location/region or unknown, Arbeitsmodell or unknown, status, and main extracted plain-text description, so that it has enough information to review.
56. As a user, I want extracted description text stored as plain text and limited in size, so that useful content is available without storing raw HTML.
57. As a user, I want no raw HTML snapshots stored, so that data stays smaller and less risky.
58. As a user, I want new or undecided Stellenanzeigen in a Stellenanzeigen-Inbox, so that I have a focused review list.
59. As a user, I want the Stellenanzeigen-Inbox to be a filterable table, so that I can work through many postings efficiently.
60. As a user, I want quick actions on Stellenanzeigen, so that I can mark them without opening every detail view.
61. As a user, I want Stellenanzeigen statuses `neu`, `interessant`, `später ansehen`, `ausgeblendet`, and `in bewerbung umgewandelt`, so that the inbox workflow stays simple.
62. As a user, I want to mark a Stellenanzeige as interesting, so that I can come back to it without creating a Bewerbung yet.
63. As a user, I want to mark a Stellenanzeige for later, so that it remains visible but not urgent.
64. As a user, I want to hide a Stellenanzeige, so that irrelevant postings leave my normal workflow.
65. As a user, I want to turn a Stellenanzeige into a Bewerbung, so that I can actively track applying for it.
66. As a user, I want every Bewerbung to belong to exactly one Stellenanzeige, so that applications remain tied to a concrete posting.
67. As a user, I want each Stellenanzeige to have at most one Bewerbung, so that the workflow stays simple.
68. As a user, I want a Bewerbung to remain editable over time, so that I can update status, notes, and dates.
69. As a user, I want Bewerbungen to have statuses `neu`, `unterlagen vorbereiten`, `beworben`, `rückmeldung`, `erstgespräch`, `technisches interview`, `angebot`, `abgelehnt`, `zurückgezogen`, and `archiviert`, so that I can track the application pipeline.
70. As a user, I want a Bewerbung to store notes, so that I can capture personal context.
71. As a user, I want a Bewerbung to store an optional applied date, so that I know when I applied.
72. As a user, I want a Bewerbung to store an optional next reminder date, so that follow-ups are not forgotten.
73. As a user, I want typed Erinnerungen, so that search reminders, follow-ups, interviews, and custom tasks are distinguishable.
74. As a user, I want Erinnerungen to have due times, so that the app can tell me what needs attention.
75. As a user, I want Erinnerungen to be marked done, so that completed prompts do not remain active.
76. As a user, I want automatic database migrations on app startup, so that app updates can evolve the schema without manual DB work.
77. As a user, I want the app to create a database backup before migrations, so that failed migrations do not put my data at unnecessary risk.
78. As a developer, I want a small frontend client module wrapping Tauri invoke calls, so that React components do not depend directly on Tauri details.
79. As a developer, I want the search engine behind a clear service boundary, so that it can later move to a worker or optional service mode if needed.
80. As a developer, I want adapter logic separated by Quellsystem, so that new source types can be added without changing the core model.

## Implementation Decisions

- Product scope is desktop-only for phase 1. Web, mobile, multi-user accounts, and profiles are not phase-one product goals.
- The React UI is kept separate from the Tauri desktop shell. The UI is the frontend bundle for the desktop app, not a standalone web product.
- The desktop shell uses Tauri v2.
- Backend logic runs in the Tauri/Rust side for phase 1.
- Local SQLite is the phase-one data store.
- SQLx is used for SQLite access.
- Database migrations run automatically at app startup.
- The SQLite database file is backed up before migrations run.
- All domain objects use UUIDv7 identifiers.
- JSON backup/restore preserves IDs.
- Technical routes and deep links use English paths, while visible UI labels use German domain language.
- UI text is centralized in a simple German translation file, without adopting a full i18n framework in phase 1.
- React components call a small frontend client module, which wraps Tauri invoke calls.
- Core modules to build:
  - Desktop shell and backend bootstrap
  - Database/migration module
  - Repository layer for SQLite persistence
  - Domain/service layer for Stellenanzeigen, Bewerbungen, Suchanfragen, Jobquellen, Suchläufe, and Erinnerungen
  - Search engine for running Suchläufe
  - Quellsystem adapter interface
  - Deduplication module
  - Trefferregel/Ausschlussbegriff evaluation module
  - Import/export module
  - Deep link routing module
  - Frontend client module
  - German UI text module
- Phase-one persisted areas:
  - Bewerbungen
  - Stellenanzeigen
  - Fundstellen
  - Suchanfragen
  - Jobquellen
  - Suchläufe and Suchlauf history
  - Ausgeschlossene Treffer with limited retention
  - Erinnerungen
- Firma is a text field in phase 1, not a separate entity.
- Contacts, documents, and communication history are not phase-one entities.
- A Suchanfrage contains one optional Suchbegriff, optional location/radius fields, selected Jobquellen, Trefferregeln, and search-specific Ausschlussbegriffe.
- A Suchanfrage can be saved without Jobquellen, but it is not runnable until valid.
- A runnable Suchanfrage must be active, have at least one active selected Jobquelle, and have either a Suchbegriff or at least one Trefferregel.
- Jobquellen can be active or inactive.
- Inactive Jobquellen selected by an active Suchanfrage are skipped with a warning.
- Trefferregeln belong to Suchanfragen.
- Phase-one Trefferregeln are title-focused and support only contains / does-not-contain checks.
- Multiple Trefferregeln on one Suchanfrage are combined with AND semantics.
- Ausschlussbegriffe are the user-facing simple form of exclusion Trefferregeln.
- Ausschlussbegriffe match title only.
- Matching Ausschlussbegriffe keep findings out of the Stellenanzeigen-Inbox.
- Excluded findings are retained only temporarily for review/debugging.
- Suchläufe are started manually after an Erinnerung/confirmation, not fully automatically in the background.
- Suchläufe run inside the Tauri app in phase 1.
- Suchläufe are long-running jobs with progress and cancellation.
- Only one Suchlauf can run at a time in phase 1.
- Already found data remains when a Suchlauf is cancelled.
- Running Suchläufe found at app startup are marked abgebrochen.
- Suchlauf statuses are `läuft`, `abgeschlossen`, `abgeschlossen_mit_fehlern`, `abgebrochen`, and `fehlgeschlagen`.
- Search progress includes current Suchanfrage, current Jobquelle, page/step, found Fundstellen, new Stellenanzeigen, excluded Treffer, errors, and current waiting/delay state.
- Jobquellen may define source-specific Schonregeln such as delay, limits, retry/backoff, and stop-on-blocking behavior.
- The retrieval strategy depends on Jobquelle/Quellsystem. A source may use HTTP/feed/API access or headless-browser style access if its adapter needs it.
- Phase 1 does not require prioritizing specific Quellsysteme yet; the architecture should allow adding adapters later.
- A Fundstelle belongs to a Jobquelle and represents a concrete found URL/result.
- A Stellenanzeige can have many Fundstellen.
- A Stellenanzeige holds the main extracted plain-text description, limited in size. Raw HTML is not stored.
- Deduplication is conservative. External ID or canonical URL may identify exact matches when available. Without that, practical identity is company, exact normalized title, primary location/region, and Arbeitsmodell.
- Title normalization removes cosmetic differences only: gender suffixes, punctuation, whitespace, and casing.
- Similar but meaningfully different titles are treated as different Stellenanzeigen.
- Arbeitsmodell is separate from Ort/Region.
- Stellenanzeigen statuses are `neu`, `interessant`, `später ansehen`, `ausgeblendet`, and `in bewerbung umgewandelt`.
- The Stellenanzeigen-Inbox is a filterable table with quick actions.
- A Bewerbung always belongs to exactly one Stellenanzeige.
- A Stellenanzeige can have at most one Bewerbung.
- Bewerbungen statuses are `neu`, `unterlagen vorbereiten`, `beworben`, `rückmeldung`, `erstgespräch`, `technisches interview`, `angebot`, `abgelehnt`, `zurückgezogen`, and `archiviert`.
- Erinnerungen have types, due times, and done status.

## Testing Decisions

- Tests should verify external behavior and domain rules, not implementation details.
- The deduplication module should have focused unit tests covering:
  - exact external ID / canonical URL matches
  - company + exact normalized title + location/region + Arbeitsmodell matches
  - cosmetic title normalization
  - similar-but-different titles remaining separate
  - remote/hybrid/on-site distinctions
- The Trefferregel/Ausschlussbegriff evaluator should have unit tests covering:
  - title contains
  - title does not contain
  - AND combination of multiple rules
  - global and Suchanfrage-specific Ausschlussbegriffe
  - title-only exclusion behavior
- The Suchanfrage validity logic should have unit tests covering:
  - inactive search query
  - no selected Jobquelle
  - selected inactive Jobquelle
  - empty Suchbegriff with no Trefferregeln
  - empty Suchbegriff with Trefferregeln
- The Suchlauf engine should have tests around:
  - only one run active at a time
  - partial success with per-Jobquelle errors
  - cancellation preserving found data
  - startup recovery marking stale running runs as abgebrochen
  - progress events emitted for user-visible state
- The repository/database layer should have integration tests against SQLite covering:
  - migrations from empty database
  - UUIDv7 persistence
  - relationships between Stellenanzeigen, Fundstellen, Bewerbungen, Suchanfragen, Jobquellen, Suchläufe, and Erinnerungen
- The JSON import/export module should have round-trip tests covering:
  - ID preservation
  - restore of linked objects
  - deep-link stability assumptions
- The frontend client should be tested at its boundary with mocked Tauri invoke calls, so UI components can be tested without a live desktop backend.
- UI tests should focus on user-visible flows:
  - review Stellenanzeigen-Inbox
  - convert Stellenanzeige to Bewerbung
  - start/cancel Suchlauf
  - manage Suchanfragen and Jobquellen
  - complete Erinnerungen

## Out of Scope

- Standalone web product.
- Mobile app.
- Multi-user support.
- Multiple profiles.
- Hosted backend/service mode in phase 1.
- Authentication and authorization.
- Login-required Jobquellen and cookie/session handling.
- Prioritizing exact phase-one Quellsysteme.
- Full plugin marketplace or external adapter packaging.
- Complex boolean query language for Suchbegriffe.
- Regex, starts-with, equals, or grouped Trefferregeln.
- Fuzzy deduplication of similar job titles.
- Separate Firma entity.
- Contacts, documents, and communication history as first-class entities.
- Full calendar/task management system.
- CSV export.
- Raw HTML archiving.
- Automatic background service that runs when the app is closed.

## Further Notes

- Current domain glossary lives in `CONTEXT.md` and should remain the source of truth for domain language.
- Existing ADRs record the phase-one decisions for SQLite, UUIDv7, SQLx, and separating React UI from the Tauri desktop shell.
- A later service mode remains conceptually possible, but phase 1 should not pay that complexity cost upfront.
- The app should prefer avoiding false merges over aggressively removing every duplicate-looking Stellenanzeige.
- The user wants searching to be intentionally slow and transparent to reduce scraping-related issues.
