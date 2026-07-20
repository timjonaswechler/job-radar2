//! Shared constrained Effective Source Config Schema implementation.
//!
//! This is deliberately not a general JSON Schema interpreter. It compiles the
//! admitted profile/path subset into an immutable neutral contract and exposes
//! incremental and final value validation to compiler and Detection callers.

use std::collections::{BTreeMap, BTreeSet};

use bigdecimal::BigDecimal;
use regex::Regex;
use serde_json::{Map, Number, Value};
use url::Url;

use crate::profile_dsl::documents::{JsonObject, JsonSchemaObject};

#[derive(Clone, Copy)]
pub(crate) struct SchemaLocation<'a> {
    pub schema: Option<&'a JsonSchemaObject>,
    pub path: &'a str,
    pub title_allowed: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct EffectiveSourceConfigContract {
    properties: BTreeMap<String, PropertyContract>,
    required: BTreeSet<String>,
    additional_properties: bool,
}

#[derive(Clone, Debug)]
struct PropertyContract {
    property_type: ScalarType,
    pattern: Option<(String, Regex)>,
    enum_values: Option<Vec<Value>>,
    format_uri: bool,
    minimum: Option<(Number, BigDecimal)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScalarType {
    String,
    Number,
    Integer,
    Boolean,
    Object,
    Array,
    Null,
}

impl ScalarType {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "string" => Some(Self::String),
            "number" => Some(Self::Number),
            "integer" => Some(Self::Integer),
            "boolean" => Some(Self::Boolean),
            "object" => Some(Self::Object),
            "array" => Some(Self::Array),
            "null" => Some(Self::Null),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Integer => "integer",
            Self::Boolean => "boolean",
            Self::Object => "object",
            Self::Array => "array",
            Self::Null => "null",
        }
    }

    fn matches(self, value: &Value) -> bool {
        match self {
            Self::String => value.is_string(),
            Self::Number => value.is_number(),
            Self::Integer => value.as_i64().is_some() || value.as_u64().is_some(),
            Self::Boolean => value.is_boolean(),
            Self::Object => value.is_object(),
            Self::Array => value.is_array(),
            Self::Null => value.is_null(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ContractViolation {
    pub code: &'static str,
    pub message: String,
    pub path: String,
    pub details: Value,
}

pub(crate) fn compile_contract(
    locations: &[SchemaLocation<'_>],
) -> Result<EffectiveSourceConfigContract, Vec<ContractViolation>> {
    let mut violations = Vec::new();
    let mut properties = BTreeMap::new();
    let mut required = BTreeSet::new();
    let mut additional_properties = true;

    for location in locations {
        let Some(schema) = location.schema else {
            continue;
        };
        validate_root_shape(schema, location.path, &mut violations);

        if schema.get("additionalProperties").and_then(Value::as_bool) == Some(false) {
            additional_properties = false;
        }

        if let Some(values) = schema.get("required").and_then(Value::as_array) {
            let mut local = BTreeSet::new();
            for (index, value) in values.iter().enumerate() {
                let Some(name) = value.as_str() else { continue };
                if !local.insert(name.to_string()) {
                    violations.push(definition_violation(
                        "duplicate_source_config_schema_required_property",
                        format!("Source Config schema required property `{name}` is duplicated"),
                        format!("{}/required/{index}", location.path),
                        serde_json::json!({ "property": name }),
                    ));
                }
                required.insert(name.to_string());
            }
        }

        if let Some(authored_properties) = schema.get("properties").and_then(Value::as_object) {
            for (name, property_schema) in authored_properties {
                let property_path = format!(
                    "{}/properties/{}",
                    location.path,
                    escape_pointer_segment(name)
                );
                if properties.contains_key(name) {
                    violations.push(definition_violation(
                        "source_config_schema_property_redefinition",
                        format!("Source Config property `{name}` is declared at more than one composed schema level"),
                        property_path,
                        serde_json::json!({ "property": name }),
                    ));
                    continue;
                }
                if let Some(property) = compile_property(
                    name,
                    property_schema,
                    &property_path,
                    location.title_allowed,
                    &mut violations,
                ) {
                    properties.insert(name.clone(), property);
                }
            }
        }
    }

    for name in &required {
        if !properties.contains_key(name) {
            violations.push(definition_violation(
                "undeclared_source_config_schema_required_property",
                format!("Required Source Config property `{name}` is not declared by the composed schema"),
                required_definition_path(locations, name),
                serde_json::json!({ "property": name }),
            ));
        }
    }

    if violations.is_empty() {
        Ok(EffectiveSourceConfigContract {
            properties,
            required,
            additional_properties,
        })
    } else {
        violations
            .sort_by(|left, right| left.path.cmp(&right.path).then(left.code.cmp(right.code)));
        violations.dedup();
        Err(violations)
    }
}

impl EffectiveSourceConfigContract {
    pub(crate) fn property_keys(&self) -> BTreeSet<String> {
        self.properties.keys().cloned().collect()
    }

    pub(crate) fn validate_incremental(&self, values: &JsonObject) -> Vec<ContractViolation> {
        self.validate(values, false)
    }

    pub(crate) fn validate_complete(&self, values: &JsonObject) -> Vec<ContractViolation> {
        self.validate(values, true)
    }

    fn validate(&self, values: &JsonObject, complete: bool) -> Vec<ContractViolation> {
        let mut violations = Vec::new();
        if complete {
            for name in &self.required {
                if !values.contains_key(name) {
                    violations.push(value_violation(
                        "missing_source_config_required_property",
                        format!("Source Config is missing required property `{name}`"),
                        name,
                        serde_json::json!({ "property": name }),
                    ));
                }
            }
        }

        let mut entries = values.iter().collect::<Vec<_>>();
        entries.sort_by(|(left, _), (right, _)| left.cmp(right));
        for (name, value) in entries {
            if is_search_request_criteria_key(name) {
                violations.push(value_violation(
                    "forbidden_search_criteria_in_source_config",
                    format!("Source Config property `{name}` looks like Search Request criteria"),
                    name,
                    serde_json::json!({ "property": name }),
                ));
            }
            let Some(property) = self.properties.get(name) else {
                if !self.additional_properties {
                    violations.push(value_violation(
                        "unknown_source_config_property",
                        format!("Source Config property `{name}` is not declared by the Effective Source Config Schema"),
                        name,
                        serde_json::json!({ "property": name }),
                    ));
                }
                continue;
            };
            validate_property_value(name, value, property, &mut violations);
        }
        violations
    }
}

fn validate_root_shape(
    schema: &Map<String, Value>,
    path: &str,
    violations: &mut Vec<ContractViolation>,
) {
    const ALLOWED: &[&str] = &["type", "properties", "required", "additionalProperties"];
    for keyword in schema.keys() {
        if !ALLOWED.contains(&keyword.as_str()) {
            violations.push(definition_violation(
                "unsupported_source_config_schema_keyword",
                format!("Source Config schema keyword `{keyword}` is not supported"),
                format!("{path}/{}", escape_pointer_segment(keyword)),
                serde_json::json!({ "keyword": keyword }),
            ));
        }
    }
    if let Some(value) = schema.get("type") {
        if value.as_str() != Some("object") {
            violations.push(definition_violation(
                "invalid_source_config_schema_root_type",
                "Source Config schema root type must be `object`",
                format!("{path}/type"),
                serde_json::json!({ "expectedType": "object" }),
            ));
        }
    }
    validate_optional_shape(
        schema,
        "properties",
        Value::is_object,
        "object",
        path,
        violations,
    );
    validate_optional_shape(
        schema,
        "required",
        Value::is_array,
        "array",
        path,
        violations,
    );
    validate_optional_shape(
        schema,
        "additionalProperties",
        Value::is_boolean,
        "boolean",
        path,
        violations,
    );
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for (index, value) in required.iter().enumerate() {
            if value.as_str().is_none() {
                violations.push(definition_violation(
                    "invalid_source_config_schema_required_property",
                    "Source Config schema required entries must be strings",
                    format!("{path}/required/{index}"),
                    serde_json::json!({ "expectedType": "string" }),
                ));
            }
        }
    }
}

fn validate_optional_shape(
    schema: &Map<String, Value>,
    keyword: &str,
    predicate: fn(&Value) -> bool,
    expected: &str,
    path: &str,
    violations: &mut Vec<ContractViolation>,
) {
    if schema.get(keyword).is_some_and(|value| !predicate(value)) {
        violations.push(definition_violation(
            "invalid_source_config_schema_keyword_shape",
            format!("Source Config schema keyword `{keyword}` must be an {expected}"),
            format!("{path}/{keyword}"),
            serde_json::json!({ "keyword": keyword, "expectedType": expected }),
        ));
    }
}

fn compile_property(
    name: &str,
    schema: &Value,
    path: &str,
    title_allowed: bool,
    violations: &mut Vec<ContractViolation>,
) -> Option<PropertyContract> {
    if is_search_request_criteria_key(name) {
        violations.push(definition_violation(
            "forbidden_search_criteria_in_source_config_schema",
            format!("Source Config schema property `{name}` looks like Search Request criteria"),
            path.to_string(),
            serde_json::json!({ "property": name }),
        ));
    }
    let Some(schema) = schema.as_object() else {
        violations.push(definition_violation(
            "invalid_source_config_property_schema",
            format!("Source Config property schema `{name}` must be an object"),
            path.to_string(),
            serde_json::json!({ "property": name, "expectedType": "object" }),
        ));
        return None;
    };
    const ALLOWED: &[&str] = &["type", "pattern", "enum", "format", "minimum", "title"];
    for keyword in schema.keys() {
        if !ALLOWED.contains(&keyword.as_str()) {
            violations.push(definition_violation(
                "unsupported_source_config_property_schema_keyword",
                format!("Source Config property schema keyword `{keyword}` is not supported"),
                format!("{path}/{}", escape_pointer_segment(keyword)),
                serde_json::json!({ "property": name, "keyword": keyword }),
            ));
        }
    }

    let property_type = match schema
        .get("type")
        .and_then(Value::as_str)
        .and_then(ScalarType::parse)
    {
        Some(property_type) => property_type,
        None => {
            violations.push(definition_violation(
                "invalid_source_config_property_schema_type",
                format!("Source Config property `{name}` requires one supported scalar type"),
                format!("{path}/type"),
                serde_json::json!({ "property": name }),
            ));
            return None;
        }
    };

    let pattern = match schema.get("pattern") {
        None => None,
        Some(Value::String(pattern)) if property_type == ScalarType::String => {
            match Regex::new(pattern) {
                Ok(regex) => Some((pattern.clone(), regex)),
                Err(error) => {
                    violations.push(definition_violation(
                        "invalid_source_config_schema_pattern",
                        format!("Source Config schema pattern for `{name}` is invalid: {error}"),
                        format!("{path}/pattern"),
                        serde_json::json!({ "property": name, "pattern": pattern }),
                    ));
                    None
                }
            }
        }
        Some(_) => {
            violations.push(incompatible_keyword(name, "pattern", "string", path));
            None
        }
    };

    let enum_values = compile_enum(name, property_type, schema.get("enum"), path, violations);
    let format_uri = match schema.get("format") {
        None => false,
        Some(Value::String(format)) if format == "uri" && property_type == ScalarType::String => {
            true
        }
        Some(Value::String(format)) if format != "uri" => {
            violations.push(definition_violation(
                "unsupported_source_config_schema_format",
                format!("Source Config property `{name}` uses unsupported format `{format}`"),
                format!("{path}/format"),
                serde_json::json!({ "property": name, "format": format }),
            ));
            false
        }
        Some(_) => {
            violations.push(incompatible_keyword(name, "format", "string", path));
            false
        }
    };

    let minimum = match schema.get("minimum") {
        None => None,
        Some(Value::Number(number))
            if matches!(property_type, ScalarType::Number | ScalarType::Integer) =>
        {
            number
                .to_string()
                .parse::<BigDecimal>()
                .ok()
                .map(|minimum| (number.clone(), minimum))
        }
        Some(_) => {
            violations.push(incompatible_keyword(
                name,
                "minimum",
                "number or integer",
                path,
            ));
            None
        }
    };

    if let Some(title) = schema.get("title") {
        if !title_allowed {
            violations.push(definition_violation(
                "source_config_schema_title_not_allowed",
                format!("Source Config property title for `{name}` is allowed only on reusable Source Profiles"),
                format!("{path}/title"),
                serde_json::json!({ "property": name }),
            ));
        } else if !title.as_str().is_some_and(|title| !title.trim().is_empty()) {
            violations.push(definition_violation(
                "invalid_source_config_schema_title",
                format!("Source Config property title for `{name}` must be a non-empty string"),
                format!("{path}/title"),
                serde_json::json!({ "property": name }),
            ));
        }
    }

    Some(PropertyContract {
        property_type,
        pattern,
        enum_values,
        format_uri,
        minimum,
    })
}

fn compile_enum(
    name: &str,
    property_type: ScalarType,
    value: Option<&Value>,
    path: &str,
    violations: &mut Vec<ContractViolation>,
) -> Option<Vec<Value>> {
    let value = value?;
    let Some(values) = value.as_array() else {
        violations.push(definition_violation(
            "invalid_source_config_schema_enum",
            format!("Source Config property enum for `{name}` must be a non-empty array"),
            format!("{path}/enum"),
            serde_json::json!({ "property": name }),
        ));
        return None;
    };
    if values.is_empty() {
        violations.push(definition_violation(
            "invalid_source_config_schema_enum",
            format!("Source Config property enum for `{name}` must not be empty"),
            format!("{path}/enum"),
            serde_json::json!({ "property": name }),
        ));
    }
    let mut seen = Vec::new();
    for (index, value) in values.iter().enumerate() {
        let scalar =
            value.is_string() || value.is_number() || value.is_boolean() || value.is_null();
        if !scalar || !property_type.matches(value) {
            violations.push(definition_violation(
                "incompatible_source_config_schema_enum_value",
                format!(
                    "Source Config property enum value for `{name}` does not match type `{}`",
                    property_type.label()
                ),
                format!("{path}/enum/{index}"),
                serde_json::json!({ "property": name, "expectedType": property_type.label() }),
            ));
        }
        if seen.iter().any(|seen_value| seen_value == value) {
            violations.push(definition_violation(
                "duplicate_source_config_schema_enum_value",
                format!("Source Config property enum value for `{name}` is duplicated"),
                format!("{path}/enum/{index}"),
                serde_json::json!({ "property": name, "value": value }),
            ));
        }
        seen.push(value.clone());
    }
    Some(values.clone())
}

fn incompatible_keyword(
    name: &str,
    keyword: &str,
    expected_type: &str,
    path: &str,
) -> ContractViolation {
    definition_violation(
        "incompatible_source_config_property_schema_keyword",
        format!("Source Config property keyword `{keyword}` for `{name}` requires type `{expected_type}`"),
        format!("{path}/{keyword}"),
        serde_json::json!({ "property": name, "keyword": keyword, "requiredType": expected_type }),
    )
}

fn validate_property_value(
    name: &str,
    value: &Value,
    property: &PropertyContract,
    violations: &mut Vec<ContractViolation>,
) {
    if !property.property_type.matches(value) {
        violations.push(value_violation(
            "invalid_source_config_property_type",
            format!(
                "Source Config property `{name}` does not match schema type `{}`",
                property.property_type.label()
            ),
            name,
            serde_json::json!({ "property": name, "expectedType": property.property_type.label() }),
        ));
        return;
    }
    if let Some(values) = &property.enum_values {
        if !values.iter().any(|allowed| allowed == value) {
            violations.push(value_violation(
                "invalid_source_config_property_enum",
                format!("Source Config property `{name}` is not one of the allowed enum values"),
                name,
                serde_json::json!({ "property": name, "allowedValues": values }),
            ));
        }
    }
    if let Some((pattern, regex)) = &property.pattern {
        if !value.as_str().is_some_and(|value| regex.is_match(value)) {
            violations.push(value_violation(
                "invalid_source_config_property_pattern",
                format!("Source Config property `{name}` does not match the required pattern"),
                name,
                serde_json::json!({ "property": name, "pattern": pattern }),
            ));
        }
    }
    if property.format_uri && !value.as_str().is_some_and(is_absolute_uri) {
        violations.push(value_violation(
            "invalid_source_config_property_uri",
            format!("Source Config property `{name}` must be an absolute URI"),
            name,
            serde_json::json!({ "property": name, "format": "uri" }),
        ));
    }
    if let Some((minimum_number, minimum)) = &property.minimum {
        let below_minimum = value
            .as_number()
            .and_then(|number| number.to_string().parse::<BigDecimal>().ok())
            .is_some_and(|number| number < *minimum);
        if below_minimum {
            violations.push(value_violation(
                "invalid_source_config_property_minimum",
                format!("Source Config property `{name}` must be at least {minimum_number}"),
                name,
                serde_json::json!({ "property": name, "minimum": minimum_number }),
            ));
        }
    }
}

fn is_absolute_uri(value: &str) -> bool {
    Url::parse(value).is_ok()
}

fn required_definition_path(locations: &[SchemaLocation<'_>], name: &str) -> String {
    for location in locations.iter().rev() {
        if let Some(required) = location
            .schema
            .and_then(|schema| schema.get("required"))
            .and_then(Value::as_array)
        {
            if let Some(index) = required
                .iter()
                .position(|value| value.as_str() == Some(name))
            {
                return format!("{}/required/{index}", location.path);
            }
        }
    }
    String::new()
}

fn definition_violation(
    code: &'static str,
    message: impl Into<String>,
    path: String,
    details: Value,
) -> ContractViolation {
    ContractViolation {
        code,
        message: message.into(),
        path,
        details,
    }
}

fn value_violation(
    code: &'static str,
    message: impl Into<String>,
    property: &str,
    details: Value,
) -> ContractViolation {
    ContractViolation {
        code,
        message: message.into(),
        path: format!("/{}", escape_pointer_segment(property)),
        details,
    }
}

pub(crate) fn escape_pointer_segment(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

pub(crate) fn is_search_request_criteria_key(key: &str) -> bool {
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
