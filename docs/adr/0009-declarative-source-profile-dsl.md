# Replace v1 source profiles with a declarative Source Profile DSL

Job Radar replaces the v1 Source Profile model with a declarative JSON Source Profile DSL that is compiled into a typed Execution Plan before runtime execution. ATS and career-site behavior is described through reusable generic capabilities such as fetch, parse, select, extract, transform, pagination, finite Strategy fallback, browser mode, and diagnostics; individual ATS profiles must not require profile-specific Rust adapters.

## Schema-v3 activation amendment

Schema version 3 is the sole active Source/Profile document model. It uses `detection`, `discovery`, and `detail`; every complete Discovery or Detail Strategy Set requires one closed typed Policy: `{ "type": "first_accepted" }`, `{ "type": "all_required" }`, `{ "type": "at_least", "count": N }`, or `{ "type": "collect_all", "minAccepted": N }`, where each authored cardinality is positive and no greater than the final merged Strategy cardinality. A profile-selected Source may author typed keyed `accessPaths` fragments directly at its root. The authoritative `compile_source` boundary merges these fragments with the Base Source Profile, validates the complete Effective Source Profile and Source Config, records provenance, and returns either a complete immutable typed plan or Diagnostics-only rejection.

There is no compatibility layer, automatic migration, version dispatcher, old phase alias, Source Override wrapper, or parallel compiler/fingerprint route. Existing schema-v2 app-data files fail strict loading and must be recreated manually. Source/Profile documents remain filesystem JSON; no SQLite migration is involved.

The DSL primitives include Source Profile, Base and Effective Source Profile, Access Path, Source-owned Access Path, Source Config, Direct Source Specialization, support metadata, Policy, Strategy, Fetch, Pagination, Parse, Select, Filter, Capture, Match, Extract, Cardinality, Transform, Combine, Template, acceptance checks, and Diagnostic. Profile-DSL Retry, pacing, rate limiting, Retry-After handling, and Bot behavior are unsupported.

Consequences: JSON Schema validates document shape, while the Profile Compiler owns semantic validation, security, boundedness, deterministic specialization, provenance, and execution-plan diagnostics. Built-in and custom profiles use the same DSL and compiler rules; custom profiles may not override built-in profile keys. Browser extraction remains a fetch mode inside the DSL, not a separate profile type.
