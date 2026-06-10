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

**Quellstatus**:
The lifecycle state that indicates whether a source-related item is being drafted, usable, intentionally disabled, or invalid.
_Avoid_: Enabled-Flag, Aktiv-Boolean

**Suchanfrage**:
A request for jobs that contains search criteria such as keywords, job roles, location, region, or country and may use one or more saved sources.
_Avoid_: Quelle, Profil
