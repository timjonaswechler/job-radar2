# Replace system-specific adapters with declarative system profiles

Job Radar treats adapters as technical runtimes, not as recruiting-system identities. Recruiting-system knowledge such as deterministic detection checks, extraction paths, source configuration templates, and source configuration schemas lives in database-backed system profiles. Users and agents can create or update non-built-in system profiles as JSON without changing Rust source code.

The built-in adapter registry should therefore contain only generic runtime capabilities such as declarative HTTP, declarative sitemap, declarative browser, and intentionally built-in job portals. Systems such as Greenhouse, Lever, Ashby, Personio, Workday, SuccessFactors, Phenom, or Milch & Zucker Global Jobboard are system profiles, not Rust adapters.

Source detection runs all active system profiles against a submitted URL and returns evidence. A profile is detected only when all required checks pass. Multiple passing profiles are ambiguous. No passing profile is unsupported. Domain-only mappings, company-specific adapter code, and confidence scoring are not acceptable substitutes for required technical evidence.

Built-in job portals remain separate from Systemprofile because they are not employer career-system profiles. StepStone is represented as a Browserprofil-backed Quelle executed by the generic `declarative_browser_inventory` adapter; Indeed remains a query-parameterized built-in portal integration until it is migrated behind a Browserprofil.
