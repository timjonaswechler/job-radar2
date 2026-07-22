//! Typed, value-minimized provenance for the dormant final compiler.
//!
//! Provenance is built while the Effective Source Profile is materialized. It
//! identifies execution-relevant terminals only; values, Source Config,
//! diagnostics, runtime data, and persistence identity never enter this model.

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::source::documents::SelectedAccessPath;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceOrigin {
    BaseSourceProfile,
    DirectSourceFragment,
    SourceOwnedAccessPath,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProvenancePathSegment {
    Field { name: String },
    AccessPath { key: String },
    Strategy { key: String },
    MapKey { key: String },
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProvenancePath {
    pub segments: Vec<ProvenancePathSegment>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProvenanceEntry {
    pub path: ProvenancePath,
    pub origin: ProvenanceOrigin,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CompiledSourceProvenance {
    Profile { entries: Vec<ProvenanceEntry> },
    SourceOwned { entries: Vec<ProvenanceEntry> },
}

/// Origin tree paired with the JSON tree already used by the keyed merger.
/// Arrays are terminals, while keyed Access Path and Strategy arrays have one
/// child per semantic entry so additions and replacements retain their origin.
#[derive(Clone, Debug)]
pub(super) enum OriginTree {
    Terminal(ProvenanceOrigin),
    Object(BTreeMap<String, OriginTree>),
    Keyed(Vec<OriginTree>),
}

impl OriginTree {
    pub(super) fn for_value(value: &Value, origin: ProvenanceOrigin) -> Self {
        match value {
            Value::Object(object) if !object.is_empty() => Self::Object(
                object
                    .iter()
                    .map(|(key, value)| (key.clone(), Self::for_value(value, origin)))
                    .collect(),
            ),
            _ => Self::Terminal(origin),
        }
    }

    pub(super) fn object(&self) -> Option<&BTreeMap<String, OriginTree>> {
        match self {
            Self::Object(object) => Some(object),
            _ => None,
        }
    }

    pub(super) fn for_access_path(value: &Value, origin: ProvenanceOrigin) -> Self {
        let mut tree = Self::for_value(value, origin);
        let Self::Object(path_origins) = &mut tree else {
            return tree;
        };
        let Some(path) = value.as_object() else {
            return tree;
        };
        for phase in ["discovery", "detail"] {
            let (Some(step), Some(Self::Object(step_origins))) = (
                path.get(phase).and_then(Value::as_object),
                path_origins.get_mut(phase),
            ) else {
                continue;
            };
            let Some(strategies) = step.get("strategies").and_then(Value::as_array) else {
                continue;
            };
            step_origins.insert(
                "strategies".to_string(),
                Self::Keyed(
                    strategies
                        .iter()
                        .map(|strategy| Self::for_value(strategy, origin))
                        .collect(),
                ),
            );
        }
        tree
    }
}

pub(super) struct RecordedProvenance {
    pub(super) value: CompiledSourceProvenance,
    expected_paths: Vec<ProvenancePath>,
}

#[derive(Default)]
struct ProvenanceRecorder {
    entries: Vec<ProvenanceEntry>,
}

impl ProvenanceRecorder {
    fn finish(self, profile: bool, expected_paths: Vec<ProvenancePath>) -> RecordedProvenance {
        let value = if profile {
            CompiledSourceProvenance::Profile {
                entries: self.entries,
            }
        } else {
            CompiledSourceProvenance::SourceOwned {
                entries: self.entries,
            }
        };
        RecordedProvenance {
            value,
            expected_paths,
        }
    }
}

pub(super) fn profile_provenance(
    materialized_profile: &Value,
    origins: &OriginTree,
) -> RecordedProvenance {
    let mut recorder = ProvenanceRecorder::default();
    let object = materialized_profile
        .as_object()
        .expect("materialized Source Profile must be an object");
    let origin_object = origins
        .object()
        .expect("Source Profile origins must be an object");

    if let Some(schema) = object.get("sourceConfigSchema") {
        collect_value(
            schema,
            origin_object
                .get("sourceConfigSchema")
                .expect("schema origin"),
            &mut vec![field("sourceConfigSchema")],
            DynamicContext::Schema,
            &mut recorder,
        );
    }

    let paths = object["accessPaths"].as_array().expect("accessPaths array");
    let OriginTree::Keyed(path_origins) = origin_object.get("accessPaths").expect("path origins")
    else {
        panic!("accessPaths origins must be keyed")
    };
    for (path, origins) in paths.iter().zip(path_origins) {
        collect_access_path(path, origins, &mut recorder);
    }

    recorder.finish(true, expected_profile_paths(object))
}

pub(super) fn source_owned_provenance(selected: &SelectedAccessPath) -> RecordedProvenance {
    let value =
        serde_json::to_value(selected).expect("typed Source-owned Access Path must serialize");
    let origins = OriginTree::for_access_path(&value, ProvenanceOrigin::SourceOwnedAccessPath);
    let mut recorder = ProvenanceRecorder::default();
    collect_access_path(&value, &origins, &mut recorder);
    recorder.finish(
        false,
        expected_access_path_paths(value.as_object().expect("Access Path object")),
    )
}

fn collect_access_path(value: &Value, origins: &OriginTree, recorder: &mut ProvenanceRecorder) {
    let object = value.as_object().expect("Access Path object");
    let origin_object = origins.object().expect("Access Path origin object");
    let key = object["key"].as_str().expect("Access Path key");
    let mut path = vec![ProvenancePathSegment::AccessPath {
        key: key.to_string(),
    }];
    push_terminal(origin_object["key"].terminal(), path.clone(), recorder);

    if object.get("name").is_some() {
        path.push(field("name"));
        push_terminal(origin_object["name"].terminal(), path.clone(), recorder);
        path.pop();
    }
    if let Some(schema) = object.get("sourceConfigSchema") {
        path.push(field("sourceConfigSchema"));
        collect_value(
            schema,
            &origin_object["sourceConfigSchema"],
            &mut path,
            DynamicContext::Schema,
            recorder,
        );
        path.pop();
    }
    for phase in ["discovery", "detail"] {
        if let Some(step) = object.get(phase) {
            path.push(field(match phase {
                "discovery" => "discovery",
                "detail" => "detail",
                _ => unreachable!(),
            }));
            collect_step(step, &origin_object[phase], &mut path, recorder);
            path.pop();
        }
    }
}

fn collect_step(
    value: &Value,
    origins: &OriginTree,
    path: &mut Vec<ProvenancePathSegment>,
    recorder: &mut ProvenanceRecorder,
) {
    let object = value.as_object().expect("phase step object");
    let origin_object = origins.object().expect("phase step origins");
    for field_name in ["policy", "strategies", "acceptWhen"] {
        let Some(value) = object.get(field_name) else {
            continue;
        };
        if field_name == "strategies" {
            let strategies = value.as_array().expect("strategies array");
            let OriginTree::Keyed(strategy_origins) = &origin_object[field_name] else {
                panic!("Strategy origins must be keyed")
            };
            for (strategy, origins) in strategies.iter().zip(strategy_origins) {
                let strategy_object = strategy.as_object().expect("Strategy object");
                let key = strategy_object["key"].as_str().expect("Strategy key");
                path.push(ProvenancePathSegment::Strategy {
                    key: key.to_string(),
                });
                collect_strategy(strategy_object, origins, path, recorder);
                path.pop();
            }
        } else {
            path.push(field(field_name));
            collect_value(
                value,
                &origin_object[field_name],
                path,
                DynamicContext::Typed,
                recorder,
            );
            path.pop();
        }
    }
}

fn collect_strategy(
    object: &Map<String, Value>,
    origins: &OriginTree,
    path: &mut Vec<ProvenancePathSegment>,
    recorder: &mut ProvenanceRecorder,
) {
    let origin_object = origins.object().expect("Strategy origins");
    push_terminal(origin_object["key"].terminal(), path.clone(), recorder);
    for name in ordered_fields(object) {
        if matches!(name.as_str(), "key" | "description" | "diagnostics") {
            continue;
        }
        path.push(field(&name));
        collect_value(
            &object[&name],
            &origin_object[&name],
            path,
            dynamic_context(&name),
            recorder,
        );
        path.pop();
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum DynamicContext {
    Typed,
    DynamicMap,
    RecursiveMap,
    Schema,
}

fn collect_value(
    value: &Value,
    origins: &OriginTree,
    path: &mut Vec<ProvenancePathSegment>,
    context: DynamicContext,
    recorder: &mut ProvenanceRecorder,
) {
    match value {
        Value::Object(object) if !object.is_empty() => {
            let origin_object = origins.object().expect("object origins");
            let names = if matches!(
                context,
                DynamicContext::DynamicMap | DynamicContext::RecursiveMap
            ) {
                let mut names = object.keys().cloned().collect::<Vec<_>>();
                names.sort();
                names
            } else {
                ordered_fields(object)
            };
            for name in names {
                let segment = if matches!(
                    context,
                    DynamicContext::DynamicMap | DynamicContext::RecursiveMap
                ) || (context == DynamicContext::Schema
                    && is_schema_dynamic_key(path))
                {
                    ProvenancePathSegment::MapKey { key: name.clone() }
                } else {
                    field(&name)
                };
                path.push(segment);
                let next_context = if context == DynamicContext::Schema {
                    DynamicContext::Schema
                } else if context == DynamicContext::RecursiveMap {
                    DynamicContext::RecursiveMap
                } else if name == "fields"
                    && path.iter().any(|segment| matches!(segment, ProvenancePathSegment::Field { name } if name == "body"))
                {
                    DynamicContext::DynamicMap
                } else {
                    dynamic_context(&name)
                };
                collect_value(
                    &object[&name],
                    &origin_object[&name],
                    path,
                    next_context,
                    recorder,
                );
                path.pop();
            }
        }
        _ => push_terminal(origins.terminal(), path.clone(), recorder),
    }
}

fn dynamic_context(field_name: &str) -> DynamicContext {
    match field_name {
        "headers" | "captures" | "postingMeta" => DynamicContext::DynamicMap,
        // Request-body JSON values and form fields are both dynamic maps. The
        // Typed extraction-output objects are deliberately not classified here.
        "value" => DynamicContext::RecursiveMap,
        "properties" => DynamicContext::DynamicMap,
        _ => DynamicContext::Typed,
    }
}

fn is_schema_dynamic_key(path: &[ProvenancePathSegment]) -> bool {
    matches!(path.last(), Some(ProvenancePathSegment::Field { name }) if name == "properties")
}

fn ordered_fields(object: &Map<String, Value>) -> Vec<String> {
    let order: &[&str] = if object.contains_key("key") && object.contains_key("fetch") {
        &[
            "key",
            "description",
            "fetch",
            "pagination",
            "parse",
            "select",
            "where",
            "captures",
            "match",
            "extract",
            "acceptWhen",
            "diagnostics",
        ]
    } else if object.contains_key("mode") {
        &[
            "mode",
            "method",
            "url",
            "headers",
            "body",
            "timeoutMs",
            "waits",
            "interactions",
        ]
    } else if object.contains_key("pageParam") {
        &[
            "type",
            "pageParam",
            "parameterLocation",
            "firstPage",
            "pageSizeParam",
            "pageSize",
            "totalPath",
            "limits",
        ]
    } else if object.contains_key("offsetParam") {
        &[
            "type",
            "offsetParam",
            "limitParam",
            "parameterLocation",
            "startOffset",
            "limit",
            "totalPath",
            "limits",
        ]
    } else if object.contains_key("cursorParam") {
        &[
            "type",
            "cursorParam",
            "parameterLocation",
            "nextCursorPath",
            "limits",
        ]
    } else if object.contains_key("childSitemapSelector")
        || object.contains_key("postingUrlSelector")
    {
        &[
            "type",
            "childSitemapSelector",
            "postingUrlSelector",
            "limits",
        ]
    } else if object.contains_key("maxRequests")
        || object.contains_key("maxItems")
        || object.contains_key("maxDepth")
    {
        &["maxRequests", "maxItems", "maxDepth"]
    } else if object.contains_key("requiredFields")
        || object.contains_key("minDescriptionLength")
        || object.contains_key("minResults")
    {
        &["requiredFields", "minDescriptionLength", "minResults"]
    } else if object.contains_key("title")
        && object.contains_key("company")
        && object.contains_key("url")
    {
        &[
            "title",
            "company",
            "url",
            "locations",
            "postingMeta",
            "descriptionText",
        ]
    } else if object.contains_key("left") && object.contains_key("right") {
        &["left", "right"]
    } else if object.contains_key("from") && object.contains_key("pattern") {
        &["from", "pattern"]
    } else if object.contains_key("properties")
        || object.contains_key("additionalProperties")
        || (object.contains_key("type")
            && ["title", "default", "enum", "format", "minimum", "maximum"]
                .iter()
                .any(|key| object.contains_key(*key)))
    {
        &[
            "type",
            "title",
            "description",
            "default",
            "enum",
            "format",
            "pattern",
            "minimum",
            "maximum",
            "required",
            "properties",
            "additionalProperties",
        ]
    } else if object.contains_key("type")
        && (object.contains_key("cardinality") || object.contains_key("transforms"))
    {
        &[
            "type",
            "value",
            "template",
            "key",
            "jsonPath",
            "textPath",
            "element",
            "selector",
            "attribute",
            "parts",
            "candidates",
            "join",
            "cardinality",
            "transforms",
        ]
    } else if object.contains_key("type")
        && (object.contains_key("value") || object.contains_key("fields"))
    {
        &["type", "value", "fields"]
    } else if object.contains_key("type") && object.contains_key("charset") {
        &["type", "charset"]
    } else if object.contains_key("type") {
        &[
            "type",
            "jsonPath",
            "element",
            "textPath",
            "selector",
            "urlPattern",
            "field",
            "pattern",
            "separator",
            "trimParts",
            "dropEmpty",
            "replacement",
            "value",
            "optional",
            "parts",
            "candidates",
            "join",
        ]
    } else {
        &[
            "policy",
            "strategies",
            "acceptWhen",
            "fields",
            "title",
            "company",
            "locations",
            "descriptionText",
        ]
    };
    let rank = |name: &str| {
        order
            .iter()
            .position(|candidate| *candidate == name)
            .unwrap_or(usize::MAX)
    };
    let mut names = object.keys().cloned().collect::<Vec<_>>();
    names.sort_by(|left, right| rank(left).cmp(&rank(right)).then_with(|| left.cmp(right)));
    names
}

fn expected_profile_paths(profile: &Map<String, Value>) -> Vec<ProvenancePath> {
    let mut expected = Vec::new();
    if let Some(schema) = profile.get("sourceConfigSchema") {
        expected_value_paths(
            schema,
            &mut vec![field("sourceConfigSchema")],
            DynamicContext::Schema,
            &mut expected,
        );
    }
    for path in profile["accessPaths"]
        .as_array()
        .expect("accessPaths array")
    {
        expected.extend(expected_access_path_paths(
            path.as_object().expect("Access Path object"),
        ));
    }
    expected
}

fn expected_access_path_paths(path: &Map<String, Value>) -> Vec<ProvenancePath> {
    let key = path["key"].as_str().expect("Access Path key");
    let mut segments = vec![ProvenancePathSegment::AccessPath { key: key.into() }];
    let mut expected = vec![ProvenancePath {
        segments: segments.clone(),
    }];
    if path.contains_key("name") {
        expected.push(ProvenancePath {
            segments: [segments.clone(), vec![field("name")]].concat(),
        });
    }
    if let Some(schema) = path.get("sourceConfigSchema") {
        segments.push(field("sourceConfigSchema"));
        expected_value_paths(schema, &mut segments, DynamicContext::Schema, &mut expected);
        segments.pop();
    }
    for (authored, canonical) in [("discovery", "discovery"), ("detail", "detail")] {
        let Some(step) = path.get(authored).and_then(Value::as_object) else {
            continue;
        };
        segments.push(field(canonical));
        expected.push(ProvenancePath {
            segments: [segments.clone(), vec![field("policy"), field("type")]].concat(),
        });
        if step
            .get("policy")
            .and_then(Value::as_object)
            .is_some_and(|policy| policy.contains_key("count"))
        {
            expected.push(ProvenancePath {
                segments: [segments.clone(), vec![field("policy"), field("count")]].concat(),
            });
        }
        for strategy_value in step["strategies"].as_array().expect("strategies array") {
            let strategy_object = strategy_value.as_object().expect("Strategy object");
            let key = strategy_object["key"].as_str().expect("Strategy key");
            segments.push(ProvenancePathSegment::Strategy { key: key.into() });
            expected.push(ProvenancePath {
                segments: segments.clone(),
            });
            for (name, value) in strategy_object {
                if matches!(name.as_str(), "key" | "description" | "diagnostics") {
                    continue;
                }
                segments.push(field(name));
                expected_value_paths(value, &mut segments, dynamic_context(name), &mut expected);
                segments.pop();
            }
            segments.pop();
        }
        if let Some(acceptance) = step.get("acceptWhen") {
            segments.push(field("acceptWhen"));
            expected_value_paths(
                acceptance,
                &mut segments,
                DynamicContext::Typed,
                &mut expected,
            );
            segments.pop();
        }
        segments.pop();
    }
    expected
}

fn expected_value_paths(
    value: &Value,
    path: &mut Vec<ProvenancePathSegment>,
    context: DynamicContext,
    expected: &mut Vec<ProvenancePath>,
) {
    match value {
        Value::Object(object) if !object.is_empty() => {
            for (name, child) in object {
                let segment = if matches!(
                    context,
                    DynamicContext::DynamicMap | DynamicContext::RecursiveMap
                ) || (context == DynamicContext::Schema
                    && is_schema_dynamic_key(path))
                {
                    ProvenancePathSegment::MapKey { key: name.clone() }
                } else {
                    field(name)
                };
                path.push(segment);
                let next = if context == DynamicContext::Schema {
                    DynamicContext::Schema
                } else if context == DynamicContext::RecursiveMap {
                    DynamicContext::RecursiveMap
                } else if name == "fields"
                    && path.iter().any(|segment| matches!(segment, ProvenancePathSegment::Field { name } if name == "body"))
                {
                    DynamicContext::DynamicMap
                } else {
                    dynamic_context(name)
                };
                expected_value_paths(child, path, next, expected);
                path.pop();
            }
        }
        _ => expected.push(ProvenancePath {
            segments: path.clone(),
        }),
    }
}

fn field(name: impl Into<String>) -> ProvenancePathSegment {
    ProvenancePathSegment::Field { name: name.into() }
}

fn push_terminal(
    origin: ProvenanceOrigin,
    segments: Vec<ProvenancePathSegment>,
    recorder: &mut ProvenanceRecorder,
) {
    let path = ProvenancePath { segments };
    recorder.entries.push(ProvenanceEntry { path, origin });
}

impl OriginTree {
    fn terminal(&self) -> ProvenanceOrigin {
        match self {
            Self::Terminal(origin) => *origin,
            _ => panic!("expected terminal origin"),
        }
    }
}

fn entries(provenance: &CompiledSourceProvenance) -> &[ProvenanceEntry] {
    match provenance {
        CompiledSourceProvenance::Profile { entries }
        | CompiledSourceProvenance::SourceOwned { entries } => entries,
    }
}

pub(super) fn validate_unique_complete(
    recorded: &RecordedProvenance,
) -> Result<(), (&'static str, ProvenancePath)> {
    let mut seen = HashSet::new();
    for entry in entries(&recorded.value) {
        if !seen.insert(entry.path.clone()) {
            return Err(("duplicate_path", entry.path.clone()));
        }
    }
    for expected in &recorded.expected_paths {
        if !seen.contains(expected) {
            return Err(("missing_path", expected.clone()));
        }
    }
    if seen.len() != recorded.expected_paths.len() {
        return Err((
            "missing_path",
            ProvenancePath {
                segments: Vec::new(),
            },
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn invariant_fault(reason: &str) -> RecordedProvenance {
    let mut recorder = ProvenanceRecorder::default();
    push_terminal(
        ProvenanceOrigin::BaseSourceProfile,
        vec![field("policy"), field("type")],
        &mut recorder,
    );
    let mut recorded = recorder.finish(
        true,
        vec![ProvenancePath {
            segments: vec![field("policy"), field("type")],
        }],
    );
    let CompiledSourceProvenance::Profile { entries } = &mut recorded.value else {
        unreachable!()
    };
    match reason {
        "duplicate_path" => entries.push(entries[0].clone()),
        "missing_path" => entries.clear(),
        _ => panic!("unknown provenance invariant fault"),
    }
    recorded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invariant_validation_reports_duplicate_and_missing_paths() {
        for reason in ["duplicate_path", "missing_path"] {
            assert_eq!(
                validate_unique_complete(&invariant_fault(reason))
                    .unwrap_err()
                    .0,
                reason
            );
        }
    }
}
