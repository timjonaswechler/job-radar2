# Job Radar

Job Radar helps users observe job sources and prepare job discovery across different recruiting platforms and career systems.

## Language

**Quelle**:
A saved, repeatable source configuration that tells Job Radar where and through which adapter job postings may be retrieved or received. A source does not contain search criteria such as keywords, job roles, location, region, or country; those belong to a search request.
_Avoid_: Plattform, Portal, Connector, Ad-hoc-Suche, Suchprofil, Quellentyp

**Quellenkonfiguration**:
The stable access configuration of a source, interpreted by the source's adapter. It does not include search criteria.
_Avoid_: Suchkonfiguration, Suchprofil, Config

**Adapter**:
The technical runtime that knows how to execute a class of source access, such as declarative HTTP, declarative sitemap, declarative browser, or built-in job portal search. An adapter is not the recruiting system itself; system-specific knowledge belongs in a system profile.
_Avoid_: Plugin, Scraper, Crawler, Quellentyp, Recruiting-System

**Systemprofil**:
A saved declarative JSON definition of a recruiting system or career-system family. It contains deterministic detection checks, extraction description, source configuration templates, and source configuration schema. Users and agents can create system profiles without changing Rust source code.
_Avoid_: Adapter, Browserprofil, Heuristik, Firmenadapter, hardcodiertes Portal

**Eingebautes Systemprofil**:
A system profile that is versioned in the repository under `system-profiles/builtin/*.json` and embedded into the application bundle. It is seeded/upserted into the database on startup with `built_in = 1`; the installed app must not depend on loose external built-in files.
_Avoid_: externe Built-in-Datei, nur in der DB gespeichertes Systemwissen

**Custom-Systemprofil**:
A user/runtime system profile stored as JSON in the OS app data directory under `system-profiles/*.json`, next to the local SQLite database. It is loaded after built-ins with `built_in = 0` and may not override a built-in key.
_Avoid_: Repo-Community-Ordner als Laufzeitquelle, Built-in-Override

**Systemerkennung**:
The deterministic process that checks a company URL against active system profiles. A system profile is detected only when all required technical checks pass and evidence can be shown; if multiple profiles pass the result is ambiguous, and if none pass the URL is unsupported.
_Avoid_: Raten, Heuristik, Domain-Mapping, Confidence-Scoring

**Browserbasierte Quelle**:
A source inspected through rendered web pages rather than through a source-specific structured interface. It uses a browser profile to interpret the target website or website family.
_Avoid_: Headless-Quelle, Scraping-Quelle

**Browser-Laufzeit**:
The locally managed browser installation that Job Radar uses to inspect browser-based sources.
_Avoid_: Systembrowser, Headless-Browser, Chromium-Download

**Browserprofil**:
A reusable understanding of a website or website family that tells Job Radar how a browser-based source should be interpreted. It defines the parameters expected from sources that use it.
_Avoid_: Scraping-Regel, Website-Adapter, Plattformtyp

**Profildefinition**:
A declarative description from which Job Radar can register or update a browser profile.
_Avoid_: Browserprofil-Datei, Scraping-Datei, Plugin-Datei

**Arbeitsstatus**:
The lifecycle state that indicates whether a managed item such as a source, profile, or search request is drafted, active, disabled, or invalid.
_Avoid_: Enabled-Flag, Aktiv-Boolean, Quellstatus, Suchanfragenstatus

**Suchanfrage**:
A user-created, saved job-search intent that contains one or more search terms, optional location criteria such as location, region, country, and radius, and selects which saved sources Job Radar should use. A search request has an Arbeitsstatus; active requests may be used for automatic or planned search runs, while disabled requests are skipped there but may still be started manually.
_Avoid_: Quelle, Profil, Suchlauf

**Suchlauf**:
A concrete execution of a search request at a specific time that produces current results rather than versioning the search request. One search run may use multiple selected sources and expose an outcome for those sources; it may be queued, running, completed, completed with errors, failed, or cancelled.
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
A job opportunity found by Job Radar with a title, company, URL, sources, and zero or more locations. Postings with the same company and title are treated as the same job opportunity, even when they are found through multiple sources; when both postings provide locations, overlapping locations are also used to distinguish opportunities.
_Avoid_: Treffer, Quelle, Suchlauf

**Treffer**:
The relationship that says a specific job posting matched a specific search request during a specific search run. The same job posting may be a Treffer for multiple search requests or search runs.
_Avoid_: Stellenanzeige, Quelle
