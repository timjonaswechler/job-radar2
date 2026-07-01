use std::collections::HashSet;

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::{JsonObject, JsonSchemaObject};
use crate::source::documents::SelectedAccessPath;

use super::compiler_error;

pub(super) fn source_config_schema_keys(
    profile_schema: Option<&JsonSchemaObject>,
    access_path_schema: Option<&JsonSchemaObject>,
) -> HashSet<String> {
    schema_property_keys(profile_schema)
        .union(&schema_property_keys(access_path_schema))
        .cloned()
        .collect()
}

pub(super) fn validate_source_config(
    profile_schema: Option<&JsonSchemaObject>,
    access_path_schema: Option<&JsonSchemaObject>,
    source_config: &JsonObject,
    access_path_index: Option<usize>,
    diagnostics: &mut Diagnostics,
) {
    validate_source_config_schema(profile_schema, "/sourceConfigSchema", diagnostics);
    let access_path_schema_path = access_path_index
        .map(|index| format!("/accessPaths/{index}/sourceConfigSchema"))
        .unwrap_or_else(|| "/selectedAccessPath/sourceConfigSchema".to_string());
    validate_source_config_schema(access_path_schema, &access_path_schema_path, diagnostics);

    let profile_properties = schema_property_keys(profile_schema);
    let access_path_properties = schema_property_keys(access_path_schema);
    for key in profile_properties.intersection(&access_path_properties) {
        diagnostics.push(compiler_error(
            "source_config_schema_property_redefinition",
            format!("Access Path Source Config schema redefines profile-level property `{key}`"),
            format!("{access_path_schema_path}/properties/{key}"),
            serde_json::json!({ "property": key }),
        ));
    }

    let required = schema_required_keys(profile_schema)
        .into_iter()
        .chain(schema_required_keys(access_path_schema))
        .collect::<HashSet<_>>();
    for key in required {
        if !source_config.contains_key(&key) {
            diagnostics.push(compiler_error(
                "missing_source_config_required_property",
                format!("Source Config is missing required property `{key}`"),
                format!("/sourceConfig/{key}"),
                serde_json::json!({ "property": key }),
            ));
        }
    }

    let allowed = profile_properties
        .union(&access_path_properties)
        .cloned()
        .collect::<HashSet<_>>();
    let additional_allowed = allowed.is_empty()
        || !(schema_forbids_additional_properties(profile_schema)
            || schema_forbids_additional_properties(access_path_schema));

    for (key, value) in source_config {
        if is_search_request_criteria_key(key) {
            diagnostics.push(compiler_error(
                "forbidden_search_criteria_in_source_config",
                format!("Source Config property `{key}` looks like Search Request criteria"),
                format!("/sourceConfig/{key}"),
                serde_json::json!({ "property": key }),
            ));
        }
        if !additional_allowed && !allowed.contains(key) {
            diagnostics.push(compiler_error(
                "unknown_source_config_property",
                format!("Source Config property `{key}` is not declared by the selected Source Config schema"),
                format!("/sourceConfig/{key}"),
                serde_json::json!({ "property": key }),
            ));
        }
        if let Some(expected_type) =
            property_type(profile_schema, key).or_else(|| property_type(access_path_schema, key))
        {
            if !json_value_matches_schema_type(value, expected_type) {
                diagnostics.push(compiler_error(
                    "invalid_source_config_property_type",
                    format!("Source Config property `{key}` does not match schema type `{expected_type}`"),
                    format!("/sourceConfig/{key}"),
                    serde_json::json!({
                        "property": key,
                        "expectedType": expected_type,
                    }),
                ));
            }
        }
    }
}

fn validate_source_config_schema(
    schema: Option<&JsonSchemaObject>,
    schema_path: &str,
    diagnostics: &mut Diagnostics,
) {
    let Some(schema) = schema else {
        return;
    };
    for key in schema_property_keys(Some(schema)) {
        if is_search_request_criteria_key(&key) {
            diagnostics.push(compiler_error(
                "forbidden_search_criteria_in_source_config_schema",
                format!("Source Config schema property `{key}` looks like Search Request criteria"),
                format!("{schema_path}/properties/{key}"),
                serde_json::json!({ "property": key }),
            ));
        }
    }
}

pub(super) fn source_owned_access_path_schema(
    selected_access_path: &SelectedAccessPath,
) -> Option<&JsonSchemaObject> {
    match selected_access_path {
        SelectedAccessPath::SourceOwnedAccessPath {
            source_config_schema,
            ..
        } => source_config_schema.as_ref(),
        SelectedAccessPath::ProfileAccessPath { .. } => None,
    }
}

fn schema_property_keys(schema: Option<&JsonSchemaObject>) -> HashSet<String> {
    schema
        .and_then(|schema| schema.get("properties"))
        .and_then(|properties| properties.as_object())
        .map(|properties| properties.keys().cloned().collect())
        .unwrap_or_default()
}

fn schema_required_keys(schema: Option<&JsonSchemaObject>) -> HashSet<String> {
    schema
        .and_then(|schema| schema.get("required"))
        .and_then(|required| required.as_array())
        .map(|required| {
            required
                .iter()
                .filter_map(|value| value.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn schema_forbids_additional_properties(schema: Option<&JsonSchemaObject>) -> bool {
    schema
        .and_then(|schema| schema.get("additionalProperties"))
        .and_then(|value| value.as_bool())
        .is_some_and(|additional_properties| !additional_properties)
}

fn property_type<'a>(schema: Option<&'a JsonSchemaObject>, key: &str) -> Option<&'a str> {
    schema
        .and_then(|schema| schema.get("properties"))
        .and_then(|properties| properties.as_object())
        .and_then(|properties| properties.get(key))
        .and_then(|property| property.get("type"))
        .and_then(|schema_type| schema_type.as_str())
}

fn json_value_matches_schema_type(value: &serde_json::Value, schema_type: &str) -> bool {
    match schema_type {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        "null" => value.is_null(),
        _ => true,
    }
}

fn is_search_request_criteria_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(|character| character.to_lowercase())
        .collect::<String>();
    matches!(
        normalized.as_str(),
        "keyword"
            | "keywords"
            | "role"
            | "roles"
            | "preferredlocation"
            | "preferredlocations"
            | "country"
            | "countries"
            | "radius"
            | "includerule"
            | "includerules"
            | "excluderule"
            | "excluderules"
            | "matchrule"
            | "matchrules"
            | "exclusionrule"
            | "exclusionrules"
    )
}
