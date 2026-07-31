//! Implementation-free global Source Behavior Language completeness gate.
//!
//! The three inputs deliberately have different producers:
//! * [`SchemaShape`] values are extracted from checked-in JSON Schema documents;
//! * [`SerdeShape`] values are emitted by exhaustive authored-family owners;
//! * [`CompiledRegistration`] values are literal metadata in canonical executable owners.
//! This module only concatenates owner slices and compares normalized typed identities.

pub mod model;
pub use model::*;

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaTraversalEvidence {
    pub pointer: String,
    pub target_context: String,
    pub resolved_ref: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchemaTraversalError {
    MissingDocument(String),
    UnresolvedRef { document: String, pointer: String },
    Cycle { document: String, pointer: String },
    UnsupportedDiscriminator { document: String, pointer: String },
}

pub fn escape_json_pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

pub fn traverse_schema_root(
    documents: &BTreeMap<String, Value>,
    document: &str,
    pointer: &str,
    target_context: &str,
) -> Result<Vec<SchemaTraversalEvidence>, SchemaTraversalError> {
    let mut evidence = BTreeSet::new();
    let mut active = BTreeSet::new();
    visit_schema(
        documents,
        document,
        pointer,
        target_context,
        false,
        &mut active,
        &mut evidence,
    )?;
    Ok(evidence
        .into_iter()
        .map(
            |(pointer, target_context, resolved_ref)| SchemaTraversalEvidence {
                pointer,
                target_context,
                resolved_ref,
            },
        )
        .collect())
}
fn visit_schema(
    documents: &BTreeMap<String, Value>,
    document: &str,
    pointer: &str,
    target_context: &str,
    guarded: bool,
    active: &mut BTreeSet<(String, String)>,
    evidence: &mut BTreeSet<(String, String, Option<String>)>,
) -> Result<(), SchemaTraversalError> {
    let schema = documents
        .get(document)
        .ok_or_else(|| SchemaTraversalError::MissingDocument(document.into()))?;
    let node = schema
        .pointer(pointer)
        .ok_or_else(|| SchemaTraversalError::UnresolvedRef {
            document: document.into(),
            pointer: pointer.into(),
        })?;
    let active_key = (document.to_owned(), pointer.to_owned());
    if !active.insert(active_key.clone()) {
        if guarded {
            return Ok(());
        }
        return Err(SchemaTraversalError::Cycle {
            document: document.into(),
            pointer: pointer.into(),
        });
    }
    evidence.insert((format!("{document}#{pointer}"), target_context.into(), None));
    if let Some(reference) = node.get("$ref").and_then(Value::as_str) {
        let (target_document, target_pointer) = split_ref(document, reference);
        evidence.insert((
            format!("{document}#{pointer}/$ref"),
            target_context.into(),
            Some(format!("{target_document}#{target_pointer}")),
        ));
        visit_schema(
            documents,
            &target_document,
            &target_pointer,
            target_context,
            guarded,
            active,
            evidence,
        )?;
    }
    for keyword in [
        "oneOf",
        "anyOf",
        "allOf",
        "prefixItems",
        "items",
        "additionalProperties",
        "if",
        "then",
        "else",
        "not",
        "contains",
    ] {
        if let Some(child) = node.get(keyword) {
            match child {
                Value::Array(values) => {
                    for index in 0..values.len() {
                        visit_schema(
                            documents,
                            document,
                            &format!("{pointer}/{keyword}/{index}"),
                            target_context,
                            guarded || guards(keyword),
                            active,
                            evidence,
                        )?
                    }
                }
                Value::Object(_) => visit_schema(
                    documents,
                    document,
                    &format!("{pointer}/{keyword}"),
                    target_context,
                    guarded || guards(keyword),
                    active,
                    evidence,
                )?,
                _ => {}
            }
        }
    }
    if let Some(properties) = node.get("properties").and_then(Value::as_object) {
        for key in properties.keys() {
            visit_schema(
                documents,
                document,
                &format!("{pointer}/properties/{}", escape_json_pointer_token(key)),
                target_context,
                true,
                active,
                evidence,
            )?
        }
    }
    active.remove(&active_key);
    Ok(())
}
fn guards(value: &str) -> bool {
    matches!(
        value,
        "items" | "prefixItems" | "additionalProperties" | "contains"
    )
}
fn split_ref(current: &str, reference: &str) -> (String, String) {
    let (document, pointer) = reference.split_once('#').unwrap_or((reference, ""));
    let document = if document.is_empty() {
        current.into()
    } else {
        let mut path = std::path::Path::new(current)
            .parent()
            .unwrap_or_else(|| std::path::Path::new(""))
            .join(document);
        let mut normalized = std::path::PathBuf::new();
        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    normalized.pop();
                }
                std::path::Component::Normal(value) => normalized.push(value),
                _ => {}
            }
        }
        path = normalized;
        path.to_string_lossy().into_owned()
    };
    (
        document,
        if pointer.is_empty() {
            String::new()
        } else {
            pointer.into()
        },
    )
}

pub fn production_primitive_inventories() -> PrimitiveCompletenessInventories {
    let compiled = production_compiled_inventory();
    PrimitiveCompletenessInventories {
        schema: production_schema_inventory(),
        serde: production_serde_inventory(),
        policy: production_completeness_policy(&compiled),
        compiled,
    }
}

fn production_completeness_policy(_compiled: &[CompiledRegistration]) -> CompletenessPolicy {
    // Independently frozen owner/file policy: registrations are checked against this table.
    let canonical_files = BTreeMap::from([
        (
            Owner::P01,
            BTreeSet::from(["src-tauri/crates/source-profile-dsl/src/definition/template.rs".to_owned()]),
        ),
        (
            Owner::P02,
            BTreeSet::from([
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/parse/html.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/parse/json.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/parse/mod.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/parse/xml.rs".to_owned(),
            ]),
        ),
        (
            Owner::P03,
            BTreeSet::from([
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/select/css.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/select/document.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/select/json_path.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/select/sitemap_urls.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/select/xml_element.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/select/xml_text.rs".to_owned(),
            ]),
        ),
        (
            Owner::P04,
            BTreeSet::from([
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/cardinality/all.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/cardinality/first.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/cardinality/one.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/cardinality/optional.rs".to_owned(),
            ]),
        ),
        (
            Owner::P05,
            BTreeSet::from([
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/transform/dedupe.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/transform/html_to_text.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/transform/join.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/transform/normalize_whitespace.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/transform/regex_replace.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/transform/slug_to_title.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/transform/split.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/transform/to_string.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/transform/trim.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/transform/url_decode.rs".to_owned(),
            ]),
        ),
        (
            Owner::P06a,
            BTreeSet::from(["src-tauri/crates/source-profile-dsl/src/definition/primitives/value/mod.rs".to_owned()]),
        ),
        (
            Owner::P06bc,
            BTreeSet::from([
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/value/capture.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/value/combine.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/value/const_value.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/value/css_attribute.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/value/css_text.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/value/first_non_empty.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/value/item_field.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/value/json_path.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/value/mod.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/value/posting_meta.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/value/source_config.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/value/template.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/value/xml_element.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/value/xml_text.rs".to_owned(),
            ]),
        ),
        (
            Owner::P07,
            BTreeSet::from([
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/predicate/equal.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/predicate/mod.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/predicate/non_empty.rs".to_owned(),
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/predicate/regex.rs".to_owned(),
            ]),
        ),
        (
            Owner::P08,
            BTreeSet::from(["src-tauri/crates/source-profile-dsl/src/definition/primitives/capture/named.rs".to_owned()]),
        ),
        (
            Owner::P09,
            BTreeSet::from(["src-tauri/crates/source-profile-dsl/src/definition/primitives/fetch/http.rs".to_owned()]),
        ),
        (
            Owner::P10,
            BTreeSet::from(["src-tauri/crates/source-profile-dsl/src/definition/primitives/pagination/mod.rs".to_owned()]),
        ),
        (
            Owner::P11,
            BTreeSet::from(["src-tauri/crates/source-profile-dsl/src/definition/primitives/acceptance/mod.rs".to_owned()]),
        ),
        (
            Owner::B01,
            BTreeSet::from(["src-tauri/crates/source-profile-dsl/src/definition/documents/limits.rs".to_owned()]),
        ),
        (
            Owner::B03a,
            BTreeSet::from(["src-tauri/crates/source-profile-dsl/src/definition/primitives/fetch/browser.rs".to_owned()]),
        ),
        (
            Owner::D02,
            BTreeSet::from(["src-tauri/crates/source-profile-dsl/src/detection/plan.rs".to_owned()]),
        ),
        (
            Owner::D03,
            BTreeSet::from(["src-tauri/crates/source-profile-dsl/src/detection/plan.rs".to_owned()]),
        ),
    ]);
    let forbidden = [
        (Family::Parse, "text"),
        (Family::Fetch, "retry"),
        (Family::Browser, "execute_script"),
        (Family::Browser, "eval"),
        (Family::Browser, "mutate_dom"),
        (Family::Browser, "login_flow"),
        (Family::Browser, "captcha_bypass"),
        (Family::Transform, "normalizeWhitespace"),
        (Family::Transform, "htmlToText"),
        (Family::Transform, "urlDecode"),
        (Family::Transform, "slugToTitle"),
        (Family::Transform, "toString"),
        (Family::Acceptance, "maxErrorRatio"),
        (Family::Predicate, "all"),
        (Family::Predicate, "any"),
        (Family::Predicate, "none"),
    ]
    .into_iter()
    .map(|(family, key)| (family, key.to_owned()))
    .collect();
    CompletenessPolicy {
        forbidden,
        canonical_files,
    }
}

/// Global assembly is intentionally behavior-free: it only concatenates canonical owner slices.
pub fn production_serde_inventory() -> Vec<SerdeShape> {
    let mut values = Vec::new();
    values.extend(crate::definition::template::completeness_serde_shapes());
    values.extend(super::parse::completeness_serde_shapes());
    values.extend(super::select::completeness_serde_shapes());
    values.extend(super::cardinality::completeness_serde_shapes());
    values.extend(super::transform::completeness_serde_shapes());
    values.extend(super::value::completeness_serde_shapes());
    values.extend(super::predicate::completeness_serde_shapes());
    values.extend(super::capture::completeness_serde_shapes());
    values.extend(crate::definition::documents::fetch::completeness_serde_shapes());
    values.extend(super::pagination::completeness_serde_shapes());
    values.extend(super::acceptance::completeness_serde_shapes());
    values.extend(crate::definition::documents::detection::completeness_serde_shapes());
    values.extend(crate::definition::documents::limits::completeness_serde_shapes());
    values.sort();
    values
}
pub fn production_compiled_inventory() -> Vec<CompiledRegistration> {
    let mut values = Vec::new();
    values.extend(crate::definition::template::completeness_compiled_registrations());
    values.extend(super::parse::completeness_compiled_registrations());
    values.extend(super::select::completeness_compiled_registrations());
    values.extend(super::cardinality::completeness_compiled_registrations());
    values.extend(super::transform::completeness_compiled_registrations());
    values.extend(super::value::completeness_compiled_registrations());
    values.extend(super::predicate::completeness_compiled_registrations());
    values.extend(super::capture::completeness_compiled_registrations());
    values.extend(super::fetch::http::completeness_compiled_registrations());
    values.extend(super::fetch::browser::completeness_compiled_registrations());
    values.extend(super::pagination::completeness_compiled_registrations());
    values.extend(super::acceptance::completeness_compiled_registrations());
    values.extend(crate::detection::completeness_compiled_registrations());
    values.extend(crate::definition::documents::limits::completeness_compiled_registrations());
    values.sort();
    values
}

/// Extracts the complete schema identity set from JSON. The extraction recipes name roots and
/// structural modes; variant and option keys always come from the resolved JSON nodes.
pub fn production_schema_inventory() -> Vec<SchemaShape> {
    schema_inventory_from_documents(checked_in_schemas())
}

pub fn schema_inventory_from_documents(docs: BTreeMap<String, Value>) -> Vec<SchemaShape> {
    let roots = discover_executable_roots(&docs);
    let mut out = Vec::new();
    let dd = roots.discovery_detail.as_slice();
    enum_shapes(
        &docs,
        &mut out,
        "source-behavior/parse.schema.json",
        "/$defs/parse/properties/type/enum",
        Family::Parse,
        dd,
        AuthoredShapeKind::Tagged,
        None,
    );
    option_shape(
        &mut out,
        Family::Parse,
        "charset",
        dd,
        AuthoredShapeKind::ParentOption,
        "source-behavior/parse.schema.json#/$defs/parse/properties/charset",
    );
    variant_shapes(
        &docs,
        &mut out,
        "source-behavior/select.schema.json",
        "/$defs/select/oneOf",
        Family::Select,
        dd,
        "type",
        true,
    );
    enum_shapes(
        &docs,
        &mut out,
        "source-behavior/extract.schema.json",
        "/$defs/cardinality/enum",
        Family::Cardinality,
        dd,
        AuthoredShapeKind::Tagged,
        None,
    );
    variant_shapes(
        &docs,
        &mut out,
        "source-behavior/transform.schema.json",
        "/$defs/transform/oneOf",
        Family::Transform,
        dd,
        "type",
        true,
    );
    let value_contexts = &[
        PrimitiveContext::DiscoveryCaptureSource,
        PrimitiveContext::DiscoveryFilterOutput,
        PrimitiveContext::DetailCaptureSource,
        PrimitiveContext::DetailMatchFilterOutput,
    ];
    variant_shapes(
        &docs,
        &mut out,
        "source-behavior/extract.schema.json",
        "/$defs/fieldExpression/oneOf",
        Family::Value,
        value_contexts,
        "type",
        false,
    );
    let list_contexts = &[
        PrimitiveContext::DiscoveryFilterOutput,
        PrimitiveContext::DetailMatchFilterOutput,
    ];
    singleton(
        &mut out,
        Family::Value,
        "list.single",
        list_contexts,
        AuthoredShapeKind::Untagged,
        "source-behavior/extract.schema.json#/$defs/listFieldExpression/oneOf/0",
    );
    singleton(
        &mut out,
        Family::Value,
        "list.multiple",
        list_contexts,
        AuthoredShapeKind::Untagged,
        "source-behavior/extract.schema.json#/$defs/listFieldExpression/oneOf/1",
    );
    object_options(
        &docs,
        &mut out,
        "source-behavior/extract.schema.json",
        "/$defs/combinePart",
        Family::Value,
        value_contexts,
        "combine.part",
        &[],
    );
    option_shape(
        &mut out,
        Family::Value,
        "combine.join",
        value_contexts,
        AuthoredShapeKind::ParentOption,
        "source-behavior/extract.schema.json#/$defs/combineExpression/properties/join",
    );
    option_shape(
        &mut out,
        Family::Value,
        "first_non_empty.candidates",
        value_contexts,
        AuthoredShapeKind::ParentOption,
        "source-behavior/extract.schema.json#/$defs/firstNonEmptyExpression/properties/candidates",
    );
    for context in value_contexts {
        singleton(
            &mut out,
            Family::ValuePlacement,
            context_key(*context),
            &[*context],
            AuthoredShapeKind::ParentOption,
            "source-behavior/extract.schema.json#/$defs/fieldExpression",
        );
    }
    predicate_shapes(&docs, &mut out);
    singleton(
        &mut out,
        Family::Capture,
        "named",
        dd,
        AuthoredShapeKind::Keyed,
        "source-behavior/select.schema.json#/$defs/captures",
    );
    object_options(
        &docs,
        &mut out,
        "source-behavior/select.schema.json",
        "/$defs/captureRule",
        Family::Capture,
        dd,
        "entry",
        &[],
    );
    fetch_shapes(&docs, &mut out);
    pagination_shapes(&docs, &mut out);
    acceptance_shapes(&docs, &mut out);
    detection_shapes(&docs, &mut out, roots.detection.as_slice());
    singleton(
        &mut out,
        Family::Template,
        "template",
        &[
            PrimitiveContext::DiscoveryValue,
            PrimitiveContext::DetailValue,
            PrimitiveContext::DiscoveryHttpUrl,
            PrimitiveContext::DiscoveryHttpHeader,
            PrimitiveContext::DiscoveryHttpBody,
            PrimitiveContext::DetailHttpUrl,
            PrimitiveContext::DetailHttpHeader,
            PrimitiveContext::DetailHttpBody,
            PrimitiveContext::DiscoveryBrowserUrl,
            PrimitiveContext::DetailBrowserUrl,
            PrimitiveContext::DetectionHttpUrl,
            PrimitiveContext::DetectionBrowserUrl,
            PrimitiveContext::DetectionProposal,
        ],
        AuthoredShapeKind::Keyed,
        "source-behavior/common.schema.json#/$defs/templateString",
    );
    option_shape(
        &mut out,
        Family::PhaseLimit,
        "maxBrowserRenderedBytes",
        &[
            PrimitiveContext::DiscoveryPhaseLimit,
            PrimitiveContext::DetailPhaseLimit,
        ],
        AuthoredShapeKind::ParentOption,
        "source-behavior/policy.schema.json#/$defs/phaseLimits/properties/maxBrowserRenderedBytes",
    );
    let mut inventory = merge_schema(out);
    for shape in &mut inventory {
        let metadata = frozen_owner_classification(
            InventoryLayer::Schema,
            shape.family,
            &shape.key,
            &shape.contexts,
        );
        shape.owner = metadata.owner;
        shape.canonical_file = metadata.canonical_file;
        shape.compiled_identity = metadata.compiled_identity;
        let definition_evidence = shape.evidence.clone();
        let mut connected = BTreeSet::new();
        let mut retained_contexts = BTreeSet::new();
        for context in shape.contexts.iter().copied() {
            let targets = definition_evidence
                .iter()
                .filter(|entry| entry.context == context)
                .map(|entry| entry.pointer.as_str())
                .collect::<Vec<_>>();
            let occurrences = schema_occurrences(shape.family, &shape.key, context);
            if targets.is_empty() || occurrences.is_empty() {
                continue;
            }
            let mut context_evidence = Vec::new();
            let mut all_connected = true;
            for occurrence in occurrences {
                let target_candidates = schema_targets_for_occurrence(
                    shape.family,
                    &shape.key,
                    context,
                    occurrence,
                    &targets,
                );
                let found = target_candidates.into_iter().find_map(|target| {
                    schema_traversal_chain(&docs, occurrence, target).map(|chain| (target, chain))
                });
                let Some((target, chain)) = found else {
                    all_connected = false;
                    break;
                };
                context_evidence.push(SchemaPointerContext {
                    occurrence: (*occurrence).to_owned(),
                    pointer: (*target).to_owned(),
                    chain,
                    context,
                });
            }
            if all_connected {
                retained_contexts.insert(context);
                connected.extend(context_evidence);
            }
        }
        shape.contexts = retained_contexts;
        shape.evidence = connected;
        shape.pointers = shape
            .evidence
            .iter()
            .map(|entry| entry.pointer.clone())
            .collect();
    }
    inventory.retain(|shape| !shape.contexts.is_empty());
    inventory
}

struct ExecutableRoots {
    discovery_detail: Vec<PrimitiveContext>,
    detection: Vec<PrimitiveContext>,
}

fn schema_occurrences(
    family: Family,
    key: &str,
    context: PrimitiveContext,
) -> &'static [&'static str] {
    use PrimitiveContext::*;
    if family == Family::Value && matches!(key, "list.single" | "list.multiple") {
        return match context {
            DiscoveryFilterOutput => &["source-behavior/strategy.schema.json#/$defs/discoveryExtraction/properties/providerValues/properties/locations"],
            DetailMatchFilterOutput => &["source-behavior/strategy.schema.json#/$defs/detailExtraction/properties/fields/properties/locations"],
            _ => &[],
        };
    }
    match context {
        Discovery => &["source-profile.schema.json#/$defs/profileAccessPath/properties/discovery"],
        Detail => &["source-profile.schema.json#/$defs/profileAccessPath/properties/detail"],
        Detection => &["source-profile.schema.json#/properties/detection"],
        DetectionHttp => &["source-profile.schema.json#/$defs/detectionHttpStrategy"],
        DetectionBrowser => &["source-profile.schema.json#/$defs/detectionBrowserStrategy"],
        DiscoveryValue => {
            &["source-behavior/strategy.schema.json#/$defs/discoveryStrategy/properties/extract"]
        }
        DetailValue => {
            &["source-behavior/strategy.schema.json#/$defs/detailStrategy/properties/extract"]
        }
        DiscoveryHttpUrl | DetailHttpUrl => {
            &["source-behavior/fetch.schema.json#/$defs/httpFetch/properties/url"]
        }
        DiscoveryHttpHeader | DetailHttpHeader => {
            &["source-behavior/fetch.schema.json#/$defs/httpFetch/properties/headers"]
        }
        DiscoveryHttpBody | DetailHttpBody => {
            &["source-behavior/fetch.schema.json#/$defs/httpFetch/properties/body"]
        }
        DiscoveryBrowserUrl | DetailBrowserUrl => {
            &["source-behavior/fetch.schema.json#/$defs/browserFetch/properties/url"]
        }
        DetectionHttpUrl => {
            &["source-profile.schema.json#/$defs/detectionHttpStrategy/properties/fetch"]
        }
        DetectionBrowserUrl => {
            &["source-profile.schema.json#/$defs/detectionBrowserStrategy/properties/fetch"]
        }
        DetectionProposal => &[
            "source-profile.schema.json#/$defs/detection/properties/keyCandidates/items",
            "source-profile.schema.json#/$defs/detection/properties/nameCandidates/items",
        ],
        DiscoveryCaptureSource => {
            &["source-behavior/strategy.schema.json#/$defs/discoveryStrategy/properties/captures"]
        }
        DetailCaptureSource => {
            &["source-behavior/strategy.schema.json#/$defs/detailStrategy/properties/captures"]
        }
        DiscoveryFilterOutput => &[
            "source-behavior/strategy.schema.json#/$defs/discoveryStrategy/properties/where",
            "source-behavior/strategy.schema.json#/$defs/discoveryStrategy/properties/extract",
        ],
        DiscoveryWhere => {
            &["source-behavior/strategy.schema.json#/$defs/discoveryStrategy/properties/where"]
        }
        DetailMatchFilterOutput => &[
            "source-behavior/strategy.schema.json#/$defs/detailStrategy/properties/match",
            "source-behavior/strategy.schema.json#/$defs/detailStrategy/properties/extract",
        ],
        DetailMatch => &[
            "source-profile.schema.json#/$defs/profileAccessPath/properties/detail",
            "source-behavior/strategy.schema.json#/$defs/detailStrategy/properties/match",
        ],
        DetailWhere => &["source-behavior/strategy.schema.json#/$defs/detailStrategy/properties/where"],
        DetectionHttpStatus => &[
            "source-profile.schema.json#/properties/detection",
            "source-profile.schema.json#/$defs/detectionHttpStrategy/properties/expectStatus",
        ],
        DetectionHttpContains => &[
            "source-profile.schema.json#/properties/detection",
            "source-profile.schema.json#/$defs/detectionHttpStrategy/properties/contains",
        ],
        DetectionBrowserContains => &[
            "source-profile.schema.json#/properties/detection",
            "source-profile.schema.json#/$defs/detectionBrowserStrategy/properties/contains",
        ],
        DetectionHttpRegex => &[
            "source-profile.schema.json#/properties/detection",
            "source-profile.schema.json#/$defs/detectionHttpStrategy/properties/regex",
        ],
        DetectionBrowserRegex => &[
            "source-profile.schema.json#/properties/detection",
            "source-profile.schema.json#/$defs/detectionBrowserStrategy/properties/regex",
        ],
        DiscoveryPhaseLimit => &[
            "source-behavior/policy.schema.json#/$defs/discoveryStrategySet/properties/limits",
            "source-behavior/policy.schema.json#/$defs/phaseLimits/properties/maxBrowserRenderedBytes",
        ],
        DetailPhaseLimit => &[
            "source-behavior/policy.schema.json#/$defs/detailStrategySet/properties/limits",
            "source-behavior/policy.schema.json#/$defs/phaseLimits/properties/maxBrowserRenderedBytes",
        ],
    }
}

fn schema_targets_for_occurrence<'a>(
    family: Family,
    _key: &str,
    context: PrimitiveContext,
    occurrence: &'a str,
    default_targets: &[&'a str],
) -> Vec<&'a str> {
    use PrimitiveContext::*;
    // Header and arbitrary JSON-body leaves are compiled as P01 Templates even though their
    // schema admits strings/JSON structurally rather than referencing `templateString`. Their
    // exact executable property is therefore the schema target, not a fabricated common ref.
    if family == Family::Template
        && matches!(
            context,
            DiscoveryHttpHeader | DetailHttpHeader | DiscoveryHttpBody | DetailHttpBody
        )
    {
        vec![occurrence]
    } else {
        default_targets.to_vec()
    }
}

fn schema_traversal_chain(
    docs: &BTreeMap<String, Value>,
    occurrence: &str,
    target: &str,
) -> Option<Vec<String>> {
    let (document, pointer) = occurrence.split_once('#')?;
    let mut active = BTreeSet::new();
    schema_chain_visit(docs, document, pointer, target, &mut active)
}

fn schema_chain_visit(
    docs: &BTreeMap<String, Value>,
    document: &str,
    pointer: &str,
    target: &str,
    active: &mut BTreeSet<(String, String)>,
) -> Option<Vec<String>> {
    let current = format!("{document}#{pointer}");
    if current == target {
        return Some(vec![current]);
    }
    let node = docs.get(document)?.pointer(pointer)?;
    let active_key = (document.to_owned(), pointer.to_owned());
    if !active.insert(active_key.clone()) {
        return None;
    }
    let mut children = Vec::<(String, String)>::new();
    if let Some(reference) = node.get("$ref").and_then(Value::as_str) {
        children.push(split_ref(document, reference));
    }
    match node {
        Value::Object(object) => {
            for key in object.keys().filter(|key| key.as_str() != "$ref") {
                children.push((
                    document.to_owned(),
                    format!("{pointer}/{}", escape_json_pointer_token(key)),
                ));
            }
        }
        Value::Array(values) => {
            for index in 0..values.len() {
                children.push((document.to_owned(), format!("{pointer}/{index}")));
            }
        }
        _ => {}
    }
    for (child_document, child_pointer) in children {
        if let Some(mut tail) =
            schema_chain_visit(docs, &child_document, &child_pointer, target, active)
        {
            let mut chain = vec![current.clone()];
            chain.append(&mut tail);
            active.remove(&active_key);
            return Some(chain);
        }
    }
    active.remove(&active_key);
    None
}

fn discover_executable_roots(docs: &BTreeMap<String, Value>) -> ExecutableRoots {
    // Start from the real Source Profile root. This catches an unresolved ref, unsupported
    // unguarded recursion, or a strategy root that is no longer reachable.
    let evidence = traverse_schema_root(docs, "source-profile.schema.json", "", "profile")
        .unwrap_or_else(|error| panic!("unclassifiable executable schema root: {error:?}"));
    for target in [
        "source-behavior/strategy.schema.json#/$defs/discoveryStrategy",
        "source-behavior/strategy.schema.json#/$defs/detailStrategy",
        "source-profile.schema.json#/$defs/detectionUrlStrategy",
        "source-profile.schema.json#/$defs/detectionHttpStrategy",
        "source-profile.schema.json#/$defs/detectionBrowserStrategy",
    ] {
        assert!(
            evidence
                .iter()
                .any(|entry| entry.resolved_ref.as_deref() == Some(target)),
            "unclassifiable or unreachable executable schema construct {target}"
        );
    }
    let mut discovery_detail = Vec::new();
    for (property, expected_target, context) in [
        (
            "discovery",
            "source-behavior/policy.schema.json#/$defs/discoveryStrategySet",
            PrimitiveContext::Discovery,
        ),
        (
            "detail",
            "source-behavior/policy.schema.json#/$defs/detailStrategySet",
            PrimitiveContext::Detail,
        ),
    ] {
        let pointer = format!("/$defs/profileAccessPath/properties/{property}");
        let node = docs["source-profile.schema.json"]
            .pointer(&pointer)
            .unwrap_or_else(|| panic!("missing executable schema occurrence {pointer}"));
        let reference = node["$ref"]
            .as_str()
            .unwrap_or_else(|| panic!("unclassifiable executable occurrence {pointer}"));
        let (document, target) = split_ref("source-profile.schema.json", reference);
        assert_eq!(
            format!("{document}#{target}"),
            expected_target,
            "unexpected executable target at {pointer}"
        );
        discovery_detail.push(context);
    }
    let detection_pointer = "/properties/detection";
    let detection_reference = docs["source-profile.schema.json"]
        .pointer(detection_pointer)
        .and_then(|node| node["$ref"].as_str())
        .unwrap_or_else(|| panic!("unclassifiable executable occurrence {detection_pointer}"));
    let (document, target) = split_ref("source-profile.schema.json", detection_reference);
    assert_eq!(
        format!("{document}#{target}"),
        "source-profile.schema.json#/$defs/detection"
    );
    ExecutableRoots {
        discovery_detail,
        detection: vec![PrimitiveContext::Detection],
    }
}

fn checked_in_schemas() -> BTreeMap<String, Value> {
    [
        (
            "source-profile.schema.json",
            include_str!("../../../schema/source-profile.schema.json"),
        ),
        (
            "source-behavior/common.schema.json",
            include_str!("../../../schema/source-behavior/common.schema.json"),
        ),
        (
            "source-behavior/diagnostics.schema.json",
            include_str!("../../../schema/source-behavior/diagnostics.schema.json"),
        ),
        (
            "source-behavior/extract.schema.json",
            include_str!("../../../schema/source-behavior/extract.schema.json"),
        ),
        (
            "source-behavior/fetch.schema.json",
            include_str!("../../../schema/source-behavior/fetch.schema.json"),
        ),
        (
            "source-behavior/pagination.schema.json",
            include_str!("../../../schema/source-behavior/pagination.schema.json"),
        ),
        (
            "source-behavior/policy.schema.json",
            include_str!("../../../schema/source-behavior/policy.schema.json"),
        ),
        (
            "source-behavior/parse.schema.json",
            include_str!("../../../schema/source-behavior/parse.schema.json"),
        ),
        (
            "source-behavior/predicate.schema.json",
            include_str!("../../../schema/source-behavior/predicate.schema.json"),
        ),
        (
            "source-behavior/select.schema.json",
            include_str!("../../../schema/source-behavior/select.schema.json"),
        ),
        (
            "source-behavior/strategy.schema.json",
            include_str!("../../../schema/source-behavior/strategy.schema.json"),
        ),
        (
            "source-behavior/transform.schema.json",
            include_str!("../../../schema/source-behavior/transform.schema.json"),
        ),
    ]
    .into_iter()
    .map(|(path, value)| (path.into(), serde_json::from_str(value).unwrap()))
    .collect()
}
fn node<'a>(
    docs: &'a BTreeMap<String, Value>,
    document: &str,
    pointer: &str,
) -> (&'a Value, String, String) {
    let value = docs.get(document).unwrap().pointer(pointer).unwrap();
    if let Some(reference) = value.get("$ref").and_then(Value::as_str) {
        let (d, p) = split_ref(document, reference);
        return node(docs, &d, &p);
    }
    (value, document.into(), pointer.into())
}
fn enum_shapes(
    docs: &BTreeMap<String, Value>,
    out: &mut Vec<SchemaShape>,
    document: &str,
    pointer: &str,
    family: Family,
    ctx: &[PrimitiveContext],
    shape: AuthoredShapeKind,
    prefix: Option<&str>,
) {
    let (value, d, p) = node(docs, document, pointer);
    let evidence_pointer = p.strip_suffix("/enum").unwrap_or(&p);
    for item in value.as_array().unwrap() {
        let key = item.as_str().unwrap();
        singleton(
            out,
            family,
            &prefix
                .map(|v| format!("{v}.{key}"))
                .unwrap_or_else(|| key.into()),
            ctx,
            shape,
            &format!("{d}#{evidence_pointer}"),
        );
    }
}
pub fn classify_tagged_variant_keys(
    docs: &BTreeMap<String, Value>,
    document: &str,
    pointer: &str,
    discriminator: &str,
) -> Result<Vec<String>, SchemaTraversalError> {
    let (array, _, _) = node(docs, document, pointer);
    let Some(branches) = array.as_array() else {
        return Err(SchemaTraversalError::UnsupportedDiscriminator {
            document: document.into(),
            pointer: pointer.into(),
        });
    };
    let mut keys = Vec::new();
    for (index, branch) in branches.iter().enumerate() {
        let Some(reference) = branch.get("$ref").and_then(Value::as_str) else {
            return Err(SchemaTraversalError::UnsupportedDiscriminator {
                document: document.into(),
                pointer: format!("{pointer}/{index}"),
            });
        };
        let (d, p) = split_ref(document, reference);
        let (base, _, _) = node(docs, &d, &p);
        let object = if base.get("properties").is_some() {
            base
        } else if let Some(object) = base
            .get("allOf")
            .and_then(Value::as_array)
            .and_then(|parts| parts.iter().find(|part| part.get("properties").is_some()))
        {
            object
        } else {
            return Err(SchemaTraversalError::UnsupportedDiscriminator {
                document: d,
                pointer: p,
            });
        };
        let discriminator = &object["properties"][discriminator];
        if let Some(value) = discriminator.get("const").and_then(Value::as_str) {
            keys.push(value.to_owned());
        } else if let Some(values) = discriminator.get("enum").and_then(Value::as_array) {
            for value in values {
                let Some(value) = value.as_str() else {
                    return Err(SchemaTraversalError::UnsupportedDiscriminator {
                        document: d.clone(),
                        pointer: p.clone(),
                    });
                };
                keys.push(value.to_owned());
            }
        } else {
            return Err(SchemaTraversalError::UnsupportedDiscriminator {
                document: d,
                pointer: p,
            });
        }
    }
    Ok(keys)
}

fn variant_shapes(
    docs: &BTreeMap<String, Value>,
    out: &mut Vec<SchemaShape>,
    document: &str,
    pointer: &str,
    family: Family,
    ctx: &[PrimitiveContext],
    discriminator: &str,
    include_options: bool,
) {
    classify_tagged_variant_keys(docs, document, pointer, discriminator)
        .unwrap_or_else(|error| panic!("unclassifiable executable schema construct: {error:?}"));
    let (array, _, _) = node(docs, document, pointer);
    for (index, branch) in array.as_array().unwrap().iter().enumerate() {
        let reference = branch.get("$ref").and_then(Value::as_str).unwrap();
        let (d, p) = split_ref(document, reference);
        let (base, rd, rp) = node(docs, &d, &p);
        let obj = if base.get("properties").is_some() {
            base
        } else {
            base.get("allOf")
                .and_then(Value::as_array)
                .and_then(|parts| parts.iter().find(|part| part.get("properties").is_some()))
                .expect("variant has one structural object")
        };
        let disc = &obj["properties"][discriminator];
        let keys = if let Some(value) = disc.get("const").and_then(Value::as_str) {
            vec![value.to_owned()]
        } else {
            disc.get("enum")
                .and_then(Value::as_array)
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect()
        };
        for key in keys {
            singleton(
                out,
                family,
                &key,
                ctx,
                AuthoredShapeKind::Tagged,
                &format!("{document}#{pointer}/{index}"),
            );
            if include_options && obj["properties"][discriminator].get("const").is_some() {
                for property in obj["properties"]
                    .as_object()
                    .unwrap()
                    .keys()
                    .filter(|v| v.as_str() != discriminator)
                {
                    option_shape(
                        out,
                        family,
                        &format!("{key}.{property}"),
                        ctx,
                        AuthoredShapeKind::ParentOption,
                        &format!(
                            "{rd}#{rp}/properties/{}",
                            escape_json_pointer_token(property)
                        ),
                    );
                }
            }
        }
    }
}
fn object_options(
    docs: &BTreeMap<String, Value>,
    out: &mut Vec<SchemaShape>,
    document: &str,
    pointer: &str,
    family: Family,
    ctx: &[PrimitiveContext],
    prefix: &str,
    skip: &[&str],
) {
    let (obj, d, p) = node(docs, document, pointer);
    for property in obj["properties"]
        .as_object()
        .unwrap()
        .keys()
        .filter(|v| !skip.contains(&v.as_str()))
    {
        option_shape(
            out,
            family,
            &format!("{prefix}.{property}"),
            ctx,
            AuthoredShapeKind::ParentOption,
            &format!("{d}#{p}/properties/{}", escape_json_pointer_token(property)),
        );
    }
}
fn singleton(
    out: &mut Vec<SchemaShape>,
    family: Family,
    key: &str,
    ctx: &[PrimitiveContext],
    shape: AuthoredShapeKind,
    pointer: &str,
) {
    let contexts = contexts(ctx);
    out.push(SchemaShape {
        family,
        key: key.into(),
        contexts,
        owner: Owner::P01,
        canonical_file: String::new(),
        compiled_identity: String::new(),
        shape,
        pointers: BTreeSet::from([pointer.into()]),
        evidence: ctx
            .iter()
            .copied()
            .map(|context| SchemaPointerContext {
                occurrence: pointer.into(),
                pointer: pointer.into(),
                chain: vec![pointer.into()],
                context,
            })
            .collect(),
    })
}
fn option_shape(
    out: &mut Vec<SchemaShape>,
    family: Family,
    key: &str,
    ctx: &[PrimitiveContext],
    shape: AuthoredShapeKind,
    pointer: &str,
) {
    singleton(out, family, key, ctx, shape, pointer)
}
fn context_key(value: PrimitiveContext) -> &'static str {
    match value {
        PrimitiveContext::DiscoveryCaptureSource => "discovery_capture_source",
        PrimitiveContext::DiscoveryFilterOutput => "discovery_filter_output",
        PrimitiveContext::DetailCaptureSource => "detail_capture_source",
        PrimitiveContext::DetailMatchFilterOutput => "detail_match_filter_output",
        _ => unreachable!(),
    }
}

fn predicate_shapes(docs: &BTreeMap<String, Value>, out: &mut Vec<SchemaShape>) {
    use PrimitiveContext::*;
    let dw = &[DiscoveryWhere, DetailWhere];
    variant_shapes(
        docs,
        out,
        "source-behavior/predicate.schema.json",
        "/$defs/where/oneOf",
        Family::Predicate,
        dw,
        "type",
        true,
    );
    let dm = &[DetailMatch];
    singleton(
        out,
        Family::Predicate,
        "equal",
        &[DetailMatch],
        AuthoredShapeKind::Tagged,
        "source-behavior/predicate.schema.json#/$defs/equal",
    );
    singleton(
        out,
        Family::Predicate,
        "equal",
        &[DetectionHttpStatus],
        AuthoredShapeKind::Tagged,
        "source-profile.schema.json#/$defs/detectionHttpStrategy/properties/expectStatus",
    );
    singleton(
        out,
        Family::Predicate,
        "detail.match",
        dm,
        AuthoredShapeKind::ParentOption,
        "source-behavior/strategy.schema.json#/$defs/detailStrategy/properties/match",
    );
    object_options(
        docs,
        out,
        "source-behavior/predicate.schema.json",
        "/$defs/equal",
        Family::Predicate,
        dm,
        "detail.match",
        &["type"],
    );
    for (strategy, contains_context, regex_context) in [
        (
            "detectionHttpStrategy",
            DetectionHttpContains,
            DetectionHttpRegex,
        ),
        (
            "detectionBrowserStrategy",
            DetectionBrowserContains,
            DetectionBrowserRegex,
        ),
    ] {
        singleton(
            out,
            Family::Predicate,
            "non_empty",
            &[contains_context],
            AuthoredShapeKind::Tagged,
            &format!("source-profile.schema.json#/$defs/{strategy}/properties/contains"),
        );
        singleton(
            out,
            Family::Predicate,
            "regex",
            &[regex_context],
            AuthoredShapeKind::Tagged,
            &format!("source-profile.schema.json#/$defs/{strategy}/properties/regex"),
        );
    }
}
fn fetch_shapes(docs: &BTreeMap<String, Value>, out: &mut Vec<SchemaShape>) {
    use PrimitiveContext::*;
    let http = &[Discovery, Detail, DetectionHttp];
    let browser = &[Discovery, Detail, DetectionBrowser];
    let (obj, d, p) = node(
        docs,
        "source-behavior/fetch.schema.json",
        "/$defs/httpFetch",
    );
    let key = obj["properties"]["mode"]["const"].as_str().unwrap();
    singleton(
        out,
        Family::Fetch,
        key,
        http,
        AuthoredShapeKind::Tagged,
        &format!("{d}#{p}"),
    );
    for property in obj["properties"]
        .as_object()
        .unwrap()
        .keys()
        .filter(|property| !matches!(property.as_str(), "mode" | "method" | "body"))
    {
        option_shape(
            out,
            Family::Fetch,
            &format!("{key}.{property}"),
            http,
            AuthoredShapeKind::ParentOption,
            &format!("{d}#{p}/properties/{property}"),
        );
    }
    enum_shapes(
        docs,
        out,
        "source-behavior/fetch.schema.json",
        "/$defs/httpFetch/properties/method/enum",
        Family::Fetch,
        http,
        AuthoredShapeKind::ParentOption,
        Some("http.method"),
    );
    for body in ["jsonBody", "textBody", "formBody"] {
        let (value, bd, bp) = node(
            docs,
            "source-behavior/fetch.schema.json",
            &format!("/$defs/{body}"),
        );
        let kind = value["properties"]["type"]["const"].as_str().unwrap();
        singleton(
            out,
            Family::Fetch,
            &format!("http.body.{kind}"),
            http,
            AuthoredShapeKind::ParentOption,
            &format!("{bd}#{bp}"),
        );
        for property in value["properties"]
            .as_object()
            .unwrap()
            .keys()
            .filter(|v| v.as_str() != "type")
        {
            option_shape(
                out,
                Family::Fetch,
                &format!("http.body.{kind}.{property}"),
                http,
                AuthoredShapeKind::ParentOption,
                &format!("{bd}#{bp}/properties/{property}"),
            );
        }
    }
    let (fetch, fd, fp) = node(
        docs,
        "source-behavior/fetch.schema.json",
        "/$defs/browserFetch",
    );
    let fetchkey = fetch["properties"]["mode"]["const"].as_str().unwrap();
    singleton(
        out,
        Family::Browser,
        fetchkey,
        browser,
        AuthoredShapeKind::Tagged,
        &format!("{fd}#{fp}"),
    );
    for property in fetch["properties"]
        .as_object()
        .unwrap()
        .keys()
        .filter(|v| v.as_str() != "mode")
    {
        option_shape(
            out,
            Family::Browser,
            &format!("{fetchkey}.{property}"),
            browser,
            AuthoredShapeKind::ParentOption,
            &format!("{fd}#{fp}/properties/{property}"),
        );
    }
    for root in ["browserWait", "browserInteraction"] {
        let (array, _, _) = node(
            docs,
            "source-behavior/fetch.schema.json",
            &format!("/$defs/{root}/oneOf"),
        );
        for branch in array.as_array().unwrap() {
            let r = branch["$ref"].as_str().unwrap();
            let (dd, pp) = split_ref("source-behavior/fetch.schema.json", r);
            let raw = docs.get(&dd).unwrap().pointer(&pp).unwrap();
            let k = raw["properties"]["type"]["const"].as_str().unwrap();
            let (value, option_document, option_pointer) =
                if let Some(reference) = raw.get("$ref").and_then(Value::as_str) {
                    let (bd, bp) = split_ref(&dd, reference);
                    node(docs, &bd, &bp)
                } else {
                    node(docs, &dd, &pp)
                };
            singleton(
                out,
                Family::Browser,
                k,
                browser,
                AuthoredShapeKind::Tagged,
                &format!("{dd}#{pp}"),
            );
            for property in value["properties"]
                .as_object()
                .unwrap()
                .keys()
                .filter(|v| v.as_str() != "type")
            {
                option_shape(
                    out,
                    Family::Browser,
                    &format!("{k}.{property}"),
                    browser,
                    AuthoredShapeKind::ParentOption,
                    &format!("{option_document}#{option_pointer}/properties/{property}"),
                );
            }
        }
    }
}
fn pagination_shapes(docs: &BTreeMap<String, Value>, out: &mut Vec<SchemaShape>) {
    let ctx = &[PrimitiveContext::Discovery];
    let (array, _, _) = node(
        docs,
        "source-behavior/pagination.schema.json",
        "/$defs/pagination/oneOf",
    );
    for branch in array.as_array().unwrap() {
        let (d, p) = split_ref(
            "source-behavior/pagination.schema.json",
            branch["$ref"].as_str().unwrap(),
        );
        let (value, rd, rp) = node(docs, &d, &p);
        let key = value["properties"]["type"]["const"].as_str().unwrap();
        singleton(
            out,
            Family::Pagination,
            key,
            ctx,
            AuthoredShapeKind::Tagged,
            &format!("{rd}#{rp}"),
        );
        for property in value["properties"]
            .as_object()
            .unwrap()
            .keys()
            .filter(|v| v.as_str() != "type")
        {
            option_shape(
                out,
                Family::Pagination,
                &format!("{key}.{property}"),
                ctx,
                AuthoredShapeKind::ParentOption,
                &format!("{rd}#{rp}/properties/{property}"),
            );
            if property == "limits" {
                let (limits, ld, lp) = node(
                    docs,
                    "source-behavior/pagination.schema.json",
                    "/$defs/limits",
                );
                for nested in limits["properties"].as_object().unwrap().keys() {
                    option_shape(
                        out,
                        Family::Pagination,
                        &format!("{key}.limits.{nested}"),
                        ctx,
                        AuthoredShapeKind::ParentOption,
                        &format!("{ld}#{lp}/properties/{nested}"),
                    );
                }
            }
        }
    }
    enum_shapes(
        docs,
        out,
        "source-behavior/pagination.schema.json",
        "/$defs/parameterLocation/enum",
        Family::PaginationLocation,
        ctx,
        AuthoredShapeKind::Tagged,
        None,
    );
}
fn acceptance_shapes(docs: &BTreeMap<String, Value>, out: &mut Vec<SchemaShape>) {
    let (value, d, p) = node(
        docs,
        "source-behavior/strategy.schema.json",
        "/$defs/acceptance",
    );
    for property in value["properties"].as_object().unwrap().keys() {
        let ctx = if property == "minResults" {
            vec![PrimitiveContext::Discovery]
        } else {
            vec![PrimitiveContext::Discovery, PrimitiveContext::Detail]
        };
        option_shape(
            out,
            Family::Acceptance,
            property,
            &ctx,
            AuthoredShapeKind::ParentOption,
            &format!("{d}#{p}/properties/{property}"),
        );
    }
}
fn detection_shapes(
    docs: &BTreeMap<String, Value>,
    out: &mut Vec<SchemaShape>,
    ctx: &[PrimitiveContext],
) {
    let (strategy_variants, _, _) = node(
        docs,
        "source-profile.schema.json",
        "/$defs/detectionStrategy/oneOf",
    );
    for branch in strategy_variants
        .as_array()
        .expect("Detection strategies are finite")
    {
        let reference = branch["$ref"]
            .as_str()
            .expect("Detection strategy branches use explicit local refs");
        let (document, pointer) = split_ref("source-profile.schema.json", reference);
        let (value, d, p) = node(docs, &document, &pointer);
        let key = value["properties"]["type"]["const"].as_str().unwrap();
        singleton(
            out,
            Family::Detection,
            key,
            ctx,
            AuthoredShapeKind::Tagged,
            &format!("{d}#{p}"),
        );
        for property in value["properties"]
            .as_object()
            .unwrap()
            .keys()
            .filter(|v| v.as_str() != "type")
        {
            option_shape(
                out,
                Family::Detection,
                &format!("{key}.{property}"),
                ctx,
                AuthoredShapeKind::ParentOption,
                &format!("{d}#{p}/properties/{property}"),
            );
        }
    }
    let (input, _, _) = node(
        docs,
        "source-profile.schema.json",
        "/$defs/detectionUrlStrategy/properties/input/oneOf",
    );
    for (index, branch) in input.as_array().unwrap().iter().enumerate() {
        let key = branch["properties"]["type"]["const"].as_str().unwrap();
        singleton(
            out,
            Family::Detection,
            key,
            ctx,
            AuthoredShapeKind::Tagged,
            &format!(
                "source-profile.schema.json#/$defs/detectionUrlStrategy/properties/input/oneOf/{index}"
            ),
        );
        for property in branch["properties"]
            .as_object()
            .unwrap()
            .keys()
            .filter(|v| v.as_str() != "type")
        {
            option_shape(
                out,
                Family::Detection,
                &format!("{key}.{property}"),
                ctx,
                AuthoredShapeKind::ParentOption,
                &format!(
                    "source-profile.schema.json#/$defs/detectionUrlStrategy/properties/input/oneOf/{index}/properties/{}",
                    escape_json_pointer_token(property)
                ),
            );
        }
    }
    let (value, d, p) = node(docs, "source-profile.schema.json", "/$defs/inputUrlPattern");
    singleton(
        out,
        Family::Detection,
        "input_url_pattern",
        ctx,
        AuthoredShapeKind::Keyed,
        &format!("{d}#{p}"),
    );
    for property in value["properties"].as_object().unwrap().keys() {
        option_shape(
            out,
            Family::Detection,
            &format!("input_url_pattern.{property}"),
            ctx,
            AuthoredShapeKind::ParentOption,
            &format!("{d}#{p}/properties/{property}"),
        );
    }
}
fn merge_schema(values: Vec<SchemaShape>) -> Vec<SchemaShape> {
    let mut map = BTreeMap::<(Family, String, AuthoredShapeKind), SchemaShape>::new();
    for value in values {
        let key = (value.family, value.key.clone(), value.shape);
        map.entry(key)
            .and_modify(|old| {
                old.contexts.extend(value.contexts.iter().copied());
                old.pointers.extend(value.pointers.iter().cloned());
                old.evidence.extend(value.evidence.iter().cloned());
            })
            .or_insert(value);
    }
    let mut out = map.into_values().collect::<Vec<_>>();
    out.sort();
    out
}
