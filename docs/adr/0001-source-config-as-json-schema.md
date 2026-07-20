# Keep Source Config as profile-declared JSON separate from Direct Source Specialization

Job Radar keeps each Source's stable access configuration as JSON because ATS and career-site configuration varies by profile and should not require database columns or migrations. Source Config contains only stable access values such as host, tenant, site, board slug, language, feed URL, or start URL; it must not contain Search Request criteria such as keywords, roles, preferred locations, countries, radius, include rules, or exclude rules.

Source Profiles and their selected Access Paths declare the Effective Source Config Schema used for validation and UI/agent authoring. Schema-v3 Sources may additionally author typed Direct Source Specialization through root `accessPaths` fragments. Those fragments affect behavior and may specialize the admitted Source Config schema subset, but they are not Source Config and cannot introduce Search Request criteria or profile-only `title` metadata.

The authoritative Profile Compiler materializes and validates the complete Effective Source Profile, then validates the concrete Source Config against its Effective Source Config contract. Source and Source Profile documents remain strict filesystem JSON; this decision adds no SQLite Source persistence.
