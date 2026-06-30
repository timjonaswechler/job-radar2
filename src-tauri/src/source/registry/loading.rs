use dom_query::Matcher;
use serde::{de, Deserialize, Deserializer};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};

use super::*;

impl<'de> Deserialize<'de> for SourceDocument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        parse_source_document(value).map_err(de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for SelectedAccessPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        parse_selected_access_path(value).map_err(de::Error::custom)
    }
}

#[derive(Clone, Debug)]
struct RawRegistryDocument {
    kind: SourceRegistryDocumentKind,
    origin: SourceRegistryDocumentOrigin,
    path: String,
    contents: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawSourceDocument {
    schema_version: u64,
    key: String,
    name: String,
    status: SourceDocumentStatus,
    source_config: Value,
    selected_access_path: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawProfileSelectedAccessPath {
    #[serde(rename = "type")]
    path_type: String,
    profile_key: String,
    path_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawSourceSpecificSelectedAccessPath {
    #[serde(rename = "type")]
    path_type: String,
    adapter_key: String,
    source_config_schema: Option<Value>,
    query: Option<Value>,
    inventory: Option<Value>,
    interactions: Option<Vec<BrowserInteraction>>,
    manual_release: Option<Value>,
}

pub fn load_snapshot(app_data_dir: impl AsRef<Path>) -> SourceRegistrySnapshot {
    load_snapshot_with_builtins(
        app_data_dir,
        BUILTIN_SOURCE_PROFILE_JSON_FILES,
        BUILTIN_SOURCE_JSON_FILES,
    )
}

pub fn load_snapshot_with_builtins(
    app_data_dir: impl AsRef<Path>,
    builtin_source_profiles: &[EmbeddedSourceRegistryDocument<'_>],
    builtin_sources: &[EmbeddedSourceRegistryDocument<'_>],
) -> SourceRegistrySnapshot {
    let app_data_dir = app_data_dir.as_ref();
    let mut diagnostics = Vec::new();

    let mut profile_documents = embedded_documents(
        SourceRegistryDocumentKind::SourceProfile,
        builtin_source_profiles,
    );
    profile_documents.extend(custom_documents(
        SourceRegistryDocumentKind::SourceProfile,
        app_data_dir.join("source-profiles"),
        &mut diagnostics,
    ));

    let mut source_documents =
        embedded_documents(SourceRegistryDocumentKind::Source, builtin_sources);
    source_documents.extend(custom_documents(
        SourceRegistryDocumentKind::Source,
        app_data_dir.join("sources"),
        &mut diagnostics,
    ));

    let valid_profiles = load_profile_documents(profile_documents, &mut diagnostics);
    let candidate_sources = load_source_documents(source_documents, &mut diagnostics);
    let valid_sources =
        validate_source_references(candidate_sources, &valid_profiles, &mut diagnostics);

    SourceRegistrySnapshot {
        valid_profiles,
        valid_sources,
        diagnostics,
    }
}

fn embedded_documents(
    kind: SourceRegistryDocumentKind,
    documents: &[EmbeddedSourceRegistryDocument<'_>],
) -> Vec<RawRegistryDocument> {
    documents
        .iter()
        .map(|(path, contents)| RawRegistryDocument {
            kind,
            origin: SourceRegistryDocumentOrigin::BuiltIn,
            path: (*path).to_string(),
            contents: (*contents).to_string(),
        })
        .collect()
}

fn custom_documents(
    kind: SourceRegistryDocumentKind,
    directory: PathBuf,
    diagnostics: &mut Vec<SourceRegistryDiagnostic>,
) -> Vec<RawRegistryDocument> {
    let mut paths = Vec::new();
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            diagnostics.push(SourceRegistryDiagnostic {
                code: SourceRegistryDiagnosticCode::ReadError,
                document_kind: kind,
                origin: SourceRegistryDocumentOrigin::Custom,
                path: directory.display().to_string(),
                key: None,
                message: format!("could not read registry directory: {error}"),
            });
            return Vec::new();
        }
    };

    for entry in entries {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if path.is_file() && is_json_file(&path) {
                    paths.push(path);
                }
            }
            Err(error) => diagnostics.push(SourceRegistryDiagnostic {
                code: SourceRegistryDiagnosticCode::ReadError,
                document_kind: kind,
                origin: SourceRegistryDocumentOrigin::Custom,
                path: directory.display().to_string(),
                key: None,
                message: format!("could not read registry directory entry: {error}"),
            }),
        }
    }

    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| {
            let path_label = path.display().to_string();
            match fs::read_to_string(&path) {
                Ok(contents) => Some(RawRegistryDocument {
                    kind,
                    origin: SourceRegistryDocumentOrigin::Custom,
                    path: path_label,
                    contents,
                }),
                Err(error) => {
                    diagnostics.push(SourceRegistryDiagnostic {
                        code: SourceRegistryDiagnosticCode::ReadError,
                        document_kind: kind,
                        origin: SourceRegistryDocumentOrigin::Custom,
                        path: path_label,
                        key: None,
                        message: format!("could not read registry document: {error}"),
                    });
                    None
                }
            }
        })
        .collect()
}

fn is_json_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some(extension) if extension.eq_ignore_ascii_case("json")
    )
}

fn load_profile_documents(
    documents: Vec<RawRegistryDocument>,
    diagnostics: &mut Vec<SourceRegistryDiagnostic>,
) -> Vec<RegistrySourceProfile> {
    let mut valid_profiles = Vec::new();
    let mut seen_keys = HashMap::<String, (SourceRegistryDocumentOrigin, String)>::new();

    for document in documents {
        let Some(parsed) = parse_registry_document(&document, parse_profile_document, diagnostics)
        else {
            continue;
        };
        if !filename_matches_key(&document.path, &parsed.key) {
            diagnostics.push(filename_key_mismatch_diagnostic(&document, &parsed.key));
            continue;
        }
        if let Some((first_origin, first_path)) = seen_keys.get(&parsed.key) {
            diagnostics.push(duplicate_key_diagnostic(
                &document,
                &parsed.key,
                *first_origin,
                first_path,
            ));
            continue;
        }

        seen_keys.insert(parsed.key.clone(), (document.origin, document.path.clone()));
        valid_profiles.push(RegistrySourceProfile {
            origin: document.origin,
            path: document.path,
            document: parsed,
        });
    }

    valid_profiles
}

fn load_source_documents(
    documents: Vec<RawRegistryDocument>,
    diagnostics: &mut Vec<SourceRegistryDiagnostic>,
) -> Vec<RegistrySource> {
    let mut valid_sources = Vec::new();
    let mut seen_keys = HashMap::<String, (SourceRegistryDocumentOrigin, String)>::new();

    for document in documents {
        let Some(parsed) = parse_registry_document(&document, parse_source_document, diagnostics)
        else {
            continue;
        };
        if !filename_matches_key(&document.path, &parsed.key) {
            diagnostics.push(filename_key_mismatch_diagnostic(&document, &parsed.key));
            continue;
        }
        if let Some((first_origin, first_path)) = seen_keys.get(&parsed.key) {
            diagnostics.push(duplicate_key_diagnostic(
                &document,
                &parsed.key,
                *first_origin,
                first_path,
            ));
            continue;
        }

        seen_keys.insert(parsed.key.clone(), (document.origin, document.path.clone()));
        valid_sources.push(RegistrySource {
            origin: document.origin,
            path: document.path,
            document: parsed,
        });
    }

    valid_sources
}

fn parse_registry_document<T>(
    document: &RawRegistryDocument,
    parse: impl FnOnce(Value) -> Result<T, String>,
    diagnostics: &mut Vec<SourceRegistryDiagnostic>,
) -> Option<T> {
    let value = match serde_json::from_str::<Value>(&document.contents) {
        Ok(value) => value,
        Err(error) => {
            diagnostics.push(SourceRegistryDiagnostic {
                code: SourceRegistryDiagnosticCode::InvalidJson,
                document_kind: document.kind,
                origin: document.origin,
                path: document.path.clone(),
                key: None,
                message: format!("invalid JSON: {error}"),
            });
            return None;
        }
    };

    match parse(value) {
        Ok(parsed) => Some(parsed),
        Err(error) => {
            diagnostics.push(SourceRegistryDiagnostic {
                code: SourceRegistryDiagnosticCode::InvalidShape,
                document_kind: document.kind,
                origin: document.origin,
                path: document.path.clone(),
                key: None,
                message: error,
            });
            None
        }
    }
}

fn parse_profile_document(value: Value) -> Result<SourceProfileDocument, String> {
    let document = serde_json::from_value::<SourceProfileDocument>(value)
        .map_err(|error| format!("source profile document shape is invalid: {error}"))?;

    validate_schema_version(document.schema_version)?;
    validate_technical_key("key", &document.key)?;
    validate_required_text("name", &document.name)?;
    validate_json_object_option(document.source_config_schema.as_ref(), "sourceConfigSchema")?;
    if let Some(identity) = &document.identity {
        validate_profile_identity(identity)?;
    }
    if document.access_paths.is_empty() {
        return Err("accessPaths must contain at least one access path".to_string());
    }

    let mut access_path_keys = HashSet::new();
    for (index, access_path) in document.access_paths.iter().enumerate() {
        let path = format!("accessPaths[{index}]");
        validate_technical_key(&format!("{path}.key"), &access_path.key)?;
        if !access_path_keys.insert(access_path.key.clone()) {
            return Err(format!(
                "accessPaths contains duplicate key `{}`",
                access_path.key
            ));
        }
        if let Some(name) = &access_path.name {
            validate_required_text(&format!("{path}.name"), name)?;
        }
        validate_technical_key(&format!("{path}.adapterKey"), &access_path.adapter_key)?;
        validate_access_path_json_blocks(access_path, &path)?;
        if let Some(availability) = &access_path.availability {
            validate_availability_block(availability, &format!("{path}.availability"))?;
        }
    }

    Ok(document)
}

fn validate_profile_identity(identity: &SourceProfileIdentity) -> Result<(), String> {
    validate_non_empty_strings(&identity.key_candidates, "identity.keyCandidates")?;
    validate_non_empty_strings(&identity.name_candidates, "identity.nameCandidates")?;
    validate_json_object_option(
        identity.optional_source_config.as_ref(),
        "identity.optionalSourceConfig",
    )
}

fn validate_availability_block(availability: &AvailabilityBlock, path: &str) -> Result<(), String> {
    for (index, capture_key) in availability.required_captures.iter().enumerate() {
        validate_capture_key(&format!("{path}.requiredCaptures[{index}]"), capture_key)?;
    }
    validate_json_object_option(
        availability.source_config.as_ref(),
        &format!("{path}.sourceConfig"),
    )
}

fn validate_access_path_json_blocks(
    access_path: &ProfileAccessPathDefinition,
    path: &str,
) -> Result<(), String> {
    validate_json_object_option(
        access_path.source_config_schema.as_ref(),
        &format!("{path}.sourceConfigSchema"),
    )?;
    validate_json_object_option(access_path.query.as_ref(), &format!("{path}.query"))?;
    validate_json_object_option(access_path.inventory.as_ref(), &format!("{path}.inventory"))?;
    validate_inventory_posting_meta_option(
        access_path.inventory.as_ref(),
        &format!("{path}.inventory"),
    )?;
    validate_posting_detail_option(
        access_path.posting_detail.as_ref(),
        &format!("{path}.postingDetail"),
    )?;
    validate_json_object_option(
        access_path.manual_release.as_ref(),
        &format!("{path}.manualRelease"),
    )
}

fn validate_inventory_posting_meta_option(value: Option<&Value>, path: &str) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    let object = json_object_map(value, path)?;
    let Some(fields) = object.get("fields") else {
        return Ok(());
    };
    let fields = json_object_map(fields, &format!("{path}.fields"))?;
    let Some(posting_meta) = fields.get("postingMeta") else {
        return Ok(());
    };
    let posting_meta = json_object_map(posting_meta, &format!("{path}.fields.postingMeta"))?;
    if posting_meta.is_empty() {
        return Err(format!(
            "{path}.fields.postingMeta must declare at least one technical key"
        ));
    }
    validate_allowed_object_keys(
        posting_meta,
        &format!("{path}.fields.postingMeta"),
        &["jobId"],
    )?;
    for (key, expression) in posting_meta {
        validate_inventory_posting_meta_expression(
            expression,
            &format!("{path}.fields.postingMeta.{key}"),
        )?;
    }
    Ok(())
}

fn validate_inventory_posting_meta_expression(value: &Value, path: &str) -> Result<(), String> {
    let expression = json_object_map(value, path)?;
    let has_template = expression.contains_key("template");
    let has_json_path = expression.contains_key("jsonPath");
    match (has_template, has_json_path) {
        (true, false) => {
            validate_allowed_object_keys(expression, path, &["template"])?;
            required_non_empty_string_field(expression, "template", &format!("{path}.template"))?;
            Ok(())
        }
        (false, true) => {
            validate_allowed_object_keys(expression, path, &["jsonPath"])?;
            required_non_empty_string_field(expression, "jsonPath", &format!("{path}.jsonPath"))?;
            Ok(())
        }
        (true, true) => Err(format!(
            "{path} must contain exactly one of template or jsonPath"
        )),
        (false, false) => Err(format!(
            "{path} must contain exactly one of template or jsonPath"
        )),
    }
}

fn validate_posting_detail_option(value: Option<&Value>, path: &str) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    let object = json_object_map(value, path)?;
    validate_allowed_object_keys(
        object,
        path,
        &["fetch", "parse", "fields", "items", "match"],
    )?;

    let fetch = required_json_object_field(object, "fetch", &format!("{path}.fetch"))?;
    validate_allowed_object_keys(fetch, &format!("{path}.fetch"), &["url"])?;
    required_non_empty_string_field(fetch, "url", &format!("{path}.fetch.url"))?;

    let parse = required_json_object_field(object, "parse", &format!("{path}.parse"))?;
    validate_allowed_object_keys(parse, &format!("{path}.parse"), &["as"])?;
    let parse_as = required_non_empty_string_field(parse, "as", &format!("{path}.parse.as"))?;
    if !matches!(parse_as, "html" | "json" | "xml") {
        return Err(format!(
            "{path}.parse.as must be one of `html`, `json`, or `xml` for the postingDetail language"
        ));
    }

    let fields = required_json_object_field(object, "fields", &format!("{path}.fields"))?;
    validate_allowed_object_keys(fields, &format!("{path}.fields"), &["descriptionText"])?;
    let description_text = required_json_object_field(
        fields,
        "descriptionText",
        &format!("{path}.fields.descriptionText"),
    )?;
    validate_posting_detail_description_text_field(
        description_text,
        parse_as,
        &format!("{path}.fields.descriptionText"),
    )?;

    validate_posting_detail_collection_option(object, parse_as, path)?;

    Ok(())
}

fn validate_posting_detail_collection_option(
    object: &serde_json::Map<String, Value>,
    parse_as: &str,
    path: &str,
) -> Result<(), String> {
    let has_items = object.contains_key("items");
    let has_match = object.contains_key("match");
    match (has_items, has_match) {
        (false, false) => return Ok(()),
        (true, false) => {
            return Err(format!(
                "{path}.match is required when {path}.items is declared"
            ))
        }
        (false, true) => {
            return Err(format!(
                "{path}.items is required when {path}.match is declared"
            ))
        }
        (true, true) => {}
    }

    if parse_as == "html" {
        return Err(format!(
            "{path}.items collection matching is supported only for json or xml postingDetail documents"
        ));
    }

    let items = required_json_object_field(object, "items", &format!("{path}.items"))?;
    validate_allowed_object_keys(items, &format!("{path}.items"), &["select"])?;
    let select = required_json_object_field(items, "select", &format!("{path}.items.select"))?;
    match parse_as {
        "json" => {
            validate_allowed_object_keys(select, &format!("{path}.items.select"), &["jsonPath"])?;
            required_non_empty_string_field(
                select,
                "jsonPath",
                &format!("{path}.items.select.jsonPath"),
            )?;
        }
        "xml" => {
            validate_allowed_object_keys(select, &format!("{path}.items.select"), &["xmlElement"])?;
            required_non_empty_string_field(
                select,
                "xmlElement",
                &format!("{path}.items.select.xmlElement"),
            )?;
        }
        _ => unreachable!("postingDetail parse.as was validated before collection validation"),
    }

    let match_rule = required_json_object_field(object, "match", &format!("{path}.match"))?;
    validate_allowed_object_keys(match_rule, &format!("{path}.match"), &["field", "equals"])?;
    required_non_empty_string_field(match_rule, "equals", &format!("{path}.match.equals"))?;
    let field = required_json_object_field(match_rule, "field", &format!("{path}.match.field"))?;
    match parse_as {
        "json" => {
            validate_allowed_object_keys(field, &format!("{path}.match.field"), &["jsonPath"])?;
            required_non_empty_string_field(
                field,
                "jsonPath",
                &format!("{path}.match.field.jsonPath"),
            )?;
        }
        "xml" => {
            validate_allowed_object_keys(field, &format!("{path}.match.field"), &["xmlText"])?;
            required_non_empty_string_field(
                field,
                "xmlText",
                &format!("{path}.match.field.xmlText"),
            )?;
        }
        _ => unreachable!("postingDetail parse.as was validated before collection validation"),
    }

    Ok(())
}

fn validate_posting_detail_description_text_field(
    description_text: &serde_json::Map<String, Value>,
    parse_as: &str,
    path: &str,
) -> Result<(), String> {
    match parse_as {
        "html" => {
            validate_allowed_object_keys(description_text, path, &["selectorText"])?;
            let selector = required_non_empty_string_field(
                description_text,
                "selectorText",
                &format!("{path}.selectorText"),
            )?;
            validate_css_selector(selector, &format!("{path}.selectorText"))
        }
        "json" => {
            validate_exactly_one_posting_detail_key(
                description_text,
                path,
                &["jsonPath", "jsonPathHtml"],
                "jsonPath or jsonPathHtml for JSON postingDetail extraction",
            )?;
            validate_allowed_object_keys(description_text, path, &["jsonPath", "jsonPathHtml"])?;
            for key in ["jsonPath", "jsonPathHtml"] {
                if description_text.contains_key(key) {
                    required_non_empty_string_field(
                        description_text,
                        key,
                        &format!("{path}.{key}"),
                    )?;
                }
            }
            Ok(())
        }
        "xml" => {
            validate_exactly_one_posting_detail_key(
                description_text,
                path,
                &["xmlText", "xmlTextHtml", "xmlElement"],
                "xmlText, xmlTextHtml, or xmlElement for XML postingDetail extraction",
            )?;
            validate_allowed_object_keys(
                description_text,
                path,
                &["xmlText", "xmlTextHtml", "xmlElement"],
            )?;
            for key in ["xmlText", "xmlTextHtml", "xmlElement"] {
                if description_text.contains_key(key) {
                    required_non_empty_string_field(
                        description_text,
                        key,
                        &format!("{path}.{key}"),
                    )?;
                }
            }
            Ok(())
        }
        _ => unreachable!("postingDetail parse.as was validated before field validation"),
    }
}

fn validate_exactly_one_posting_detail_key(
    object: &serde_json::Map<String, Value>,
    path: &str,
    keys: &[&str],
    expected: &str,
) -> Result<(), String> {
    let count = keys.iter().filter(|key| object.contains_key(**key)).count();
    if count == 1 {
        Ok(())
    } else {
        Err(format!("{path} must contain exactly one of {expected}"))
    }
}

fn validate_css_selector(selector: &str, path: &str) -> Result<(), String> {
    Matcher::new(selector)
        .map(|_| ())
        .map_err(|error| format!("{path} must be a valid CSS selector: {error:?}"))
}

fn validate_allowed_object_keys(
    object: &serde_json::Map<String, Value>,
    path: &str,
    allowed_keys: &[&str],
) -> Result<(), String> {
    for key in object.keys() {
        if !allowed_keys.contains(&key.as_str()) {
            return Err(format!("{path}.{key} is not supported"));
        }
    }
    Ok(())
}

fn required_json_object_field<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a serde_json::Map<String, Value>, String> {
    object
        .get(key)
        .ok_or_else(|| format!("{path} is required"))
        .and_then(|value| json_object_map(value, path))
}

fn required_non_empty_string_field<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a str, String> {
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{path} must be a non-empty string"))?;
    if value.trim().is_empty() {
        return Err(format!("{path} must be a non-empty string"));
    }
    Ok(value)
}

fn json_object_map<'a>(
    value: &'a Value,
    path: &str,
) -> Result<&'a serde_json::Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{path} must be a JSON object"))
}

fn parse_source_document(value: Value) -> Result<SourceDocument, String> {
    let raw = serde_json::from_value::<RawSourceDocument>(value)
        .map_err(|error| format!("source document shape is invalid: {error}"))?;

    validate_schema_version(raw.schema_version)?;
    validate_technical_key("key", &raw.key)?;
    validate_required_text("name", &raw.name)?;
    validate_json_object(&raw.source_config, "sourceConfig")?;
    let selected_access_path = parse_selected_access_path(raw.selected_access_path)?;

    Ok(SourceDocument {
        schema_version: raw.schema_version,
        key: raw.key,
        name: raw.name,
        status: raw.status,
        source_config: raw.source_config,
        selected_access_path,
    })
}

fn parse_selected_access_path(value: Value) -> Result<SelectedAccessPath, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "selectedAccessPath must be a JSON object".to_string())?;
    let Some(path_type) = object.get("type").and_then(Value::as_str) else {
        return Err("selectedAccessPath.type must be profile or source_specific".to_string());
    };

    match path_type {
        "profile" => {
            let raw = serde_json::from_value::<RawProfileSelectedAccessPath>(value)
                .map_err(|error| format!("selectedAccessPath profile shape is invalid: {error}"))?;
            if raw.path_type != "profile" {
                return Err("selectedAccessPath.type must be profile".to_string());
            }
            validate_technical_key("selectedAccessPath.profileKey", &raw.profile_key)?;
            validate_technical_key("selectedAccessPath.pathKey", &raw.path_key)?;
            Ok(SelectedAccessPath::Profile {
                profile_key: raw.profile_key,
                path_key: raw.path_key,
            })
        }
        "source_specific" => {
            if object.contains_key("availability") {
                return Err(
                    "selectedAccessPath.availability is not allowed for source_specific access paths"
                        .to_string(),
                );
            }
            let raw = serde_json::from_value::<RawSourceSpecificSelectedAccessPath>(value)
                .map_err(|error| {
                    format!("selectedAccessPath source_specific shape is invalid: {error}")
                })?;
            if raw.path_type != "source_specific" {
                return Err("selectedAccessPath.type must be source_specific".to_string());
            }
            validate_technical_key("selectedAccessPath.adapterKey", &raw.adapter_key)?;
            validate_json_object_option(
                raw.source_config_schema.as_ref(),
                "selectedAccessPath.sourceConfigSchema",
            )?;
            validate_json_object_option(raw.query.as_ref(), "selectedAccessPath.query")?;
            validate_json_object_option(raw.inventory.as_ref(), "selectedAccessPath.inventory")?;
            validate_json_object_option(
                raw.manual_release.as_ref(),
                "selectedAccessPath.manualRelease",
            )?;
            Ok(SelectedAccessPath::SourceSpecific {
                adapter_key: raw.adapter_key,
                source_config_schema: raw.source_config_schema,
                query: raw.query,
                inventory: raw.inventory,
                interactions: raw.interactions,
                manual_release: raw.manual_release,
            })
        }
        _ => Err(format!(
            "selectedAccessPath.type `{path_type}` must be profile or source_specific"
        )),
    }
}

fn validate_source_references(
    sources: Vec<RegistrySource>,
    profiles: &[RegistrySourceProfile],
    diagnostics: &mut Vec<SourceRegistryDiagnostic>,
) -> Vec<RegistrySource> {
    let profiles_by_key = profiles
        .iter()
        .map(|profile| (profile.document.key.as_str(), profile))
        .collect::<HashMap<_, _>>();
    let mut valid_sources = Vec::with_capacity(sources.len());

    for source in sources {
        let SelectedAccessPath::Profile {
            profile_key,
            path_key,
        } = &source.document.selected_access_path
        else {
            valid_sources.push(source);
            continue;
        };

        let Some(profile) = profiles_by_key.get(profile_key.as_str()) else {
            diagnostics.push(SourceRegistryDiagnostic {
                code: SourceRegistryDiagnosticCode::MissingProfileRef,
                document_kind: SourceRegistryDocumentKind::Source,
                origin: source.origin,
                path: source.path.clone(),
                key: Some(source.document.key.clone()),
                message: format!(
                    "source `{}` references missing profile `{profile_key}`",
                    source.document.key
                ),
            });
            continue;
        };

        if !profile
            .document
            .access_paths
            .iter()
            .any(|access_path| access_path.key == *path_key)
        {
            diagnostics.push(SourceRegistryDiagnostic {
                code: SourceRegistryDiagnosticCode::MissingPathRef,
                document_kind: SourceRegistryDocumentKind::Source,
                origin: source.origin,
                path: source.path.clone(),
                key: Some(source.document.key.clone()),
                message: format!(
                    "source `{}` references missing path `{path_key}` on profile `{profile_key}`",
                    source.document.key
                ),
            });
            continue;
        }

        valid_sources.push(source);
    }

    valid_sources
}

fn validate_schema_version(schema_version: u64) -> Result<(), String> {
    if schema_version == 1 {
        Ok(())
    } else {
        Err("schemaVersion must be 1".to_string())
    }
}

fn validate_required_text(path: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{path} must be a non-empty string"))
    } else {
        Ok(())
    }
}

fn validate_non_empty_strings(values: &[String], path: &str) -> Result<(), String> {
    for (index, value) in values.iter().enumerate() {
        validate_required_text(&format!("{path}[{index}]"), value)?;
    }
    Ok(())
}

fn validate_technical_key(path: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{path} must be a non-empty technical key"));
    }

    if value.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
    }) {
        Ok(())
    } else {
        Err(format!("{path} must match ^[a-z0-9_]+$"))
    }
}

fn validate_capture_key(path: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{path} must be a non-empty capture key"));
    }

    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        Ok(())
    } else {
        Err(format!("{path} must match ^[A-Za-z0-9_]+$"))
    }
}

fn validate_json_object_option(value: Option<&Value>, path: &str) -> Result<(), String> {
    match value {
        Some(value) => validate_json_object(value, path),
        None => Ok(()),
    }
}

fn validate_json_object(value: &Value, path: &str) -> Result<(), String> {
    if value.is_object() {
        Ok(())
    } else {
        Err(format!("{path} must be a JSON object"))
    }
}

fn filename_matches_key(path: &str, key: &str) -> bool {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem == key)
}

fn filename_key_mismatch_diagnostic(
    document: &RawRegistryDocument,
    key: &str,
) -> SourceRegistryDiagnostic {
    let filename = Path::new(&document.path)
        .file_name()
        .and_then(|filename| filename.to_str())
        .unwrap_or(document.path.as_str());
    SourceRegistryDiagnostic {
        code: SourceRegistryDiagnosticCode::FilenameKeyMismatch,
        document_kind: document.kind,
        origin: document.origin,
        path: document.path.clone(),
        key: Some(key.to_string()),
        message: format!("filename `{filename}` must be `{key}.json`"),
    }
}

fn duplicate_key_diagnostic(
    document: &RawRegistryDocument,
    key: &str,
    first_origin: SourceRegistryDocumentOrigin,
    first_path: &str,
) -> SourceRegistryDiagnostic {
    SourceRegistryDiagnostic {
        code: SourceRegistryDiagnosticCode::DuplicateKey,
        document_kind: document.kind,
        origin: document.origin,
        path: document.path.clone(),
        key: Some(key.to_string()),
        message: format!(
            "key `{key}` duplicates existing {} document `{first_path}`; duplicate ignored",
            origin_label(first_origin)
        ),
    }
}

fn origin_label(origin: SourceRegistryDocumentOrigin) -> &'static str {
    match origin {
        SourceRegistryDocumentOrigin::BuiltIn => "built-in",
        SourceRegistryDocumentOrigin::Custom => "custom",
    }
}
