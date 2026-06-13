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
    pub requires_system_profile: bool,
    pub requires_browser_profile: bool,
    pub supports_manual_release: bool,
    pub auth_mode: AdapterAuthMode,
    pub risk_level: AdapterRiskLevel,
}

pub fn list_adapters() -> Vec<AdapterMetadata> {
    vec![
        declarative_endpoint_inventory(),
        declarative_sitemap_inventory(),
        declarative_browser_jobboard(),
        stepstone_search(),
        indeed_search(),
    ]
}

pub fn get_adapter(key: &str) -> Option<AdapterMetadata> {
    list_adapters()
        .into_iter()
        .find(|adapter| adapter.key == key)
}

fn declarative_endpoint_inventory() -> AdapterMetadata {
    AdapterMetadata {
        key: "declarative_endpoint_inventory".to_string(),
        name: "Deklaratives HTTP-Jobboard".to_string(),
        description: "Technische Laufzeit für geladene Systemprofile, die HTML, JSON und HTTP-Endpunkte deterministisch beschreiben.".to_string(),
        category: AdapterCategory::Generic,
        execution_mode: AdapterExecutionMode::SourceInventory,
        requires_system_profile: true,
        requires_browser_profile: false,
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
                    "description": "URL, gegen die das Systemprofil geprüft und später abgefragt wird."
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
        description: "Technische Laufzeit für geladene Systemprofile, die Jobinventare über Sitemaps beschreiben.".to_string(),
        category: AdapterCategory::Generic,
        execution_mode: AdapterExecutionMode::SourceInventory,
        requires_system_profile: true,
        requires_browser_profile: false,
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

fn declarative_browser_jobboard() -> AdapterMetadata {
    AdapterMetadata {
        key: "declarative_browser_jobboard".to_string(),
        name: "Deklaratives Browser-Jobboard".to_string(),
        description: "Technische Laufzeit für geladene Systemprofile, die gerenderte Webseiten über eine Browser-Laufzeit beschreiben.".to_string(),
        category: AdapterCategory::Browser,
        execution_mode: AdapterExecutionMode::SourceInventory,
        requires_system_profile: true,
        requires_browser_profile: true,
        supports_manual_release: true,
        auth_mode: AdapterAuthMode::ManualCookie,
        risk_level: AdapterRiskLevel::Fragile,
        source_config_schema: json!({
            "type": "object",
            "required": ["startUrl"],
            "properties": {
                "startUrl": {
                    "type": "string",
                    "format": "uri",
                    "title": "Start-URL"
                },
                "manualReleaseStartUrl": {
                    "type": "string",
                    "format": "uri",
                    "title": "Start-URL für manuelle Freigabe"
                },
                "maxPages": {
                    "type": "number",
                    "minimum": 1,
                    "default": 1,
                    "title": "Maximale Seiten"
                }
            }
        }),
    }
}

fn stepstone_search() -> AdapterMetadata {
    AdapterMetadata {
        key: "stepstone_search".to_string(),
        name: "StepStone Suche".to_string(),
        description: "Eingebauter Job-Portal-Adapter für StepStone-Suchläufe; Suchtext, Ort und Radius gehören in Suchanfragen bzw. Einstellungen.".to_string(),
        category: AdapterCategory::JobBoard,
        execution_mode: AdapterExecutionMode::QueryParameterized,
        requires_system_profile: false,
        requires_browser_profile: true,
        supports_manual_release: true,
        auth_mode: AdapterAuthMode::ManualCookie,
        risk_level: AdapterRiskLevel::Fragile,
        source_config_schema: json!({
            "type": "object",
            "properties": {
                "baseUrl": {
                    "type": "string",
                    "format": "uri",
                    "title": "Basis-URL überschreiben",
                    "description": "Optional. Standard: https://www.stepstone.de",
                    "default": "https://www.stepstone.de"
                },
                "manualReleaseStartUrl": {
                    "type": "string",
                    "format": "uri",
                    "title": "Start-URL für manuelle Freigabe überschreiben",
                    "description": "Optional. Standard: https://www.stepstone.de/",
                    "default": "https://www.stepstone.de/"
                },
                "maxPages": {
                    "type": "number",
                    "minimum": 1,
                    "default": 1,
                    "title": "Maximale Seiten pro Suchlauf"
                }
            }
        }),
    }
}

fn indeed_search() -> AdapterMetadata {
    AdapterMetadata {
        key: "indeed_search".to_string(),
        name: "Indeed Suche".to_string(),
        description: "Eingebauter Job-Portal-Adapter für Indeed-Suchläufe; Suchtext, Ort und Radius gehören in Suchanfragen bzw. Einstellungen.".to_string(),
        category: AdapterCategory::JobBoard,
        execution_mode: AdapterExecutionMode::QueryParameterized,
        requires_system_profile: false,
        requires_browser_profile: true,
        supports_manual_release: true,
        auth_mode: AdapterAuthMode::ManualCookie,
        risk_level: AdapterRiskLevel::Restricted,
        source_config_schema: json!({
            "type": "object",
            "properties": {
                "baseUrl": {
                    "type": "string",
                    "format": "uri",
                    "title": "Basis-URL überschreiben",
                    "description": "Optional. Standard: https://de.indeed.com",
                    "default": "https://de.indeed.com"
                },
                "manualReleaseStartUrl": {
                    "type": "string",
                    "format": "uri",
                    "title": "Start-URL für manuelle Freigabe überschreiben",
                    "description": "Optional. Standard: https://de.indeed.com/",
                    "default": "https://de.indeed.com/"
                },
                "maxPages": {
                    "type": "number",
                    "minimum": 1,
                    "default": 1,
                    "title": "Maximale Seiten pro Suchlauf"
                }
            }
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
                "declarative_browser_jobboard",
                "stepstone_search",
                "indeed_search"
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
    fn declarative_runtimes_require_system_profiles() {
        for adapter_key in [
            "declarative_endpoint_inventory",
            "declarative_sitemap_inventory",
            "declarative_browser_jobboard",
        ] {
            let adapter = get_adapter(adapter_key).unwrap();
            assert!(adapter.requires_system_profile);
            assert_eq!(
                adapter.execution_mode,
                AdapterExecutionMode::SourceInventory
            );
        }

        let stepstone = get_adapter("stepstone_search").unwrap();
        assert!(!stepstone.requires_system_profile);
        assert!(stepstone.requires_browser_profile);
    }

    #[test]
    fn adapter_metadata_serializes_camel_case_runtime_flags() {
        let value =
            serde_json::to_value(get_adapter("declarative_endpoint_inventory").unwrap()).unwrap();
        assert_eq!(value["requiresSystemProfile"], true);
        assert_eq!(value["requiresBrowserProfile"], false);
        assert_eq!(value["category"], "generic");
    }
}
