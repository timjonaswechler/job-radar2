# T10 — Share byte-preserving HTTP responses and bounded decoding

## Result

Every HTTP acquisition made by a Discovery or Detail Strategy preserves the final response body as bytes, cumulatively admits at most 67,108,864 response-body bytes per Strategy Set invocation, exposes typed transport metadata, and is decoded explicitly and strictly before parsing. Discovery and Detail share one production reqwest client, one deterministic scripted client, one bounded collector, and one decoder; phase-specific HTTP response/client families and hidden transport decoding are removed.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#177 — T9 — Enforce cumulative Strategy Set budgets](https://github.com/timjonaswechler/job-radar2/issues/177).
- Blocking: [#179 — T11a — Establish the Primitive registry and shared parse Primitives](https://github.com/timjonaswechler/job-radar2/issues/179) and [#205 — T14a — Run URL and HTTP Profile Detection through Strategy Sets](https://github.com/timjonaswechler/job-radar2/issues/205).
- Readiness: **Blocked pending #177**. Re-baseline all provisional paths and names against the landed T8/T9 runtime before implementation.
- Open decision: none. The immutable ceiling, tightening rules, decoding precedence, malformed/conflict behavior, and Diagnostic sanitization policy are settled.

Stop for design review rather than widening this ticket if the landed T9 ledger cannot gain one private byte dimension without exposing mutable budget state, if HTTP byte enforcement would require Browser or Detection migration, or if typed Discovery and Detail operations cannot be preserved without a public generic Strategy executor.

## Consumed contracts

- [#166](https://github.com/timjonaswechler/job-radar2/issues/166) / [PRD Implementation Decisions 27–30](../../docs/prd/declarative-profile-strategy-algebra.md#implementation-decisions): cumulative Strategy Set budgets, immutable ceilings, byte-preserving HTTP, and explicit decoding.
- [PRD Strategy Set Runtime module decision](../../docs/prd/declarative-profile-strategy-algebra.md#module-and-interface-decisions): typed public Discovery and Detail operations use one crate-private kernel; HTTP is the external seam while budget, decoding, parsing, acceptance, reducers, and Diagnostics remain in-process.
- [#177/T9](https://github.com/timjonaswechler/job-radar2/issues/177) supplies one private parent/child ledger, immutable/compiled/caller limit precedence, checked admission/debit, exact usage and typed budget completion, typed Cancellation, and immutable typed plan input. T10 extends those owners rather than creating parallel byte state.
- The [Effective Profile Compiler decision](../../docs/prd/declarative-profile-strategy-algebra.md#module-and-interface-decisions) keeps the directly supplied Source authoritative, Source-owned Access Paths distinct, and the runtime dependent only on immutable typed plans.
- [`handoff/issue-166-delivery.md`](../issue-166-delivery.md) owns shared readiness, hard-cut, test, migration, deletion, and PR-evidence rules.

## Current gap

This section describes the blocked ticket's current pre-T8/T9 baseline and is provisional until readiness review.

- `profile_dsl/runtime/posting_discovery.rs` and `posting_detail.rs` separately define fetch request/response/error/client families. Both reqwest implementations call `error_for_status()` and `Response::text()`, losing status responses, repeated/raw headers, final URL, wire bytes, exact byte length, and charset evidence.
- Phase fetch modules render requests and race acquisition with Cancellation; their Browser branches wrap `ProfileBrowserFetchResponse { body: String }` into HTTP-shaped phase responses. Strategy/document modules pass `response.body` directly to JSON/XML/HTML parsers as `&str`.
- `profile_dsl/documents/parse.rs` and `schema/profile-dsl/parse.schema.json` admit optional `parse.charset`, while `profile_dsl/execution_plan/capabilities.rs` only carries the current parse document; there is no supported-label compilation or runtime charset use.
- `profile_dsl/runtime/cancellation.rs` has the obsolete resettable Discovery control. T10 must extend the T9-landed ledger/control/result algebra, not this current type.
- `profile_dsl/runtime/browser.rs` and `source_profile/detection/http.rs` intentionally retain separate `String` response contracts.
- Search Run Discovery, Source Live Check Discovery/Detail, and lazy Detail depend on the phase-specific clients. Their deterministic implementations in `posting_discovery_runtime`, `posting_detail_runtime`, `greenhouse_profile_dsl`, `workday_profile_dsl`, `successfactors_profile_dsl`, `source_live_check`, Search Run support, and posting-service tests return strings.
- `reqwest` already uses `default-features = false` with `rustls-tls` and `stream`; `futures-util` is present. No direct strict character-decoding dependency exists.

The missing capability is therefore not a charset helper alone: bytes and metadata are destroyed at acquisition, oversized bodies cannot be stopped before buffering/decoding, fallback and pagination cannot share byte usage, Cancellation or stream failure can lose prefix usage, and every phase/test fake recreates transport behavior.

## Target delta

### Shared HTTP boundary and bounded collector

Introduce one domain-owned `ProfileHttpClient` responsibility for Discovery and Detail. Exact landed names may differ, but its request contains method, URL, repeated-safe request headers/body, and timeout; every call receives an immutable positive remaining-body allowance plus typed runtime Cancellation context. It returns a report containing one typed outcome and the exact admitted response-byte count.

A successful response preserves numeric status, deterministically normalized header names with repeated value order and non-lossy values, parsed `ContentType`, redirect-final URL, and exact final-response `Vec<u8>`. Outcomes distinguish response, sanitized typed transport failure, response-body limit exhaustion, and Cancellation. HTTP 4xx/5xx are responses, not transport failures: the client does not call `error_for_status()`. One shared phase acquisition rule rejects non-2xx before decoding and emits a sanitized Diagnostic unless T9 already provides an equivalent typed rule.

There are exactly two implementations:

1. the reqwest production implementation captures final metadata and consumes `bytes_stream()`;
2. a deterministic scripted implementation validates ordered expected requests and emits ordered body events: chunks, failure after any prefix, and named pending gates that tests may release or cancel. It also models absent/present/inaccurate `Content-Length`.

Both feed the same bounded collector. The scripted client may not concatenate first, decode, bypass allowance checks, or invent provider behavior. The client observes Cancellation internally and reports its admitted prefix; callers must not race and drop its future in a way that loses usage.

`Content-Length` is only an optimization/evidence hint. A valid value above allowance rejects before body polling. Otherwise the collector uses checked addition, copies at most the allowed prefix, and stops as soon as an excess byte is proven. Collector-owned retained storage and copies never exceed allowance; an upstream reqwest/hyper chunk may already exceed it before inspection, but excess is neither copied nor exposed. Missing, malformed, duplicate-conflicting, compressed, or inaccurate length never weakens streaming enforcement.

Automatic reqwest response decompression remains disabled. The charged bytes are those delivered by the body stream; bounded content-encoding decompression is not added here.

### One cumulative response-byte dimension

Extend the landed T9 canonical limit, usage, ledger, exhaustion, report, and caller-control types with authored/caller `maxResponseBytes` and serialized usage/exhaustion dimension `response_bytes`:

```text
backend ceiling = 67,108,864 bytes
invocation maximum = min(backend ceiling, compiled Strategy Set limit, optional caller limit)
next request allowance = invocation maximum - committed response bytes
```

The ceiling applies separately to each public Discovery or Detail Strategy Set invocation and is shared by every fallback Strategy and every pagination/sitemap HTTP request within it. It never resets per Strategy, request, or page. Base Source Profile, direct Source fragment, complete Source-added Strategy Set, and Source-owned Access Path follow T9 inheritance and tighten-only compilation. Omission means no additional tightening. `null`, zero, unlimited, unknown fields, values above the backend ceiling, and Source attempts to raise/remove inherited limits are invalid rather than clamped. A successful plan contains a mandatory resolved limit.

Caller control is typed and tighten-only. Search Run and lazy Detail invent no product limit. Source Live Check keeps T9's caller behavior and only tightens bytes if an independently existing requirement says so.

Before a request, checked admission must leave positive capacity; otherwise required HTTP work is denied before side effect under T9 terminal rules. Request and byte admission do not partially mutate usage. A started logical request retains T9 request usage even when headers reject an oversized body. Redirect hops remain one logical request; only the final response body is charged.

Every client outcome reports exact admitted bytes, including prefix-then-failure, limit excess, and Cancellation. The sequential kernel commits that report exactly once before translating the outcome. An exact-boundary body followed by EOF succeeds; equality alone is not exhaustion. Once an excess byte is proven, the allowed prefix is charged, no response, decoded value, or phase partial output is exposed, no later work starts, and one typed T9 byte-budget terminal plus its single ordered terminal Diagnostic results; it emits no `fallback_exhausted`. Transport/network/TLS/timeout/header/body-stream failure after a prefix also charges the prefix, remains an ordinary Strategy failure, and may continue `FirstAccepted` fallback while budget remains. Cancellation after a prefix charges the prefix but retains T9 terminal precedence, exposes no partial decode, starts no fallback, and is not persistable Partial Completion.

Browser-rendered strings are neither charged nor relabeled as HTTP byte responses.

### Strict explicit decoding

A private decoder converts a bounded response and optional compiled authored charset into `BoundedDecodedBody { text, canonical encoding, selected_by }`. Parsing receives its text only after complete success.

1. Gather authored charset, recognized UTF-8/UTF-16LE/UTF-16BE BOM, and every parsed HTTP `Content-Type` charset. Resolve labels case-insensitively using WHATWG mappings (for example through `encoding_rs`), canonicalize aliases by encoding identity, and reject unsupported labels and the replacement pseudo-encoding.
2. Validate every present declaration even when a higher-precedence source exists. Malformed or unsupported declarations fail. Different canonical identities produce one stable conflict Diagnostic; equivalent aliases do not.
3. When valid and non-conflicting, select authored `parse.charset`, otherwise BOM, otherwise HTTP charset, otherwise UTF-8. Consume a compatible BOM from decoded text while preserving it in raw bytes.
4. Decode incrementally without replacement. Invalid or incomplete UTF-8/UTF-16 and decoder failure return one Diagnostic and no prefix/text. Lossy conversion and reqwest text decoding are forbidden.
5. Before output allocation/growth, compute the decoder's worst-case UTF-8 expansion from bounded input with checked arithmetic. Overflow or unavailable maximum is deterministic internal runtime failure, not allocation or fallback. Decode in fixed bounded chunks, check each growth, finish explicitly, and expose text only when complete.
6. Decoded expansion is not charged as response bytes. Avoid unnecessary simultaneous clones beyond the intentional raw response plus decoded value.
7. Compile authored labels into supported canonical charsets at the parse charset path where possible; runtime still owns remote/scripted header/BOM conflicts and malformed bytes.

T10 adapts existing landed parser call sites but does not consolidate or implement JSON/XML/HTML/text parsing; T11a owns that work.

### Diagnostics and retention

Use one explicit sanitizer while preserving distinct Diagnostic responsibilities and stable codes for status rejection, transport/read failure, byte-limit exhaustion, unsupported/conflicting charset declarations, and decode failure. Diagnostics may contain only numeric status, method, final URL scheme/normalized host/port, normalized media type and canonical/declared charset labels, encoding selection source, observed/admitted bytes, allowance/effective limit, and a safe typed transport error kind. They contain no body bytes/text, raw header names or values, cookies, credentials, userinfo, path, query, fragment, request body, tokens/secrets, raw URL, or raw reqwest error.

Request/response/header/raw body/outcome/unsanitized error containers have no Serde implementation and either no `Debug` or manually redacted `Debug`. Production code does not log, persist, panic-format, or attach them through generic error context. Diagnostic ordering follows T9 attempt order. Byte exhaustion emits one phase terminal Diagnostic; decode/declaration failures remain Strategy-scoped and may permit `FirstAccepted` fallback while budget remains.

## Dependency and deletion decision

- HTTP is the true external dependency: one domain-owned client has reqwest and deterministic scripted implementations.
- Charset resolution/decoding, limit precedence, ledger arithmetic, request rendering, status rejection, parsing, reducers, and Diagnostic projection remain concrete in-process code.
- Browser remains a separate local-substitutable runtime; Detection HTTP remains a separate external subsystem. SQLite/persistence does not change.
- No public mutable ledger, child scope, decoder/parser plugin, callback, or generic Strategy executor is introduced.

**Deletion test:** Without the shared HTTP boundary, reqwest construction, final metadata capture, streamed enforcement, typed failures, and deterministic event behavior would spread across Discovery, Detail, pagination, Search Run, Source Live Check, and lazy Detail. Without the private decoder, precedence, alias/BOM/conflict handling, strict incremental state, expansion checks, and sanitized failures would spread across both phase parser call sites and later parse Primitives. A forwarding wrapper fails this test.

## Examples

1. **Default UTF-8:** a 24-byte JSON response with no charset declaration preserves and charges 24 bytes, selects UTF-8 default, and reaches the existing parser.
2. **Authored encoding/BOM:** authored `windows-1252` strictly decodes XML byte `0x80` as `€`; without authored charset, a UTF-16LE BOM is selected and consumed from text while raw bytes retain it.
3. **Conflict/malformed:** authored/header Windows-1252 plus UTF-8 BOM produces one sanitized conflict Diagnostic and no parser input; incomplete default UTF-8 similarly exposes no replacement character or prefix.
4. **Cumulative fallback:** after Strategy A consumes six of ten bytes and is rejected, Strategy B receives four. `Content-Length: 5` rejects before polling, charges the started request, and returns typed byte exhaustion without later Strategy work.
5. **Unknown one-over versus boundary:** chunks of three then two under allowance four admit/charge four and expose no response once excess is proven; exactly four then EOF succeeds.
6. **Cancellation:** Cancellation after 8 KiB admission returns typed Cancellation, charges 8 KiB, runs no decoder/parser/fallback, and creates no Partial Completion.
7. **Sanitization:** a redirected 500 URL containing credentials, secret path/query/fragment, and `Set-Cookie` may report only 500 plus `https`, normalized host/port, safe content type, and counts.

## Scope

- Extend the landed T9 schema/Serde fragments, compiler plan, caller control, ledger, usage, exhaustion, and terminal Diagnostic with the immutable/tighten-only response-byte dimension.
- Add the shared typed HTTP family, bounded collector, reqwest implementation, and ordered-event scripted implementation.
- Add an offline ephemeral-localhost integration test through the real reqwest adapter covering redirect, repeated and non-UTF-8 response headers, non-success status, final URL, and exact non-text bytes.
- Add canonical authored charset compilation plus shared strict incremental decoding and sanitization.
- Route every landed Discovery/Detail HTTP path, including pagination/sitemap, through the seam and bounded decoder.
- Migrate Search Run, Source Live Check, lazy Detail, exports, deterministic clients, and tests directly.
- Preserve generic Greenhouse JSON, Workday paginated POST/JSON, and SuccessFactors sitemap/XML/HTML behavior with byte fixtures.
- If T4b has landed, apply its existing compiler-partition rule to new charset/limit validation and plan material and its runtime-partition rule to transport/decoding/execution behavior; do not redesign fingerprint shape or freshness.
- Delete phase-specific HTTP clients/responses/reqwest implementations, Browser-to-HTTP response wrapping, premature text conversion, duplicate fakes, wrappers, aliases, and superseded tests.

## Adjacent non-goals

- Shared parse Primitive extraction/registry (T11a/#179) or Select/Value Primitive consolidation.
- Browser byte preservation, decoding, budgets, or migration; Detection HTTP/Strategy Set convergence.
- Additional Strategy Policies, retries where none exist, Candidate Resolution, hints/provider values, requested Detail fields, matching, persistence, statuses, caching, compression expansion, rate limiting, parallel/speculative requests, or resumability.
- XML declaration sniffing beyond authored/BOM/HTTP/default precedence, new parse formats, or provider-specific charset behavior.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| Default/authored/BOM/header charset | Exact bytes charged; selected strict text reaches both phases | External Discovery/Detail transport tests |
| Equivalent aliases | Same canonical identity; no conflict | Decoder-through-phase test |
| Conflict or unsupported/malformed declaration | One stable sanitized Diagnostic; no parser/text | Both phase tests; compiler test for authored label |
| Malformed/incomplete bytes | Strict failure without replacement/prefix | Phase test plus narrow EOF test |
| Expansion arithmetic | Overflow/unavailable maximum fails before allocation | Narrow private arithmetic test |
| Metadata/localhost adapter | Status, repeated/raw header values, type, final URL, bytes preserved | Scripted tests and offline ephemeral-server integration |
| Script chunk/failure/pending/Cancellation | Ordered events and exact admitted prefix/outcome | Scripted-client tests through shared collector |
| Oversized upstream chunk | Retained/copied prefix never exceeds allowance; excess absent | Collector edge test |
| Non-2xx | Typed response then shared sanitized rejection; no decode | Both phase tests |
| Sensitive transport data | No Serde/unredacted Debug/log/persistence; exact Diagnostic allowlist | Static review and secret-leak test |
| Decompression | Automatic gzip/Brotli/deflate/zstd response decoding disabled | Cargo feature/build review |
| Omitted/profile/Source limit | Ceiling or tighter inherited result compiled | Compiler integration tests |
| Invalid/weakening/caller widening | Rejected or grants no capacity; never clamped | Schema/Serde/compiler and phase tests |
| Exact boundary/known one-over/unknown one-over | EOF equality succeeds; excess terminates with exact usage and no output | Both phase and scripted stream tests |
| Cumulative fallback/pagination | Each request receives only invocation remainder; no reset | Strategy Set and Workday-shaped tests |
| Stream/transport failure after prefix | Prefix charged; ordinary typed Strategy failure with distinct sanitized Diagnostic; no decode, but later `FirstAccepted` fallback may run while budget remains | Both phase and scripted-client tests |
| Cancellation after prefix | Prefix charged; typed Cancellation wins, no decode, fallback, later Strategy, or Partial Completion | Both phase, scripted-client, and Search Run tests |
| Byte-budget terminal | No response, decoded value, phase partial output, later work, or `fallback_exhausted`; one ordered terminal Diagnostic with exact usage | Both phase and Strategy Set budget tests |
| Browser/Detection separation | Existing String contracts unchanged and uncharged | Existing regressions plus static search |
| Caller migration | Search Run, Source Live Check, lazy Detail retain semantics through shared client | Caller regressions with scripted HTTP/temp SQLite as applicable |
| Acceptance profiles | Greenhouse, Workday, SuccessFactors outputs/order remain generic | Existing three profile targets with byte fixtures |
| Immutable runtime input/deletion | No raw authored runtime limit/charset; one HTTP seam/decoder; old phase families absent | Call-graph and repository searches |

### Focused commands

Re-baseline target names after T9; substitute renamed targets only with the exact landed tests covering the same behavior.

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test http_response_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_set_budget
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_set_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test discovery_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test detail_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_resolution
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_security_boundedness
cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation
cargo test --manifest-path src-tauri/Cargo.toml --test source_live_check
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml search::run
cargo test --manifest-path src-tauri/Cargo.toml search::posting

# No alternate aggregation or premature/lossy decoding in Discovery/Detail.
rg -n 'Response::text|\.text(_with_charset)?\(|\.bytes\(|\.chunk\(|response\.json|serde_json::from_slice|from_utf8(_unchecked|_lossy)?|String::from_utf8|decode(_to_string)?(_without_replacement)?|read_to_end|collect::<(Vec<u8>|.*Bytes)|try_concat|concat\(|fold\(|aggregate|copy_to_bytes|BytesMut|extend_from_slice|\.to_vec\(\)|body:\s*String' \
  src-tauri/src/profile_dsl src-tauri/src/search src-tauri/tests --glob '*.rs'

# One shared seam; classify every retained String response and old phase symbol.
rg -n 'body:\s*String|DetectionHttpResponse|ProfileBrowserFetchResponse' src-tauri/src src-tauri/tests --glob '*.rs'
rg -n 'Posting(Discovery|Detail)Fetch(Response|Error|er)|ReqwestPosting(Discovery|Detail)Fetcher|ProfileHttp(Client|Request|Response)' src-tauri/src src-tauri/tests --glob '*.rs'

# One cumulative byte owner; one charset owner; no provider dispatch.
rg -n 'maxResponseBytes|response_bytes|bytes_used|remaining.*bytes|body\.len\(\)|BudgetLedger|debit|charge' src-tauri/src/profile_dsl src-tauri/tests --glob '*.rs' --glob '*.json'
rg -n 'charset|encoding_rs|decode_to_|BOM|greenhouse|workday|successfactors|profile_key|source_key.*(match|==)' src-tauri/src/profile_dsl/runtime src-tauri/tests --glob '*.rs'
rg -n 'ProfileBrowserFetchResponse|DetectionHttpResponse|DetectionHttpClient|ProfileHttpClient' src-tauri/src --glob '*.rs'

# Sensitive containers and Diagnostic projections: review every hit.
rg -n -U '#\[derive\([^]]*(Serialize|Deserialize|Debug)[^]]*\)\]\s*(?:pub(?:\([^)]*\))?\s+)?(?:struct|enum)\s+(ProfileHttpRequest|BoundedHttpResponse|ResponseHeaders|ProfileHttpOutcome|TransportFailure)' src-tauri/src --glob '*.rs'
rg -n '(ProfileHttpRequest|BoundedHttpResponse|ResponseHeaders|ProfileHttpOutcome|TransportFailure)|println!|eprintln!|tracing::|log::|panic!|format!|context\(|with_context\(|serde_json::to_' src-tauri/src/profile_dsl src-tauri/src/search --glob '*.rs'
rg -n 'set-cookie|authorization|cookie|token|secret|response.*body|headers|final_url|error\.to_string\(\)' src-tauri/src/profile_dsl/runtime src-tauri/src/search --glob '*.rs'

# Automatic response decompression remains disabled.
cargo tree --manifest-path src-tauri/Cargo.toml -e features -i reqwest
rg -n 'reqwest\s*=|gzip|brotli|deflate|zstd|no_gzip|no_brotli|no_deflate|no_zstd|content-encoding' src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src --glob '*.rs' --glob '*.toml'

# No public mutable ledger, generic Strategy executor, or decoder plugin.
rg -n '\bpub\s+(trait|struct|enum|fn)\s+[A-Za-z0-9_]*(BudgetLedger|BudgetScope|StrategyExecutor|DecoderPlugin|CharsetResolver)' src-tauri/src/profile_dsl/runtime src-tauri/src/lib.rs --glob '*.rs'

npm run build
```

Classify retained text conversions, `body: String`, logging/formatting, transport metadata, charset/provider-name, and sensitive-container derive hits. Only unchanged Browser/Detection response strings may remain; no Discovery/Detail HTTP alternate aggregation/decoding path, public mutable ledger/generic executor/decoder plugin, provider branch, or resettable byte counter may remain.

## Ticket-specific migration items

- [ ] Add `maxResponseBytes`/`response_bytes` to the exact landed canonical T9 owners and prove 67,108,864-byte ceiling plus tighten-only compilation/caller behavior.
- [ ] Add one shared HTTP request/byte-response/outcome/report/client family, reqwest implementation, ordered scripted implementation, and common bounded collector.
- [ ] Add the localhost production-adapter test and classify automatic decompression features.
- [ ] Add strict charset compilation/decoding, checked expansion, sanitizer, sensitive-container protections, and exact leak tests.
- [ ] Move all Discovery/Detail HTTP and pagination/sitemap paths, Search Run, Source Live Check, lazy Detail, exports, and deterministic fixtures directly.
- [ ] Delete `PostingDiscoveryFetch*`, `PostingDetailFetch*`, both phase reqwest implementations, Browser-to-phase-response wrappers, premature `Response::text()`, duplicate fakes, aliases, and forwarding compatibility paths.
- [ ] Verify Browser and Detection contracts remain separate and every request/page/fallback uses only the invocation ledger remainder.
- [ ] Run and classify the focused searches above, including every remaining response `String`, body aggregation/decode, transport formatting/logging, and charset/provider-key hit.

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
