use serde_json::{json, Value};
use std::path::Path;

use crate::source_registry::{
    RegistrySource, SelectedAccessPath, SourceDocumentStatus, SourceRegistrySnapshot,
};

use super::constants::{
    SCHOTT_SITEMAP_URL, SCHOTT_SOURCE_KEY, SCHOTT_SOURCE_NAME, SUCCESSFACTORS_PROFILE_KEY,
};

pub(super) fn validate_smoke_source(source: &RegistrySource) -> Result<(), String> {
    let document = &source.document;
    if document.status != SourceDocumentStatus::Active {
        return Err(format!(
            "smoke source `{}` must be active, found {:?}",
            document.key, document.status
        ));
    }

    match &document.selected_access_path {
        SelectedAccessPath::Profile {
            profile_key,
            path_key,
        } if profile_key == SUCCESSFACTORS_PROFILE_KEY && path_key == "sitemap_inventory" => {}
        SelectedAccessPath::Profile {
            profile_key,
            path_key,
        } => {
            return Err(format!(
                "smoke source `{}` must use source profile `{SUCCESSFACTORS_PROFILE_KEY}` path `sitemap_inventory`, found `{profile_key}` path `{path_key}`",
                document.key
            ));
        }
        SelectedAccessPath::SourceSpecific { adapter_key, .. } => {
            return Err(format!(
                "smoke source `{}` must use source profile `{SUCCESSFACTORS_PROFILE_KEY}` path `sitemap_inventory`, found source-specific adapter `{adapter_key}`",
                document.key
            ));
        }
    }

    validate_schott_source_config(source)
}

fn validate_schott_source_config(source: &RegistrySource) -> Result<(), String> {
    let document = &source.document;
    let url = document
        .source_config
        .get("url")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let recursive = document
        .source_config
        .get("recursive")
        .and_then(|value| value.as_bool());

    if url != SCHOTT_SITEMAP_URL || recursive != Some(false) {
        return Err(format!(
            "smoke source `{}` must use sourceConfig {{\"url\":\"{}\",\"recursive\":false}}",
            document.key, SCHOTT_SITEMAP_URL
        ));
    }

    Ok(())
}

pub(super) fn ensure_schott_smoke_source(app_data_dir: &Path) -> Result<RegistrySource, String> {
    let snapshot = crate::source_registry::load_snapshot(app_data_dir);
    if let Some(source) = snapshot.source(SCHOTT_SOURCE_KEY) {
        validate_smoke_source(source)?;
        return Ok(source.clone());
    }

    write_schott_smoke_source_file(app_data_dir)?;
    let snapshot = crate::source_registry::load_snapshot(app_data_dir);
    fail_on_schott_registry_diagnostics(&snapshot)?;
    let source = snapshot.source(SCHOTT_SOURCE_KEY).ok_or_else(|| {
        format!("source registry did not load `{SCHOTT_SOURCE_KEY}` after writing its source JSON")
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
        "schemaVersion": 1,
        "key": SCHOTT_SOURCE_KEY,
        "name": SCHOTT_SOURCE_NAME,
        "status": "active",
        "sourceConfig": {
            "url": SCHOTT_SITEMAP_URL,
            "recursive": false
        },
        "selectedAccessPath": {
            "type": "profile",
            "profileKey": SUCCESSFACTORS_PROFILE_KEY,
            "pathKey": "sitemap_inventory"
        }
    })
}

fn fail_on_schott_registry_diagnostics(snapshot: &SourceRegistrySnapshot) -> Result<(), String> {
    let diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.key.as_deref() == Some(SCHOTT_SOURCE_KEY))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "source registry rejected `{SCHOTT_SOURCE_KEY}`: {}",
            diagnostics.join("; ")
        ))
    }
}
