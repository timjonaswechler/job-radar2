
 Handoff: Source onboarding UI + agent skill discussion

 Next session focus

 Continue the product/design discussion for making source setup easier and eventually agent-assisted.

 The user wants to discuss two related ideas:

 1. Can we integrate a skill/workflow like the old add-job-source-strategy skill?
     - Reference old project skill:
       /Users/tim-jonaswechler/GitHub-Projekte/rhein-main-physik-radar-export/rhein-main-physik-radar/docs/skills/add-job-source-strategy/SKILL.md
     - Do not copy its terminology blindly. The old project used Jobquellen-Strategie; the current Job Radar language uses Adapter, Browserprofil,
       Profildefinition, Quelle, and Suchanfrage.
 2. Can source/browser-profile setup become a guided UI flow?
     - Stepper instead of raw forms only.
     - Helpful examples and generated JSON skeletons.
     - Reset buttons for JSON fields.
     - Long-term AI button to auto-fill source/profile fields from a URL or user description.

 This handoff is for talking/planning further, not necessarily immediate implementation.

 Current domain language to preserve

 Read CONTEXT.md first. Important constraints:

 - Quelle = stable source/access configuration; no search criteria.
 - Quellenkonfiguration = stable access config interpreted by the adapter.
 - Adapter = capability/code boundary that knows how to retrieve or receive job postings.
 - Browserprofil = reusable understanding of a website/website family for browser-based sources.
 - Profildefinition = declarative description from which a browser profile can be registered/updated.
 - Suchanfrage = later model for keywords, job roles, location, region, country, filters.

 Important: Do not put keywords, roles, country, region, or location in source config examples. Those belong to future search requests.

 Existing relevant artifacts

 Read/reference instead of duplicating:

 - CONTEXT.md
 - docs/adr/0001-source-config-as-json-schema.md
 - docs/adr/0002-browser-profiles-from-declarative-profile-definitions.md
 - docs/adr/0003-managed-browser-runtime.md
 - src/lib/api/sources.ts
 - src/lib/api/browser-runtime.ts
 - Current schema: src-tauri/migrations/20260609000000_current_schema.sql

 The backend currently exposes:

 - Browser runtime control: status/install/uninstall/check/progress events.
 - Browser profile CRUD.
 - Source CRUD.

 The source model currently has no source_types. Sources reference adapterKey directly.

 Old skill summary and how it maps conceptually

 Old skill path:

 /Users/tim-jonaswechler/GitHub-Projekte/rhein-main-physik-radar-export/rhein-main-physik-radar/docs/skills/add-job-source-strategy/SKILL.md

 Useful ideas from it:

 - Guide agents through adding a source-like capability without duplicating the technical contract.
 - Prefer stable data over brittle browser parsing:
     1. known ATS/platform
     2. sitemap/JSON/XML/structured data
     3. browser strategy only when needed
     4. selectors as last resort
 - Reuse existing capabilities before adding new ones.
 - Add fixtures/tests, not live web calls.
 - Keep platform-specific knowledge behind the strategy/adapter seam.
 - Do not seed hardcoded runtime defaults.

 Mapping to current Job Radar:

 - Old Jobquellen-Strategie ≈ current Adapter and/or Browserprofil depending on whether code is needed.
 - Old Strategie-Konfiguration ≈ current Quellenkonfiguration / sourceConfig.
 - Old Browser-Strategie ≈ current browser-based adapter + Browserprofil + managed Browser-Laufzeit.
 - Old Suchausführung ≈ future Suchanfrage execution, currently out of scope.

 Do not resurrect Quellentyp as a separate model unless the domain discussion explicitly reverses the recent decision.

 Possible new skill direction

 Potential skill name ideas:

 - add-job-radar-adapter
 - add-job-radar-source-capability
 - add-browser-profile
 - add-job-source

 Recommended split:

 1. Skill for agent/developer work: add or extend an adapter/browser profile capability.
 2. UI for user work: configure a source and browser profile using existing backend commands.

 Possible skill responsibilities:

 - Ask whether an existing adapter/browser profile can express the source.
 - If yes: create/update browser profile and/or source config through existing API/DB/UI path.
 - If no: guide implementation of a new adapter capability.
 - Require fixtures and tests.
 - Keep source execution separate from DB writes: adapter returns candidates/results; ingestion writes later.
 - Keep search criteria out of source config.
 - Document expected source config schema and later search criteria schema.

 Open design question for the next discussion:

 - Where should adapter metadata live long-term?
     - Code-only descriptor?
     - Declarative file next to adapter?
     - Skill-generated manifest?
     - Backend command like list_available_adapters?

 Currently there is no adapter registry/list backend; source UI uses free adapterKey.

 Guided UI / stepper idea

 The user wants configuring a browser profile and a source to be more descriptive than raw JSON forms.

 Potential UX structure:

 ### Stepper: Add Source

 1. Choose adapter
     - Free-text adapter key for now.
     - Later: adapter catalog once backend supports it.
     - Explain what an adapter is.
 2. Choose or create browser profile if adapter is browser-based.
     - Existing profile dropdown.
     - Link/button: create browser profile.
 3. Source identity
     - key, name, description, status.
 4. Source configuration
     - JSON editor with generated skeleton.
     - Examples that contain stable access parameters only, e.g. baseUrl, careerSiteUrl, sitemapUrl.
     - No keywords/location/region/country examples.
     - Reset to generated defaults.
 5. Review
     - Show final source object.
     - Save.

 ### Stepper: Add Browser Profile

 1. Profile identity
     - key, name, description, status.
 2. Website understanding
     - definition JSON with skeleton and examples.
     - URL patterns, extraction hints, profile metadata.
 3. Source config schema
     - JSON schema for stable source parameters.
     - Reset to example skeleton.
 4. Review
     - Save profile.

 AI button idea

 Long-term button examples:

 - AI aus URL vorbereiten
 - Mit KI ausfüllen
 - Profil vorschlagen

 Possible behavior:

 - User enters URL and maybe source description.
 - AI proposes:
     - adapter key
     - browser profile draft
     - source config draft
     - source config schema draft
     - warnings about fragility / missing info
 - UI shows diff/editable fields before saving.
 - Nothing auto-executes arbitrary code.
 - AI output should produce draft status by default, not active.

 Important constraints:

 - Do not let AI introduce search criteria into sourceConfig.
 - Do not let AI choose arbitrary runtime download URLs.
 - AI-generated browser profile definitions remain declarative unless a later adapter-development flow explicitly writes reviewed code.

 UX helper ideas

 For JSON fields:

 - Show field-specific example.
 - Button: Format JSON.
 - Button: Reset to skeleton.
 - Inline parse error.
 - Maybe show schema/helper text beside editor.
 - Save disabled while JSON invalid.

 For sources:

 - If a browser profile is selected, use its sourceConfigSchema to generate a starter skeleton if possible.
 - If no schema, use {}.

 For browser profiles:

 - definitionSchemaVersion defaults to 1.
 - definition defaults to a minimal skeleton.
 - sourceConfigSchema defaults to a minimal object schema.

 Open questions for next agent to ask

 1. Should the next step be to write a new skill first, or prototype the guided UI stepper first?
 2. Should adapter metadata become a backend API (listAvailableAdapters) before the UI stepper?
 3. Should browser profile setup and source setup be one combined flow or two separate flows?
 4. How should AI autofill be triggered and reviewed?
 5. Should AI autofill be a local prompt template/agent workflow first, before adding an in-app button?
 6. What are the minimal adapter/profile examples we can safely show without implying StepStone execution already works?

 Out of scope for the next discussion unless user changes scope

 - Implementing source execution.
 - Implementing search requests.
 - StepStone scraping.
 - Login/session persistence.
 - Arbitrary user-code plugins.
 - Reintroducing source_types.

 Suggested skills

 - grill-with-docs — best fit for the next conversation; challenge terminology and update CONTEXT.md/ADRs if decisions change.
 - write-a-skill — use if the decision is to create a new Job Radar skill based on the old add-job-source-strategy idea.
 - prototype — use if the decision is to explore several guided stepper UI variants before implementation.
 - tdd — use only once implementation starts.
 - handoff — use again if the discussion produces an implementation-ready plan.

 Security / privacy

 No secrets, credentials, or personal data were discussed. The old project path is local only and contains no credentials in the referenced skill. Do not add
 credential/login fields to this slice unless the user explicitly opens that topic.
