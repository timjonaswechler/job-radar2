use serde::Serialize;
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterExecutionMode {
    SourceInventory,
    QueryParameterized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterCategory {
    Generic,
    JobBoard,
    Browser,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterAuthMode {
    None,
    ManualCookie,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterRiskLevel {
    Stable,
    Fragile,
    Restricted,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterMetadata {
    pub key: String,
    pub name: String,
    pub description: String,
    pub category: AdapterCategory,
    pub execution_mode: AdapterExecutionMode,
    pub source_config_schema: Value,
    pub supports_manual_release: bool,
    pub auth_mode: AdapterAuthMode,
    pub risk_level: AdapterRiskLevel,
}

pub fn list_adapters() -> Vec<AdapterMetadata> {
    vec![
        declarative_endpoint_inventory(),
        declarative_sitemap_inventory(),
        declarative_browser_inventory(),
    ]
}

#[cfg(test)]
fn get_adapter(key: &str) -> Option<AdapterMetadata> {
    list_adapters()
        .into_iter()
        .find(|adapter| adapter.key == key)
}

fn declarative_endpoint_inventory() -> AdapterMetadata {
    AdapterMetadata {
        key: "declarative_endpoint_inventory".to_string(),
        name: "Deklaratives HTTP-Jobboard".to_string(),
        description: "Technische Laufzeit für Quellenprofile mit deklarativen HTML-, JSON- und HTTP-Endpunkt-Inventaren.".to_string(),
        category: AdapterCategory::Generic,
        execution_mode: AdapterExecutionMode::SourceInventory,
        supports_manual_release: false,
        auth_mode: AdapterAuthMode::None,
        risk_level: AdapterRiskLevel::Stable,
        source_config_schema: json!({
            "type": "object",
            "required": ["startUrl"],
            "properties": {
                "startUrl": {
                    "type": "string",
                    "format": "uri",
                    "title": "Start-URL",
                    "description": "URL, die der gewählte Zugriffspfad später abfragt."
                },
                "maxJobs": {
                    "type": "number",
                    "minimum": 1,
                    "title": "Maximale Jobs"
                }
            }
        }),
    }
}

fn declarative_sitemap_inventory() -> AdapterMetadata {
    AdapterMetadata {
        key: "declarative_sitemap_inventory".to_string(),
        name: "Deklaratives Sitemap-Jobboard".to_string(),
        description: "Technische Laufzeit für Quellenprofile mit deklarativen Sitemap-Inventaren."
            .to_string(),
        category: AdapterCategory::Generic,
        execution_mode: AdapterExecutionMode::SourceInventory,
        supports_manual_release: false,
        auth_mode: AdapterAuthMode::None,
        risk_level: AdapterRiskLevel::Stable,
        source_config_schema: json!({
            "type": "object",
            "required": ["url"],
            "properties": {
                "url": {
                    "type": "string",
                    "format": "uri",
                    "title": "Sitemap-URL"
                },
                "recursive": {
                    "type": "boolean",
                    "default": true,
                    "title": "Sitemap-Index rekursiv auswerten"
                },
                "maxUrls": {
                    "type": "number",
                    "minimum": 1,
                    "title": "Maximale URLs"
                }
            }
        }),
    }
}

fn declarative_browser_inventory() -> AdapterMetadata {
    AdapterMetadata {
        key: "declarative_browser_inventory".to_string(),
        name: "Deklaratives Browser-Inventar".to_string(),
        description: "Technische Laufzeit für browserbasierte Zugriffspfade, die gerenderte Webseiten über die Browser-Laufzeit als Quellenbestand extrahieren.".to_string(),
        category: AdapterCategory::Browser,
        execution_mode: AdapterExecutionMode::SourceInventory,
        supports_manual_release: true,
        auth_mode: AdapterAuthMode::ManualCookie,
        risk_level: AdapterRiskLevel::Fragile,
        source_config_schema: json!({
            "type": "object"
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_generic_runtimes_and_builtin_job_portals_are_registered() {
        let adapters = list_adapters();
        let keys = adapters
            .iter()
            .map(|adapter| adapter.key.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            keys,
            vec![
                "declarative_endpoint_inventory",
                "declarative_sitemap_inventory",
                "declarative_browser_inventory",
            ]
        );

        for old_system_adapter in [
            "successfactors_sitemap",
            "phenom_sitemap",
            "workday",
            "greenhouse",
            "lever",
            "personio",
            "ashby",
        ] {
            assert!(get_adapter(old_system_adapter).is_none());
        }
    }

    #[test]
    fn declarative_inventory_runtimes_are_source_inventory_adapters() {
        for adapter_key in [
            "declarative_endpoint_inventory",
            "declarative_sitemap_inventory",
            "declarative_browser_inventory",
        ] {
            let adapter = get_adapter(adapter_key).unwrap();
            assert_eq!(
                adapter.execution_mode,
                AdapterExecutionMode::SourceInventory
            );
        }

        let browser_inventory = get_adapter("declarative_browser_inventory").unwrap();
        assert_eq!(
            browser_inventory.source_config_schema,
            json!({ "type": "object" })
        );
    }

    #[test]
    fn adapter_metadata_omits_legacy_profile_requirement_flags() {
        let value =
            serde_json::to_value(get_adapter("declarative_endpoint_inventory").unwrap()).unwrap();
        assert!(value.get("requiresSystemProfile").is_none());
        assert!(value.get("requiresBrowserProfile").is_none());
        assert_eq!(value["category"], "generic");
    }
}
