# Historischer Entwurfsstand

> Diese Datei dokumentiert die damalige Herleitung und ist keine aktive Implementierungsanleitung. Das schema-v3-Zielbild wurde durch #242 aktiviert; aktuelle normative Sprache steht in `CONTEXT.md`, den ADRs und den kanonischen PRDs.

Ich würde eine gemeinsame Strategy-/Primitive-Algebra entwerfen, aber nicht alle denkbaren Primitives sofort implementieren.

## Festgehaltene Namens- und Source-Entscheidungen

- Die drei Profilphasen heißen im Schema-v3-Zielbild `detection`, `discovery` und `detail` statt `detect`, `postingDiscovery` und `postingDetail`.
- Eine Source darf die ausführungsrelevanten Source-Profile-Bereiche direkt auf ihrer Root-Ebene in derselben verschachtelten Form partiell wiederholen. Es gibt weder eine `overrides`-/`overlay`-Zwischenebene noch eine Liste aus `operation`/`target`/`value`-Anweisungen.
- Vorhandene Profilwerte werden rekursiv überschrieben, neue Profilbereiche, Access Paths und Strategies können vollständig eingefügt werden. Source-Identität (`schemaVersion`, `key`, `name`, `status`) und die Basisprofil-Referenz bleiben eigenständig; Profile Detection gehört nicht in die Source.
- Access Paths und Strategies werden beim Merge über ihren stabilen `key` identifiziert. Geordnete Strategies bleiben Arrays; vorhandene Strategies behalten zunächst ihre Profile-Reihenfolge, neue Source-Strategies werden angehängt.
- Historischer Übergangshinweis: Vor der Aktivierung existierten vorübergehend schema-v2-Typen und alte Dateinamen. Der schema-v3-Hard-Cut ist inzwischen abgeschlossen; aktive Dokumentfelder, Rust-Typen und Zieldateien verwenden `detection`/`discovery`/`detail`.

 Die drei untersuchten Varianten waren:

 1. Universelle Primitive-AST: maximal flexibel, aber großer Rewrite und Gefahr einer Programmiersprache in JSON.
 2. Nur kleine additive Strategy Policy: sehr migrationsfreundlich, aber Detection und Runtime bleiben zunächst getrennt.
 3. Gemeinsamer Strategy-Set-Vertrag mit inkrementeller Migration: kleine gemeinsame Schnittstelle, bestehende Strategy-Struktur bleibt erhalten.

 Meine Empfehlung ist Variante 3:

 │ Ein gemeinsamer Strategy-Set-Vertrag für Detection, Discovery und Detail, aber weiterhin typisierte Phasenverträge und die bestehende feste Strategy-Struktur.

 ```text
   Phase
     = Input Context
     + Strategy Set
     + Strategy Policy
     + Acceptance
     + Phase Output Contract
 ```

 Eine Strategy bleibt ungefähr:

 ```text
   fetch → paginate → parse → select → filter/capture → extract → accept
 ```

 ────────────────────────────────────────────────────────────────────────────────

 1. Kandidatenkatalog aller Primitives

 Legende:

 - Vorhanden: bereits vollständig oder weitgehend implementiert.
 - Kandidat: wahrscheinlich allgemein nützlich.
 - Später: erst bei konkreter Evidenz implementieren.
 - Nicht DSL: bewusst außerhalb der Profile DSL.

 1.1 Strategy-Set-Policies

 Diese arbeiten auf vollständigen Strategies.

 ┌──────────────────┬─────────────────────────────────────────────────────────────────────────────────┐
 │ Primitive        │ Bedeutung                                                                       │
 ├──────────────────┼─────────────────────────────────────────────────────────────────────────────────┤
 │ first_accepted   │ Geordneter Fallback; erste akzeptierte Strategy gewinnt. Vorhandenes Verhalten. │
 ├──────────────────┼─────────────────────────────────────────────────────────────────────────────────┤
 │ any_accepted     │ Alle dürfen versucht werden; mindestens eine muss akzeptiert werden.            │
 ├──────────────────┼─────────────────────────────────────────────────────────────────────────────────┤
 │ all_required     │ Alle Strategies müssen akzeptiert werden.                                       │
 ├──────────────────┼─────────────────────────────────────────────────────────────────────────────────┤
 │ collect_all      │ Alle accepted Outputs werden phasenspezifisch zusammengeführt.                  │
 ├──────────────────┼─────────────────────────────────────────────────────────────────────────────────┤
 │ at_least         │ Mindestens n Strategies müssen akzeptiert werden.                               │
 ├──────────────────┼─────────────────────────────────────────────────────────────────────────────────┤
 │ exactly_one      │ Genau eine Strategy darf akzeptiert werden.                                     │
 ├──────────────────┼─────────────────────────────────────────────────────────────────────────────────┤
 │ successful_range │ Anzahl erfolgreicher Strategies muss in einem Bereich liegen.                   │
 ├──────────────────┼─────────────────────────────────────────────────────────────────────────────────┤
 │ optional         │ Fehlschlag wird diagnostiziert, lässt das Strategy Set aber nicht scheitern.    │
 ├──────────────────┼─────────────────────────────────────────────────────────────────────────────────┤
 │ sequential       │ Strategies werden deterministisch nacheinander ausgeführt.                      │
 ├──────────────────┼─────────────────────────────────────────────────────────────────────────────────┤
 │ parallel         │ Begrenzte parallele Ausführung mit zwingendem maxConcurrency.                   │
 └──────────────────┴─────────────────────────────────────────────────────────────────────────────────┘

 Kein unpräzises manyOf. Dafür at_least oder successful_range.

 1.2 Logische Predicate-Primitives

 Diese arbeiten auf Wahrheitswerten, nicht auf Strategies.

 - all_of
 - any_of
 - none_of
 - not
 - xor
 - at_least
 - exactly
 - exists
 - missing
 - non_empty
 - is_empty
 - equals
 - not_equals
 - contains
 - contains_any
 - contains_all
 - starts_with
 - ends_with
 - matches_regex
 - in
 - not_in
 - less_than
 - less_than_or_equal
 - greater_than
 - greater_than_or_equal
 - between
 - count_equals
 - count_at_least
 - count_at_most
 - is_url
 - same_origin
 - content_type_is
 - status_is
 - has_fields
 - json_shape_matches
 - xml_root_is
 - html_marker_exists

 1.3 Kontext- und Wertquellen

 - const
 - template
 - input
 - entry_point
 - source
 - source.key
 - source.name
 - source.status
 - source_config
 - candidate
 - posting.url
 - posting.title
 - posting.company
 - posting.locations
 - posting_meta
 - capture
 - item
 - item_field
 - response.body
 - response.bytes
 - response.status
 - response.header
 - response.content_type
 - response.final_url
 - pagination.page
 - pagination.offset
 - pagination.limit
 - pagination.cursor
 - pagination.accumulated_count
 - strategy_output – später, weil es Cross-Strategy-Abhängigkeiten erzeugt
 - runtime.timestamp – nur wenn fachlich erforderlich

 Nicht verfügbar als Profile-Kontext:

 - Search-Request-Kriterien;
 - Datenbankzugriff;
 - beliebige Secrets;
 - lokales Dateisystem.

 1.4 HTTP-/I/O-Primitives

 - http_get
 - http_post
 - http_head
 - request_headers
 - json_body
 - text_body
 - form_urlencoded_body
 - multipart_body
 - follow_redirects
 - reject_redirects
 - max_redirects
 - capture_final_url
 - capture_response_header
 - capture_status
 - capture_content_type
 - max_response_bytes
 - timeout
 - retry
 - retry_statuses
 - retry_error_classes
 - bounded_backoff
 - minimum_request_delay
 - per_host_rate_limit
 - same_run_request_cache
 - conditional_request
 - etag
 - if_modified_since
 - cookie_jar
 - session
 - byte_response
 - decode_response
 - linked_resource_fetch
 - robots_txt_fetch

 Später oder außerhalb:

 - authentifizierte Requests über sichere Credential References;
 - kein Secret direkt im JSON;
 - kein beliebiger lokaler Datei-Download.

 1.5 Browser-Primitives

 - navigate
 - wait_for_selector
 - wait_for_text
 - wait_for_url
 - wait_for_network_idle
 - wait_bounded_time
 - click
 - click_if_visible
 - click_until_gone
 - type_text
 - clear_input
 - select_option
 - submit_form
 - follow_link
 - scroll
 - scroll_until
 - load_more
 - dismiss_consent
 - switch_frame
 - capture_html
 - capture_text
 - capture_attribute
 - capture_final_url
 - screenshot_for_diagnostics
 - dom_excerpt_for_diagnostics

 Jede Interaktion benötigt harte Grenzen:

 - maxCount
 - maxDurationMs
 - maxItems
 - maxDepth

 Nicht erlauben:

 - beliebiges JavaScript;
 - eval;
 - beliebige DOM-Mutation;
 - CAPTCHA-Bypass;
 - unbeschränkte Login-Automation.

 1.6 Decode-/Parse-Primitives

 - decode_charset
 - decode_bom
 - decode_xml_encoding
 - parse_json
 - parse_xml
 - parse_html
 - parse_text
 - parse_url
 - parse_query
 - parse_form
 - parse_csv
 - parse_tsv
 - parse_json_lines
 - parse_json_ld
 - parse_microdata
 - parse_robots_txt
 - parse_link_header
 - parse_date
 - parse_number
 - parse_boolean

 RSS und Atom benötigen wahrscheinlich keinen eigenen Parser. Sie können über XML-Primitives projiziert werden.

 Transport-Dekompression wie gzip oder Brotli sollte Runtime-Verantwortung bleiben.

 1.7 Select-/Traversal-Primitives

 - document
 - json_path
 - xml_element
 - xml_text
 - css
 - xpath – später, falls XML-Element/Text nicht ausreichen
 - sitemap_urls
 - sitemap_index
 - rss_items
 - atom_entries
 - json_ld_node
 - microdata_item
 - response_header
 - response_status
 - response_final_url
 - url_scheme
 - url_host
 - url_port
 - url_path
 - url_path_segment
 - url_query
 - url_query_parameter
 - url_fragment
 - children
 - descendants
 - parent
 - siblings
 - attribute
 - array_items
 - object_entries
 - table_rows
 - links_by_rel
 - first
 - last
 - nth
 - slice
 - take
 - skip
 - distinct

 1.8 Filter-Primitives

 - filter_non_empty
 - filter_exists
 - filter_regex
 - filter_equals
 - filter_not_equals
 - filter_contains
 - filter_prefix
 - filter_suffix
 - filter_in
 - filter_not_in
 - filter_number
 - filter_date
 - filter_url_host
 - filter_url_path
 - filter_same_origin
 - filter_content_type
 - filter_required_fields
 - filter_capture_exists
 - filter_unique_by
 - filter_negative_marker
 - filter_count_range

 Keine Search-Request-Include- oder Exclusion-Regeln als Profile-Filter. Profile filtern Providerdaten; die Search Request entscheidet, ob der User das Posting möchte.

 1.9 Capture-Primitives

 - regex_capture
 - named_capture
 - url_component_capture
 - path_segment_capture
 - query_parameter_capture
 - json_path_capture
 - xml_capture
 - css_text_capture
 - css_attribute_capture
 - response_header_capture
 - final_url_capture
 - capture_with_transform
 - capture_many
 - capture_default
 - capture_first_non_empty
 - capture_conflict_detection

 Detection Captures sollten Provenienz besitzen:

 ```json
   {
     "value": "join.schott.com",
     "strategyKey": "rmk_html_marker",
     "source": "response.finalUrl"
   }
 ```

 1.10 Match-/Correlation-/Join-Primitives

 - field_equals
 - composite_field_equals
 - normalized_equals
 - match_by_id
 - match_by_url
 - match_by_external_path
 - lookup_by_key
 - join_by_key
 - left_join
 - inner_join
 - zip
 - group_by
 - index_by
 - canonical_link_match

 Diese sind besonders für APIs relevant, bei denen Discovery und Detail aus unterschiedlichen Collections stammen.

 1.11 Extract-/Construct-Primitives

 - extract_scalar
 - extract_list
 - extract_object
 - extract_field_map
 - extract_posting_meta
 - extract_evidence
 - extract_diagnostic
 - construct_object
 - construct_list
 - construct_url
 - combine
 - first_non_empty
 - coalesce
 - default
 - conditional
 - flatten
 - compact
 - field_provenance
 - canonical_link
 - meta_content
 - microdata_property
 - json_ld_property

 1.12 Field-Expression-Primitives

 Bereits weitgehend vorhanden:

 - const
 - template
 - source_config
 - posting_meta
 - capture
 - item_field
 - json_path
 - xml_text
 - xml_element
 - css_text
 - css_attribute
 - combine

 Mögliche Ergänzungen:

 - input
 - source
 - candidate
 - response_metadata
 - url_component
 - first_non_empty
 - default
 - conditional
 - map
 - filter_values
 - flatten
 - object
 - list
 - lookup
 - field_provenance

 1.13 Transform-Primitives

 - trim
 - normalize_whitespace
 - html_to_text
 - decode_html_entities
 - decode_xml_entities
 - url_decode
 - url_encode
 - slug_to_search_text
 - lowercase
 - uppercase
 - titlecase
 - unicode_casefold
 - unicode_normalize
 - strip_prefix
 - strip_suffix
 - literal_replace
 - regex_replace
 - split
 - join
 - compact
 - flatten
 - dedupe
 - sort
 - reverse
 - take
 - slice
 - map
 - filter
 - resolve_url
 - normalize_url
 - remove_fragment
 - normalize_host
 - to_string
 - to_number
 - to_boolean
 - parse_date
 - format_date
 - lookup_table
 - default
 - coalesce

 Keine ATS-spezifische Location-Normalisierung. Das Profil soll den Providerwert verlustfrei liefern; #57 normalisiert zentral.

 1.14 Pagination-/Iteration-Primitives

 - page_number
 - offset_limit
 - cursor
 - continuation_token
 - next_link
 - link_header
 - sitemap
 - sitemap_index
 - load_more
 - infinite_scroll
 - batch
 - for_each_item
 - for_each_candidate
 - max_requests
 - max_items
 - max_depth
 - max_concurrency
 - stop_on_empty_page
 - stop_on_total_reached
 - stop_on_unchanged_cursor
 - stop_on_duplicate_page
 - stop_on_duplicate_item
 - stop_when
 - accumulate
 - dedupe_while_accumulating

 for_each_candidate darf nur begrenzt und vom Backend auf eine vorselektierte Kandidatenmenge angewandt werden. Kein beliebiges rekursives Crawling.

 1.15 Acceptance-/Validation-Primitives

 - required_fields
 - required_captures
 - required_evidence
 - required_evidence_keys
 - minimum_results
 - maximum_results
 - minimum_items
 - maximum_items
 - minimum_description_length
 - maximum_error_ratio
 - maximum_diagnostic_severity
 - status_is
 - content_type_is
 - canonical_url_required
 - unique_by
 - field_predicate
 - json_schema
 - field_semantics
 - minimum_accepted_strategies
 - maximum_accepted_strategies
 - no_error_diagnostics

 Eine Strategy ist erst erfolgreich, wenn Acceptance erfüllt ist. HTTP 200 allein reicht nicht.

 1.16 Merge-/Reducer-Primitives

 Diese sollten größtenteils durch die Phase vorgegeben werden.

 ### Detection

 - evidence_union
 - capture_merge_equal
 - source_config_merge_equal
 - proposal_merge
 - capture_conflict_error

 ### Discovery

 - candidate_concat
 - candidate_union
 - candidate_intersection
 - candidate_unique_by_url
 - candidate_unique_by_provider_id
 - candidate_field_merge_equal

 ### Detail

 - field_patch_merge
 - first_non_empty_field
 - prefer_strategy_order
 - merge_lists
 - merge_objects
 - field_conflict_error

 Nicht verwenden:

 - implizites last_write_wins.

 1.17 Bounds-/Resilience-/Control-Primitives

 - timeout_ms
 - max_duration_ms
 - max_requests
 - max_items
 - max_depth
 - max_retries
 - max_redirects
 - max_response_bytes
 - max_request_bytes
 - max_concurrency
 - max_strategies
 - max_browser_actions
 - max_pages
 - max_candidates
 - minimum_delay_ms
 - rate_limit
 - cancellation
 - request_budget
 - item_budget
 - browser_budget
 - detail_budget

 1.18 Diagnostics-/Provenance-Primitives

 - emit_diagnostic
 - diagnostic_code
 - diagnostic_category
 - diagnostic_severity
 - diagnostic_path
 - diagnostic_details
 - strategy_attempt
 - recovered_attempt
 - field_provenance
 - capture_provenance
 - response_provenance
 - trace_id
 - redact
 - diagnostic_sample_limit
 - diagnostic_count_summary

 1.19 Bewusste Nicht-Primitives

 Diese gehören nicht in die Profile DSL:

 - Search Request Include Rules;
 - Search Request Exclusion Rules;
 - Location-Radius-Entscheidung;
 - finale Job-Posting-Deduplizierung;
 - Datenbank-Persistenz;
 - Source-Status-Transition;
 - beliebiger Rust-/JavaScript-/Shell-Code;
 - beliebiger Dateisystemzugriff;
 - Inline-Secrets;
 - CAPTCHA-Bypass;
 - unbeschränkte Rekursion;
 - profilspezifische Rust-Adapter.

 Der Katalog ist ein Kandidatenraum, keine Implementierungs-Checkliste.

 ────────────────────────────────────────────────────────────────────────────────

 2. Empfohlenes Schema

 2.1 Gemeinsamer Strategy-Set-Vertrag

 ```json
   {
     "policy": {
       "type": "first_accepted"
     },
     "budget": {
       "maxStrategies": 4,
       "maxRequests": 20,
       "maxItems": 2000,
       "maxResponseBytes": 10000000,
       "timeoutMs": 30000
     },
     "strategies": [],
     "acceptWhen": {}
   }
 ```

 2.2 Strategy Policy

 ```json
   {
     "$defs": {
       "strategyPolicy": {
         "oneOf": [
           {
             "type": "object",
             "additionalProperties": false,
             "required": ["type"],
             "properties": {
               "type": { "const": "first_accepted" }
             }
           },
           {
             "type": "object",
             "additionalProperties": false,
             "required": ["type"],
             "properties": {
               "type": { "const": "all_required" }
             }
           },
           {
             "type": "object",
             "additionalProperties": false,
             "required": ["type"],
             "properties": {
               "type": { "const": "collect_all" },
               "minAccepted": {
                 "type": "integer",
                 "minimum": 1
               }
             }
           },
           {
             "type": "object",
             "additionalProperties": false,
             "required": ["type", "count"],
             "properties": {
               "type": { "const": "at_least" },
               "count": {
                 "type": "integer",
                 "minimum": 1
               }
             }
           },
           {
             "type": "object",
             "additionalProperties": false,
             "required": ["type"],
             "properties": {
               "type": { "const": "exactly_one" }
             }
           },
           {
             "type": "object",
             "additionalProperties": false,
             "required": ["type", "minimum", "maximum"],
             "properties": {
               "type": { "const": "successful_range" },
               "minimum": { "type": "integer", "minimum": 0 },
               "maximum": { "type": "integer", "minimum": 1 }
             }
           }
         ]
       }
     }
   }
 ```

 Der Profile Compiler prüft zusätzlich:

 - Anzahl gegen Strategy-Anzahl;
 - eindeutige Strategy Keys;
 - kumulative Budgets;
 - Phasenkompatibilität;
 - Merge-Kompatibilität.

 2.3 Gemeinsame Strategy-Form

 ```json
   {
     "key": "stable_strategy_key",
     "description": "Optional explanation",

     "input": {
       "type": "phase_context"
     },

     "fetch": {
       "mode": "http",
       "method": "GET",
       "url": "{{sourceConfig:endpoint}}",
       "timeoutMs": 10000,
       "limits": {
         "maxResponseBytes": 5000000
       }
     },

     "pagination": {
       "type": "cursor",
       "limits": {
         "maxRequests": 20,
         "maxItems": 2000
       }
     },

     "parse": {
       "type": "json"
     },

     "select": {
       "type": "json_path",
       "jsonPath": "$.items[*]"
     },

     "where": {
       "type": "all_of",
       "predicates": []
     },

     "captures": {},

     "extract": {
       "fields": {}
     },

     "acceptWhen": {
       "requiredFields": [],
       "minResults": 1
     },

     "diagnostics": []
   }
 ```

 Nicht jede Strategy benötigt jeden Abschnitt. Der Compiler prüft die Datenabhängigkeiten:

 - parse ohne Text/Bytes-Input ist ungültig;
 - select muss zum Parse-Typ passen;
 - posting_meta ist in Detection ungültig;
 - Discovery-Output muss dem Discovery-Vertrag entsprechen.

 2.4 Source Profile

 ```json
   {
     "schemaVersion": 3,
     "key": "successfactors",
     "name": "SAP SuccessFactors",
     "kind": "recruiting_system",

     "support": {
       "level": "experimental",
       "summary": "Reusable public RMK behavior."
     },

     "detection": {
       "policy": {
         "type": "at_least",
         "count": 2
       },
       "strategies": [
         {
           "key": "entry_url",
           "input": {
             "type": "entry_point"
           },
           "captures": {
             "host": {
               "type": "url_component",
               "component": "host"
             }
           },
           "extract": {
             "fields": {
               "evidence": [
                 {
                   "kind": "url",
                   "message": "Valid public HTTP entry point."
                 }
               ]
             }
           },
           "acceptWhen": {
             "requiredCaptures": ["host"]
           }
         },
         {
           "key": "rmk_html_marker",
           "fetch": {
             "mode": "http",
             "method": "GET",
             "url": "{{entryPoint}}",
             "timeoutMs": 10000
           },
           "parse": {
             "type": "html"
           },
           "select": {
             "type": "css",
             "selector": "link[href*='rmkcdn.successfactors.com'], [itemtype='http://schema.org/JobPosting']"
           },
           "extract": {
             "fields": {
               "evidence": [
                 {
                   "kind": "html",
                   "message": "RMK assets or JobPosting markup found."
                 }
               ]
             }
           },
           "acceptWhen": {
             "minResults": 1
           }
         },
         {
           "key": "public_catalog",
           "fetch": {
             "mode": "http",
             "method": "GET",
             "url": "https://{{capture:host}}/sitemap.xml",
             "timeoutMs": 10000
           },
           "parse": {
             "type": "xml"
           },
           "select": {
             "type": "sitemap_urls",
             "urlPattern": "(?i)/job/"
           },
           "extract": {
             "fields": {
               "sourceConfig": {
                 "sitemapUrl": "https://{{capture:host}}/sitemap.xml"
               },
               "recommendedAccessPathKey": "public_catalog",
               "evidence": [
                 {
                   "kind": "http",
                   "message": "Public catalog contains job URLs."
                 }
               ]
             }
           },
           "acceptWhen": {
             "minResults": 1
           }
         }
       ],
       "acceptWhen": {
         "requiredFields": [
           "sourceConfig.sitemapUrl",
           "recommendedAccessPathKey"
         ]
       }
     },

     "sourceConfigSchema": {
       "type": "object",
       "additionalProperties": false,
       "required": ["sitemapUrl"],
       "properties": {
         "sitemapUrl": {
           "type": "string",
           "format": "uri"
         }
       }
     },

     "accessPaths": [
       {
         "key": "public_catalog",
         "name": "Public catalog",

         "discovery": {
           "policy": {
             "type": "first_accepted"
           },
           "strategies": [
             {
               "key": "rss",
               "fetch": {
                 "mode": "http",
                 "method": "GET",
                 "url": "{{sourceConfig:sitemapUrl}}",
                 "timeoutMs": 10000
               },
               "parse": {
                 "type": "xml"
               },
               "select": {
                 "type": "xml_element",
                 "element": "item"
               },
               "extract": {
                 "fields": {
                   "title": {
                     "type": "xml_text",
                     "textPath": "title",
                     "semantics": "provider_value"
                   },
                   "company": {
                     "type": "template",
                     "template": "{{source:name}}",
                     "semantics": "provider_value"
                   },
                   "url": {
                     "type": "xml_text",
                     "textPath": "link",
                     "semantics": "provider_value"
                   },
                   "locations": {
                     "type": "xml_text",
                     "textPath": "g:location",
                     "semantics": "provider_value"
                   }
                 }
               },
               "acceptWhen": {
                 "requiredFields": ["title", "company", "url"],
                 "minResults": 1
               }
             },
             {
               "key": "sitemap",
               "fetch": {
                 "mode": "http",
                 "method": "GET",
                 "url": "{{sourceConfig:sitemapUrl}}",
                 "timeoutMs": 10000
               },
               "pagination": {
                 "type": "sitemap",
                 "postingUrlSelector": {
                   "type": "sitemap_urls",
                   "urlPattern": "(?i)/job/"
                 },
                 "limits": {
                   "maxRequests": 10,
                   "maxItems": 2000,
                   "maxDepth": 2
                 }
               },
               "parse": {
                 "type": "xml"
               },
               "select": {
                 "type": "document"
               },
               "extract": {
                 "fields": {
                   "searchText": {
                     "type": "item_field",
                     "key": "value",
                     "transforms": [
                       { "type": "url_decode" },
                       { "type": "slug_to_search_text" }
                     ],
                     "semantics": "hint"
                   },
                   "company": {
                     "type": "template",
                     "template": "{{source:name}}",
                     "semantics": "provider_value"
                   },
                   "url": {
                     "type": "item_field",
                     "key": "value",
                     "semantics": "provider_value"
                   }
                 }
               },
               "acceptWhen": {
                 "requiredFields": ["searchText", "company", "url"],
                 "minResults": 1
               }
             }
           ]
         },

         "detail": {
           "policy": {
             "type": "first_accepted"
           },
           "strategies": [
             {
               "key": "structured_html",
               "fetch": {
                 "mode": "http",
                 "method": "GET",
                 "url": "{{posting:url}}",
                 "timeoutMs": 10000
               },
               "parse": {
                 "type": "html"
               },
               "select": {
                 "type": "document"
               },
               "extract": {
                 "fields": {
                   "title": {
                     "type": "first_non_empty",
                     "expressions": [
                       {
                         "type": "css_attribute",
                         "selector": "meta[property='og:title']",
                         "attribute": "content"
                       },
                       {
                         "type": "css_text",
                         "selector": "[itemprop='title'], #job-title"
                       }
                     ],
                     "semantics": "provider_value"
                   },
                   "locations": {
                     "type": "first_non_empty",
                     "expressions": [
                       {
                         "type": "css_attribute",
                         "selector": "[itemprop='streetAddress']",
                         "attribute": "content"
                       },
                       {
                         "type": "css_text",
                         "selector": ".jobGeoLocation"
                       }
                     ],
                     "semantics": "provider_value"
                   },
                   "descriptionText": {
                     "type": "css_text",
                     "selector": "[itemprop='description'], .job-description, .jobdescription",
                     "semantics": "provider_value"
                   }
                 }
               },
               "acceptWhen": {
                 "requiredFields": ["title"]
               }
             }
           ]
         }
       }
     ]
   }
 ```

 semantics unterscheidet nur:

 - hint: darf für konservativen Vorfilter verwendet werden;
 - provider_value: explizit vom Provider geliefert und darf in die zentrale Normalisierung.

 normalized ist kein Profilwert. Das ist Ergebnis der Backend-Normalisierung.

 2.5 Source

 Eine Source bleibt ein eigenes Dokument, darf die ausführungsrelevanten Profilbereiche aber direkt auf Root-Ebene partiell wiederholen. Dadurch entsteht keine zusätzliche `override`- oder `overlay`-Ebene:

 ```json
 {
   "schemaVersion": 3,
   "key": "schott",
   "name": "SCHOTT",
   "status": "active",
   "sourceConfig": {
     "sitemapUrl": "https://join.schott.com/sitemap.xml",
     "detailLocale": "de-DE"
   },
   "selectedAccessPath": {
     "type": "profile_access_path",
     "profileKey": "successfactors",
     "pathKey": "public_catalog"
   },
   "sourceConfigSchema": {
     "properties": {
       "detailLocale": {
         "type": "string"
       }
     }
   },
   "accessPaths": [
     {
       "key": "public_catalog",
       "detail": {
         "policy": {
           "type": "collect_all"
         },
         "strategies": [
           {
             "key": "structured_html",
             "fetch": {
               "timeoutMs": 15000
             },
             "extract": {
               "fields": {
                 "descriptionText": {
                   "selector": ".description"
                 }
               }
             }
           },
           {
             "key": "schott_structured_data",
             "fetch": {
               "mode": "http",
               "method": "GET",
               "url": "{{posting:url}}",
               "timeoutMs": 10000
             },
             "parse": { "type": "html" },
             "select": { "type": "document" },
             "extract": {
               "fields": {
                 "locations": {
                   "type": "css_attribute",
                   "selector": "[itemprop='addressLocality']",
                   "attribute": "content",
                   "semantics": "provider_value"
                 }
               }
             },
             "acceptWhen": {
               "requiredFields": ["locations"]
             }
           }
         ]
       }
     }
   ]
 }
 ```

 `sourceConfigSchema` und `accessPaths` stehen direkt dort, wo sie auch in einem Source Profile stehen. `structured_html` wird über seinen Key gefunden und partiell verändert; `schott_structured_data` wird als neue vollständige Strategy eingefügt.

 Die strukturelle Spezialisierung besitzt folgende Merge-Algebra:

 - fehlender Datenpunkt: Basisprofil-Wert bleibt unverändert;
 - Objekt: rekursiver Merge;
 - Scalar: Source-Wert ersetzt Basisprofil-Wert;
 - `accessPaths`: keyed Merge über `key`; neue vollständige Access Paths sind erlaubt;
 - `strategies`: keyed Merge über `key`; neue vollständige Strategies sind erlaubt;
 - bestehende Strategy: partieller Source-Eintrag wird mit der vollständigen Basisprofil-Strategy gemerged;
 - neue Strategy: Source-Eintrag muss nach dem Merge vollständig sein und wird standardmäßig nach den geerbten Strategies angehängt;
 - sonstige Arrays, etwa `transforms` oder `requiredFields`: vollständiger Ersatz statt Merge nach Index;
 - `null`: zunächst verboten; Entfernen benötigt später eine explizite Semantik wie ein typisiertes `enabled: false`;
 - nach dem Merge wird das vollständige effektive Ausführungsprofil erneut durch Schema, Profile Compiler, Security- und Boundedness-Prüfung validiert.

 Compiler-Reihenfolge:

 ```text
 Basisprofil laden
   → direkte Profilfragmente der Source strukturell mergen
   → effektives Ausführungsprofil vollständig validieren
   → Source Config gegen das effektive sourceConfigSchema validieren
   → selectedAccessPath im effektiven Profil auflösen
   → Execution Plan kompilieren
 ```

 Direkt spezialisierbar sind insbesondere:

 - `sourceConfigSchema`;
 - `accessPaths`;
 - Policies und Acceptance;
 - bestehende und neue Strategies;
 - Fetch, Pagination, Parse, Select, Predicates, Captures, Extract und Transforms;
 - Source-bezogene Supporthinweise oder Diagnostics, sofern wir diese Felder im Source-Schema vorsehen.

 Nicht aus dem Profil übernommen beziehungsweise überschrieben werden:

 - `detection`, weil die konkrete Source zu diesem Zeitpunkt bereits existiert;
 - `schemaVersion` und Profile-Key als Dokument-/Referenzidentität;
 - Profile-Name und Profile-Kind, weil `name` auf Source-Root die konkrete Source bezeichnet;
 - Search-Request-Kriterien und Persistenzlogik.

 Neue Access Paths können direkt ergänzt und danach über `selectedAccessPath.pathKey` ausgewählt werden. Wenn kein Basisprofil passt, verwendet die Source weiterhin einen Source-owned Access Path oder ein Custom Source Profile.

 Das Basisprofil, alle direkten Source-Spezialisierungen und die Source Config fließen in die Freshness-Fingerprints ein. Wiederholte Spezialisierungen können als Hinweis für einen gemeinsamen Access Path oder ein Custom Source Profile diagnostiziert werden, werden aber nicht blockiert oder automatisch umgebaut; diese Pflege bleibt beim User.

 ────────────────────────────────────────────────────────────────────────────────

 3. Vorgeschlagene Codebase-Struktur

 Ich empfehle keinen Big-Bang-Rewrite. Die gemeinsame Runtime ist zunächst ein Deepening Candidate und der gemeinsame Execution-Seam bleibt hypothetisch, bis mindestens
 Discovery und Detail nachweislich dieselben Implementierungen verwenden.

 Phase 0: Vertrag festschreiben

 Ändern:

 ```text
   CONTEXT.md
   docs/prd/declarative-source-profile-dsl.md
   docs/adr/0009-declarative-source-profile-dsl.md
 ```

 Festlegen:

 - Phase Inputs und Outputs;
 - Strategy-Acceptance;
 - Policy-Semantik;
 - Merge-Regeln;
 - kumulative Budgets;
 - hint gegen provider_value;
 - Search Request und Persistenz bleiben backend-owned.

 Phase 1: Strategy Policy additiv einführen

 Neue Dateien:

 ```text
   src-tauri/src/schema/profile-dsl/strategy-policy.schema.json

   src-tauri/src/profile_dsl/documents/strategy_policy.rs
   src-tauri/src/profile_dsl/compiler/strategy_policy.rs
   src-tauri/src/profile_dsl/execution_plan/strategy_policy.rs
   src-tauri/src/profile_dsl/runtime/strategy_policy.rs

   src-tauri/tests/profile_dsl_strategy_policy.rs
 ```

 Ändern:

 ```text
   src-tauri/src/schema/profile-dsl/strategy.schema.json

   src-tauri/src/profile_dsl/documents/mod.rs
   src-tauri/src/profile_dsl/documents/posting_discovery.rs
   src-tauri/src/profile_dsl/documents/posting_detail.rs

   src-tauri/src/profile_dsl/compiler/mod.rs
   src-tauri/src/profile_dsl/compiler/resolution.rs
   src-tauri/src/profile_dsl/compiler/boundedness.rs

   src-tauri/src/profile_dsl/execution_plan/mod.rs
   src-tauri/src/profile_dsl/execution_plan/posting_discovery.rs
   src-tauri/src/profile_dsl/execution_plan/posting_detail.rs

   src-tauri/src/profile_dsl/runtime/posting_discovery.rs
   src-tauri/src/profile_dsl/runtime/posting_detail.rs

   src-tauri/src/profile_dsl/documents/serde_tests.rs
   src-tauri/tests/profile_dsl_profiles/schema.rs
 ```

 Migration:

 - fehlende Policy bedeutet zunächst first_accepted;
 - bestehende Built-ins verhalten sich unverändert;
 - zuerst nur heutiges Verhalten explizit modellieren.

 Phase 2: Phasenspezifische Reducer

 Neue Dateien:

 ```text
   src-tauri/src/profile_dsl/runtime/attempt.rs
   src-tauri/src/profile_dsl/runtime/posting_discovery/reducer.rs
   src-tauri/src/profile_dsl/runtime/posting_detail/reducer.rs
   src-tauri/src/source_profile/detection/reducer.rs
 ```

 Aufgaben:

 - StrategyAttempt<O>;
 - Acceptance getrennt von HTTP-Erfolg;
 - Candidate-Union;
 - Detail-Field-Patches;
 - Capture-/Config-Konflikte;
 - recovered fallback diagnostics;
 - kumulative Budgets.

 Phase 3: Gemeinsame Primitive-Implementierung extrahieren

 Neue Struktur:

 ```text
   src-tauri/src/profile_dsl/runtime/shared/
   ├── mod.rs
   ├── document.rs
   ├── values.rs
   ├── field_expression.rs
   ├── predicates.rs
   ├── attempt.rs
   ├── budget.rs
   └── provenance.rs

   src-tauri/src/profile_dsl/runtime/fetch/
   ├── mod.rs
   ├── http.rs
   └── browser.rs

   src-tauri/src/profile_dsl/runtime/parse/
   ├── mod.rs
   ├── json.rs
   ├── xml.rs
   ├── html.rs
   └── text.rs
 ```

 Schrittweise verkleinern:

 ```text
   src-tauri/src/profile_dsl/runtime/posting_discovery/document.rs
   src-tauri/src/profile_dsl/runtime/posting_discovery/values.rs
   src-tauri/src/profile_dsl/runtime/posting_discovery/extract/fields.rs
   src-tauri/src/profile_dsl/runtime/posting_discovery/fetch.rs

   src-tauri/src/profile_dsl/runtime/posting_detail/document.rs
   src-tauri/src/profile_dsl/runtime/posting_detail/values.rs
   src-tauri/src/profile_dsl/runtime/posting_detail/extract/fields.rs
   src-tauri/src/profile_dsl/runtime/posting_detail/fetch.rs
 ```

 Discovery und Detail bleiben dünne Phasenmodule mit eigenen:

 - Input Contexts;
 - Output Contracts;
 - Acceptance;
 - Reducern;
 - Diagnostics.

 Phase 4: Gemeinsames Dokumentmodell erweitern

 Neue Dateien:

 ```text
   src-tauri/src/profile_dsl/documents/strategy_set.rs
   src-tauri/src/profile_dsl/documents/predicate.rs
   src-tauri/src/profile_dsl/documents/value.rs
   src-tauri/src/profile_dsl/documents/output.rs
   src-tauri/src/profile_dsl/documents/budget.rs
 ```

 Ändern:

 ```text
   src-tauri/src/profile_dsl/documents/extract.rs
   src-tauri/src/profile_dsl/documents/fetch.rs
   src-tauri/src/profile_dsl/documents/select.rs
   src-tauri/src/profile_dsl/documents/pagination.rs
   src-tauri/src/profile_dsl/documents/strategy.rs
   src-tauri/src/profile_dsl/documents/access_path.rs
 ```

 Compiler-Erweiterungen:

 ```text
   src-tauri/src/profile_dsl/compiler/typing.rs
   src-tauri/src/profile_dsl/compiler/contexts.rs
   src-tauri/src/profile_dsl/compiler/strategy_sets.rs
   src-tauri/src/profile_dsl/compiler/outputs.rs
   src-tauri/src/profile_dsl/compiler/merges.rs
   src-tauri/src/profile_dsl/compiler/budgets.rs
 ```

 Phase 5: Detection auf Strategy Sets heben

 Heute bleibt Detection separat unter source_profile/detection/. Die Migration sollte bewusst später erfolgen.

 Neue Dateien:

 ```text
   src-tauri/src/source_profile/documents/detection.rs
   src-tauri/src/source_profile/detection/strategy.rs
   src-tauri/src/source_profile/detection/policy.rs
   src-tauri/src/source_profile/detection/reducer.rs
 ```

 Ändern:

 ```text
   src-tauri/src/source_profile/documents.rs
   src-tauri/src/source_profile/detection/mod.rs
   src-tauri/src/source_profile/detection/http.rs
   src-tauri/src/source_profile/detection/browser.rs
   src-tauri/src/source_profile/detection/proposal.rs
   src-tauri/src/source_profile/detection/templates.rs
   src-tauri/src/schema/source-profile.schema.json
   src-tauri/tests/source_profile_detection.rs
 ```

 proposal.rs sollte der einzige Ort bleiben, der aus akzeptierter Evidenz eine SourceProposal konstruiert.

 Phase 6: Detailfelder und Search-Run-Finalisierung

 Ändern:

 ```text
   src-tauri/src/schema/profile-dsl/common.schema.json
   src-tauri/src/schema/profile-dsl/strategy.schema.json

   src-tauri/src/profile_dsl/documents/posting_discovery.rs
   src-tauri/src/profile_dsl/documents/posting_detail.rs

   src-tauri/src/profile_dsl/execution_plan/posting_discovery.rs
   src-tauri/src/profile_dsl/execution_plan/posting_detail.rs

   src-tauri/src/profile_dsl/runtime/posting_detail.rs
   src-tauri/src/profile_dsl/runtime/posting_detail/strategy.rs
   src-tauri/src/profile_dsl/runtime/posting_detail/acceptance.rs
   src-tauri/src/profile_dsl/runtime/posting_detail/reducer.rs
 ```

 Neue Search-Run-Datei:

 ```text
   src-tauri/src/search/run/service/finalization.rs
 ```

 Weitere Änderungen:

 ```text
   src-tauri/src/search/run/types.rs
   src-tauri/src/search/run/execution.rs
   src-tauri/src/search/run/service/runner.rs
   src-tauri/src/search/run/service/rules.rs
   src-tauri/src/search/run/service/persistence.rs
   src-tauri/src/search/posting/service.rs
 ```

 Pipeline:

 ```text
   Discovery
     → konservativer Hint-Vorfilter
     → bounded Detail für plausible Kandidaten
     → zentrale Normalisierung
     → finale Regeln
     → Persistenz
 ```

 Phase 7: Direkte strukturelle Source-Spezialisierung einführen

 Neu:

 ```text
   src-tauri/src/schema/profile-dsl/source-profile-fragment.schema.json
   src-tauri/src/profile_dsl/documents/source_profile_fragment.rs
   src-tauri/src/profile_dsl/compiler/source_specialization.rs
   src-tauri/tests/profile_dsl_source_specialization.rs
 ```

 Ändern:

 ```text
   src-tauri/src/schema/source.schema.json
   src-tauri/src/source/documents.rs
   src-tauri/src/profile_dsl/compiler/resolution.rs
   src-tauri/src/checks/source_live/mod.rs
 ```

 Nach dem Schema-v3-Hard-Cut entfernen oder ersetzen:

 ```text
   src-tauri/src/schema/profile-dsl/overrides.schema.json
   src-tauri/src/profile_dsl/documents/overrides.rs
   src-tauri/src/profile_dsl/compiler/overrides.rs
 ```

 Der Compiler arbeitet nicht mit einer öffentlichen JSON-Patch-Sprache und benötigt keine `override`-/`overlay`-Zwischenebene. Er lädt das referenzierte Basisprofil, liest die direkt auf Source-Root vorhandenen Profilfragmente, merged sie strukturell in dieselbe Dokumentform, validiert das vollständige effektive Ausführungsprofil und löst erst danach Source Config und ausgewählten Access Path auf.

 Überschreibbar oder ergänzbar sind grundsätzlich alle ausführungsrelevanten und source-bezogenen Bereiche des Source Profiles:

 - Source-bezogene Supporthinweise und Diagnostics;
 - Source Config Schema;
 - Access Paths;
 - Policies und Acceptance;
 - bestehende und neue Strategies;
 - Fetch, Pagination, Parse, Select, Predicates, Captures, Extract und Transforms.

 Zunächst verboten:

 - Strategies oder Access Paths löschen oder umsortieren, solange dafür noch keine explizite Semantik existiert;
 - `detection` in einer bereits bestehenden Source definieren;
 - `schemaVersion`, Profile-Key, Profile-Name oder Profile-Kind überschreiben;
 - Search-Request-Kriterien in das Profil verschieben;
 - Array-Merge nach Index;
 - `null` als implizite Löschoperation;
 - unbekannte oder beliebige JSON Pointer.

 Phase 8: Built-ins migrieren und beweisen

 Ändern:

 ```text
   src-tauri/resources/profiles/greenhouse.json
   src-tauri/resources/profiles/workday.json
   src-tauri/resources/profiles/successfactors.json
 ```

 Neue Integration Tests:

 ```text
   src-tauri/tests/profile_dsl_strategy_sets.rs
   src-tauri/tests/profile_dsl_typing.rs
   src-tauri/tests/profile_dsl_budgets.rs
   src-tauri/tests/profile_dsl_merge.rs
   src-tauri/tests/profile_dsl_source_specialization.rs
   src-tauri/tests/profile_dsl_phase_contracts.rs
   src-tauri/tests/search_run_candidate_finalization.rs
 ```

 Bestehende Tests erweitern:

 ```text
   src-tauri/tests/profile_dsl_profiles/schema.rs
   src-tauri/tests/source_profile_detection.rs
   src-tauri/tests/profile_dsl_profiles/greenhouse.rs
   src-tauri/tests/profile_dsl_profiles/workday.rs
   src-tauri/tests/profile_dsl_profiles/successfactors.rs
   src-tauri/tests/source_live_check.rs
 ```

 ────────────────────────────────────────────────────────────────────────────────

 4. Empfohlene Reihenfolge

 Nicht sofort Schema v3 komplett implementieren.

 1. Phasenverträge und Policy-Semantik als PRD/ADR festlegen.
 2. first_accepted als explizite, rückwärtskompatible Policy einführen.
 3. Bestehende Discovery-/Detail-Duplikation hinter gemeinsame Primitive-Implementierungen ziehen.
 4. collect_all und all_required mit Reducern ergänzen.
 5. Detection auf dasselbe Strategy-Set-Modell migrieren.
 6. Erst dann hint/provider_value und erweiterte Detailfelder einführen.
 7. Greenhouse, Workday und SuccessFactors als drei Akzeptanzfälle verwenden.
 8. Schema-v3-Hard-Cut durchführen: `detection`/`discovery`/`detail`, direkte strukturelle Source-Spezialisierung ohne Wrapper-Ebene und konsistente Rust-/Dateinamen.
 9. Weitere Primitives nur evidenzbasiert aus dem Katalog auswählen.

 So bekommen wir eine flexible gemeinsame Algebra, ohne die Profile DSL sofort in eine universelle Programmiersprache zu verwandeln.
