# Logische Predicate-Primitives

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
     "value": "jobs.example.test",
     "strategyKey": "final_url_marker",
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
