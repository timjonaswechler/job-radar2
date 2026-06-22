use serde::Serialize;
use serde_json::Value;

use super::*;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrySourceProfile {
    pub origin: SourceRegistryDocumentOrigin,
    pub path: String,
    pub document: SourceProfileDocument,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrySource {
    pub origin: SourceRegistryDocumentOrigin,
    pub path: String,
    pub document: SourceDocument,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRegistrySnapshot {
    pub valid_profiles: Vec<RegistrySourceProfile>,
    pub valid_sources: Vec<RegistrySource>,
    pub diagnostics: Vec<SourceRegistryDiagnostic>,
}

impl SourceRegistrySnapshot {
    pub fn profile(&self, key: &str) -> Option<&RegistrySourceProfile> {
        self.valid_profiles
            .iter()
            .find(|profile| profile.document.key == key)
    }

    pub fn source(&self, key: &str) -> Option<&RegistrySource> {
        self.valid_sources
            .iter()
            .find(|source| source.document.key == key)
    }

    pub fn resolve_source(&self, key: &str) -> Result<ResolvedSourceExecutionPlan, String> {
        let source = self.source(key).ok_or_else(|| {
            format!("sourceKey `{key}` was not found in the source registry snapshot")
        })?;

        self.resolve_registry_source(source)
    }

    fn resolve_registry_source(
        &self,
        source: &RegistrySource,
    ) -> Result<ResolvedSourceExecutionPlan, String> {
        match &source.document.selected_access_path {
            SelectedAccessPath::Profile {
                profile_key,
                path_key,
            } => {
                let profile = self.profile(profile_key).ok_or_else(|| {
                    format!(
                        "source `{}` references missing profile `{profile_key}`",
                        source.document.key
                    )
                })?;
                let access_path = profile
                    .document
                    .access_paths
                    .iter()
                    .find(|access_path| access_path.key == *path_key)
                    .ok_or_else(|| {
                        format!(
                            "source `{}` references missing path `{path_key}` on profile `{profile_key}`",
                            source.document.key
                        )
                    })?;

                Ok(ResolvedSourceExecutionPlan {
                    key: source.document.key.clone(),
                    name: source.document.name.clone(),
                    adapter_key: access_path.adapter_key.clone(),
                    source_config: source.document.source_config.clone(),
                    effective_source_config_schema: effective_source_config_schema(
                        profile.document.source_config_schema.as_ref(),
                        access_path.source_config_schema.as_ref(),
                    ),
                    selected_access_path: ResolvedSelectedAccessPath::Profile {
                        profile_key: profile_key.clone(),
                        path_key: path_key.clone(),
                        query: access_path.query.clone(),
                        inventory: access_path.inventory.clone(),
                        interactions: access_path.interactions.clone(),
                        manual_release: access_path.manual_release.clone(),
                    },
                })
            }
            SelectedAccessPath::SourceSpecific {
                adapter_key,
                source_config_schema,
                query,
                inventory,
                interactions,
                manual_release,
            } => Ok(ResolvedSourceExecutionPlan {
                key: source.document.key.clone(),
                name: source.document.name.clone(),
                adapter_key: adapter_key.clone(),
                source_config: source.document.source_config.clone(),
                effective_source_config_schema: effective_source_config_schema(
                    None,
                    source_config_schema.as_ref(),
                ),
                selected_access_path: ResolvedSelectedAccessPath::SourceSpecific {
                    query: query.clone(),
                    inventory: inventory.clone(),
                    interactions: interactions.clone(),
                    manual_release: manual_release.clone(),
                },
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedSourceExecutionPlan {
    pub key: String,
    pub name: String,
    pub adapter_key: String,
    pub source_config: Value,
    pub effective_source_config_schema: Value,
    pub selected_access_path: ResolvedSelectedAccessPath,
}

impl ResolvedSourceExecutionPlan {
    pub fn query(&self) -> Option<&Value> {
        self.selected_access_path.query()
    }

    pub fn inventory(&self) -> Option<&Value> {
        self.selected_access_path.inventory()
    }

    pub fn interactions(&self) -> Option<&[BrowserInteraction]> {
        self.selected_access_path.interactions()
    }

    pub fn manual_release(&self) -> Option<&Value> {
        self.selected_access_path.manual_release()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResolvedSelectedAccessPath {
    #[serde(rename_all = "camelCase")]
    Profile {
        profile_key: String,
        path_key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        query: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        inventory: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        interactions: Option<Vec<BrowserInteraction>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        manual_release: Option<Value>,
    },
    #[serde(rename_all = "camelCase")]
    SourceSpecific {
        #[serde(skip_serializing_if = "Option::is_none")]
        query: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        inventory: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        interactions: Option<Vec<BrowserInteraction>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        manual_release: Option<Value>,
    },
}

impl ResolvedSelectedAccessPath {
    fn query(&self) -> Option<&Value> {
        match self {
            Self::Profile { query, .. } | Self::SourceSpecific { query, .. } => query.as_ref(),
        }
    }

    fn inventory(&self) -> Option<&Value> {
        match self {
            Self::Profile { inventory, .. } | Self::SourceSpecific { inventory, .. } => {
                inventory.as_ref()
            }
        }
    }

    fn interactions(&self) -> Option<&[BrowserInteraction]> {
        match self {
            Self::Profile { interactions, .. } | Self::SourceSpecific { interactions, .. } => {
                interactions.as_deref()
            }
        }
    }

    fn manual_release(&self) -> Option<&Value> {
        match self {
            Self::Profile { manual_release, .. } | Self::SourceSpecific { manual_release, .. } => {
                manual_release.as_ref()
            }
        }
    }
}

fn effective_source_config_schema(
    profile_schema: Option<&Value>,
    path_schema: Option<&Value>,
) -> Value {
    match (profile_schema, path_schema) {
        (Some(profile_schema), Some(path_schema)) => serde_json::json!({
            "allOf": [profile_schema.clone(), path_schema.clone()]
        }),
        (Some(profile_schema), None) => profile_schema.clone(),
        (None, Some(path_schema)) => path_schema.clone(),
        (None, None) => serde_json::json!({ "type": "object" }),
    }
}
