# Declarative Source Profile DSL

## Problem Statement

Job Radar needs to describe ATS and career-site integrations entirely as JSON so built-in profiles can be changed quickly and, later, a user with an agent can create the Source Profiles and Sources they need. The removed v1 Source Profile model had grown by adding portal-specific schema branches and executor special cases. Each new real-world ATS variant exposed another missing feature, which made the JSON Schema larger, harder to understand, and harder for agents or users to author safely.

The removed v1 model also split execution capability across specialized declarative adapters, such as endpoint inventory, sitemap inventory, browser inventory, and posting detail extraction. That made the access model harder to reason about: the removed adapter key decided execution shape instead of the profile's declared capabilities. It also prevented a single Source Profile from naturally combining HTTP, browser, sitemap, XML, JSON, HTML, fallback, transform, and validation behavior.

Job Radar is not in production yet. There is no requirement to preserve the v1 profile format, v1 schemas, or v1 executor compatibility. The correct product direction is a hard replacement: define a declarative Profile DSL, compile JSON into an execution plan, and remove v1 concepts that would otherwise become legacy baggage.

## Solution

Job Radar will replace the v1 Source Profile model with a declarative JSON Profile DSL. A Source Profile will describe reusable ATS or career-site knowledge through generic capabilities rather than profile-specific Rust code. A Source can select one reusable profile Access Path, apply controlled Source Overrides, or define a Source-owned Access Path inline when no reusable profile fits.

The DSL will be validated in layers:

1. JSON Schema validation checks document shape.
2. A Profile Compiler validates semantics and produces a typed Execution Plan.
3. The declarative runtime executes the Execution Plan.
4. Fixture and smoke-test tooling validate real extraction behavior.

The runtime will have one declarative profile execution path. It will not route profiles through multiple declarative adapter keys. Capabilities inside each Access Path determine how execution works.

The new DSL will use `postingDiscovery` for source-wide discovery of available postings and `postingDetail` for lazy loading of details for one concrete posting source occurrence. Both steps share the same capability modules where possible, but they have different outputs and runtime semantics.

## DSL Primitives and Behavior

The Profile DSL is built from a small set of generic primitives. A new ATS should be modeled by composing these primitives, not by adding ATS-specific Rust code or portal-specific schema branches.

### Profile document primitives

- **Source Profile** describes reusable source knowledge: detection, profile-level support metadata, Source Config requirements, and one or more Access Paths.
- **Access Path** is a named reusable execution variant inside a Source Profile. It owns `postingDiscovery` and optionally `postingDetail` strategies. It does not choose an adapter. It may declare Access Path-specific limitations or known issues, but it does not replace the Source Profile's required support level.
- **Source-owned Access Path** is the same Access Path shape stored inline on one Source when no reusable profile fits. It is not reusable and is not used during detection. Its robustness is described by Source-level support metadata, not by reusable profile support metadata.
- **Source Config** provides stable source access values. It is validated by profile-level and Access Path-level Source Config schemas.
- **Source Overrides** are controlled structured behavior changes applied to a selected profile Access Path before compilation. They patch allowed DSL behavior, not the Source Config contract.
- **Support metadata** declares `verified`, `best_effort`, `experimental`, or `unsupported`, plus known issues and validation evidence where applicable. Reusable Source Profiles declare this as `support.level`; Sources with Source-owned Access Paths declare this as `sourceSupport.level`.

### Execution primitives

- **Strategy** is the atomic executable branch inside `postingDiscovery`, `postingDetail`, or detection. Strategies have stable keys, run in declared order, and may act as fallbacks. A strategy succeeds only when all required sub-primitives succeed and acceptance checks pass.
- **Fetch** retrieves one document or rendered page. It supports `http` and `browser` modes. HTTP fetch describes method, URL, public headers, body, and timeout. Browser fetch describes URL, waits, and bounded interactions. Fetch never owns pagination; it represents one request or one browser retrieval.
- **Pagination** belongs to a `postingDiscovery` strategy and describes how repeated fetches are generated and bounded. It supports finite page, offset/limit, cursor, sitemap-style, or equivalent strategies as generic capabilities. Every pagination strategy must have an explicit stop condition such as max pages, max items, max URLs, max depth, missing cursor, or total count.
- **Parse** turns the fetched response into a typed document shape: JSON, XML, HTML, text, or a future supported parse type. Parse errors stop the current strategy and produce semantic diagnostics.
- **Select** chooses the item or items to process from the parsed document. Examples include JSONPath arrays, XML elements/text, CSS selectors, sitemap URLs, or a direct document. `postingDiscovery` usually selects many posting candidates. `postingDetail` may select one direct document or select a collection and then match exactly one item.
- **Where / filter** optionally keeps or rejects selected items before extraction. It is used for bounded filtering such as URL regex checks or non-empty field checks. It must not encode user Search Request criteria in this PRD version.
- **Capture** extracts named values from text using regex named groups. Named capture groups become capture variables. Unnamed groups are not part of the DSL contract. Captures may be produced during detection or per selected posting item.
- **Match** identifies exactly one item in a fetched detail collection for a concrete posting. It compares extracted item values with rendered values such as `postingMeta.jobId`. Zero matches and multiple matches are semantic errors.
- **Extract** reads field values from the current context. Sources include JSONPath, XML text/element, CSS text, CSS attribute, capture, item field, Source Config, postingMeta, and template composition.
- **Cardinality** defines how many values an extraction expects. `one` requires exactly one value. `first` requires at least one value and uses the first non-empty value. `optional` allows zero or one. `all` returns an ordered list of zero or more values.
- **Transform** applies explicit ordered value transformations. Examples include trim, normalize whitespace, HTML-to-text, URL decode, slug-to-title, split, join, dedupe, regex replace, and type/string conversion. Transform logic must be visible in `transforms[]`, not hidden in template pipes.
- **Combine** builds one field from multiple extraction parts in declared order. Parts are required by default and may be marked optional. The final value uses an explicit join string and then optional final transforms.
- **Template** performs string composition with known variable namespaces only. It may compose URLs or constant strings such as company names. It must not contain transform pipes or arbitrary expression logic.
- **Validate / acceptWhen** checks whether a strategy result is good enough to count as success. Examples include required fields present, minimum description length, non-empty candidate list, maximum error ratio, or URL validity. Failed acceptance checks produce diagnostics and allow fallback to the next strategy.
- **Diagnostic** is a structured machine-readable result from validation, compilation, or runtime execution. Diagnostics have at least a code, message, severity, path, and strategy key where applicable. They are the feedback channel for UI and agents.

### Step behavior

`postingDiscovery` strategies discover source-wide posting candidates. They return at least `title`, `company`, and `url`; they may return `locations`, `postingMeta`, and `descriptionText` only when description text is already present in the discovery response. `postingDiscovery` must not perform one detail fetch per candidate to fill detail fields.

`postingDetail` strategies run lazily for one concrete posting source occurrence. They may fetch the posting URL directly, fetch an API detail document, or fetch a collection/feed and match one item using postingMeta. The minimum required detail field for this PRD is `descriptionText`.

Detection strategies produce a Source Proposal rather than executing a Source. Detection may use input URL checks, HTTP checks, HTML/script/network checks, and bounded browser probes. Detection captures may feed Source Config proposals and key/name candidates.

### Security and boundedness behavior

All primitives must be safe for user- and agent-authored JSON. Profiles must not contain secrets, credentials, authorization headers, cookies, session tokens, arbitrary JavaScript, login flows, CAPTCHA bypass, unbounded loops, or unbounded crawling. Every network, pagination, browser, and fallback primitive must have explicit bounds and timeouts.

## User Stories

1. As a Job Radar user, I want an ATS integration to be described by JSON, so that I can use a profile without waiting for a profile-specific code adapter.
2. As a Job Radar user, I want an agent to generate a Source Profile for my required career site, so that I can add sources that are not built into the app.
3. As a Job Radar user, I want an agent to generate a Source from a detected profile, so that I do not need to manually understand ATS host, tenant, site, or board identifiers.
4. As a Job Radar user, I want a Source to override small profile differences, so that one tenant variant does not require copying an entire Source Profile.
5. As a Job Radar user, I want invalid Sources to produce clear diagnostics, so that I know what must be fixed.
6. As a Job Radar user, I want partial source failures to affect only the failing Source Run, so that other valid Sources still produce postings.
7. As a Job Radar user, I want unsupported detected portals to be explained honestly, so that I know when Job Radar recognized a system but cannot extract it.
8. As a Job Radar user, I want built-in profiles to declare their support level, so that I can understand whether a profile is verified, best-effort, experimental, or unsupported.
9. As a Job Radar user, I want custom profiles to follow the same rules as built-ins, so that custom profiles behave predictably.
10. As a Job Radar user, I want built-in profiles not to be silently overridden by custom profiles, so that app behavior remains reproducible.
11. As a Job Radar user, I want source-specific one-off extraction to remain possible, so that individual career pages can still be handled when no reusable profile fits.
12. As a Job Radar user, I want browser-based extraction to work inside the same declarative model, so that JavaScript-rendered career pages can be handled without a separate profile type.
13. As a Job Radar user, I want profile execution to be bounded, so that a bad profile cannot crawl indefinitely, click forever, or hang the app.
14. As a Job Radar user, I want profiles not to contain secrets, so that generated or shared profile JSON is safe.
15. As a Job Radar user, I want Search Requests to keep search criteria outside Source Config, so that Sources remain stable reusable access definitions.
16. As a Job Radar user, I want Job Radar to discover postings source-wide and filter locally for now, so that the first DSL version stays focused and reliable.
17. As a profile author, I want one generic fetch model for posting discovery and posting detail, so that HTTP, POST bodies, headers, timeouts, and browser fetching are described consistently.
18. As a profile author, I want fallback strategies, so that a profile can try an API first and browser HTML second without profile-specific code.
19. As a profile author, I want extraction transforms to be explicit pipelines, so that profile behavior is visible and testable.
20. As a profile author, I want templates only for string composition, so that transform logic is not hidden in template pipes.
21. As a profile author, I want named captures from regexes, so that detection and extraction can carry multiple stable values such as tenant, site, host, title, or external path.
22. As a profile author, I want extraction cardinality to be explicit, so that the runtime can distinguish exactly one value, first matching value, optional value, and all values.
23. As a profile author, I want to combine multiple fields in order, so that description text can be assembled from multiple API or XML fields.
24. As a profile author, I want missing combined parts to fail by default unless marked optional, so that data loss is not hidden.
25. As a profile author, I want locations to normalize into ordered de-duplicated strings, so that posting output is stable.
26. As a profile author, I want pagination as a strategy capability, so that a single fetch describes one request and pagination describes repeated bounded requests.
27. As a profile author, I want posting detail collection matching, so that XML or JSON feeds containing many jobs can be used to load details for one selected posting.
28. As a profile author, I want stable Access Path and Strategy keys, so that diagnostics, overrides, fixtures, and source references remain stable.
29. As a profile author, I want profile-level and Access Path-level Source Config schemas, so that common configuration and path-specific configuration can be expressed separately.
30. As a profile author, I want Source Overrides to be validated structurally, so that a Source can adapt profile behavior without changing the Source Config contract.
31. As a profile author, I want the final profile-plus-source plan compiled before execution, so that semantic errors are found early.
32. As an agent, I want compiler diagnostics in machine-readable form, so that I can iteratively repair generated profiles.
33. As an agent, I want smoke-test diagnostics in machine-readable form, so that I can use real extraction failures as feedback.
34. As an agent, I want newly generated profiles to default to experimental, so that unproven profiles are not presented as verified.
35. As an agent, I want generated profiles to become ready only after schema, compiler, and test validation, so that users are not given untested profile JSON as robust.
36. As a developer, I want the v1 format removed rather than migrated, so that the codebase does not carry pre-production legacy compatibility.
37. As a developer, I want the schema split into capability modules, so that each DSL area has a clear responsibility.
38. As a developer, I want Workday to be expressible without profile-specific Rust code, so that the DSL proves it can model complex API-heavy ATS behavior.
39. As a developer, I want SAP SuccessFactors to be expressible without profile-specific Rust code, so that the DSL proves it can model sitemap, XML, HTML, fallback, and browser-heavy ATS behavior.
40. As a developer, I want Greenhouse or Personio to be expressible without profile-specific Rust code, so that simple API/XML profiles remain easy.
41. As a developer, I want fixture tests for verified profiles, so that profile behavior is deterministic in CI.
42. As a developer, I want live smoke tests to be optional/manual or periodic, so that external site flakiness does not break normal CI.
43. As a developer, I want runtime errors to have semantic codes, so that UI, logs, and agents can act on failures instead of parsing strings.
44. As a developer, I want Source validation separate from Profile validation, so that a valid profile and invalid source config are reported correctly.
45. As a developer, I want a Source Proposal from detection, so that detection outputs an actionable source setup rather than only a profile match.

## Implementation Decisions

- ATS and career-site behavior must be describable through JSON profiles and Sources. A single ATS such as Workday, Personio, SuccessFactors, Greenhouse, or a custom career page must not require profile-specific Rust code.
- New generic capabilities may require Rust implementation. Once added, a capability must be reusable across profiles rather than tied to one ATS.
- The v1 Source Profile format is replaced with no compatibility layer, no automatic migration, no v1/v2 parallel runtime, and no legacy warnings for old profile JSON.
- There is one declarative profile runtime. Profile execution is not selected by the removed v1 `adapterKey` concept.
- The v1 `adapterKey` field is removed from Source Profiles and Source-owned Access Paths. The selected profile, selected Access Path, Source Config, Source Overrides, and compiled Execution Plan determine execution.
- `Access Path` remains a core concept. It is a selectable reusable variant within a Source Profile and can define Source Config requirements, `postingDiscovery`, `postingDetail`, and Access Path-specific limitations. It does not define or replace the reusable Source Profile's required `support.level`.
- A concrete Source selects exactly one reusable profile Access Path or contains exactly one Source-owned Access Path. It cannot do both.
- A Source-owned Access Path is an inline Access Path stored on one Source. It uses the same DSL capabilities as profile Access Paths, is not reusable, and is not considered during profile detection.
- Source Overrides apply only when a Source selects a reusable profile Access Path. A Source-owned Access Path is edited directly instead of overridden.
- Source Overrides are structured JSON overlays for allowed behavior areas such as fetch, select, extract, transforms, and validation thresholds. They are not free string-path patches.
- The compiler validates Source Overrides by compiling the final effective Execution Plan. Overrides cannot change the selected profile, selected Access Path, support level of the profile, or Source Config schema.
- Source Config remains stable access configuration. It must not contain search criteria such as keyword, role, location preference, country, radius, include rules, or exclude rules.
- Source Config and Source Overrides are separate concepts. Source Config is normal per-source configuration; Source Overrides are exceptional behavior changes.
- Profile-level Source Config schema and Access Path-level Source Config schema are both allowed. The effective Source Config schema combines profile common fields with Access Path-specific fields.
- Access Path Source Config schema may add fields but must not redefine profile-level fields in this first version.
- `postingDiscovery` replaces the v1 term `inventory` in the new DSL. It means source-wide discovery of available postings.
- `postingDiscovery` runs during Search Runs. It discovers available postings from the Source and does not encode user search criteria. Job Radar applies match and exclusion rules after discovery.
- `postingDetail` remains a separate lazy step for loading detail fields for one concrete persisted posting source occurrence.
- `postingDiscovery` may output `descriptionText` only when the text is already available in the discovery response. It must not fan out to every detail page just to populate descriptions.
- The required normalized posting discovery output is `title`, `company`, and `url`.
- Optional posting discovery outputs are `locations`, `postingMeta`, and `descriptionText` when already available without detail-page fanout.
- `postingDetail` must support `descriptionText` extraction. The model may allow additional canonical detail fields later, but they are not required for this PRD.
- `postingMeta` remains hidden technical metadata stored per posting source occurrence. It is used to re-identify or load the source-specific posting later.
- `postingMeta` must not become a dumping ground for user-facing metadata. Department, employment type, remote mode, salary, posted date, and deadlines must become explicit canonical fields if needed later.
- `postingMeta.jobId` remains a generic source-local re-identifier. Vendor raw names stay inside extraction rules.
- Both `postingDiscovery` and `postingDetail` use strategies. Strategies are ordered and may act as fallbacks.
- Every strategy has a stable key unique within its parent step.
- Every Access Path has a stable key unique within its Source Profile.
- A strategy succeeds only when fetch, parse, select/match, extraction, transforms, and acceptance validation succeed.
- Fallback execution must preserve diagnostics from failed strategies instead of hiding them.
- Fetch is a shared capability used by both `postingDiscovery` and `postingDetail`.
- Fetch supports HTTP mode and browser mode.
- HTTP fetch supports method, URL, public headers, body, and timeout.
- Browser fetch supports bounded waits and bounded interactions such as waiting for selectors, clicking if visible, and clicking up to a maximum count.
- Arbitrary JavaScript execution, inline scripts, eval-like behavior, arbitrary DOM mutation, login flows, and CAPTCHA bypass are prohibited in profiles.
- Profiles and Sources must not contain secrets or credentials.
- Public request headers such as accept, content-type, user-agent, x-requested-with, and referer may be allowed when justified.
- Auth- or secret-like headers such as authorization, cookie, set-cookie, x-api-key, and proxy-authorization are prohibited.
- Request bodies must also be checked for obvious secret-like fields such as password, token, apiKey, auth, session, or credential.
- Static technical request body parameters are allowed when they are public API parameters and not user search criteria.
- Search Request criteria mapping into portal query parameters is out of scope for this PRD, but the DSL must not make that future extension impossible.
- Pagination is a strategy capability, not part of fetch itself. Fetch describes one request; pagination describes how repeated bounded requests are produced and combined.
- Pagination is supported for `postingDiscovery` only in this PRD version.
- `postingDetail` may load one collection document and match a single item, but it does not paginate through detail collections in this PRD version.
- `postingDetail` collection matching must yield exactly one item. Zero matches and multiple matches are semantic errors.
- Detection is declarative JSON and produces a Source Proposal, not just a profile match.
- Detection output includes the recommended profile key, Access Path key, Source Config proposal, key/name candidates, captures, evidence, support level, and diagnostics.
- Detection may use capabilities such as input URL regex, HTTP fetch checks, HTML contains, HTML regex, script matching, network request matching, and bounded browser probes.
- Regex-based detection and item extraction use named captures. Named capture groups become captures; unnamed capture groups are not part of the DSL contract.
- Transform logic is modeled as explicit ordered transform pipelines.
- Template strings remain only for string composition using known variable namespaces. Templates must not contain transform pipes.
- Field expressions support extraction from sources such as JSONPath, XML element/text, CSS text/attribute, captures, item fields, source config, posting metadata, and templates as appropriate.
- Field extraction declares cardinality explicitly.
- `one` means exactly one value must be found; zero or multiple values are errors.
- `first` means at least one value must be found and the first non-empty value is used.
- `optional` means zero or one value is allowed; multiple values are errors.
- `all` means zero, one, or many values are allowed and the output is a list.
- Multiple field parts can be combined in order with an explicit join string.
- Combined parts are required by default. Missing parts fail unless marked optional.
- Locations normalize to an ordered list of non-empty strings, trim whitespace, remove duplicates while preserving first-seen order, and do not split on punctuation unless explicitly configured.
- All execution must be bounded. Pagination needs explicit stop rules or limits; browser clicks need maximum counts; waits need timeouts; sitemap recursion needs URL/depth limits; fallback lists are finite; HTTP requests need timeouts.
- Runtime failures use semantic diagnostic codes, not only free text.
- Diagnostics include code, message, path, strategy key when applicable, severity, and enough context for UI and agent feedback.
- Example diagnostic codes include fetch errors, parse errors, selector errors, JSON/XML path errors, missing posting metadata, no or multiple detail matches, empty or too-short descriptions, pagination limit reached, browser wait timeout, browser interaction failure, and fallback exhausted.
- Source Profile documents and Source documents remain authoritative JSON documents, not SQLite-owned domain records.
- Built-in profiles are versioned with the application and embedded in the app bundle.
- Custom profiles live in the app data directory and use the same schema and compiler rules as built-ins.
- Custom profile keys must not collide with built-in profile keys. A collision is a diagnostic and the custom profile is ignored.
- Reusable Source Profiles require profile-level support metadata at `support.level`.
- Access Paths may declare path-specific limitations or known issues, but they do not replace or override `support.level`.
- Sources with Source-owned Access Paths require Source-level support metadata at `sourceSupport.level`.
- Support levels are `verified`, `best_effort`, `experimental`, and `unsupported`.
- `unsupported` is allowed for profiles that can detect a system but do not define an executable Access Path.
- Any executable profile must have at least one Access Path with `postingDiscovery`.
- A verified reusable ATS profile must support both `postingDiscovery` and lazy `postingDetail.descriptionText` and must have fixtures for both.
- `verified` requires fixture evidence. Live smoke success alone is not enough.
- `best_effort` may exist without full fixture coverage but must document known limitations and compile successfully.
- Newly agent-authored profiles default to `experimental` unless validation evidence justifies a higher level.
- Sources keep a user-controlled `status` with only `draft`, `active`, and `disabled`.
- `invalid` is not a persisted Source status. Validity is a derived `validationState` from schema, registry, and compiler diagnostics.
- Search Runs execute only active and valid Sources.
- A Search Request may reference draft or disabled Sources, but runtime outcomes must report skipped or failed source-level outcomes clearly.
- Multi-source Search Runs keep per-source outcomes and can complete with errors when only some Sources fail.
- The JSON Schema is physically modular. Source Profile and Source schema entrypoints reference capability modules for common, fetch, parse, select, extract, transform, pagination, strategy, support, overrides, diagnostics, and related definitions.
- Capability schema modules are reused by both `postingDiscovery` and `postingDetail` where semantics overlap.
- The PRD is the primary specification for this DSL effort. Additional DSL documentation files are intentionally avoided for now to prevent multiple sources of truth.
- The PRD defines required concepts and semantics. Exact JSON property names may be finalized during implementation as long as the documented concepts and canonical names are preserved.
- Canonical names for this effort include Source, Source Profile, Access Path, Source Config, Source Overrides, Source-owned Access Path, Profile DSL, Profile Compiler, Execution Plan, Capability, Strategy, `postingDiscovery`, `postingDetail`, `postingMeta`, `support.level`, `sourceSupport.level`, and `validationState`.

### Removal Scope

Because this is a pre-production hard cut, implementation must remove or replace v1 structures rather than preserving compatibility. The removal scope includes:

- the v1 Source Profile JSON format;
- the v1 Source JSON shape where it depends on `adapterKey` or persistent `invalid` status;
- v1 monolithic Source/Profile schemas;
- v1 source registry semantic validation that is tied to the old `inventory` and `postingDetail` shape;
- specialized declarative runtime routing based on endpoint, sitemap, browser inventory, and posting detail adapter keys;
- v1 built-in profile JSON documents rewritten in the new DSL;
- v1 tests that assert old profile shape, old adapter keys, or old inventory terminology;
- old source registry model documentation superseded by this PRD and the new ADR.

Reusable implementation pieces may be kept when they fit the new architecture, such as generic template rendering without pipes, HTML normalization, managed browser infrastructure, HTTP clients, XML/JSON parsing helpers, and existing persistence for job postings.

## Testing Decisions

- Tests should verify behavior at the highest useful seam: schema/registry load, Profile Compiler, Source validation, Execution Plan execution, and Search Run source outcomes.
- Tests should prefer external behavior over implementation details. For example, a profile fixture test should assert normalized posting candidates and diagnostics, not private helper calls.
- Profile validation tests cover schema validity, compiler validity, support metadata, forbidden capabilities, bounded execution requirements, forbidden secrets, source config schema merging, and override validation.
- Source validation tests cover selected profile path existence, Source Config validation, Source Overrides, Source-owned Access Paths, derived validation state, and duplicate built-in/custom profile keys.
- Execution tests cover `postingDiscovery` strategies, fallback behavior, semantic diagnostics, pagination limits, fetch modes, parse modes, extraction cardinality, transforms, combine behavior, and location normalization.
- Posting detail tests cover direct detail documents, collection matching, missing posting metadata, no match, multiple matches, empty descriptions, and fallback strategy diagnostics.
- Detection tests cover Source Proposal generation, named captures, evidence, access path recommendation, source config proposals, ambiguity, unsupported profiles, and browser-assisted bounded probes where available.
- Search Run tests cover active/valid Sources, draft and disabled source skipping, invalid source failures, per-source outcomes, and completed-with-errors aggregation.
- Verified profiles require deterministic fixture tests. Fixture tests include posting discovery input responses, expected posting candidates, posting detail input responses when supported, expected detail fields, and expected diagnostics.
- Browser-related verified behavior should use deterministic saved rendered HTML where possible. Live browser smoke tests can exist separately but should not be required for normal CI.
- Live smoke tests are optional/manual or periodic. They are useful for detecting external site changes but are not sufficient for `verified` support and should not make normal CI flaky.
- Acceptance must include at least one simple profile, either Greenhouse or Personio, fully represented in the new DSL without profile-specific Rust code.
- Acceptance must include Workday as a complex API-heavy ATS profile represented in the new DSL without profile-specific Rust code.
- Acceptance must include SAP SuccessFactors as a complex sitemap/XML/HTML/fallback-oriented ATS profile represented in the new DSL without profile-specific Rust code.
- The Workday acceptance profile must exercise named captures, HTTP POST discovery, JSON parsing, bounded offset/limit pagination, posting metadata, detail GET, HTML-in-JSON transform, and Source Config from detection.
- The SAP SuccessFactors acceptance profile must exercise sitemap/XML or feed-based discovery, posting metadata, fallback detail strategies, XML or HTML detail extraction, and browser detail as a possible strategy where needed.
- Agent-authored profiles must not be presented as ready unless schema validation, compiler validation, and fixture or smoke diagnostics support that state.

## Out of Scope

- Profile-specific Rust adapters for individual ATS systems.
- v1 profile compatibility.
- Automatic migration of old profile JSON documents.
- Keeping v1 and the new DSL in parallel.
- Arbitrary JavaScript in profiles.
- Login flows, authenticated sources, stored credentials, session cookies, or CAPTCHA bypass.
- Unbounded crawling, unbounded pagination, unbounded waits, or unbounded browser interactions.
- Perfect coverage of every career portal.
- LLM-based production extraction at runtime.
- Mapping Search Request criteria into portal-specific query parameters in this PRD version.
- Search criteria in Source Config.
- Making every possible posting metadata field part of `postingMeta`.
- Requiring live smoke tests in normal CI.
- Creating separate DSL documentation pages beyond this PRD for the initial design.

## Further Notes

The new DSL should be powerful enough to describe ATS behavior entirely through JSON, but it is still intentionally declarative. If a new source requires behavior that cannot be expressed, the preferred change is to add a generic capability to the Profile DSL/runtime, not to add source-specific code.

The design deliberately favors a Profile Compiler and typed Execution Plan because JSON Schema alone is the wrong tool for all semantic validation. Schema files should stay modular and readable; the compiler should own cross-field rules, override application, capability compatibility, security checks, boundedness checks, and execution-plan diagnostics.

The existing managed browser runtime decision remains compatible with this PRD. Browser is treated as a fetch mode inside the DSL, not as a separate source/profile type.

The existing job posting persistence model remains compatible with this PRD. `postingDiscovery` still produces normalized posting candidates, and `postingDetail` still loads additional detail for a concrete posting source occurrence.
