# Job Radar

Job Radar helps users observe job sources, discover job postings across recruiting platforms and career systems, and manage postings as work items.

## Language

Primary architecture and code vocabulary is English. German UI copy may use translated terms later, but Source/Profile DSL work uses the canonical English terms below.

**Source**:
A saved, repeatable source configuration that tells Job Radar which concrete endpoint or entry point job postings may be discovered from. A Source either selects one Access Path from a reusable Source Profile or owns one inline Source-owned Access Path. A Source does not contain search criteria such as keywords, roles, preferred locations, countries, radius, include rules, or exclude rules; those belong to a Search Request.
_Avoid_: Quelle, platform, portal, connector, search profile, source type

**Source Config**:
The stable access configuration of a Source, validated against the selected Source Profile and Access Path. Examples include host, tenant, site, board slug, language, feed URL, sitemap URL, or start URL. Source Config is normal configuration, not behavior override, and it does not contain search criteria.
_Avoid_: Quellenkonfiguration, search config, search profile, source overrides

**Source Overrides**:
Controlled Source-specific behavior changes applied to a selected reusable Source Profile Access Path before compiling the final Execution Plan. Source Overrides are for exceptional differences such as a tenant-specific selector, JSON path, transform, fetch detail, or validation threshold. They do not change the selected profile/path, do not alter the Source Config schema, and do not replace Source Config.
_Avoid_: source config, plugin code, profile fork, arbitrary patch

**Source Profile**:
A reusable declarative understanding of a source class, recruiting system, career-system family, website, or website family. A Source Profile describes how matching Sources can be detected, which stable Source Config they need, which Access Paths they offer, and how postings are discovered and interpreted. A Source Profile is not tied to one individual Source and is not a runtime adapter.
_Avoid_: Quellenprofil, adapter, search profile, heuristic, company adapter, hardcoded portal

**Built-in Source Profile**:
A Source Profile versioned with the application and embedded into the app bundle. A custom profile may not override a built-in profile key.
_Avoid_: loose external built-in file, database-owned built-in profile

**Custom Source Profile**:
A user- or agent-authored Source Profile stored in the app data directory. It uses the same DSL, schema, compiler, and validation rules as built-in profiles. It may not use a key that collides with a built-in Source Profile.
_Avoid_: built-in override, weaker custom model, runtime plugin

**Access Path**:
A selectable profile-owned execution variant that describes how a Source can discover postings and load posting details through the declarative Profile DSL. An Access Path defines Source Config requirements and DSL strategies such as `postingDiscovery` and `postingDetail`; it does not select a runtime adapter.
_Avoid_: Zugriffspfad, adapter key, browser switch, runtime hook

**Selected Access Path**:
The one Access Path selected by a concrete Source. A Source either selects a reusable Source Profile Access Path or owns one inline Source-owned Access Path; it does not combine multiple Access Paths.
_Avoid_: adapter selection, runtime selection, profile mix

**Source-owned Access Path**:
An inline Access Path stored on exactly one Source when no reusable Source Profile fits. It uses the same declarative DSL capabilities as a profile Access Path, is not reusable, and is not considered during profile detection. It may later be promoted into a reusable Source Profile.
_Avoid_: source-specific extraction, Quellenprofil, one-off profile, adapter

**Profile DSL**:
The declarative JSON language used to describe Source Profiles, Access Paths, detection, posting discovery, posting detail loading, fallback strategies, extraction, transforms, bounded browser interactions, diagnostics, and support metadata. The DSL is configuration, not arbitrary code.
_Avoid_: script, plugin API, scraper code, profile-specific Rust

**Profile Compiler**:
The semantic validation and compilation step that turns a concrete Source, its selected Source Profile Access Path or Source-owned Access Path, Source Config, and Source Overrides into a typed Execution Plan. The compiler checks capability compatibility, boundedness, forbidden secrets, override validity, template variables, support metadata, and executable strategy shape.
_Avoid_: JSON Schema only, direct execution, runtime guessing, Rust compiler

**Execution Plan**:
The typed, validated plan produced by the Profile Compiler and executed by the declarative runtime. Runtime execution should use the Execution Plan rather than interpreting raw profile JSON directly.
_Avoid_: raw profile document, adapter config, unvalidated JSON

**Structured Diagnostic**:
A machine-readable issue emitted by schema validation, registry loading, the Profile Compiler, source validation, detection, or runtime execution. A Structured Diagnostic has a category, stable code, human-readable message, severity, JSON Pointer path, optional strategy key, and optional machine-readable details. Diagnostic categories include `schema`, `registry`, `compiler`, `runtime`, and `source_validation`. The `compiler` category means diagnostics emitted while compiling a concrete Source and its selected Source Profile/Access Path/Source Config/Source Overrides into an Execution Plan; it does not refer to the Rust compiler.
_Avoid_: free-form error string only, UI-only copy, Rust compiler diagnostic

**Capability**:
A generic DSL behavior that can be reused across Source Profiles, such as fetch, parse, select, extract, transform, pagination, fallback, browser interaction, validation, or diagnostics. New capabilities may require runtime code, but they must be generic and not tied to one ATS.
_Avoid_: ATS adapter, profile-specific feature, special case

**Strategy**:
A named, ordered execution option within `postingDiscovery` or `postingDetail`. Strategies can act as fallbacks; each strategy must be bounded and must produce diagnostics when it fails.
_Avoid_: hidden fallback, unnamed branch, runtime guess

**Profile Detection**:
The deterministic process that checks a submitted source entry point against valid Source Profiles and produces a Source Proposal. Detection may use HTTP and bounded browser probes, but browser use during detection does not imply that the selected Access Path uses browser fetch.
_Avoid_: guessing, domain mapping, confidence-only scoring

**Source Proposal**:
The actionable result of Profile Detection. It includes the detected profile key, recommended Access Path key, proposed Source Config, key/name candidates, captures, evidence, support level, and diagnostics.
_Avoid_: profile match only, unsupported guess

**postingDiscovery**:
The DSL step that discovers available postings from a Source during Search Runs. It can use API, feed, sitemap, HTML, or browser strategies and returns normalized posting candidates with at least title, company, and URL. It may return locations, postingMeta, and descriptionText only when already available from discovery responses. It must not fetch every detail page just to populate descriptions.
_Avoid_: inventory, crawling, postingDetail, Search Request criteria

**postingDetail**:
The lazy DSL step that loads detail fields for one concrete posting source occurrence. It can use the posting URL, Source Config, and postingMeta and may try multiple fallback strategies. In this DSL version it must support descriptionText extraction; additional canonical detail fields may be added later.
_Avoid_: postingDiscovery, bulk detail fanout, inventory fields

**postingMeta**:
Hidden technical metadata captured during postingDiscovery for one posting source occurrence and used later by postingDetail. It is for source-local re-identifiers or detail-loading helpers such as jobId, externalPath, requisitionId, or detail API IDs. It is not user-facing metadata.
_Avoid_: department, employment type, salary, remote mode, posted date, generic metadata

**Support Level**:
A declared robustness level for reusable profiles and source-owned access. Supported values are `verified`, `best_effort`, `experimental`, and `unsupported`. `verified` requires fixture evidence; `unsupported` may describe detection knowledge without executable Access Paths.
_Avoid_: validity, status, confidence only

**Validation State**:
A derived state indicating whether a Source or profile can currently compile and execute. It is computed from schema, registry, compiler, and source validation diagnostics; it is not a persisted user status.
_Avoid_: source status invalid, profile status

**Source Status**:
The user-controlled lifecycle state of a Source: `draft`, `active`, or `disabled`. `invalid` is not a persisted Source Status; invalidity is a derived Validation State.
_Avoid_: enabled flag, validation state, support level

**Browser Runtime**:
The locally managed browser installation that Job Radar uses for browser fetch and bounded browser probes.
_Avoid_: system browser, profile type, browser profile

**Search Request**:
A user-created, saved job-search intent containing search terms, optional location criteria, and selected Sources. Search criteria belong here, not in Source Config.
_Avoid_: Source, profile, Search Run

**Search Run**:
A concrete execution of a Search Request at a specific time. A Search Run may include multiple Source Runs and may complete with errors when only some Sources fail.
_Avoid_: Search Request, Source, run history

**Source Run**:
The part of a Search Run that executes one selected Source and exposes that Source's outcome. Source Runs make partial failures visible.
_Avoid_: Search Run, Source

**Match Rule**:
The user-defined rule set of a Search Request that decides whether a discovered job posting counts as a match. For source-wide discovery, Job Radar initially applies matching locally after postingDiscovery.
_Avoid_: Source, Source Config, portal query mapping

**Exclusion Rule**:
The user-defined rule set of a Search Request that removes postings from the match list after Match Rules have found them.
_Avoid_: Source, Source Config, anti-pattern

**Job Posting**:
A job opportunity found by Job Radar with a title, company, URL, sources, and zero or more locations. Duplicate detection uses company and title; when both postings provide locations, postings are treated as the same opportunity only when at least one location overlaps.
_Avoid_: match, Source, Search Run

**Job Posting Queue**:
A user-facing workflow slice for persisted Job Postings, derived from posting decision and application states. A queue helps users decide what needs attention next; it is not an additional storage lifecycle state.
_Avoid_: backend status, table filter, Search Request, result list

**Job Posting Inbox**:
The Job Posting Queue for postings that still need a user decision. Read-state indicators can mark rows as unread/read, but they are not independent queues or hard-filtered lifecycle states.
_Avoid_: all unread postings, application status, archive

**Match**:
The relationship that says a specific Job Posting matched a specific Search Request during a specific Search Run. The same Job Posting may be a Match for multiple Search Requests or Search Runs.
_Avoid_: Job Posting, Source
