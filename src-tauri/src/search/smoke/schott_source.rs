use serde_json::{json, Value};
use std::path::Path;

use crate::{
    source::documents::{SelectedAccessPath, SourceStatus},
    source_profile::registry::{RegistrySource, SourceProfileRegistrySnapshot},
};

use super::constants::{
    SCHOTT_SITEMAP_URL, SCHOTT_SOURCE_KEY, SCHOTT_SOURCE_NAME, SUCCESSFACTORS_PROFILE_KEY,
};

const SUCCESSFACTORS_SMOKE_ACCESS_PATH_KEY: &str = "rmk_sitemap_html";
const SCHOTT_BASE_URL: &str = "https://join.schott.com";

pub(super) fn validate_smoke_source(source: &RegistrySource) -> Result<(), String> {
    let document = &source.document;
    if document.status != SourceStatus::Active {
        return Err(format!(
            "smoke source `{}` must be active, found {:?}",
            document.key, document.status
        ));
    }

    match &document.selected_access_path {
        SelectedAccessPath::ProfileAccessPath {
            profile_key,
            path_key,
        } if profile_key == SUCCESSFACTORS_PROFILE_KEY
            && path_key == SUCCESSFACTORS_SMOKE_ACCESS_PATH_KEY => {}
        SelectedAccessPath::ProfileAccessPath {
            profile_key,
            path_key,
        } => {
            return Err(format!(
                "smoke source `{}` must use Source Profile `{SUCCESSFACTORS_PROFILE_KEY}` Access Path `{SUCCESSFACTORS_SMOKE_ACCESS_PATH_KEY}`, found `{profile_key}` path `{path_key}`",
                document.key
            ));
        }
        SelectedAccessPath::SourceOwnedAccessPath { key, .. } => {
            return Err(format!(
                "smoke source `{}` must use Source Profile `{SUCCESSFACTORS_PROFILE_KEY}` Access Path `{SUCCESSFACTORS_SMOKE_ACCESS_PATH_KEY}`, found Source-owned Access Path `{key}`",
                document.key
            ));
        }
    }

    validate_schott_source_config(source)
}

fn validate_schott_source_config(source: &RegistrySource) -> Result<(), String> {
    let document = &source.document;
    let sitemap_url = document
        .source_config
        .get("sitemapUrl")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let base_url = document
        .source_config
        .get("baseUrl")
        .and_then(|value| value.as_str())
        .unwrap_or_default();

    if sitemap_url != SCHOTT_SITEMAP_URL || base_url != SCHOTT_BASE_URL {
        return Err(format!(
            "smoke source `{}` must use Source Config {{\"baseUrl\":\"{}\",\"sitemapUrl\":\"{}\"}}",
            document.key, SCHOTT_BASE_URL, SCHOTT_SITEMAP_URL
        ));
    }

    Ok(())
}

pub(super) fn ensure_schott_smoke_source(app_data_dir: &Path) -> Result<RegistrySource, String> {
    let snapshot = crate::source_profile::registry::load_snapshot(app_data_dir);
    if let Some(source) = snapshot.source(SCHOTT_SOURCE_KEY) {
        validate_smoke_source(source)?;
        return Ok(source.clone());
    }

    write_schott_smoke_source_file(app_data_dir)?;
    let snapshot = crate::source_profile::registry::load_snapshot(app_data_dir);
    fail_on_schott_registry_diagnostics(&snapshot)?;
    let source = snapshot.source(SCHOTT_SOURCE_KEY).ok_or_else(|| {
        format!("source registry did not load `{SCHOTT_SOURCE_KEY}` after writing its Source JSON")
    })?;
    validate_smoke_source(source)?;
    Ok(source.clone())
}

pub(super) fn write_schott_smoke_source_file(app_data_dir: &Path) -> Result<(), String> {
    let path = app_data_dir
        .join("sources")
        .join(format!("{SCHOTT_SOURCE_KEY}.json"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let document = schott_smoke_source_json();
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&document).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

fn schott_smoke_source_json() -> Value {
    json!({
        "schemaVersion": 3,
        "key": SCHOTT_SOURCE_KEY,
        "name": SCHOTT_SOURCE_NAME,
        "status": "active",
        "sourceConfig": {
            "baseUrl": SCHOTT_BASE_URL,
            "sitemapUrl": SCHOTT_SITEMAP_URL
        },
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": SUCCESSFACTORS_PROFILE_KEY,
            "pathKey": SUCCESSFACTORS_SMOKE_ACCESS_PATH_KEY
        }
    })
}

fn fail_on_schott_registry_diagnostics(
    snapshot: &SourceProfileRegistrySnapshot,
) -> Result<(), String> {
    let diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic
                .details
                .as_ref()
                .and_then(|details| details.get("sourceKey"))
                .and_then(|value| value.as_str())
                == Some(SCHOTT_SOURCE_KEY)
        })
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Source Profile registry rejected `{SCHOTT_SOURCE_KEY}`: {}",
            diagnostics.join("; ")
        ))
    }
}
