pub mod migrations;
pub mod seed;
use crate::db::seed::seed_database;

use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::{error::Error, path::Path};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::seed::parse_system_profile_seed;
    use std::fs;

    #[test]
    fn bundled_builtin_system_profiles_are_valid_seed_files() {
        let keys = crate::db::seed::BUILTIN_SYSTEM_PROFILE_JSON_FILES
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
    fn connect_and_migrate_rejects_custom_system_profile_with_invalid_inventory() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let database_path = temp_dir.path().join("job_radar.db");
            let custom_profiles_dir = temp_dir.path().join("system-profiles");
            fs::create_dir_all(&custom_profiles_dir).unwrap();
            fs::write(
                custom_profiles_dir.join("invalid_inventory.json"),
                r#"{
                  "key": "invalid_inventory",
                  "name": "Invalid Inventory",
                  "description": "Local custom profile with broken inventory.",
                  "adapterKey": "declarative_endpoint_inventory",
                  "definitionSchemaVersion": 1,
                  "status": "active",
                  "definition": {
                    "detect": { "required": [{ "htmlContains": "custom-board" }] },
                    "inventory": {
                      "fetch": { "url": "{{sourceConfig:startUrl}}" },
                      "parse": { "as": "json" },
                      "items": { "select": { "jsonPath": "$.jobs" } },
                      "fields": {
                        "title": { "jsonPath": "$.title" },
                        "company": { "template": "{{sourceName}}" },
                        "locations": [{ "jsonPath": "$.location" }]
                      }
                    }
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
            assert!(error.contains("invalid_inventory.json"));
            assert!(error.contains("definition.inventory.fields.url is required"));
        });
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
                  "adapterKey": "declarative_endpoint_inventory",
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
                  "adapterKey": "declarative_endpoint_inventory",
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
