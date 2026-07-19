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
use crate::profile_dsl::documents::AccessPathFragment;
use crate::source_profile::documents::SourceProfileDocument;

use super::{compiler_error, has_error_diagnostics};

#[derive(Clone, Copy)]
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
            Self::AccessPath => &["name", "postingDiscovery"],
            Self::DiscoveryStrategy | Self::DetailStrategy => {
                &["extract", "fetch", "parse", "select"]
            }
        }
    }
}

pub(super) fn specialize_profile(
    base: &SourceProfileDocument,
    fragments: Option<&[AccessPathFragment]>,
    diagnostics: &mut Diagnostics,
) -> Option<SourceProfileDocument> {
    let Some(fragments) = fragments else {
        return Some(base.clone());
    };

    let mut profile = serde_json::to_value(base).expect("Source Profile documents must serialize");
    let fragment_values = fragments
        .iter()
        .map(|fragment| {
            serde_json::to_value(fragment).expect("typed Access Path fragments must serialize")
        })
        .collect::<Vec<_>>();
    let initial_diagnostic_count = diagnostics.len();

    let base_paths = profile
        .get("accessPaths")
        .and_then(Value::as_array)
        .expect("Source Profile accessPaths must serialize as an array")
        .clone();
    let effective_paths = merge_keyed_collection(
        &base_paths,
        &fragment_values,
        KeyedEntryKind::AccessPath,
        "/accessPaths",
        diagnostics,
    );
    profile["accessPaths"] = Value::Array(effective_paths);

    if diagnostics.len() != initial_diagnostic_count && has_error_diagnostics(diagnostics) {
        return None;
    }

    match serde_json::from_value(profile) {
        Ok(profile) => Some(profile),
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

fn merge_keyed_collection(
    base: &[Value],
    fragments: &[Value],
    kind: KeyedEntryKind,
    path: &str,
    diagnostics: &mut Diagnostics,
) -> Vec<Value> {
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

    for base_entry in base {
        let key = entry_key(base_entry);
        match fragments_by_key.get(key) {
            Some((fragment_index, fragment)) => effective.push(merge_keyed_entry(
                Some(base_entry),
                fragment,
                kind,
                &format!("{path}/{fragment_index}"),
                diagnostics,
            )),
            None => effective.push(base_entry.clone()),
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
        effective.push(merge_keyed_entry(
            None,
            fragment,
            kind,
            &fragment_path,
            diagnostics,
        ));
    }

    effective
}

fn merge_keyed_entry(
    base: Option<&Value>,
    fragment: &Value,
    kind: KeyedEntryKind,
    path: &str,
    diagnostics: &mut Diagnostics,
) -> Value {
    let base_object = base.and_then(Value::as_object);
    let fragment_object = fragment
        .as_object()
        .expect("typed keyed fragments must serialize as objects");
    let mut effective = base_object.cloned().unwrap_or_default();

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
        if (field == "name" && matches!(kind, KeyedEntryKind::AccessPath) && base.is_some())
            || (matches!(kind, KeyedEntryKind::AccessPath)
                && matches!(field.as_str(), "postingDiscovery" | "postingDetail"))
        {
            continue;
        }
        let merged = merge_value(effective.get(field), fragment_value);
        effective.insert(field.clone(), merged);
    }

    if matches!(kind, KeyedEntryKind::AccessPath) {
        for (field, strategy_kind) in [
            ("postingDiscovery", KeyedEntryKind::DiscoveryStrategy),
            ("postingDetail", KeyedEntryKind::DetailStrategy),
        ] {
            let Some(fragment_value) = fragment_object.get(field) else {
                continue;
            };
            let merged = merge_phase_step(
                base_object.and_then(|object| object.get(field)),
                fragment_value,
                strategy_kind,
                &format!("{path}/{field}"),
                diagnostics,
            );
            effective.insert(field.to_string(), merged);
        }
    }

    Value::Object(effective)
}

fn merge_phase_step(
    base: Option<&Value>,
    fragment: &Value,
    strategy_kind: KeyedEntryKind,
    path: &str,
    diagnostics: &mut Diagnostics,
) -> Value {
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
    for (field, fragment_value) in fragment_object {
        let merged = if field == "strategies" {
            let empty = Vec::new();
            let base_strategies = base_object
                .and_then(|object| object.get(field))
                .and_then(Value::as_array)
                .unwrap_or(&empty);
            Value::Array(merge_keyed_collection(
                base_strategies,
                fragment_value
                    .as_array()
                    .expect("typed Strategy fragments must serialize as an array"),
                strategy_kind,
                &format!("{path}/strategies"),
                diagnostics,
            ))
        } else {
            merge_value(effective.get(field), fragment_value)
        };
        effective.insert(field.clone(), merged);
    }
    Value::Object(effective)
}

fn merge_value(base: Option<&Value>, fragment: &Value) -> Value {
    match (base, fragment) {
        (Some(Value::Object(base)), Value::Object(fragment)) => {
            let mut effective = base.clone();
            for (key, fragment_value) in fragment {
                let merged = merge_value(effective.get(key), fragment_value);
                effective.insert(key.clone(), merged);
            }
            Value::Object(effective)
        }
        _ => fragment.clone(),
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
        require_fields(field_match, "match", &["left", "right"], missing);
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
    require_fields(extract, "extract", &["fields"], missing);
    let Some(fields) = extract.get("fields") else {
        return;
    };
    let required: &[&str] = match kind {
        KeyedEntryKind::DiscoveryStrategy => &["title", "company", "url"],
        KeyedEntryKind::DetailStrategy => &["descriptionText"],
        KeyedEntryKind::AccessPath => unreachable!(),
    };
    require_fields(fields, "extract.fields", required, missing);
    if let Some(object) = fields.as_object() {
        for (field, expression) in object {
            if field == "postingMeta" {
                if let Some(values) = expression.as_object() {
                    for (key, expression) in values {
                        collect_expression_missing_fields(
                            expression,
                            &format!("extract.fields.postingMeta.{key}"),
                            missing,
                        );
                    }
                }
            } else if field == "locations" {
                match expression {
                    Value::Array(expressions) => {
                        for (index, expression) in expressions.iter().enumerate() {
                            collect_expression_missing_fields(
                                expression,
                                &format!("extract.fields.locations.{index}"),
                                missing,
                            );
                        }
                    }
                    _ => collect_expression_missing_fields(
                        expression,
                        "extract.fields.locations",
                        missing,
                    ),
                }
            } else {
                collect_expression_missing_fields(
                    expression,
                    &format!("extract.fields.{field}"),
                    missing,
                );
            }
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
        _ => &[],
    };
    require_fields(expression, prefix, required, missing);
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
