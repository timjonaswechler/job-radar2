use serde::Deserialize;
use serde_json::Value;
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::{
    error::Error,
    fs, io,
    path::{Path, PathBuf},
};

const BUILTIN_SYSTEM_PROFILE_JSON_FILES: &[(&str, &str)] = &[
    (
        "system-profiles/builtin/muz_global_jobboard.json",
        include_str!("../../system-profiles/builtin/muz_global_jobboard.json"),
    ),
    (
        "system-profiles/builtin/greenhouse.json",
        include_str!("../../system-profiles/builtin/greenhouse.json"),
    ),
    (
        "system-profiles/builtin/lever.json",
        include_str!("../../system-profiles/builtin/lever.json"),
    ),
    (
        "system-profiles/builtin/ashby.json",
        include_str!("../../system-profiles/builtin/ashby.json"),
    ),
    (
        "system-profiles/builtin/personio.json",
        include_str!("../../system-profiles/builtin/personio.json"),
    ),
    (
        "system-profiles/builtin/workday.json",
        include_str!("../../system-profiles/builtin/workday.json"),
    ),
    (
        "system-profiles/builtin/magnolia_esmp_job_search.json",
        include_str!("../../system-profiles/builtin/magnolia_esmp_job_search.json"),
    ),
    (
        "system-profiles/builtin/successfactors.json",
        include_str!("../../system-profiles/builtin/successfactors.json"),
    ),
    (
        "system-profiles/builtin/phenom.json",
        include_str!("../../system-profiles/builtin/phenom.json"),
    ),
];

type SeedResult<T> = Result<T, Box<dyn Error>>;

pub async fn connect_and_migrate(
    db_path: &Path,
    custom_system_profiles_dir: &Path,
) -> Result<SqlitePool, Box<dyn Error>> {
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .foreign_keys(true);

    let pool = SqlitePool::connect_with(options).await?;

    // Embedded at compile time, so packaged builds do not need loose SQL files.
    sqlx::migrate!("./migrations").run(&pool).await?;
    seed_database(&pool, custom_system_profiles_dir).await?;

    Ok(pool)
}

async fn seed_database(pool: &SqlitePool, custom_system_profiles_dir: &Path) -> SeedResult<()> {
    sqlx::query(
        "INSERT OR IGNORE INTO app_metadata (key, value)
         VALUES ('database_initialized', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
    )
    .execute(pool)
    .await?;

    seed_builtin_system_profiles(pool).await?;
    seed_custom_system_profiles(pool, custom_system_profiles_dir).await?;
    seed_builtin_job_portal_sources(pool).await?;

    Ok(())
}

async fn seed_builtin_system_profiles(pool: &SqlitePool) -> SeedResult<()> {
    for (source_label, contents) in BUILTIN_SYSTEM_PROFILE_JSON_FILES {
        let seed = parse_system_profile_seed(contents, source_label)?;
        upsert_system_profile(pool, &seed, true, source_label).await?;
    }

    Ok(())
}

async fn seed_custom_system_profiles(
    pool: &SqlitePool,
    custom_system_profiles_dir: &Path,
) -> SeedResult<()> {
    for path in custom_system_profile_files(custom_system_profiles_dir)? {
        let source_label = path.to_string_lossy().to_string();
        let contents = fs::read_to_string(&path).map_err(|error| {
            seed_error(format!(
                "{source_label}: could not read system profile: {error}"
            ))
        })?;
        let seed = parse_system_profile_seed(&contents, &source_label)?;
        upsert_system_profile(pool, &seed, false, &source_label).await?;
    }

    Ok(())
}

fn custom_system_profile_files(custom_system_profiles_dir: &Path) -> SeedResult<Vec<PathBuf>> {
    fs::create_dir_all(custom_system_profiles_dir).map_err(|error| {
        seed_error(format!(
            "{}: could not create custom system profile directory: {error}",
            custom_system_profiles_dir.display()
        ))
    })?;

    let mut files = Vec::new();
    for entry in fs::read_dir(custom_system_profiles_dir).map_err(|error| {
        seed_error(format!(
            "{}: could not read custom system profile directory: {error}",
            custom_system_profiles_dir.display()
        ))
    })? {
        let path = entry
            .map_err(|error| {
                seed_error(format!(
                    "{}: could not read custom system profile entry: {error}",
                    custom_system_profiles_dir.display()
                ))
            })?
            .path();
        if path.is_file()
            && matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some(extension) if extension.eq_ignore_ascii_case("json")
            )
        {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SystemProfileSeed {
    key: String,
    name: String,
    description: Option<String>,
    adapter_key: String,
    definition_schema_version: i64,
    #[serde(default = "empty_json_object")]
    definition: Value,
    #[serde(default = "empty_json_object")]
    source_config_schema: Value,
    #[serde(default = "default_active_status")]
    status: String,
    validation_error: Option<String>,
}

async fn upsert_system_profile(
    pool: &SqlitePool,
    seed: &SystemProfileSeed,
    built_in: bool,
    source_label: &str,
) -> SeedResult<()> {
    if !built_in {
        let existing_built_in =
            sqlx::query_scalar::<_, i64>("SELECT built_in FROM system_profiles WHERE key = ?1")
                .bind(&seed.key)
                .fetch_optional(pool)
                .await?;

        if existing_built_in == Some(1) {
            return Err(seed_error(format!(
                "{source_label}: custom system profile cannot override built-in key `{}`",
                seed.key
            )));
        }
    }

    let definition_json = serde_json::to_string(&seed.definition).map_err(|error| {
        seed_error(format!(
            "{source_label}: could not serialize definition JSON: {error}"
        ))
    })?;
    let source_config_schema_json =
        serde_json::to_string(&seed.source_config_schema).map_err(|error| {
            seed_error(format!(
                "{source_label}: could not serialize source config schema JSON: {error}"
            ))
        })?;

    sqlx::query(
        "INSERT INTO system_profiles (
           key, name, description, adapter_key, definition_schema_version,
           definition_json, source_config_schema_json, built_in, status, validation_error
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(key) DO UPDATE SET
           name = excluded.name,
           description = excluded.description,
           adapter_key = excluded.adapter_key,
           definition_schema_version = excluded.definition_schema_version,
           definition_json = excluded.definition_json,
           source_config_schema_json = excluded.source_config_schema_json,
           built_in = excluded.built_in,
           status = excluded.status,
           validation_error = excluded.validation_error,
           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .bind(&seed.key)
    .bind(&seed.name)
    .bind(seed.description.as_deref())
    .bind(&seed.adapter_key)
    .bind(seed.definition_schema_version)
    .bind(definition_json)
    .bind(source_config_schema_json)
    .bind(if built_in { 1_i64 } else { 0_i64 })
    .bind(&seed.status)
    .bind(seed.validation_error.as_deref())
    .execute(pool)
    .await?;

    Ok(())
}

fn parse_system_profile_seed(contents: &str, source_label: &str) -> SeedResult<SystemProfileSeed> {
    let seed = serde_json::from_str::<SystemProfileSeed>(contents)
        .map_err(|error| seed_error(format!("{source_label}: invalid JSON: {error}")))?;
    validate_system_profile_seed(&seed, source_label)?;
    Ok(seed)
}

fn validate_system_profile_seed(seed: &SystemProfileSeed, source_label: &str) -> SeedResult<()> {
    validate_technical_key("key", &seed.key, source_label)?;
    validate_required_text("name", &seed.name, source_label)?;
    validate_technical_key("adapterKey", &seed.adapter_key, source_label)?;

    let adapter = crate::adapter_registry::get_adapter(&seed.adapter_key).ok_or_else(|| {
        seed_error(format!(
            "{source_label}: adapterKey `{}` is not registered",
            seed.adapter_key
        ))
    })?;
    if !adapter.requires_system_profile {
        return Err(seed_error(format!(
            "{source_label}: adapterKey `{}` cannot be used by system profiles",
            seed.adapter_key
        )));
    }

    if seed.definition_schema_version < 1 {
        return Err(seed_error(format!(
            "{source_label}: definitionSchemaVersion must be greater than zero"
        )));
    }

    if !matches!(
        seed.status.as_str(),
        "draft" | "active" | "disabled" | "invalid"
    ) {
        return Err(seed_error(format!(
            "{source_label}: status must be draft, active, disabled, or invalid"
        )));
    }

    if !seed.definition.is_object() {
        return Err(seed_error(format!(
            "{source_label}: definition must be a JSON object"
        )));
    }

    if !seed.source_config_schema.is_object() {
        return Err(seed_error(format!(
            "{source_label}: sourceConfigSchema must be a JSON object"
        )));
    }

    if seed.status == "active" {
        let required_checks = seed
            .definition
            .pointer("/detect/required")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                seed_error(format!(
                    "{source_label}: active profiles require definition.detect.required"
                ))
            })?;
        if required_checks.is_empty() {
            return Err(seed_error(format!(
                "{source_label}: active profiles require at least one detection check"
            )));
        }
    }

    Ok(())
}

fn validate_required_text(field: &str, value: &str, source_label: &str) -> SeedResult<()> {
    if value.trim().is_empty() {
        return Err(seed_error(format!(
            "{source_label}: {field} must not be empty"
        )));
    }

    Ok(())
}

fn validate_technical_key(field: &str, value: &str, source_label: &str) -> SeedResult<()> {
    if value.is_empty() {
        return Err(seed_error(format!(
            "{source_label}: {field} must not be empty"
        )));
    }

    if !value.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
    }) {
        return Err(seed_error(format!(
            "{source_label}: {field} must use lowercase snake case with only a-z, 0-9, and _"
        )));
    }

    Ok(())
}

fn empty_json_object() -> Value {
    Value::Object(serde_json::Map::new())
}

fn default_active_status() -> String {
    "active".to_string()
}

fn seed_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(io::ErrorKind::InvalidData, message.into()))
}

async fn seed_builtin_job_portal_sources(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR IGNORE INTO browser_profiles (
           key, name, description, definition_schema_version,
           definition_json, source_config_schema_json, status
         )
         VALUES (
           'job_portal_manual_release',
           'Job-Portal Browserfreigabe',
           'Eingebautes Browserprofil für nutzerassistierte StepStone- und Indeed-Suchläufe.',
           1,
           '{}',
           '{}',
           'active'
         )",
    )
    .execute(pool)
    .await?;

    let browser_profile_id = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM browser_profiles WHERE key = 'job_portal_manual_release'",
    )
    .fetch_one(pool)
    .await?;

    seed_builtin_source(
        pool,
        "stepstone_de",
        "stepstone_search",
        browser_profile_id,
        "StepStone Deutschland",
        "Eingebautes Job-Portal. URL-Muster und Such-Templates sind im Adapter fest hinterlegt.",
    )
    .await?;
    seed_builtin_source(
        pool,
        "indeed_de",
        "indeed_search",
        browser_profile_id,
        "Indeed Deutschland",
        "Eingebautes Job-Portal. URL-Muster und Such-Templates sind im Adapter fest hinterlegt.",
    )
    .await?;

    Ok(())
}

async fn seed_builtin_source(
    pool: &SqlitePool,
    key: &str,
    adapter_key: &str,
    browser_profile_id: i64,
    name: &str,
    description: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR IGNORE INTO sources (
           key, adapter_key, system_profile_id, browser_profile_id, name, description,
           source_config_json, status
         )
         VALUES (?1, ?2, NULL, ?3, ?4, ?5, '{}', 'active')",
    )
    .bind(key)
    .bind(adapter_key)
    .bind(browser_profile_id)
    .bind(name)
    .bind(description)
    .execute(pool)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_builtin_system_profiles_are_valid_seed_files() {
        let keys = BUILTIN_SYSTEM_PROFILE_JSON_FILES
            .iter()
            .map(|(source_label, contents)| {
                parse_system_profile_seed(contents, source_label)
                    .expect("built-in system profile must parse")
                    .key
            })
            .collect::<Vec<_>>();

        assert_eq!(
            keys,
            vec![
                "muz_global_jobboard",
                "greenhouse",
                "lever",
                "ashby",
                "personio",
                "workday",
                "magnolia_esmp_job_search",
                "successfactors",
                "phenom"
            ]
        );
    }

    #[test]
    fn connect_and_migrate_seeds_builtin_and_custom_system_profiles() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let database_path = temp_dir.path().join("job_radar.db");
            let custom_profiles_dir = temp_dir.path().join("system-profiles");
            fs::create_dir_all(&custom_profiles_dir).unwrap();
            fs::write(
                custom_profiles_dir.join("custom_greenhouse_variant.json"),
                r#"{
                  "key": "custom_greenhouse_variant",
                  "name": "Custom Greenhouse Variant",
                  "description": "Local custom profile kept in the app data directory.",
                  "adapterKey": "declarative_http_jobboard",
                  "definitionSchemaVersion": 1,
                  "status": "active",
                  "definition": {
                    "detect": {
                      "required": [
                        { "htmlContains": "custom-greenhouse-marker" }
                      ]
                    },
                    "sourceConfig": { "startUrl": "{{inputUrl}}" }
                  },
                  "sourceConfigSchema": {
                    "type": "object",
                    "required": ["startUrl"],
                    "properties": {
                      "startUrl": { "type": "string", "format": "uri" }
                    }
                  }
                }"#,
            )
            .unwrap();

            let pool = connect_and_migrate(&database_path, &custom_profiles_dir)
                .await
                .unwrap();
            let profiles = crate::source_model::list_system_profiles(&pool)
                .await
                .unwrap();

            assert!(profiles
                .iter()
                .any(|profile| profile.key == "muz_global_jobboard" && profile.built_in));
            assert!(profiles
                .iter()
                .any(|profile| profile.key == "custom_greenhouse_variant" && !profile.built_in));
        });
    }

    #[test]
    fn custom_system_profiles_cannot_override_bundled_keys() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let database_path = temp_dir.path().join("job_radar.db");
            let custom_profiles_dir = temp_dir.path().join("system-profiles");
            fs::create_dir_all(&custom_profiles_dir).unwrap();
            fs::write(
                custom_profiles_dir.join("greenhouse.json"),
                r#"{
                  "key": "greenhouse",
                  "name": "Shadow Greenhouse",
                  "description": null,
                  "adapterKey": "declarative_http_jobboard",
                  "definitionSchemaVersion": 1,
                  "status": "active",
                  "definition": {
                    "detect": {
                      "required": [
                        { "htmlContains": "shadow-greenhouse" }
                      ]
                    },
                    "sourceConfig": { "startUrl": "{{inputUrl}}" }
                  },
                  "sourceConfigSchema": {
                    "type": "object",
                    "required": ["startUrl"],
                    "properties": {
                      "startUrl": { "type": "string", "format": "uri" }
                    }
                  }
                }"#,
            )
            .unwrap();

            let error = connect_and_migrate(&database_path, &custom_profiles_dir)
                .await
                .unwrap_err()
                .to_string();

            assert!(error.contains("cannot override built-in key `greenhouse`"));
        });
    }
}
