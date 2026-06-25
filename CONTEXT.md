# Job Radar

Job Radar helps users observe job sources and prepare job discovery across different recruiting platforms and career systems.

## Language

**Quelle**:
A saved, repeatable source configuration that tells Job Radar which concrete endpoint or entry point job postings may be retrieved or received from. A source may use a reusable profile and one selected access path from that profile or, as a fallback, source-specific extraction. A source does not contain search criteria such as keywords, job roles, location, region, or country; those belong to a search request.
_Avoid_: Plattform, Portal, Connector, Ad-hoc-Suche, Suchprofil, Quellentyp

**Quellenkonfiguration**:
The stable access configuration of a source, interpreted by the source's adapter. It does not include search criteria.
_Avoid_: Suchkonfiguration, Suchprofil, Config

**Zugriffspfad**:
The profile-owned or source-specific execution description that tells Job Radar which technical access class retrieves source data, such as HTTP endpoint inventory, sitemap inventory, browser-rendered inventory, or query-parameterized portal access. A reusable source profile may offer multiple access paths. For sources that use a reusable profile, each allowed access path belongs to that profile; for sources with source-specific extraction, the access path belongs to that source-specific extraction.
_Avoid_: Quelle, Quellenkonfiguration, Suchanfrage, Runtime-Haken

**Gewählter Zugriffspfad**:
The one access path selected by a concrete source from the access paths allowed by its reusable source profile. A source may not freely choose a runtime outside its profile; it only selects which profile-defined access path is used for this specific endpoint.
_Avoid_: Runtime-Haken, Browser-Schalter, Adapterauswahl

**Quellenspezifische Extraktion**:
An interpretation rule set that belongs to exactly one source because no reusable profile describes that website or website family. It may explain how source data is accessed and how job postings and their fields are read for that source, but it is not a reusable profile and does not contain search criteria.
_Avoid_: Quellenprofil, Browserprofil, Systemprofil, Suchprofil, Quellenkonfiguration

**Adapter**:
The technical runtime that knows how to execute a class of source access, such as declarative HTTP, declarative sitemap, declarative browser, or built-in job portal search. An adapter is not the recruiting system itself; reusable source-specific knowledge belongs in source profiles.
_Avoid_: Plugin, Scraper, Crawler, Quellentyp, Recruiting-System

**Quellenprofil**:
A reusable declarative understanding of a source class, recruiting system, career-system family, website, or website family. It describes how matching sources can be detected, which stable source configuration they need, which access path retrieves source data, and how retrieved data is interpreted as job postings. It is not tied to one individual source.
_Avoid_: Adapter, Quelle, Suchprofil, Heuristik, Firmenadapter, hardcodiertes Portal

**Systemprofil** (historisch/abgelöst):
A legacy specialization of Quellenprofil for a recruiting system or career-system family. It is superseded by Quellenprofil documents in the Source Registry (`source-profiles/*.json`). Do not expose Systemprofil as an active public API or storage concept.
_Avoid_: Adapter, Browserprofil, Heuristik, Firmenadapter, hardcodiertes Portal

**Eingebautes Quellenprofil**:
A source profile that is versioned in the repository under `source-profiles/builtin/*.json` and embedded into the application bundle. The installed app must not depend on loose external built-in files.
_Avoid_: externe Built-in-Datei, nur in der DB gespeichertes Profilwissen

**Custom-Quellenprofil**:
A user/runtime source profile stored as JSON in the OS app data directory under `source-profiles/*.json`. It may not override a built-in key.
_Avoid_: Repo-Community-Ordner als Laufzeitquelle, Built-in-Override

**Profilerkennung**:
The deterministic process that checks a submitted source entry point against valid source profiles. A source profile is valid when its profile definition can be loaded and passes runtime validation. A profile is detected only when all required technical checks pass and evidence can be shown. When a profile is detected, profile detection should also recommend the selected access path and source configuration for the concrete source. Profile detection may use direct HTTP checks first and browser-assisted analysis as a second phase, but using a browser during detection does not imply that the detected source must use a browser access path. If multiple profiles pass the result is ambiguous, and if none pass the source entry point is unsupported by reusable profiles.
_Avoid_: Raten, Heuristik, Domain-Mapping, Confidence-Scoring

**Browserbasierte Quelle**:
A source inspected through rendered web pages rather than through a source-specific structured interface. It uses either a Quellenprofil with browser-based access or source-specific extraction to interpret the target website.
_Avoid_: Headless-Quelle, Scraping-Quelle

**Browser-Laufzeit**:
The locally managed browser installation that Job Radar uses to inspect browser-based sources.
_Avoid_: Systembrowser, Headless-Browser, Chromium-Download

**Browserprofil** (historisch/abgelöst):
A legacy specialization of Quellenprofil for a website or website family whose access or extraction depends on rendered web pages. It is superseded by Quellenprofil documents with browser-capable Zugriffspfade, or by source-specific extraction for one-off pages. Do not expose Browserprofil as an active public API or storage concept.
_Avoid_: Scraping-Regel, Website-Adapter, Plattformtyp, quellenspezifische Extraktion

**Profildefinition**:
A declarative JSON description from which Job Radar can use or update a Quellenprofil. Validity is determined at runtime when the profile is loaded or used.
_Avoid_: Scraping-Datei, Plugin-Datei, DB-Profil, Profilstatus, Profil-ID, Profil-Timestamp

**Arbeitsstatus**:
The lifecycle state that indicates whether a managed item such as a source or search request is drafted, active, disabled, or invalid.
_Avoid_: Enabled-Flag, Aktiv-Boolean, Quellstatus, Suchanfragenstatus, Profilstatus

**Suchanfrage**:
A user-created, saved job-search intent that contains one or more search terms, optional location criteria such as location, region, country, and radius, and selects which saved sources Job Radar should use. A search request has an Arbeitsstatus; active requests may be used for automatic or planned search runs, while disabled requests are skipped there but may still be started manually.
_Avoid_: Quelle, Profil, Suchlauf

**Suchlauf**:
A concrete execution of a search request at a specific time that produces current results. Search runs use the current saved request and do not create search-request versions or snapshots. One search run may use multiple selected sources and expose an outcome for those sources; it may be queued, running, completed, completed with errors, failed, or cancelled.
_Avoid_: Suchanfrage, Quelle, Profil, Historie

**Quellenlauf**:
The part of a search run that executes one selected source and exposes that source's outcome. A search run has one source run per selected source so that partial failures remain visible.
_Avoid_: Suchlauf, Quelle

**Trefferregel**:
The user-defined rule set of a search request that decides whether a retrieved job posting counts as a match. Job portals may apply search terms directly through their own search interface; source-inventory sources may require Job Radar to match locally, initially against the job title. A posting matches when at least one positive search term or expression matches.
_Avoid_: Quelle, Suchlauf, Portal-Suche

**Ausschlussregel**:
The user-defined rule set of a search request that removes job postings from the match list after Trefferregeln have found them, regardless of which source produced the posting. Initially, exclusion rules apply to the job title, for example to remove postings containing terms such as CEO or internship. A posting is excluded when at least one exclusion term or expression matches.
_Avoid_: Antipattern, Quelle, Suchlauf

**Stellenanzeige**:
A job opportunity found by Job Radar with a title, company, URL, sources, and zero or more locations. Duplicate detection uses company + title; when both postings provide locations, postings are treated as the same opportunity only when at least one location overlaps.
_Avoid_: Treffer, Quelle, Suchlauf

**Stellenanzeigen-Queue**:
A user-facing workflow slice for persisted job postings, derived from posting decision and application states. A queue helps users decide what needs attention next; it is not an additional storage lifecycle state.
_Avoid_: Backend-Status, Tabellenfilter, Suchanfrage, Trefferliste

**Stellenanzeigen-Inbox**:
The Stellenanzeigen-Queue for postings that still need a user decision. Read-state indicators such as `Neu` and `Gelesen` can mark rows like unread/read mail, but they are not independent queues or hard-filtered lifecycle states.
_Avoid_: Alle ungelesenen Anzeigen, Bewerbungsstatus, Archiv, Suchlauf-Inbox

**Treffer**:
The relationship that says a specific job posting matched a specific search request during a specific search run. The same job posting may be a Treffer for multiple search requests or search runs.
_Avoid_: Stellenanzeige, Quelle
