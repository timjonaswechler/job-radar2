use crate::{
    declarative_template::is_supported_template_filter, simple_json_path::resolve_simple_json_path,
};

use regex::Regex;
use reqwest::Url;
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
pub struct SystemProfileJsonDocument {
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub adapter_key: String,
    pub definition_schema_version: i64,
    pub definition: Value,
    pub source_config_schema: Value,
    pub status: SourceStatus,
    pub validation_error: Option<String>,
}

impl From<SystemProfile> for SystemProfileJsonDocument {
    fn from(profile: SystemProfile) -> Self {
        Self {
            key: profile.key,
            name: profile.name,
            description: profile.description,
            adapter_key: profile.adapter_key,
            definition_schema_version: profile.definition_schema_version,
            definition: profile.definition,
            source_config_schema: profile.source_config_schema,
            status: profile.status,
            validation_error: profile.validation_error,
        }
    }
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
    validate_system_profile_definition_and_schema(
        &input.definition,
        &input.source_config_schema,
        input.status,
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
    validate_system_profile_definition_and_schema(
        &input.definition,
        &input.source_config_schema,
        input.status,
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

pub async fn export_system_profile_json(pool: &SqlitePool, id: i64) -> Result<String, String> {
    let profile = get_system_profile(pool, id).await?;
    let document = SystemProfileJsonDocument::from(profile);
    serde_json::to_string_pretty(&document).map_err(|error| error.to_string())
}

pub async fn import_system_profile_json(
    pool: &SqlitePool,
    contents: &str,
) -> Result<SystemProfile, String> {
    let document = serde_json::from_str::<SystemProfileJsonDocument>(contents)
        .map_err(|error| format!("system profile JSON is invalid: {error}"))?;
    validate_system_profile_document(&document)?;

    let existing = sqlx::query("SELECT id, built_in FROM system_profiles WHERE key = ?1")
        .bind(&document.key)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?;

    let create_input = CreateSystemProfileInput {
        key: document.key,
        name: document.name,
        description: document.description,
        adapter_key: document.adapter_key,
        definition_schema_version: document.definition_schema_version,
        definition: document.definition,
        source_config_schema: document.source_config_schema,
        status: document.status,
        validation_error: document.validation_error,
    };

    match existing {
        Some(row) => {
            let id: i64 = row.try_get("id").map_err(db_error)?;
            let built_in: i64 = row.try_get("built_in").map_err(db_error)?;
            if built_in == 1 {
                return Err(format!(
                    "built-in system profile {} cannot be overwritten by import",
                    create_input.key
                ));
            }
            update_system_profile(
                pool,
                id,
                UpdateSystemProfileInput {
                    name: create_input.name,
                    description: create_input.description,
                    adapter_key: create_input.adapter_key,
                    definition_schema_version: create_input.definition_schema_version,
                    definition: create_input.definition,
                    source_config_schema: create_input.source_config_schema,
                    status: create_input.status,
                    validation_error: create_input.validation_error,
                },
            )
            .await
        }
        None => create_system_profile(pool, create_input).await,
    }
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

fn validate_system_profile_document(document: &SystemProfileJsonDocument) -> Result<(), String> {
    validate_technical_key("key", &document.key)?;
    validate_system_profile_parts(
        &document.name,
        &document.adapter_key,
        document.definition_schema_version,
    )?;

    validate_system_profile_definition_and_schema(
        &document.definition,
        &document.source_config_schema,
        document.status,
    )
}

pub(crate) fn validate_system_profile_definition_and_schema(
    definition: &Value,
    source_config_schema: &Value,
    status: SourceStatus,
) -> Result<(), String> {
    if !definition.is_object() {
        return Err("definition must be a JSON object".to_string());
    }

    validate_json_schema_document(source_config_schema, "sourceConfigSchema")?;

    let required_checks = definition.pointer("/detect/required");
    if status == SourceStatus::Active {
        let required_checks = required_checks
            .and_then(Value::as_array)
            .ok_or_else(|| "active profiles require definition.detect.required".to_string())?;
        if required_checks.is_empty() {
            return Err("active profiles require at least one detection check".to_string());
        }
    }

    if let Some(required_checks) = required_checks {
        let required_checks = required_checks
            .as_array()
            .ok_or_else(|| "definition.detect.required must be an array".to_string())?;
        for (index, check) in required_checks.iter().enumerate() {
            validate_detection_check(check, &format!("definition.detect.required[{index}]"))?;
        }
    }

    validate_identity_definition(definition)?;
    validate_inventory_definition(definition)?;

    Ok(())
}

fn validate_inventory_definition(definition: &Value) -> Result<(), String> {
    let Some(inventory) = definition.get("inventory") else {
        return Ok(());
    };
    let inventory = inventory
        .as_object()
        .ok_or_else(|| "definition.inventory must be a JSON object".to_string())?;
    validate_allowed_keys(
        inventory,
        &["fetch", "parse", "items", "fields"],
        "definition.inventory",
    )?;

    let fetch = require_object(inventory, "fetch", "definition.inventory.fetch")?;
    validate_allowed_keys(fetch, &["url"], "definition.inventory.fetch")?;
    let fetch_url = require_non_empty_string(fetch, "url", "definition.inventory.fetch.url")?;
    validate_inventory_template_string(
        fetch_url,
        "definition.inventory.fetch.url",
        InventoryTemplateScope::Fetch,
    )?;

    let parse = require_object(inventory, "parse", "definition.inventory.parse")?;
    validate_allowed_keys(parse, &["as"], "definition.inventory.parse")?;
    let parse_as = require_non_empty_string(parse, "as", "definition.inventory.parse.as")?;
    if !matches!(parse_as, "xml" | "json") {
        return Err("definition.inventory.parse.as must be xml or json".to_string());
    }

    let items = require_object(inventory, "items", "definition.inventory.items")?;
    validate_allowed_keys(
        items,
        &["select", "where", "captures"],
        "definition.inventory.items",
    )?;
    validate_inventory_item_select(items, parse_as)?;
    validate_inventory_regex_entries(items.get("where"), "definition.inventory.items.where")?;
    validate_inventory_regex_entries(items.get("captures"), "definition.inventory.items.captures")?;

    let fields = require_object(inventory, "fields", "definition.inventory.fields")?;
    validate_inventory_required_field(fields, "title")?;
    validate_inventory_required_field(fields, "url")?;
    validate_inventory_required_field(fields, "company")?;
    let locations = fields
        .get("locations")
        .ok_or_else(|| "definition.inventory.fields.locations must be an array".to_string())?;
    let locations = locations
        .as_array()
        .ok_or_else(|| "definition.inventory.fields.locations must be an array".to_string())?;
    for (index, location) in locations.iter().enumerate() {
        validate_inventory_field_expression(
            location,
            &format!("definition.inventory.fields.locations[{index}]"),
        )?;
    }

    Ok(())
}

fn validate_inventory_item_select(
    items: &Map<String, Value>,
    parse_as: &str,
) -> Result<(), String> {
    let select = require_object(items, "select", "definition.inventory.items.select")?;
    match parse_as {
        "xml" => {
            validate_allowed_keys(select, &["xmlText"], "definition.inventory.items.select")?;
            require_non_empty_string(
                select,
                "xmlText",
                "definition.inventory.items.select.xmlText",
            )?;
        }
        "json" => {
            validate_allowed_keys(select, &["jsonPath"], "definition.inventory.items.select")?;
            let json_path = require_non_empty_string(
                select,
                "jsonPath",
                "definition.inventory.items.select.jsonPath",
            )?;
            validate_simple_json_path(json_path, "definition.inventory.items.select.jsonPath")?;
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn validate_inventory_regex_entries(value: Option<&Value>, path: &str) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    let entries = value
        .as_array()
        .ok_or_else(|| format!("{path} must be an array"))?;
    for (index, entry) in entries.iter().enumerate() {
        let entry_path = format!("{path}[{index}]");
        let object = entry
            .as_object()
            .ok_or_else(|| format!("{entry_path} must be a JSON object"))?;
        validate_allowed_keys(object, &["regex"], &entry_path)?;
        validate_regex_value(object, "regex", &format!("{entry_path}.regex"))?;
    }
    Ok(())
}

fn validate_inventory_required_field(
    fields: &Map<String, Value>,
    field_name: &str,
) -> Result<(), String> {
    let path = format!("definition.inventory.fields.{field_name}");
    let field = fields
        .get(field_name)
        .ok_or_else(|| format!("{path} is required"))?;
    validate_inventory_field_expression(field, &path)
}

fn validate_inventory_field_expression(value: &Value, path: &str) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{path} must be a JSON object"))?;
    validate_allowed_keys(object, &["template", "jsonPath"], path)?;

    let has_template = object.contains_key("template");
    let has_json_path = object.contains_key("jsonPath");
    match (has_template, has_json_path) {
        (true, false) => {
            let template =
                require_non_empty_string(object, "template", &format!("{path}.template"))?;
            validate_inventory_template_string(
                template,
                &format!("{path}.template"),
                InventoryTemplateScope::Field,
            )
        }
        (false, true) => {
            let json_path =
                require_non_empty_string(object, "jsonPath", &format!("{path}.jsonPath"))?;
            validate_simple_json_path(json_path, &format!("{path}.jsonPath"))
        }
        _ => Err(format!(
            "{path} must contain exactly one field expression: template or jsonPath"
        )),
    }
}

fn validate_simple_json_path(json_path: &str, path: &str) -> Result<(), String> {
    resolve_simple_json_path(&Value::Object(Map::new()), json_path)
        .map(|_| ())
        .map_err(|error| format!("{path} {error}"))
}

#[derive(Clone, Copy)]
enum InventoryTemplateScope {
    Fetch,
    Field,
}

fn validate_inventory_template_string(
    template: &str,
    path: &str,
    scope: InventoryTemplateScope,
) -> Result<(), String> {
    let placeholder_regex = Regex::new(r"\{\{\s*([^{}]+?)\s*\}\}").unwrap();
    for captures in placeholder_regex.captures_iter(template) {
        validate_inventory_template_expression(&captures[1], path, scope)?;
    }
    Ok(())
}

fn validate_inventory_template_expression(
    expression: &str,
    path: &str,
    scope: InventoryTemplateScope,
) -> Result<(), String> {
    let mut parts = expression.split('|').map(str::trim);
    let variable = parts
        .next()
        .filter(|variable| !variable.is_empty())
        .ok_or_else(|| format!("{path} contains an empty template expression"))?;

    validate_inventory_template_variable(variable, path, scope)?;

    for filter in parts {
        if !is_supported_template_filter(filter) {
            return Err(format!(
                "{path} contains unsupported template filter `{filter}`"
            ));
        }
    }

    Ok(())
}

fn validate_inventory_template_variable(
    variable: &str,
    path: &str,
    scope: InventoryTemplateScope,
) -> Result<(), String> {
    if matches!(variable, "sourceName" | "sourceKey") {
        return Ok(());
    }

    if variable == "itemText" {
        return match scope {
            InventoryTemplateScope::Field => Ok(()),
            InventoryTemplateScope::Fetch => Err(format!(
                "{path} contains unsupported template variable `itemText`"
            )),
        };
    }

    if let Some(source_config_key) = variable.strip_prefix("sourceConfig:") {
        if source_config_key.is_empty() {
            return Err(format!(
                "{path} contains invalid sourceConfig template variable `{variable}`"
            ));
        }
        return Ok(());
    }

    if let Some(capture_key) = variable.strip_prefix("capture:") {
        if matches!(scope, InventoryTemplateScope::Fetch) {
            return Err(format!(
                "{path} contains unsupported template variable `{variable}`"
            ));
        }
        if capture_key.is_empty()
            || !capture_key
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            return Err(format!(
                "{path} contains invalid capture template variable `{variable}`"
            ));
        }
        return Ok(());
    }

    Err(format!(
        "{path} contains unsupported template variable `{variable}`"
    ))
}

fn validate_identity_definition(definition: &Value) -> Result<(), String> {
    let Some(identity) = definition.get("identity") else {
        return Ok(());
    };
    let identity = identity
        .as_object()
        .ok_or_else(|| "definition.identity must be a JSON object".to_string())?;
    validate_allowed_keys(
        identity,
        &[
            "extract",
            "keyCandidates",
            "nameCandidates",
            "optionalSourceConfig",
        ],
        "definition.identity",
    )?;

    if let Some(extracts) = identity.get("extract") {
        let extracts = extracts
            .as_array()
            .ok_or_else(|| "definition.identity.extract must be an array".to_string())?;
        for (index, check) in extracts.iter().enumerate() {
            validate_detection_check(check, &format!("definition.identity.extract[{index}]"))?;
        }
    }

    validate_template_candidates(identity, "keyCandidates")?;
    validate_template_candidates(identity, "nameCandidates")?;

    if let Some(optional_source_config) = identity.get("optionalSourceConfig") {
        let optional_source_config = optional_source_config.as_object().ok_or_else(|| {
            "definition.identity.optionalSourceConfig must be a JSON object".to_string()
        })?;
        for (key, value) in optional_source_config {
            validate_template_value(
                value,
                &format!("definition.identity.optionalSourceConfig.{key}"),
            )?;
        }
    }

    Ok(())
}

fn validate_template_candidates(identity: &Map<String, Value>, key: &str) -> Result<(), String> {
    let Some(candidates) = identity.get(key) else {
        return Ok(());
    };
    let candidates = candidates
        .as_array()
        .ok_or_else(|| format!("definition.identity.{key} must be an array"))?;
    for (index, candidate) in candidates.iter().enumerate() {
        let path = format!("definition.identity.{key}[{index}]");
        let candidate = candidate
            .as_str()
            .ok_or_else(|| format!("{path} must be a string"))?;
        if candidate.trim().is_empty() {
            return Err(format!("{path} must be a non-empty string"));
        }
        validate_template_string(candidate, &path)?;
    }
    Ok(())
}

fn validate_template_value(value: &Value, path: &str) -> Result<(), String> {
    match value {
        Value::String(template) => validate_template_string(template, path),
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_template_value(value, &format!("{path}[{index}]"))?;
            }
            Ok(())
        }
        Value::Object(object) => {
            for (key, value) in object {
                validate_template_value(value, &format!("{path}.{key}"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_template_string(template: &str, path: &str) -> Result<(), String> {
    let placeholder_regex = Regex::new(r"\{\{\s*([^{}]+?)\s*\}\}").unwrap();
    for captures in placeholder_regex.captures_iter(template) {
        validate_template_expression(&captures[1], path)?;
    }
    Ok(())
}

fn validate_template_expression(expression: &str, path: &str) -> Result<(), String> {
    let mut parts = expression.split('|').map(str::trim);
    let variable = parts
        .next()
        .filter(|variable| !variable.is_empty())
        .ok_or_else(|| format!("{path} contains an empty template expression"))?;

    if variable == "inputUrl" || variable == "origin" {
        // supported built-in variable
    } else if let Some(capture_key) = variable.strip_prefix("capture:") {
        if capture_key.is_empty()
            || !capture_key
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            return Err(format!(
                "{path} contains invalid capture template variable `{variable}`"
            ));
        }
    } else {
        return Err(format!(
            "{path} contains unsupported template variable `{variable}`"
        ));
    }

    for filter in parts {
        if !is_supported_template_filter(filter) {
            return Err(format!(
                "{path} contains unsupported template filter `{filter}`"
            ));
        }
    }

    Ok(())
}

fn validate_detection_check(check: &Value, path: &str) -> Result<(), String> {
    let object = check
        .as_object()
        .ok_or_else(|| format!("{path} must be a JSON object"))?;
    let check_types = [
        "htmlContains",
        "htmlRegex",
        "fetchText",
        "fetchJson",
        "fetchScript",
    ];
    let present_check_types = check_types
        .iter()
        .filter(|check_type| object.contains_key(**check_type))
        .copied()
        .collect::<Vec<_>>();

    let check_type = match present_check_types.as_slice() {
        [] => return Err(format!("{path} uses unsupported detection check")),
        [check_type] => *check_type,
        _ => {
            return Err(format!(
                "{path} must declare exactly one detection check type"
            ))
        }
    };

    for key in object.keys() {
        let supported = match check_type {
            "htmlRegex" => matches!(key.as_str(), "htmlRegex" | "captureAs"),
            "htmlContains" => key == "htmlContains",
            "fetchText" => key == "fetchText",
            "fetchJson" => key == "fetchJson",
            "fetchScript" => key == "fetchScript",
            _ => false,
        };
        if !supported {
            return Err(format!("{path}.{key} is not supported for {check_type}"));
        }
    }

    match check_type {
        "htmlContains" => {
            require_non_empty_string(object, "htmlContains", &format!("{path}.htmlContains"))?;
        }
        "htmlRegex" => {
            validate_regex_value(object, "htmlRegex", &format!("{path}.htmlRegex"))?;
            validate_optional_string(object, "captureAs", &format!("{path}.captureAs"))?;
        }
        "fetchText" => {
            let fetch_text = require_object(object, "fetchText", &format!("{path}.fetchText"))?;
            validate_allowed_keys(
                fetch_text,
                &["url", "contains", "regex", "captureAs"],
                &format!("{path}.fetchText"),
            )?;
            validate_fetch_url(fetch_text, &format!("{path}.fetchText.url"))?;
            validate_optional_string(
                fetch_text,
                "contains",
                &format!("{path}.fetchText.contains"),
            )?;
            if fetch_text.contains_key("regex") {
                validate_regex_value(fetch_text, "regex", &format!("{path}.fetchText.regex"))?;
            }
            validate_optional_string(
                fetch_text,
                "captureAs",
                &format!("{path}.fetchText.captureAs"),
            )?;
        }
        "fetchJson" => {
            let fetch_json = require_object(object, "fetchJson", &format!("{path}.fetchJson"))?;
            validate_allowed_keys(
                fetch_json,
                &["url", "pathExists"],
                &format!("{path}.fetchJson"),
            )?;
            validate_fetch_url(fetch_json, &format!("{path}.fetchJson.url"))?;
            validate_optional_string(
                fetch_json,
                "pathExists",
                &format!("{path}.fetchJson.pathExists"),
            )?;
        }
        "fetchScript" => {
            let fetch_script =
                require_object(object, "fetchScript", &format!("{path}.fetchScript"))?;
            validate_allowed_keys(
                fetch_script,
                &["srcContains", "srcRegex", "contains", "regex", "captureAs"],
                &format!("{path}.fetchScript"),
            )?;
            validate_optional_string(
                fetch_script,
                "srcContains",
                &format!("{path}.fetchScript.srcContains"),
            )?;
            if fetch_script.contains_key("srcRegex") {
                validate_regex_value(
                    fetch_script,
                    "srcRegex",
                    &format!("{path}.fetchScript.srcRegex"),
                )?;
            }
            validate_optional_string(
                fetch_script,
                "contains",
                &format!("{path}.fetchScript.contains"),
            )?;
            if fetch_script.contains_key("regex") {
                validate_regex_value(fetch_script, "regex", &format!("{path}.fetchScript.regex"))?;
            }
            validate_optional_string(
                fetch_script,
                "captureAs",
                &format!("{path}.fetchScript.captureAs"),
            )?;
        }
        _ => unreachable!(),
    }

    Ok(())
}

fn validate_allowed_keys(
    object: &Map<String, Value>,
    allowed_keys: &[&str],
    path: &str,
) -> Result<(), String> {
    for key in object.keys() {
        if !allowed_keys.contains(&key.as_str()) {
            return Err(format!("{path}.{key} is not supported"));
        }
    }
    Ok(())
}

fn require_object<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a Map<String, Value>, String> {
    object
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{path} must be a JSON object"))
}

fn require_non_empty_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a str, String> {
    let Some(value) = object.get(key) else {
        return Err(format!("{path} must be a non-empty string"));
    };
    let value = value
        .as_str()
        .ok_or_else(|| format!("{path} must be a string"))?;
    if value.trim().is_empty() {
        return Err(format!("{path} must be a non-empty string"));
    }
    Ok(value)
}

fn validate_optional_string(
    object: &Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<(), String> {
    if object.get(key).is_some_and(|value| !value.is_string()) {
        return Err(format!("{path} must be a string"));
    }
    Ok(())
}

fn validate_regex_value(object: &Map<String, Value>, key: &str, path: &str) -> Result<(), String> {
    let pattern = require_non_empty_string(object, key, path)?;
    Regex::new(pattern).map_err(|error| format!("{path} is invalid: {error}"))?;
    Ok(())
}

fn validate_fetch_url(object: &Map<String, Value>, path: &str) -> Result<(), String> {
    let url = require_non_empty_string(object, "url", path)?;
    if url.trim() != url {
        return Err(format!("{path} is invalid: leading or trailing whitespace"));
    }
    let base = Url::parse("https://example.com/").map_err(|error| error.to_string())?;
    base.join(url)
        .map_err(|error| format!("{path} is invalid: {error}"))?;
    Ok(())
}

fn validate_json_schema_document(schema: &Value, path: &str) -> Result<(), String> {
    let object = schema
        .as_object()
        .ok_or_else(|| format!("{path} must be a JSON object"))?;

    if let Some(schema_type) = object.get("type") {
        let schema_type = schema_type
            .as_str()
            .ok_or_else(|| format!("{path}.type must be a string"))?;
        if !matches!(
            schema_type,
            "string" | "boolean" | "number" | "integer" | "object" | "array"
        ) {
            return Err(format!(
                "{path}.type must be string, boolean, number, integer, object, or array"
            ));
        }
    }

    if let Some(required_fields) = object.get("required") {
        let required_fields = required_fields
            .as_array()
            .ok_or_else(|| format!("{path}.required must be an array"))?;
        if required_fields.iter().any(|field| !field.is_string()) {
            return Err(format!("{path}.required entries must be strings"));
        }
    }

    if let Some(properties) = object.get("properties") {
        let properties = properties
            .as_object()
            .ok_or_else(|| format!("{path}.properties must be a JSON object"))?;
        for (property_key, property_schema) in properties {
            validate_json_schema_document(
                property_schema,
                &format!("{path}.properties.{property_key}"),
            )?;
        }
    }

    if let Some(enum_values) = object.get("enum") {
        enum_values
            .as_array()
            .ok_or_else(|| format!("{path}.enum must be an array"))?;
    }

    for number_keyword in ["minimum", "maximum"] {
        if object
            .get(number_keyword)
            .is_some_and(|value| !value.is_number())
        {
            return Err(format!("{path}.{number_keyword} must be a number"));
        }
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
                    adapter_key: "declarative_endpoint_inventory".to_string(),
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
            assert_eq!(created.adapter_key, "declarative_endpoint_inventory");
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
                    adapter_key: "declarative_endpoint_inventory".to_string(),
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
    fn system_profile_json_export_imports_as_new_profile() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let original = create_system_profile(
                &pool,
                CreateSystemProfileInput {
                    key: "portable_board".to_string(),
                    name: "Portable Board".to_string(),
                    description: Some("Portable profile".to_string()),
                    adapter_key: "declarative_endpoint_inventory".to_string(),
                    definition_schema_version: 1,
                    definition: json!({
                        "detect": { "required": [{ "htmlContains": "portable-board" }] },
                        "sourceConfig": { "startUrl": "{{inputUrl}}" }
                    }),
                    source_config_schema: json!({
                        "type": "object",
                        "required": ["startUrl"],
                        "properties": {
                            "startUrl": { "type": "string", "format": "uri" }
                        }
                    }),
                    status: SourceStatus::Active,
                    validation_error: None,
                },
            )
            .await
            .unwrap();

            let exported = export_system_profile_json(&pool, original.id)
                .await
                .unwrap();
            delete_system_profile(&pool, original.id).await.unwrap();

            let imported = import_system_profile_json(&pool, &exported).await.unwrap();
            assert_eq!(imported.key, "portable_board");
            assert_eq!(imported.name, "Portable Board");
            assert_eq!(imported.adapter_key, "declarative_endpoint_inventory");
            assert_eq!(
                imported.definition["detect"]["required"][0]["htmlContains"],
                "portable-board"
            );
            assert_eq!(imported.source_config_schema["required"][0], "startUrl");
            assert!(!imported.built_in);
        });
    }

    #[test]
    fn system_profile_json_import_updates_existing_custom_profile() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let existing = create_system_profile(
                &pool,
                CreateSystemProfileInput {
                    key: "portable_board".to_string(),
                    name: "Portable Board".to_string(),
                    description: None,
                    adapter_key: "declarative_endpoint_inventory".to_string(),
                    definition_schema_version: 1,
                    definition: json!({
                        "detect": { "required": [{ "htmlContains": "old-marker" }] }
                    }),
                    source_config_schema: json!({ "type": "object" }),
                    status: SourceStatus::Active,
                    validation_error: None,
                },
            )
            .await
            .unwrap();

            let imported = import_system_profile_json(
                &pool,
                r#"{
                  "key": "portable_board",
                  "name": "Portable Board v2",
                  "description": "Updated from JSON",
                  "adapterKey": "declarative_endpoint_inventory",
                  "definitionSchemaVersion": 2,
                  "definition": {
                    "detect": { "required": [{ "htmlContains": "new-marker" }] }
                  },
                  "sourceConfigSchema": { "type": "object" },
                  "status": "active",
                  "validationError": null
                }"#,
            )
            .await
            .unwrap();

            assert_eq!(imported.id, existing.id);
            assert_eq!(imported.name, "Portable Board v2");
            assert_eq!(imported.definition_schema_version, 2);
            assert_eq!(
                imported.definition["detect"]["required"][0]["htmlContains"],
                "new-marker"
            );
        });
    }

    #[test]
    fn system_profile_json_import_rejects_invalid_profile_without_persistence() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;

            let invalid = import_system_profile_json(
                &pool,
                r#"{
                  "key": "invalid_profile",
                  "name": "Invalid Profile",
                  "adapterKey": "stepstone_search",
                  "definitionSchemaVersion": 1,
                  "definition": {},
                  "sourceConfigSchema": { "type": "object" },
                  "status": "active"
                }"#,
            )
            .await
            .unwrap_err();

            assert!(
                invalid.contains("adapterKey stepstone_search cannot be used by system profiles")
            );
            assert!(get_system_profile_by_key(&pool, "invalid_profile")
                .await
                .is_err());
        });
    }

    #[test]
    fn system_profile_json_import_rejects_malformed_detection_checks_without_persistence() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;

            let cases = [
                (
                    "invalid_html_contains",
                    json!({ "htmlContains": 42 }),
                    "definition.detect.required[0].htmlContains must be a string",
                ),
                (
                    "invalid_html_regex",
                    json!({ "htmlRegex": "[" }),
                    "definition.detect.required[0].htmlRegex is invalid",
                ),
                (
                    "missing_fetch_text_url",
                    json!({ "fetchText": { "contains": "marker" } }),
                    "definition.detect.required[0].fetchText.url must be a non-empty string",
                ),
                (
                    "invalid_fetch_text_url",
                    json!({ "fetchText": { "url": "http://[" } }),
                    "definition.detect.required[0].fetchText.url is invalid",
                ),
                (
                    "invalid_fetch_text_regex",
                    json!({ "fetchText": { "url": "/sitemap.xml", "regex": "[" } }),
                    "definition.detect.required[0].fetchText.regex is invalid",
                ),
                (
                    "missing_fetch_json_url",
                    json!({ "fetchJson": { "pathExists": "$.jobs" } }),
                    "definition.detect.required[0].fetchJson.url must be a non-empty string",
                ),
                (
                    "invalid_fetch_json_url",
                    json!({ "fetchJson": { "url": "http://[" } }),
                    "definition.detect.required[0].fetchJson.url is invalid",
                ),
                (
                    "malformed_fetch_script_src_regex",
                    json!({ "fetchScript": { "srcRegex": "[", "contains": "marker" } }),
                    "definition.detect.required[0].fetchScript.srcRegex is invalid",
                ),
                (
                    "malformed_fetch_script_regex",
                    json!({ "fetchScript": { "regex": "[" } }),
                    "definition.detect.required[0].fetchScript.regex is invalid",
                ),
                (
                    "unknown_domain_contains",
                    json!({ "domainContains": "example.com" }),
                    "definition.detect.required[0] uses unsupported detection check",
                ),
            ];

            for (key, check, expected_error) in cases {
                let profile_json = json!({
                    "key": key,
                    "name": "Invalid Detection",
                    "adapterKey": "declarative_endpoint_inventory",
                    "definitionSchemaVersion": 1,
                    "definition": {
                        "detect": { "required": [check] }
                    },
                    "sourceConfigSchema": { "type": "object" },
                    "status": "active",
                    "validationError": null
                })
                .to_string();

                let error = import_system_profile_json(&pool, &profile_json)
                    .await
                    .unwrap_err();
                assert!(
                    error.contains(expected_error),
                    "expected `{error}` to contain `{expected_error}`"
                );
                assert!(get_system_profile_by_key(&pool, key).await.is_err());
            }
        });
    }

    #[test]
    fn system_profile_json_import_validates_identity_templates() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;

            let valid = import_system_profile_json(
                &pool,
                r#"{
                  "key": "identity_board",
                  "name": "Identity Board",
                  "adapterKey": "declarative_endpoint_inventory",
                  "definitionSchemaVersion": 1,
                  "definition": {
                    "detect": { "required": [{ "htmlRegex": "https://jobs.example.com/([a-z0-9_-]+)", "captureAs": "boardSlug" }] },
                    "identity": {
                      "extract": [{ "htmlRegex": "\"publicWebsite\":\"(https?://[^\"\\\\]+)\"", "captureAs": "companyWebsite" }],
                      "keyCandidates": ["{{capture:companyWebsite|domainKey}}_careers", "{{capture:boardSlug|technicalKey}}_careers"],
                      "nameCandidates": ["{{capture:companyWebsite|domainTitle}} Karriere"],
                      "optionalSourceConfig": { "companyWebsite": "{{capture:companyWebsite}}" }
                    },
                    "sourceConfig": { "startUrl": "{{inputUrl}}" }
                  },
                  "sourceConfigSchema": { "type": "object" },
                  "status": "active"
                }"#,
            )
            .await
            .unwrap();
            assert_eq!(valid.key, "identity_board");

            let invalid = import_system_profile_json(
                &pool,
                r#"{
                  "key": "invalid_identity_board",
                  "name": "Invalid Identity Board",
                  "adapterKey": "declarative_endpoint_inventory",
                  "definitionSchemaVersion": 1,
                  "definition": {
                    "detect": { "required": [{ "htmlContains": "marker" }] },
                    "identity": {
                      "keyCandidates": ["{{capture:companyWebsite|unknownFilter}}_careers"]
                    }
                  },
                  "sourceConfigSchema": { "type": "object" },
                  "status": "active"
                }"#,
            )
            .await
            .unwrap_err();
            assert!(invalid.contains("definition.identity.keyCandidates[0] contains unsupported template filter `unknownFilter`"));
            assert!(get_system_profile_by_key(&pool, "invalid_identity_board")
                .await
                .is_err());
        });
    }

    #[test]
    fn system_profile_validation_accepts_xml_and_json_inventory_definitions() {
        for inventory in [valid_xml_inventory(), valid_json_inventory()] {
            let definition = profile_definition_with_inventory(inventory);

            validate_system_profile_definition_and_schema(
                &definition,
                &json!({ "type": "object" }),
                SourceStatus::Active,
            )
            .unwrap();
        }
    }

    #[test]
    fn system_profile_validation_rejects_invalid_inventory_definitions_with_actionable_paths() {
        let cases = vec![
            (
                "missing_fetch_url",
                {
                    let mut inventory = valid_json_inventory();
                    inventory["fetch"] = json!({});
                    inventory
                },
                "definition.inventory.fetch.url must be a non-empty string",
            ),
            (
                "unsupported_parse_as",
                {
                    let mut inventory = valid_json_inventory();
                    inventory["parse"]["as"] = json!("html");
                    inventory
                },
                "definition.inventory.parse.as must be xml or json",
            ),
            (
                "unknown_selector_shape",
                {
                    let mut inventory = valid_json_inventory();
                    inventory["items"]["select"] = json!({ "css": ".job" });
                    inventory
                },
                "definition.inventory.items.select.css is not supported",
            ),
            (
                "invalid_where_regex",
                {
                    let mut inventory = valid_xml_inventory();
                    inventory["items"]["where"] = json!([{ "regex": "[" }]);
                    inventory
                },
                "definition.inventory.items.where[0].regex is invalid",
            ),
            (
                "invalid_capture_regex",
                {
                    let mut inventory = valid_xml_inventory();
                    inventory["items"]["captures"] = json!([{ "regex": "[" }]);
                    inventory
                },
                "definition.inventory.items.captures[0].regex is invalid",
            ),
            (
                "missing_title_field",
                {
                    let mut inventory = valid_json_inventory();
                    inventory["fields"].as_object_mut().unwrap().remove("title");
                    inventory
                },
                "definition.inventory.fields.title is required",
            ),
            (
                "missing_url_field",
                {
                    let mut inventory = valid_json_inventory();
                    inventory["fields"].as_object_mut().unwrap().remove("url");
                    inventory
                },
                "definition.inventory.fields.url is required",
            ),
            (
                "invalid_field_expression_object",
                {
                    let mut inventory = valid_json_inventory();
                    inventory["fields"]["title"] = json!({ "literal": "Engineer" });
                    inventory
                },
                "definition.inventory.fields.title.literal is not supported",
            ),
        ];

        for (case_name, inventory, expected_error) in cases {
            let definition = profile_definition_with_inventory(inventory);
            let error = validate_system_profile_definition_and_schema(
                &definition,
                &json!({ "type": "object" }),
                SourceStatus::Active,
            )
            .expect_err(case_name);

            assert!(
                error.contains(expected_error),
                "case {case_name}: expected `{error}` to contain `{expected_error}`"
            );
        }
    }

    #[test]
    fn system_profile_create_update_and_import_validate_inventory() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let invalid_definition = profile_definition_with_inventory({
                let mut inventory = valid_json_inventory();
                inventory["fields"].as_object_mut().unwrap().remove("url");
                inventory
            });

            let create_error = create_system_profile(
                &pool,
                CreateSystemProfileInput {
                    key: "invalid_create_inventory".to_string(),
                    name: "Invalid Create Inventory".to_string(),
                    description: None,
                    adapter_key: "declarative_endpoint_inventory".to_string(),
                    definition_schema_version: 1,
                    definition: invalid_definition.clone(),
                    source_config_schema: json!({ "type": "object" }),
                    status: SourceStatus::Active,
                    validation_error: None,
                },
            )
            .await
            .unwrap_err();
            assert!(create_error.contains("definition.inventory.fields.url is required"));
            assert!(get_system_profile_by_key(&pool, "invalid_create_inventory")
                .await
                .is_err());

            let existing = create_system_profile(
                &pool,
                CreateSystemProfileInput {
                    key: "valid_inventory".to_string(),
                    name: "Valid Inventory".to_string(),
                    description: None,
                    adapter_key: "declarative_endpoint_inventory".to_string(),
                    definition_schema_version: 1,
                    definition: profile_definition_with_inventory(valid_json_inventory()),
                    source_config_schema: json!({ "type": "object" }),
                    status: SourceStatus::Active,
                    validation_error: None,
                },
            )
            .await
            .unwrap();

            let update_error = update_system_profile(
                &pool,
                existing.id,
                UpdateSystemProfileInput {
                    name: "Invalid Update Inventory".to_string(),
                    description: None,
                    adapter_key: "declarative_endpoint_inventory".to_string(),
                    definition_schema_version: 2,
                    definition: invalid_definition.clone(),
                    source_config_schema: json!({ "type": "object" }),
                    status: SourceStatus::Active,
                    validation_error: None,
                },
            )
            .await
            .unwrap_err();
            assert!(update_error.contains("definition.inventory.fields.url is required"));
            let unchanged = get_system_profile(&pool, existing.id).await.unwrap();
            assert_eq!(unchanged.name, "Valid Inventory");
            assert_eq!(unchanged.definition_schema_version, 1);

            let import_document = json!({
                "key": "invalid_import_inventory",
                "name": "Invalid Import Inventory",
                "adapterKey": "declarative_endpoint_inventory",
                "definitionSchemaVersion": 1,
                "definition": invalid_definition,
                "sourceConfigSchema": { "type": "object" },
                "status": "active"
            })
            .to_string();
            let import_error = import_system_profile_json(&pool, &import_document)
                .await
                .unwrap_err();
            assert!(import_error.contains("definition.inventory.fields.url is required"));
            assert!(get_system_profile_by_key(&pool, "invalid_import_inventory")
                .await
                .is_err());
        });
    }

    #[test]
    fn system_profile_json_import_rejects_invalid_detection_update_without_persistence() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let existing = create_system_profile(
                &pool,
                CreateSystemProfileInput {
                    key: "portable_board".to_string(),
                    name: "Portable Board".to_string(),
                    description: None,
                    adapter_key: "declarative_endpoint_inventory".to_string(),
                    definition_schema_version: 1,
                    definition: json!({
                        "detect": { "required": [{ "htmlContains": "old-marker" }] }
                    }),
                    source_config_schema: json!({ "type": "object" }),
                    status: SourceStatus::Active,
                    validation_error: None,
                },
            )
            .await
            .unwrap();

            let error = import_system_profile_json(
                &pool,
                r#"{
                  "key": "portable_board",
                  "name": "Portable Board with Bad Detection",
                  "adapterKey": "declarative_endpoint_inventory",
                  "definitionSchemaVersion": 2,
                  "definition": {
                    "detect": { "required": [{ "domainContains": "example.com" }] }
                  },
                  "sourceConfigSchema": { "type": "object" },
                  "status": "active"
                }"#,
            )
            .await
            .unwrap_err();

            let unchanged = get_system_profile(&pool, existing.id).await.unwrap();
            assert!(
                error.contains("definition.detect.required[0] uses unsupported detection check")
            );
            assert_eq!(unchanged.name, "Portable Board");
            assert_eq!(unchanged.definition_schema_version, 1);
            assert_eq!(
                unchanged.definition["detect"]["required"][0]["htmlContains"],
                "old-marker"
            );
        });
    }

    #[test]
    fn system_profile_json_import_rejects_invalid_definition_and_schema() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;

            let invalid_definition = import_system_profile_json(
                &pool,
                r#"{
                  "key": "invalid_definition",
                  "name": "Invalid Definition",
                  "adapterKey": "declarative_endpoint_inventory",
                  "definitionSchemaVersion": 1,
                  "definition": [],
                  "sourceConfigSchema": { "type": "object" },
                  "status": "active"
                }"#,
            )
            .await
            .unwrap_err();
            assert!(invalid_definition.contains("definition must be a JSON object"));
            assert!(get_system_profile_by_key(&pool, "invalid_definition")
                .await
                .is_err());

            let invalid_schema = import_system_profile_json(
                &pool,
                r#"{
                  "key": "invalid_schema",
                  "name": "Invalid Schema",
                  "adapterKey": "declarative_endpoint_inventory",
                  "definitionSchemaVersion": 1,
                  "definition": {
                    "detect": { "required": [{ "htmlContains": "marker" }] }
                  },
                  "sourceConfigSchema": [],
                  "status": "active"
                }"#,
            )
            .await
            .unwrap_err();
            assert!(invalid_schema.contains("sourceConfigSchema must be a JSON object"));
            assert!(get_system_profile_by_key(&pool, "invalid_schema")
                .await
                .is_err());

            let invalid_schema_shape = import_system_profile_json(
                &pool,
                r#"{
                  "key": "invalid_schema_shape",
                  "name": "Invalid Schema Shape",
                  "adapterKey": "declarative_endpoint_inventory",
                  "definitionSchemaVersion": 1,
                  "definition": {
                    "detect": { "required": [{ "htmlContains": "marker" }] }
                  },
                  "sourceConfigSchema": { "type": "object", "required": "startUrl" },
                  "status": "active"
                }"#,
            )
            .await
            .unwrap_err();
            assert!(invalid_schema_shape.contains("sourceConfigSchema.required must be an array"));
            assert!(get_system_profile_by_key(&pool, "invalid_schema_shape")
                .await
                .is_err());
        });
    }

    #[test]
    fn system_profile_json_import_does_not_overwrite_built_in_profiles() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            sqlx::query(
                "INSERT INTO system_profiles (
                   key, name, adapter_key, definition_schema_version,
                   definition_json, source_config_schema_json, built_in, status
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7)",
            )
            .bind("greenhouse")
            .bind("Greenhouse")
            .bind("declarative_endpoint_inventory")
            .bind(1_i64)
            .bind(r#"{"detect":{"required":[{"htmlContains":"old"}]}}"#)
            .bind(r#"{"type":"object"}"#)
            .bind("active")
            .execute(&pool)
            .await
            .unwrap();

            let error = import_system_profile_json(
                &pool,
                r#"{
                  "key": "greenhouse",
                  "name": "Overwritten Greenhouse",
                  "adapterKey": "declarative_endpoint_inventory",
                  "definitionSchemaVersion": 1,
                  "definition": {
                    "detect": { "required": [{ "htmlContains": "new" }] }
                  },
                  "sourceConfigSchema": { "type": "object" },
                  "status": "active"
                }"#,
            )
            .await
            .unwrap_err();

            let greenhouse = get_system_profile_by_key(&pool, "greenhouse")
                .await
                .unwrap();
            assert!(error
                .contains("built-in system profile greenhouse cannot be overwritten by import"));
            assert_eq!(greenhouse.name, "Greenhouse");
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
                    adapter_key: "declarative_endpoint_inventory".to_string(),
                    definition_schema_version: 1,
                    definition: json!({
                        "detect": { "required": [{ "htmlContains": "greenhouse" }] }
                    }),
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
                    adapter_key: "declarative_endpoint_inventory".to_string(),
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
            assert_eq!(source.adapter_key, "declarative_endpoint_inventory");

            let missing_profile = create_source(
                &pool,
                CreateSourceInput {
                    key: "missing_profile".to_string(),
                    adapter_key: "declarative_endpoint_inventory".to_string(),
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
                    adapter_key: "declarative_endpoint_inventory".to_string(),
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

    fn profile_definition_with_inventory(inventory: Value) -> Value {
        json!({
            "detect": { "required": [{ "htmlContains": "fixture-board" }] },
            "inventory": inventory
        })
    }

    fn valid_xml_inventory() -> Value {
        json!({
            "fetch": { "url": "{{sourceConfig:url}}" },
            "parse": { "as": "xml" },
            "items": {
                "select": { "xmlText": "loc" },
                "where": [{ "regex": "(?i)/job/" }],
                "captures": [{
                    "regex": "(?i)/job/(?P<location>[^/-]+)-(?P<title>.+?)(?:-\\d+)?/?$"
                }]
            },
            "fields": {
                "title": { "template": "{{capture:title|urlDecode|slugToTitle}}" },
                "url": { "template": "{{itemText}}" },
                "company": { "template": "{{sourceName|stripCareerSuffix}}" },
                "locations": [
                    { "template": "{{capture:location|urlDecode|slugToTitle}}" }
                ]
            }
        })
    }

    fn valid_json_inventory() -> Value {
        json!({
            "fetch": { "url": "{{sourceConfig:startUrl}}" },
            "parse": { "as": "json" },
            "items": {
                "select": { "jsonPath": "$.jobs" }
            },
            "fields": {
                "title": { "jsonPath": "$.title" },
                "url": { "jsonPath": "$.jobUrl" },
                "company": { "template": "{{sourceName}}" },
                "locations": [
                    { "jsonPath": "$.location" }
                ]
            }
        })
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
