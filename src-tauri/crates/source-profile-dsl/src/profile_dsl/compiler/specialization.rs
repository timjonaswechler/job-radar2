//! Private deterministic construction of an Effective Source Profile.
//!
//! Access Paths and Strategies merge by stable key. Existing entries retain
//! base order, complete additions append in first-fragment order, objects merge
//! recursively, and every unkeyed array replaces as a whole. Completeness and
//! duplicate diagnostics point to authored direct-fragment array locations.
//! The complete result is materialized before whole-profile validation; no
//! fragment representation crosses into the Execution Plan or runtime.

use std::collections::{BTreeMap, HashSet};

use serde_json::Value;

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::{
    AccessPathFragment, Captures, DetailStepFragment, DiscoveryStepFragment, PhaseLimits,
};
use crate::source_profile::documents::SourceProfileDocument;

use super::provenance::{self, OriginTree, ProvenanceOrigin, RecordedProvenance};
use super::{compiler_error, has_error_diagnostics};

#[derive(Clone, Copy, Eq, PartialEq)]
enum KeyedEntryKind {
    AccessPath,
    DiscoveryStrategy,
    DetailStrategy,
}

impl KeyedEntryKind {
    fn label(self) -> &'static str {
        match self {
            Self::AccessPath => "Access Path",
            Self::DiscoveryStrategy | Self::DetailStrategy => "Strategy",
        }
    }

    fn required_fields(self) -> &'static [&'static str] {
        match self {
            Self::AccessPath => &["name", "discovery"],
            Self::DiscoveryStrategy | Self::DetailStrategy => {
                &["extract", "fetch", "parse", "select"]
            }
        }
    }
}

pub(super) fn specialize_profile_with_provenance(
    base: &SourceProfileDocument,
    fragments: Option<&[AccessPathFragment]>,
    diagnostics: &mut Diagnostics,
) -> Option<(SourceProfileDocument, RecordedProvenance)> {
    let (materialized, origins) = materialize_serialized_profile(base, fragments, diagnostics)?;
    // Finalize typed paths from the merge's own materialization and origin
    // tree before constructing the completed typed document.
    let provenance = provenance::profile_provenance(&materialized, &origins);
    let mut profile = deserialize_effective_profile(materialized, diagnostics)?;
    restore_capture_order(&mut profile, base, fragments);
    Some((profile, provenance))
}

fn restore_capture_order(
    profile: &mut SourceProfileDocument,
    base: &SourceProfileDocument,
    fragments: Option<&[AccessPathFragment]>,
) {
    for path in &mut profile.access_paths {
        let base_path = base
            .access_paths
            .iter()
            .find(|candidate| candidate.key == path.key);
        let fragment_path = fragments
            .into_iter()
            .flatten()
            .find(|candidate| candidate.key == path.key);

        restore_discovery_capture_order(
            &mut path.discovery,
            base_path.map(|path| &path.discovery),
            fragment_path.and_then(|path| path.discovery.as_ref()),
        );
        if let Some(detail) = &mut path.detail {
            restore_detail_capture_order(
                detail,
                base_path.and_then(|path| path.detail.as_ref()),
                fragment_path.and_then(|path| path.detail.as_ref()),
            );
        }
    }
}

fn restore_discovery_capture_order(
    effective: &mut crate::profile_dsl::documents::DiscoveryStep,
    base: Option<&crate::profile_dsl::documents::DiscoveryStep>,
    fragment: Option<&DiscoveryStepFragment>,
) {
    for strategy in &mut effective.strategies {
        let base_captures = base
            .and_then(|step| step.strategies.iter().find(|item| item.key == strategy.key))
            .and_then(|item| item.captures.as_ref());
        let fragment_captures = fragment
            .and_then(|step| step.strategies.as_ref())
            .and_then(|items| items.iter().find(|item| item.key == strategy.key))
            .and_then(|item| item.captures.as_ref());
        restore_capture_map_order(
            strategy.captures.as_mut(),
            base_captures.map(|captures| captures.keys()),
            fragment_captures.map(|captures| captures.keys()),
        );
    }
}

fn restore_detail_capture_order(
    effective: &mut crate::profile_dsl::documents::DetailStep,
    base: Option<&crate::profile_dsl::documents::DetailStep>,
    fragment: Option<&DetailStepFragment>,
) {
    for strategy in &mut effective.strategies {
        let base_captures = base
            .and_then(|step| step.strategies.iter().find(|item| item.key == strategy.key))
            .and_then(|item| item.captures.as_ref());
        let fragment_captures = fragment
            .and_then(|step| step.strategies.as_ref())
            .and_then(|items| items.iter().find(|item| item.key == strategy.key))
            .and_then(|item| item.captures.as_ref());
        restore_capture_map_order(
            strategy.captures.as_mut(),
            base_captures.map(|captures| captures.keys()),
            fragment_captures.map(|captures| captures.keys()),
        );
    }
}

fn restore_capture_map_order<'a, B, F>(
    effective: Option<&mut Captures>,
    base_keys: Option<B>,
    fragment_keys: Option<F>,
) where
    B: Iterator<Item = &'a String>,
    F: Iterator<Item = &'a String>,
{
    let Some(effective) = effective else {
        return;
    };
    let mut desired = Vec::new();
    let mut seen = HashSet::new();
    for key in base_keys
        .into_iter()
        .flatten()
        .chain(fragment_keys.into_iter().flatten())
    {
        if seen.insert(key.clone()) {
            desired.push(key.clone());
        }
    }

    let mut current = std::mem::take(effective);
    for key in desired {
        if let Some(rule) = current.shift_remove(&key) {
            effective.insert(key, rule);
        }
    }
    effective.extend(current);
}

fn materialize_serialized_profile<T, F>(
    base: &T,
    fragments: Option<&[F]>,
    diagnostics: &mut Diagnostics,
) -> Option<(Value, OriginTree)>
where
    T: serde::Serialize,
    F: serde::Serialize,
{
    let mut profile = serde_json::to_value(base).expect("Source Profile documents must serialize");
    let mut profile_origins = OriginTree::for_value(&profile, ProvenanceOrigin::BaseSourceProfile);
    let profile_origin_object = match &mut profile_origins {
        OriginTree::Object(object) => object,
        _ => unreachable!("Source Profile serializes as an object"),
    };
    let base_paths = profile
        .get("accessPaths")
        .and_then(Value::as_array)
        .expect("Source Profile accessPaths must serialize as an array")
        .clone();
    let base_path_origins = base_paths
        .iter()
        .map(|value| OriginTree::for_access_path(value, ProvenanceOrigin::BaseSourceProfile))
        .collect::<Vec<_>>();

    let Some(fragments) = fragments else {
        profile_origin_object.insert(
            "accessPaths".to_string(),
            OriginTree::Keyed(base_path_origins),
        );
        return Some((profile, profile_origins));
    };

    let fragment_values = fragments
        .iter()
        .map(|fragment| {
            serde_json::to_value(fragment).expect("typed Access Path fragments must serialize")
        })
        .collect::<Vec<_>>();
    validate_direct_source_config_schema_fragments(&fragment_values, diagnostics);
    let initial_diagnostic_count = diagnostics.len();

    let (effective_paths, effective_origins) = merge_keyed_collection(
        &base_paths,
        &base_path_origins,
        &fragment_values,
        KeyedEntryKind::AccessPath,
        "/accessPaths",
        diagnostics,
    );
    profile["accessPaths"] = Value::Array(effective_paths);
    profile_origin_object.insert(
        "accessPaths".to_string(),
        OriginTree::Keyed(effective_origins),
    );

    if diagnostics.len() != initial_diagnostic_count && has_error_diagnostics(diagnostics) {
        return None;
    }

    Some((profile, profile_origins))
}

fn deserialize_effective_profile<T: serde::de::DeserializeOwned>(
    materialized: Value,
    diagnostics: &mut Diagnostics,
) -> Option<T> {
    match serde_json::from_value(materialized) {
        Ok(document) => Some(document),
        Err(error) => {
            diagnostics.push(compiler_error(
                "invalid_effective_profile_fragment",
                format!("Direct Source fragments did not produce a complete valid Source Profile: {error}"),
                "/accessPaths",
                serde_json::json!({ "reason": error.to_string() }),
            ));
            None
        }
    }
}

fn validate_direct_source_config_schema_fragments(
    fragments: &[Value],
    diagnostics: &mut Diagnostics,
) {
    for (fragment_index, fragment) in fragments.iter().enumerate() {
        let Some(properties) = fragment
            .get("sourceConfigSchema")
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
        else {
            continue;
        };
        for (property, schema) in properties {
            if schema.get("title").is_some() {
                diagnostics.push(compiler_error(
                    "source_config_schema_title_not_allowed",
                    format!("Direct Source Config schema fragment cannot add or replace title for `{property}`"),
                    format!(
                        "/accessPaths/{fragment_index}/sourceConfigSchema/properties/{}/title",
                        crate::profile_dsl::source_config::escape_pointer_segment(property)
                    ),
                    serde_json::json!({ "property": property }),
                ));
            }
        }
    }
}

fn merge_keyed_collection(
    base: &[Value],
    base_origins: &[OriginTree],
    fragments: &[Value],
    kind: KeyedEntryKind,
    path: &str,
    diagnostics: &mut Diagnostics,
) -> (Vec<Value>, Vec<OriginTree>) {
    let mut seen = HashSet::new();
    let mut unique_fragments = Vec::new();

    for (index, fragment) in fragments.iter().enumerate() {
        let key = entry_key(fragment);
        if !seen.insert(key.to_string()) {
            diagnostics.push(compiler_error(
                "duplicate_profile_fragment_key",
                format!(
                    "Direct Source fragments contain more than one {} with key `{key}`",
                    kind.label()
                ),
                format!("{path}/{index}/key"),
                serde_json::json!({ "key": key, "entryKind": kind.label() }),
            ));
            continue;
        }
        unique_fragments.push((index, fragment));
    }

    let fragments_by_key = unique_fragments
        .iter()
        .map(|(index, fragment)| (entry_key(fragment), (*index, *fragment)))
        .collect::<BTreeMap<_, _>>();
    let base_keys = base.iter().map(entry_key).collect::<HashSet<_>>();
    let mut effective = Vec::with_capacity(base.len() + fragments.len());
    let mut effective_origins = Vec::with_capacity(base.len() + fragments.len());

    for (base_entry, base_origin) in base.iter().zip(base_origins) {
        let key = entry_key(base_entry);
        match fragments_by_key.get(key) {
            Some((fragment_index, fragment)) => {
                let (value, origins) = merge_keyed_entry(
                    Some(base_entry),
                    Some(base_origin),
                    fragment,
                    kind,
                    &format!("{path}/{fragment_index}"),
                    diagnostics,
                );
                effective.push(value);
                effective_origins.push(origins);
            }
            None => {
                effective.push(base_entry.clone());
                effective_origins.push(base_origin.clone());
            }
        }
    }

    for (fragment_index, fragment) in unique_fragments {
        if base_keys.contains(entry_key(fragment)) {
            continue;
        }
        let fragment_path = format!("{path}/{fragment_index}");
        if !validate_complete_addition(fragment, kind, &fragment_path, diagnostics) {
            continue;
        }
        let (value, origins) =
            merge_keyed_entry(None, None, fragment, kind, &fragment_path, diagnostics);
        effective.push(value);
        effective_origins.push(origins);
    }

    (effective, effective_origins)
}

fn merge_keyed_entry(
    base: Option<&Value>,
    base_origins: Option<&OriginTree>,
    fragment: &Value,
    kind: KeyedEntryKind,
    path: &str,
    diagnostics: &mut Diagnostics,
) -> (Value, OriginTree) {
    let base_object = base.and_then(Value::as_object);
    let fragment_object = fragment
        .as_object()
        .expect("typed keyed fragments must serialize as objects");
    let mut effective = base_object.cloned().unwrap_or_default();
    let mut effective_origins = base_origins
        .and_then(OriginTree::object)
        .cloned()
        .unwrap_or_default();

    if matches!(kind, KeyedEntryKind::AccessPath)
        && base.is_some()
        && fragment_object.contains_key("name")
    {
        diagnostics.push(compiler_error(
            "access_path_name_not_specializable",
            "An existing Access Path name cannot be specialized; name is admitted only for a complete new Access Path",
            format!("{path}/name"),
            serde_json::json!({ "accessPathKey": entry_key(fragment) }),
        ));
        effective.remove("name");
        if let Some(name) = base_object.and_then(|object| object.get("name")) {
            effective.insert("name".to_string(), name.clone());
        }
    }

    for (field, fragment_value) in fragment_object {
        if (field == "key" && base.is_some())
            || (field == "name" && matches!(kind, KeyedEntryKind::AccessPath) && base.is_some())
            || (matches!(kind, KeyedEntryKind::AccessPath)
                && matches!(field.as_str(), "discovery" | "detail"))
        {
            continue;
        }
        let (merged, origins) = merge_value(
            effective.get(field),
            effective_origins.get(field),
            fragment_value,
        );
        effective.insert(field.clone(), merged);
        effective_origins.insert(field.clone(), origins);
    }

    if matches!(kind, KeyedEntryKind::AccessPath) {
        for (field, strategy_kind) in [
            ("discovery", KeyedEntryKind::DiscoveryStrategy),
            ("detail", KeyedEntryKind::DetailStrategy),
        ] {
            let Some(fragment_value) = fragment_object.get(field) else {
                continue;
            };
            let (merged, origins) = merge_phase_step(
                base_object.and_then(|object| object.get(field)),
                base_origins
                    .and_then(OriginTree::object)
                    .and_then(|object| object.get(field)),
                fragment_value,
                strategy_kind,
                &format!("{path}/{field}"),
                diagnostics,
            );
            effective.insert(field.to_string(), merged);
            effective_origins.insert(field.to_string(), origins);
        }
    }

    // Existing keyed locators remain base even when repeated by a fragment;
    // complete additions have no base and are direct throughout.
    if base.is_none() {
        effective_origins = match OriginTree::for_access_path(
            &Value::Object(effective.clone()),
            ProvenanceOrigin::DirectSourceFragment,
        ) {
            OriginTree::Object(object) => object,
            _ => unreachable!(),
        };
    }
    (
        Value::Object(effective),
        OriginTree::Object(effective_origins),
    )
}

fn merge_phase_step(
    base: Option<&Value>,
    base_origins: Option<&OriginTree>,
    fragment: &Value,
    strategy_kind: KeyedEntryKind,
    path: &str,
    diagnostics: &mut Diagnostics,
) -> (Value, OriginTree) {
    let fragment_object = fragment
        .as_object()
        .expect("typed phase fragments must serialize as objects");
    if base.is_none() && !fragment_object.contains_key("strategies") {
        push_incomplete_diagnostic(
            "phase step",
            entry_key_from_path(path),
            vec!["strategies".to_string()],
            path,
            diagnostics,
        );
    }

    let base_object = base.and_then(Value::as_object);
    let mut effective = base_object.cloned().unwrap_or_default();
    let mut effective_origins = base_origins
        .and_then(OriginTree::object)
        .cloned()
        .unwrap_or_default();
    for (field, fragment_value) in fragment_object {
        let (merged, origins) = if field == "strategies" {
            let empty = Vec::new();
            let base_strategies = base_object
                .and_then(|object| object.get(field))
                .and_then(Value::as_array)
                .unwrap_or(&empty);
            let empty_origins = Vec::new();
            let base_strategy_origins = match effective_origins.get(field) {
                Some(OriginTree::Keyed(origins)) => origins,
                _ => &empty_origins,
            };
            let (values, origins) = merge_keyed_collection(
                base_strategies,
                base_strategy_origins,
                fragment_value
                    .as_array()
                    .expect("typed Strategy fragments must serialize as an array"),
                strategy_kind,
                &format!("{path}/strategies"),
                diagnostics,
            );
            (Value::Array(values), OriginTree::Keyed(origins))
        } else if field == "limits" {
            let backend =
                serde_json::to_value(PhaseLimits::BACKEND).expect("backend limits serialize");
            let inherited = effective.get(field).unwrap_or(&backend);
            validate_tightened_phase_limits(
                inherited,
                fragment_value,
                &format!("{path}/limits"),
                diagnostics,
            );
            merge_value(
                Some(inherited),
                effective_origins.get(field),
                fragment_value,
            )
        } else {
            merge_value(
                effective.get(field),
                effective_origins.get(field),
                fragment_value,
            )
        };
        effective.insert(field.clone(), merged);
        effective_origins.insert(field.clone(), origins);
    }
    (
        Value::Object(effective),
        OriginTree::Object(effective_origins),
    )
}

fn validate_tightened_phase_limits(
    inherited: &Value,
    fragment: &Value,
    path: &str,
    diagnostics: &mut Diagnostics,
) {
    let Some(fragment) = fragment.as_object() else {
        return;
    };
    let inherited = inherited
        .as_object()
        .expect("complete phase limits serialize as an object");
    for (field, value) in fragment {
        let Some(value) = value.as_u64() else {
            continue;
        };
        let inherited_value = inherited
            .get(field)
            .and_then(Value::as_u64)
            .expect("inherited phase limit exists");
        if value > inherited_value {
            diagnostics.push(compiler_error(
                "phase_limit_cannot_raise_inherited",
                format!("{field} may only tighten the inherited limit of {inherited_value}"),
                format!("{path}/{field}"),
                serde_json::json!({ "value": value, "inheritedLimit": inherited_value }),
            ));
        }
    }
}

fn merge_value(
    base: Option<&Value>,
    base_origins: Option<&OriginTree>,
    fragment: &Value,
) -> (Value, OriginTree) {
    match (base, fragment) {
        (Some(Value::Object(base)), Value::Object(fragment)) => {
            let mut effective = base.clone();
            let mut origins = base_origins
                .and_then(OriginTree::object)
                .cloned()
                .unwrap_or_default();
            for (key, fragment_value) in fragment {
                let (merged, merged_origins) =
                    merge_value(effective.get(key), origins.get(key), fragment_value);
                effective.insert(key.clone(), merged);
                origins.insert(key.clone(), merged_origins);
            }
            (Value::Object(effective), OriginTree::Object(origins))
        }
        _ => (
            fragment.clone(),
            OriginTree::for_value(fragment, ProvenanceOrigin::DirectSourceFragment),
        ),
    }
}

fn validate_complete_addition(
    fragment: &Value,
    kind: KeyedEntryKind,
    path: &str,
    diagnostics: &mut Diagnostics,
) -> bool {
    let object = fragment
        .as_object()
        .expect("typed keyed fragments must serialize as objects");
    let mut missing_fields = kind
        .required_fields()
        .iter()
        .filter(|field| !object.contains_key(**field))
        .map(|field| (*field).to_string())
        .collect::<Vec<_>>();
    if matches!(
        kind,
        KeyedEntryKind::DiscoveryStrategy | KeyedEntryKind::DetailStrategy
    ) {
        collect_strategy_missing_fields(fragment, kind, &mut missing_fields);
    }
    missing_fields.sort();
    missing_fields.dedup();
    if missing_fields.is_empty() {
        return true;
    }

    push_incomplete_diagnostic(
        kind.label(),
        entry_key(fragment),
        missing_fields,
        path,
        diagnostics,
    );
    false
}

fn push_incomplete_diagnostic(
    entry_kind: &str,
    key: &str,
    missing_fields: Vec<String>,
    path: &str,
    diagnostics: &mut Diagnostics,
) {
    diagnostics.push(compiler_error(
        "incomplete_profile_fragment_addition",
        format!(
            "New {entry_kind} `{key}` is incomplete; missing fields: {}",
            missing_fields.join(", ")
        ),
        path,
        serde_json::json!({
            "entryKind": entry_kind,
            "key": key,
            "missingFields": missing_fields,
        }),
    ));
}

fn collect_strategy_missing_fields(
    strategy: &Value,
    kind: KeyedEntryKind,
    missing: &mut Vec<String>,
) {
    let object = strategy
        .as_object()
        .expect("typed Strategy fragment object");
    if let Some(fetch) = object.get("fetch") {
        collect_fetch_missing_fields(fetch, "fetch", missing);
    }
    if let Some(pagination) = object.get("pagination") {
        collect_pagination_missing_fields(pagination, "pagination", missing);
    }
    if let Some(parse) = object.get("parse") {
        require_fields(parse, "parse", &["type"], missing);
    }
    if let Some(select) = object.get("select") {
        collect_select_missing_fields(select, "select", missing);
    }
    if let Some(captures) = object.get("captures").and_then(Value::as_object) {
        for (key, capture) in captures {
            let prefix = format!("captures.{key}");
            require_fields(capture, &prefix, &["from", "pattern"], missing);
            if let Some(expression) = capture.get("from") {
                collect_expression_missing_fields(expression, &format!("{prefix}.from"), missing);
            }
        }
    }
    if let Some(field_match) = object.get("match") {
        require_fields(field_match, "match", &["type", "left", "right"], missing);
        if let Some(left) = field_match.get("left") {
            collect_expression_missing_fields(left, "match.left", missing);
        }
        if let Some(right) = field_match.get("right") {
            collect_expression_missing_fields(right, "match.right", missing);
        }
    }
    if let Some(extract) = object.get("extract") {
        collect_extract_missing_fields(extract, kind, missing);
    }
}

fn collect_fetch_missing_fields(fetch: &Value, prefix: &str, missing: &mut Vec<String>) {
    require_fields(fetch, prefix, &["mode", "url", "timeoutMs"], missing);
    let Some(object) = fetch.as_object() else {
        return;
    };
    if let Some(body) = object.get("body") {
        require_fields(body, &format!("{prefix}.body"), &["type"], missing);
        let body_type = body.get("type").and_then(Value::as_str);
        let required = match body_type {
            Some("json" | "text") => Some("value"),
            Some("form") => Some("fields"),
            _ => None,
        };
        if let Some(required) = required {
            require_fields(body, &format!("{prefix}.body"), &[required], missing);
        }
    }
}

fn collect_pagination_missing_fields(pagination: &Value, prefix: &str, missing: &mut Vec<String>) {
    require_fields(pagination, prefix, &["type", "limits"], missing);
    let Some(object) = pagination.as_object() else {
        return;
    };
    let required: &[&str] = match object.get("type").and_then(Value::as_str) {
        Some("page") => &["pageParam"],
        Some("offset_limit") => &["offsetParam", "limitParam", "limit"],
        Some("cursor") => &["cursorParam", "nextCursorPath"],
        Some("sitemap") | None | Some(_) => &[],
    };
    require_fields(pagination, prefix, required, missing);
    if let Some(limits) = object.get("limits") {
        require_fields(
            limits,
            &format!("{prefix}.limits"),
            &["maxRequests"],
            missing,
        );
    }
}

fn collect_select_missing_fields(select: &Value, prefix: &str, missing: &mut Vec<String>) {
    require_fields(select, prefix, &["type"], missing);
    let Some(object) = select.as_object() else {
        return;
    };
    let required = match object.get("type").and_then(Value::as_str) {
        Some("json_path") => Some("jsonPath"),
        Some("xml_element") => Some("element"),
        Some("xml_text") => Some("textPath"),
        Some("css") => Some("selector"),
        _ => None,
    };
    if let Some(required) = required {
        require_fields(select, prefix, &[required], missing);
    }
}

fn collect_extract_missing_fields(
    extract: &Value,
    kind: KeyedEntryKind,
    missing: &mut Vec<String>,
) {
    if kind == KeyedEntryKind::DetailStrategy {
        require_fields(extract, "extract", &["fields"], missing);
        if let Some(fields) = extract.get("fields").and_then(Value::as_object) {
            if fields.is_empty() {
                missing.push("extract.fields".to_string());
            }
            for (field, expression) in fields {
                collect_expression_missing_fields(
                    expression,
                    &format!("extract.fields.{field}"),
                    missing,
                );
            }
        }
        return;
    }

    require_fields(extract, "extract", &["reference"], missing);
    let Some(object) = extract.as_object() else {
        return;
    };
    if let Some(reference) = object.get("reference") {
        require_fields(reference, "extract.reference", &["url"], missing);
        if let Some(reference) = reference.as_object() {
            for (field, expression) in reference {
                collect_expression_missing_fields(
                    expression,
                    &format!("extract.reference.{field}"),
                    missing,
                );
            }
        }
    }
    if let Some(values) = object.get("providerValues").and_then(Value::as_object) {
        for (field, expression) in values {
            if field == "locations" {
                match expression {
                    Value::Array(expressions) => {
                        for (index, expression) in expressions.iter().enumerate() {
                            collect_expression_missing_fields(
                                expression,
                                &format!("extract.providerValues.locations.{index}"),
                                missing,
                            );
                        }
                    }
                    _ => collect_expression_missing_fields(
                        expression,
                        "extract.providerValues.locations",
                        missing,
                    ),
                }
            } else {
                collect_expression_missing_fields(
                    expression,
                    &format!("extract.providerValues.{field}"),
                    missing,
                );
            }
        }
    }
    if let Some(hints) = object.get("hints").and_then(Value::as_object) {
        for (key, hint) in hints {
            require_fields(hint, &format!("extract.hints.{key}"), &["value"], missing);
            if let Some(expression) = hint.get("value") {
                collect_expression_missing_fields(
                    expression,
                    &format!("extract.hints.{key}.value"),
                    missing,
                );
            }
        }
    }
    if let Some(meta) = object.get("postingMeta").and_then(Value::as_object) {
        for (key, expression) in meta {
            collect_expression_missing_fields(
                expression,
                &format!("extract.postingMeta.{key}"),
                missing,
            );
        }
    }
}

fn collect_expression_missing_fields(expression: &Value, prefix: &str, missing: &mut Vec<String>) {
    require_fields(expression, prefix, &["type"], missing);
    let Some(object) = expression.as_object() else {
        return;
    };
    let required: &[&str] = match object.get("type").and_then(Value::as_str) {
        Some("const") => &["value"],
        Some("template") => &["template"],
        Some("source_config" | "posting_meta" | "capture" | "item_field") => &["key"],
        Some("json_path") => &["jsonPath"],
        Some("xml_text") => &["textPath"],
        Some("xml_element") => &["element"],
        Some("css_text") => &["selector"],
        Some("css_attribute") => &["selector", "attribute"],
        Some("combine") => &["parts"],
        Some("first_non_empty") => &["candidates"],
        _ => &[],
    };
    require_fields(expression, prefix, required, missing);
    if let Some(candidates) = object.get("candidates").and_then(Value::as_array) {
        for (index, candidate) in candidates.iter().enumerate() {
            collect_expression_missing_fields(
                candidate,
                &format!("{prefix}.candidates.{index}"),
                missing,
            );
        }
    }
    if let Some(parts) = object.get("parts").and_then(Value::as_array) {
        for (index, part) in parts.iter().enumerate() {
            let part_prefix = format!("{prefix}.parts.{index}");
            require_fields(part, &part_prefix, &["value"], missing);
            if let Some(value) = part.get("value") {
                collect_expression_missing_fields(value, &format!("{part_prefix}.value"), missing);
            }
        }
    }
}

fn require_fields(value: &Value, prefix: &str, required: &[&str], missing: &mut Vec<String>) {
    let object = value.as_object();
    for field in required {
        if object.map_or(true, |object| !object.contains_key(*field)) {
            missing.push(format!("{prefix}.{field}"));
        }
    }
}

fn entry_key(value: &Value) -> &str {
    value
        .get("key")
        .and_then(Value::as_str)
        .expect("typed keyed fragments must contain a string key")
}

fn entry_key_from_path(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}
