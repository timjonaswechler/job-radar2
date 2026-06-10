use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceStatus {
    Draft,
    Active,
    Disabled,
    Invalid,
}

impl SourceStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Disabled => "disabled",
            Self::Invalid => "invalid",
        }
    }
}

impl TryFrom<&str> for SourceStatus {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "draft" => Ok(Self::Draft),
            "active" => Ok(Self::Active),
            "disabled" => Ok(Self::Disabled),
            "invalid" => Ok(Self::Invalid),
            _ => Err(format!("unknown source status: {value}")),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserProfile {
    pub id: i64,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub name_i18n_key: Option<String>,
    pub description_i18n_key: Option<String>,
    pub definition_path: Option<String>,
    pub definition_hash: Option<String>,
    pub definition_schema_version: i64,
    pub definition: Value,
    pub source_config_schema: Value,
    pub status: SourceStatus,
    pub validation_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBrowserProfileInput {
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub name_i18n_key: Option<String>,
    pub description_i18n_key: Option<String>,
    pub definition_path: Option<String>,
    pub definition_hash: Option<String>,
    pub definition_schema_version: i64,
    #[serde(default = "empty_json_object")]
    pub definition: Value,
    #[serde(default = "empty_json_object")]
    pub source_config_schema: Value,
    pub status: SourceStatus,
    pub validation_error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBrowserProfileInput {
    pub name: String,
    pub description: Option<String>,
    pub name_i18n_key: Option<String>,
    pub description_i18n_key: Option<String>,
    pub definition_path: Option<String>,
    pub definition_hash: Option<String>,
    pub definition_schema_version: i64,
    pub definition: Value,
    pub source_config_schema: Value,
    pub status: SourceStatus,
    pub validation_error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemProfile {
    pub id: i64,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub adapter_key: String,
    pub definition_schema_version: i64,
    pub definition: Value,
    pub source_config_schema: Value,
    pub built_in: bool,
    pub status: SourceStatus,
    pub validation_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSystemProfileInput {
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub adapter_key: String,
    pub definition_schema_version: i64,
    #[serde(default = "empty_json_object")]
    pub definition: Value,
    #[serde(default = "empty_json_object")]
    pub source_config_schema: Value,
    pub status: SourceStatus,
    pub validation_error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSystemProfileInput {
    pub name: String,
    pub description: Option<String>,
    pub adapter_key: String,
    pub definition_schema_version: i64,
    pub definition: Value,
    pub source_config_schema: Value,
    pub status: SourceStatus,
    pub validation_error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: i64,
    pub key: String,
    pub adapter_key: String,
    pub system_profile_id: Option<i64>,
    pub browser_profile_id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub source_config: Value,
    pub status: SourceStatus,
    pub validation_error: Option<String>,
    pub built_in: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSourceInput {
    pub key: String,
    pub adapter_key: String,
    pub system_profile_id: Option<i64>,
    pub browser_profile_id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "empty_json_object")]
    pub source_config: Value,
    pub status: SourceStatus,
    pub validation_error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSourceInput {
    pub adapter_key: String,
    pub system_profile_id: Option<i64>,
    pub browser_profile_id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub source_config: Value,
    pub status: SourceStatus,
    pub validation_error: Option<String>,
}

pub async fn create_browser_profile(
    pool: &SqlitePool,
    input: CreateBrowserProfileInput,
) -> Result<BrowserProfile, String> {
    validate_technical_key("key", &input.key)?;
    validate_browser_profile_parts(&input.name, input.definition_schema_version)?;
    let definition_json = json_to_string(&input.definition)?;
    let source_config_schema_json = json_to_string(&input.source_config_schema)?;

    let result = sqlx::query(
        "INSERT INTO browser_profiles (
           key, name, description, name_i18n_key, description_i18n_key,
           definition_path, definition_hash, definition_schema_version,
           definition_json, source_config_schema_json, status, validation_error
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )
    .bind(&input.key)
    .bind(&input.name)
    .bind(input.description.as_deref())
    .bind(input.name_i18n_key.as_deref())
    .bind(input.description_i18n_key.as_deref())
    .bind(input.definition_path.as_deref())
    .bind(input.definition_hash.as_deref())
    .bind(input.definition_schema_version)
    .bind(definition_json)
    .bind(source_config_schema_json)
    .bind(input.status.as_str())
    .bind(input.validation_error.as_deref())
    .execute(pool)
    .await
    .map_err(db_error)?;

    get_browser_profile(pool, result.last_insert_rowid()).await
}

pub async fn list_browser_profiles(pool: &SqlitePool) -> Result<Vec<BrowserProfile>, String> {
    let rows = sqlx::query(
        "SELECT id, key, name, description, name_i18n_key, description_i18n_key,
                definition_path, definition_hash, definition_schema_version,
                definition_json, source_config_schema_json, status, validation_error,
                created_at, updated_at
         FROM browser_profiles
         ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .map_err(db_error)?;

    rows.into_iter().map(browser_profile_from_row).collect()
}

pub async fn get_browser_profile(pool: &SqlitePool, id: i64) -> Result<BrowserProfile, String> {
    let row = sqlx::query(
        "SELECT id, key, name, description, name_i18n_key, description_i18n_key,
                definition_path, definition_hash, definition_schema_version,
                definition_json, source_config_schema_json, status, validation_error,
                created_at, updated_at
         FROM browser_profiles
         WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;

    row.map(browser_profile_from_row)
        .transpose()?
        .ok_or_else(|| format!("browser profile {id} not found"))
}

pub async fn update_browser_profile(
    pool: &SqlitePool,
    id: i64,
    input: UpdateBrowserProfileInput,
) -> Result<BrowserProfile, String> {
    get_browser_profile(pool, id).await?;
    validate_browser_profile_parts(&input.name, input.definition_schema_version)?;
    let definition_json = json_to_string(&input.definition)?;
    let source_config_schema_json = json_to_string(&input.source_config_schema)?;

    sqlx::query(
        "UPDATE browser_profiles
         SET name = ?1,
             description = ?2,
             name_i18n_key = ?3,
             description_i18n_key = ?4,
             definition_path = ?5,
             definition_hash = ?6,
             definition_schema_version = ?7,
             definition_json = ?8,
             source_config_schema_json = ?9,
             status = ?10,
             validation_error = ?11,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?12",
    )
    .bind(&input.name)
    .bind(input.description.as_deref())
    .bind(input.name_i18n_key.as_deref())
    .bind(input.description_i18n_key.as_deref())
    .bind(input.definition_path.as_deref())
    .bind(input.definition_hash.as_deref())
    .bind(input.definition_schema_version)
    .bind(definition_json)
    .bind(source_config_schema_json)
    .bind(input.status.as_str())
    .bind(input.validation_error.as_deref())
    .bind(id)
    .execute(pool)
    .await
    .map_err(db_error)?;

    get_browser_profile(pool, id).await
}

pub async fn delete_browser_profile(pool: &SqlitePool, id: i64) -> Result<(), String> {
    get_browser_profile(pool, id).await?;

    sqlx::query("DELETE FROM browser_profiles WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(db_error)?;

    Ok(())
}

pub async fn create_system_profile(
    pool: &SqlitePool,
    input: CreateSystemProfileInput,
) -> Result<SystemProfile, String> {
    validate_technical_key("key", &input.key)?;
    validate_system_profile_parts(
        &input.name,
        &input.adapter_key,
        input.definition_schema_version,
    )?;
    let definition_json = json_to_string(&input.definition)?;
    let source_config_schema_json = json_to_string(&input.source_config_schema)?;

    let result = sqlx::query(
        "INSERT INTO system_profiles (
           key, name, description, adapter_key, definition_schema_version,
           definition_json, source_config_schema_json, built_in, status, validation_error
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9)",
    )
    .bind(&input.key)
    .bind(&input.name)
    .bind(input.description.as_deref())
    .bind(&input.adapter_key)
    .bind(input.definition_schema_version)
    .bind(definition_json)
    .bind(source_config_schema_json)
    .bind(input.status.as_str())
    .bind(input.validation_error.as_deref())
    .execute(pool)
    .await
    .map_err(db_error)?;

    get_system_profile(pool, result.last_insert_rowid()).await
}

pub async fn list_system_profiles(pool: &SqlitePool) -> Result<Vec<SystemProfile>, String> {
    let rows = sqlx::query(
        "SELECT id, key, name, description, adapter_key, definition_schema_version,
                definition_json, source_config_schema_json, built_in, status, validation_error,
                created_at, updated_at
         FROM system_profiles
         ORDER BY built_in DESC, name",
    )
    .fetch_all(pool)
    .await
    .map_err(db_error)?;

    rows.into_iter().map(system_profile_from_row).collect()
}

pub async fn get_system_profile(pool: &SqlitePool, id: i64) -> Result<SystemProfile, String> {
    let row = sqlx::query(
        "SELECT id, key, name, description, adapter_key, definition_schema_version,
                definition_json, source_config_schema_json, built_in, status, validation_error,
                created_at, updated_at
         FROM system_profiles
         WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;

    row.map(system_profile_from_row)
        .transpose()?
        .ok_or_else(|| format!("system profile {id} not found"))
}

#[allow(dead_code)]
pub async fn get_system_profile_by_key(
    pool: &SqlitePool,
    key: &str,
) -> Result<SystemProfile, String> {
    let row = sqlx::query(
        "SELECT id, key, name, description, adapter_key, definition_schema_version,
                definition_json, source_config_schema_json, built_in, status, validation_error,
                created_at, updated_at
         FROM system_profiles
         WHERE key = ?1",
    )
    .bind(key)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;

    row.map(system_profile_from_row)
        .transpose()?
        .ok_or_else(|| format!("system profile {key} not found"))
}

pub async fn update_system_profile(
    pool: &SqlitePool,
    id: i64,
    input: UpdateSystemProfileInput,
) -> Result<SystemProfile, String> {
    let existing_profile = get_system_profile(pool, id).await?;
    if existing_profile.built_in {
        return Err("built-in system profiles cannot be edited".to_string());
    }
    validate_system_profile_parts(
        &input.name,
        &input.adapter_key,
        input.definition_schema_version,
    )?;
    let definition_json = json_to_string(&input.definition)?;
    let source_config_schema_json = json_to_string(&input.source_config_schema)?;

    sqlx::query(
        "UPDATE system_profiles
         SET name = ?1,
             description = ?2,
             adapter_key = ?3,
             definition_schema_version = ?4,
             definition_json = ?5,
             source_config_schema_json = ?6,
             status = ?7,
             validation_error = ?8,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?9",
    )
    .bind(&input.name)
    .bind(input.description.as_deref())
    .bind(&input.adapter_key)
    .bind(input.definition_schema_version)
    .bind(definition_json)
    .bind(source_config_schema_json)
    .bind(input.status.as_str())
    .bind(input.validation_error.as_deref())
    .bind(id)
    .execute(pool)
    .await
    .map_err(db_error)?;

    get_system_profile(pool, id).await
}

pub async fn delete_system_profile(pool: &SqlitePool, id: i64) -> Result<(), String> {
    let profile = get_system_profile(pool, id).await?;
    if profile.built_in {
        return Err("built-in system profiles cannot be deleted".to_string());
    }

    sqlx::query("DELETE FROM system_profiles WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(db_error)?;

    Ok(())
}

pub async fn create_source(pool: &SqlitePool, input: CreateSourceInput) -> Result<Source, String> {
    validate_technical_key("key", &input.key)?;
    validate_source_parts(
        pool,
        &input.adapter_key,
        input.system_profile_id,
        input.browser_profile_id,
        &input.name,
        &input.source_config,
    )
    .await?;
    let source_config_json = json_to_string(&input.source_config)?;

    let result = sqlx::query(
        "INSERT INTO sources (
           key, adapter_key, system_profile_id, browser_profile_id, name, description,
           source_config_json, status, validation_error
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )
    .bind(&input.key)
    .bind(&input.adapter_key)
    .bind(input.system_profile_id)
    .bind(input.browser_profile_id)
    .bind(&input.name)
    .bind(input.description.as_deref())
    .bind(source_config_json)
    .bind(input.status.as_str())
    .bind(input.validation_error.as_deref())
    .execute(pool)
    .await
    .map_err(db_error)?;

    get_source(pool, result.last_insert_rowid()).await
}

pub async fn list_sources(pool: &SqlitePool) -> Result<Vec<Source>, String> {
    let rows = sqlx::query(
        "SELECT id, key, adapter_key, system_profile_id, browser_profile_id, name, description,
                source_config_json, status, validation_error, created_at, updated_at
         FROM sources
         ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .map_err(db_error)?;

    rows.into_iter().map(source_from_row).collect()
}

pub async fn get_source(pool: &SqlitePool, id: i64) -> Result<Source, String> {
    let row = sqlx::query(
        "SELECT id, key, adapter_key, system_profile_id, browser_profile_id, name, description,
                source_config_json, status, validation_error, created_at, updated_at
         FROM sources
         WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;

    row.map(source_from_row)
        .transpose()?
        .ok_or_else(|| format!("source {id} not found"))
}

pub async fn update_source(
    pool: &SqlitePool,
    id: i64,
    input: UpdateSourceInput,
) -> Result<Source, String> {
    let existing_source = get_source(pool, id).await?;
    if existing_source.built_in && input.adapter_key != existing_source.adapter_key {
        return Err("built-in sources cannot change adapterKey".to_string());
    }
    validate_source_parts(
        pool,
        &input.adapter_key,
        input.system_profile_id,
        input.browser_profile_id,
        &input.name,
        &input.source_config,
    )
    .await?;
    let source_config_json = json_to_string(&input.source_config)?;

    sqlx::query(
        "UPDATE sources
         SET adapter_key = ?1,
             system_profile_id = ?2,
             browser_profile_id = ?3,
             name = ?4,
             description = ?5,
             source_config_json = ?6,
             status = ?7,
             validation_error = ?8,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?9",
    )
    .bind(&input.adapter_key)
    .bind(input.system_profile_id)
    .bind(input.browser_profile_id)
    .bind(&input.name)
    .bind(input.description.as_deref())
    .bind(source_config_json)
    .bind(input.status.as_str())
    .bind(input.validation_error.as_deref())
    .bind(id)
    .execute(pool)
    .await
    .map_err(db_error)?;

    get_source(pool, id).await
}

pub async fn delete_source(pool: &SqlitePool, id: i64) -> Result<(), String> {
    let source = get_source(pool, id).await?;
    if source.built_in {
        return Err("built-in sources cannot be deleted".to_string());
    }

    sqlx::query("DELETE FROM sources WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(db_error)?;

    Ok(())
}

fn empty_json_object() -> Value {
    Value::Object(Map::new())
}

fn validate_browser_profile_parts(
    name: &str,
    definition_schema_version: i64,
) -> Result<(), String> {
    validate_required_text("name", name)?;

    if definition_schema_version < 1 {
        return Err("definitionSchemaVersion must be greater than zero".to_string());
    }

    Ok(())
}

fn validate_system_profile_parts(
    name: &str,
    adapter_key: &str,
    definition_schema_version: i64,
) -> Result<(), String> {
    validate_required_text("name", name)?;
    validate_technical_key("adapterKey", adapter_key)?;

    let adapter = crate::adapter_registry::get_adapter(adapter_key)
        .ok_or_else(|| format!("adapterKey {adapter_key} is not registered"))?;
    if !adapter.requires_system_profile {
        return Err(format!(
            "adapterKey {adapter_key} cannot be used by system profiles"
        ));
    }

    if definition_schema_version < 1 {
        return Err("definitionSchemaVersion must be greater than zero".to_string());
    }

    Ok(())
}

async fn validate_source_parts(
    pool: &SqlitePool,
    adapter_key: &str,
    system_profile_id: Option<i64>,
    browser_profile_id: Option<i64>,
    name: &str,
    source_config: &Value,
) -> Result<(), String> {
    validate_required_text("name", name)?;
    validate_technical_key("adapterKey", adapter_key)?;

    let adapter = crate::adapter_registry::get_adapter(adapter_key)
        .ok_or_else(|| format!("adapterKey {adapter_key} is not registered"))?;

    validate_json_schema(&adapter.source_config_schema, source_config, "sourceConfig")?;

    if adapter.requires_system_profile {
        let system_profile_id = system_profile_id
            .ok_or_else(|| format!("adapterKey {adapter_key} requires a systemProfileId"))?;
        let system_profile = get_system_profile(pool, system_profile_id).await?;
        if system_profile.adapter_key != adapter_key {
            return Err(format!(
                "systemProfileId {system_profile_id} uses adapterKey {}, not {adapter_key}",
                system_profile.adapter_key
            ));
        }
        if system_profile.status != SourceStatus::Active {
            return Err(format!(
                "systemProfileId {system_profile_id} must reference an active system profile"
            ));
        }
        validate_json_schema(
            &system_profile.source_config_schema,
            source_config,
            "sourceConfig",
        )?;
    } else if system_profile_id.is_some() {
        return Err(format!(
            "adapterKey {adapter_key} does not allow a systemProfileId"
        ));
    }

    if adapter.requires_browser_profile {
        let browser_profile_id = browser_profile_id
            .ok_or_else(|| format!("adapterKey {adapter_key} requires a browserProfileId"))?;
        let browser_profile = get_browser_profile(pool, browser_profile_id).await?;
        validate_json_schema(
            &browser_profile.source_config_schema,
            source_config,
            "sourceConfig",
        )?;
    } else if browser_profile_id.is_some() {
        return Err(format!(
            "adapterKey {adapter_key} does not allow a browserProfileId"
        ));
    }

    Ok(())
}

fn validate_json_schema(schema: &Value, value: &Value, path: &str) -> Result<(), String> {
    let Some(schema_object) = schema.as_object() else {
        return Ok(());
    };

    if schema_object.is_empty() {
        return Ok(());
    }

    validate_json_schema_node(schema, value, path)
}

fn validate_json_schema_node(schema: &Value, value: &Value, path: &str) -> Result<(), String> {
    let schema_type = schema.get("type").and_then(Value::as_str);

    if let Some(schema_type) = schema_type {
        validate_json_value_type(schema_type, value, path)?;
    }

    if let Some(enum_values) = schema.get("enum").and_then(Value::as_array) {
        if !enum_values.iter().any(|enum_value| enum_value == value) {
            return Err(format!(
                "{path} must be one of {}",
                enum_values
                    .iter()
                    .map(format_json_for_message)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    if schema_type == Some("object")
        || schema.get("required").is_some()
        || schema.get("properties").is_some()
    {
        validate_json_object_schema(schema, value, path)?;
    }

    if schema_type == Some("string") {
        if matches!(
            schema.get("format").and_then(Value::as_str),
            Some("uri" | "url")
        ) {
            let value = value
                .as_str()
                .ok_or_else(|| format!("{path} must be a string"))?;
            validate_http_url(value, path)?;
        }
    }

    if matches!(schema_type, Some("number" | "integer")) {
        validate_number_bounds(schema, value, path)?;
    }

    Ok(())
}

fn validate_json_value_type(schema_type: &str, value: &Value, path: &str) -> Result<(), String> {
    let valid = match schema_type {
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        _ => true,
    };

    if valid {
        Ok(())
    } else {
        Err(format!("{path} must be {}", type_label(schema_type)))
    }
}

fn validate_json_object_schema(schema: &Value, value: &Value, path: &str) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{path} must be an object"))?;

    if let Some(required_fields) = schema.get("required").and_then(Value::as_array) {
        for required_field in required_fields.iter().filter_map(Value::as_str) {
            if !object.contains_key(required_field) {
                return Err(format!("{path}.{required_field} is required"));
            }
        }
    }

    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        for (property_key, property_schema) in properties {
            if let Some(property_value) = object.get(property_key) {
                validate_json_schema_node(
                    property_schema,
                    property_value,
                    &format!("{path}.{property_key}"),
                )?;
            }
        }
    }

    Ok(())
}

fn validate_http_url(value: &str, path: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(value)
        .map_err(|_| format!("{path} must be an absolute http or https URL"))?;

    if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() {
        Ok(())
    } else {
        Err(format!("{path} must be an absolute http or https URL"))
    }
}

fn validate_number_bounds(schema: &Value, value: &Value, path: &str) -> Result<(), String> {
    let number = value
        .as_f64()
        .ok_or_else(|| format!("{path} must be a number"))?;

    if let Some(minimum) = schema.get("minimum").and_then(Value::as_f64) {
        if number < minimum {
            return Err(format!(
                "{path} must be greater than or equal to {}",
                format_number_for_message(minimum)
            ));
        }
    }

    if let Some(maximum) = schema.get("maximum").and_then(Value::as_f64) {
        if number > maximum {
            return Err(format!(
                "{path} must be less than or equal to {}",
                format_number_for_message(maximum)
            ));
        }
    }

    Ok(())
}

fn type_label(schema_type: &str) -> &'static str {
    match schema_type {
        "string" => "a string",
        "boolean" => "a boolean",
        "number" => "a number",
        "integer" => "an integer",
        "object" => "an object",
        "array" => "an array",
        _ => "the expected type",
    }
}

fn format_json_for_message(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}

fn format_number_for_message(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn validate_required_text(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} must not be empty"));
    }

    Ok(())
}

fn validate_technical_key(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{field} must not be empty"));
    }

    if !value.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
    }) {
        return Err(format!(
            "{field} must use lowercase snake case with only a-z, 0-9, and _"
        ));
    }

    Ok(())
}

fn json_to_string(value: &Value) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| error.to_string())
}

fn browser_profile_from_row(row: SqliteRow) -> Result<BrowserProfile, String> {
    let status = status_from_row(&row)?;

    Ok(BrowserProfile {
        id: row.try_get("id").map_err(db_error)?,
        key: row.try_get("key").map_err(db_error)?,
        name: row.try_get("name").map_err(db_error)?,
        description: row.try_get("description").map_err(db_error)?,
        name_i18n_key: row.try_get("name_i18n_key").map_err(db_error)?,
        description_i18n_key: row.try_get("description_i18n_key").map_err(db_error)?,
        definition_path: row.try_get("definition_path").map_err(db_error)?,
        definition_hash: row.try_get("definition_hash").map_err(db_error)?,
        definition_schema_version: row.try_get("definition_schema_version").map_err(db_error)?,
        definition: json_from_row(&row, "definition_json")?,
        source_config_schema: json_from_row(&row, "source_config_schema_json")?,
        status,
        validation_error: row.try_get("validation_error").map_err(db_error)?,
        created_at: row.try_get("created_at").map_err(db_error)?,
        updated_at: row.try_get("updated_at").map_err(db_error)?,
    })
}

fn system_profile_from_row(row: SqliteRow) -> Result<SystemProfile, String> {
    let status = status_from_row(&row)?;
    let built_in: i64 = row.try_get("built_in").map_err(db_error)?;

    Ok(SystemProfile {
        id: row.try_get("id").map_err(db_error)?,
        key: row.try_get("key").map_err(db_error)?,
        name: row.try_get("name").map_err(db_error)?,
        description: row.try_get("description").map_err(db_error)?,
        adapter_key: row.try_get("adapter_key").map_err(db_error)?,
        definition_schema_version: row.try_get("definition_schema_version").map_err(db_error)?,
        definition: json_from_row(&row, "definition_json")?,
        source_config_schema: json_from_row(&row, "source_config_schema_json")?,
        built_in: built_in == 1,
        status,
        validation_error: row.try_get("validation_error").map_err(db_error)?,
        created_at: row.try_get("created_at").map_err(db_error)?,
        updated_at: row.try_get("updated_at").map_err(db_error)?,
    })
}

fn source_from_row(row: SqliteRow) -> Result<Source, String> {
    let status = status_from_row(&row)?;
    let key: String = row.try_get("key").map_err(db_error)?;

    Ok(Source {
        id: row.try_get("id").map_err(db_error)?,
        built_in: is_builtin_source_key(&key),
        key,
        adapter_key: row.try_get("adapter_key").map_err(db_error)?,
        system_profile_id: row.try_get("system_profile_id").map_err(db_error)?,
        browser_profile_id: row.try_get("browser_profile_id").map_err(db_error)?,
        name: row.try_get("name").map_err(db_error)?,
        description: row.try_get("description").map_err(db_error)?,
        source_config: json_from_row(&row, "source_config_json")?,
        status,
        validation_error: row.try_get("validation_error").map_err(db_error)?,
        created_at: row.try_get("created_at").map_err(db_error)?,
        updated_at: row.try_get("updated_at").map_err(db_error)?,
    })
}

fn is_builtin_source_key(key: &str) -> bool {
    matches!(key, "stepstone_de" | "indeed_de")
}

fn status_from_row(row: &SqliteRow) -> Result<SourceStatus, String> {
    let status: String = row.try_get("status").map_err(db_error)?;
    SourceStatus::try_from(status.as_str())
}

fn json_from_row(row: &SqliteRow, column: &str) -> Result<Value, String> {
    let json: String = row.try_get(column).map_err(db_error)?;
    serde_json::from_str(&json).map_err(|error| error.to_string())
}

fn db_error(error: sqlx::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    #[test]
    fn migration_starts_with_empty_source_inventory() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;

            assert!(list_browser_profiles(&pool).await.unwrap().is_empty());
            assert!(list_system_profiles(&pool).await.unwrap().is_empty());
            assert!(list_sources(&pool).await.unwrap().is_empty());
        });
    }

    #[test]
    fn system_profile_crud_round_trips_declarative_definition() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;

            let created = create_system_profile(
                &pool,
                CreateSystemProfileInput {
                    key: "muz_global_jobboard".to_string(),
                    name: "Milch & Zucker Global Jobboard".to_string(),
                    description: Some("HTTP-basiertes Systemprofil".to_string()),
                    adapter_key: "declarative_http_jobboard".to_string(),
                    definition_schema_version: 1,
                    definition: json!({
                        "detect": {
                            "required": [
                                { "htmlContains": "global-jobboard-client" },
                                { "fetchText": { "url": "/script/gjb_scripts.js", "regex": "gjbAddress" } }
                            ]
                        }
                    }),
                    source_config_schema: json!({
                        "type": "object",
                        "required": ["startUrl", "apiBaseUrl"],
                        "properties": {
                            "startUrl": { "type": "string", "format": "uri" },
                            "apiBaseUrl": { "type": "string", "format": "uri" }
                        }
                    }),
                    status: SourceStatus::Active,
                    validation_error: None,
                },
            )
            .await
            .unwrap();

            assert_eq!(created.key, "muz_global_jobboard");
            assert_eq!(created.adapter_key, "declarative_http_jobboard");
            assert!(!created.built_in);

            let fetched = get_system_profile_by_key(&pool, "muz_global_jobboard")
                .await
                .unwrap();
            assert_eq!(fetched.id, created.id);

            let updated = update_system_profile(
                &pool,
                created.id,
                UpdateSystemProfileInput {
                    name: "MUZ Global Jobboard".to_string(),
                    description: None,
                    adapter_key: "declarative_http_jobboard".to_string(),
                    definition_schema_version: 2,
                    definition: json!({ "detect": { "required": [] } }),
                    source_config_schema: json!({ "type": "object" }),
                    status: SourceStatus::Draft,
                    validation_error: Some("in Arbeit".to_string()),
                },
            )
            .await
            .unwrap();
            assert_eq!(updated.name, "MUZ Global Jobboard");
            assert_eq!(updated.status, SourceStatus::Draft);

            delete_system_profile(&pool, created.id).await.unwrap();
            assert!(get_system_profile(&pool, created.id).await.is_err());
        });
    }

    #[test]
    fn declarative_sources_require_matching_active_system_profile() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;

            let profile = create_system_profile(
                &pool,
                CreateSystemProfileInput {
                    key: "greenhouse".to_string(),
                    name: "Greenhouse".to_string(),
                    description: None,
                    adapter_key: "declarative_http_jobboard".to_string(),
                    definition_schema_version: 1,
                    definition: json!({}),
                    source_config_schema: json!({
                        "type": "object",
                        "required": ["startUrl", "boardToken"],
                        "properties": {
                            "startUrl": { "type": "string", "format": "uri" },
                            "boardToken": { "type": "string" }
                        }
                    }),
                    status: SourceStatus::Active,
                    validation_error: None,
                },
            )
            .await
            .unwrap();

            let source = create_source(
                &pool,
                CreateSourceInput {
                    key: "acme_greenhouse".to_string(),
                    adapter_key: "declarative_http_jobboard".to_string(),
                    system_profile_id: Some(profile.id),
                    browser_profile_id: None,
                    name: "Acme Karriere".to_string(),
                    description: None,
                    source_config: json!({
                        "startUrl": "https://job-boards.greenhouse.io/acme",
                        "boardToken": "acme"
                    }),
                    status: SourceStatus::Active,
                    validation_error: None,
                },
            )
            .await
            .unwrap();

            assert_eq!(source.system_profile_id, Some(profile.id));
            assert_eq!(source.adapter_key, "declarative_http_jobboard");

            let missing_profile = create_source(
                &pool,
                CreateSourceInput {
                    key: "missing_profile".to_string(),
                    adapter_key: "declarative_http_jobboard".to_string(),
                    system_profile_id: None,
                    browser_profile_id: None,
                    name: "Missing".to_string(),
                    description: None,
                    source_config: json!({ "startUrl": "https://example.com/jobs" }),
                    status: SourceStatus::Draft,
                    validation_error: None,
                },
            )
            .await
            .unwrap_err();
            assert!(missing_profile.contains("requires a systemProfileId"));

            let schema_error = create_source(
                &pool,
                CreateSourceInput {
                    key: "invalid_profile_config".to_string(),
                    adapter_key: "declarative_http_jobboard".to_string(),
                    system_profile_id: Some(profile.id),
                    browser_profile_id: None,
                    name: "Invalid".to_string(),
                    description: None,
                    source_config: json!({ "startUrl": "https://example.com/jobs" }),
                    status: SourceStatus::Draft,
                    validation_error: None,
                },
            )
            .await
            .unwrap_err();
            assert!(schema_error.contains("sourceConfig.boardToken is required"));
        });
    }

    #[test]
    fn job_board_adapters_stay_separate_from_system_profiles() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let browser_profile = create_browser_profile(
                &pool,
                CreateBrowserProfileInput {
                    key: "manual_release".to_string(),
                    name: "Manuelle Freigabe".to_string(),
                    description: None,
                    name_i18n_key: None,
                    description_i18n_key: None,
                    definition_path: None,
                    definition_hash: None,
                    definition_schema_version: 1,
                    definition: json!({}),
                    source_config_schema: json!({ "type": "object" }),
                    status: SourceStatus::Active,
                    validation_error: None,
                },
            )
            .await
            .unwrap();

            let source = create_source(
                &pool,
                CreateSourceInput {
                    key: "stepstone_de".to_string(),
                    adapter_key: "stepstone_search".to_string(),
                    system_profile_id: None,
                    browser_profile_id: Some(browser_profile.id),
                    name: "StepStone Deutschland".to_string(),
                    description: None,
                    source_config: json!({}),
                    status: SourceStatus::Active,
                    validation_error: None,
                },
            )
            .await
            .unwrap();

            assert_eq!(source.system_profile_id, None);
            assert_eq!(source.browser_profile_id, Some(browser_profile.id));

            let disallowed_profile = create_source(
                &pool,
                CreateSourceInput {
                    key: "stepstone_with_system".to_string(),
                    adapter_key: "stepstone_search".to_string(),
                    system_profile_id: Some(1),
                    browser_profile_id: Some(browser_profile.id),
                    name: "Invalid".to_string(),
                    description: None,
                    source_config: json!({}),
                    status: SourceStatus::Draft,
                    validation_error: None,
                },
            )
            .await
            .unwrap_err();
            assert!(disallowed_profile.contains("does not allow a systemProfileId"));
        });
    }

    async fn migrated_pool() -> SqlitePool {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();

        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        pool
    }
}
