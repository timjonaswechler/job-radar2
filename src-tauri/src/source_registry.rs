use serde::{de, Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};

pub type EmbeddedSourceRegistryDocument<'a> = (&'a str, &'a str);

pub const BUILTIN_SOURCE_PROFILE_JSON_FILES: &[EmbeddedSourceRegistryDocument<'static>] = &[
    (
        "source-profiles/builtin/ashby.json",
        include_str!("../../source-profiles/builtin/ashby.json"),
    ),
    (
        "source-profiles/builtin/greenhouse.json",
        include_str!("../../source-profiles/builtin/greenhouse.json"),
    ),
    (
        "source-profiles/builtin/lever.json",
        include_str!("../../source-profiles/builtin/lever.json"),
    ),
    (
        "source-profiles/builtin/magnolia_esmp_job_search.json",
        include_str!("../../source-profiles/builtin/magnolia_esmp_job_search.json"),
    ),
    (
        "source-profiles/builtin/muz_global_jobboard.json",
        include_str!("../../source-profiles/builtin/muz_global_jobboard.json"),
    ),
    (
        "source-profiles/builtin/personio.json",
        include_str!("../../source-profiles/builtin/personio.json"),
    ),
    (
        "source-profiles/builtin/phenom.json",
        include_str!("../../source-profiles/builtin/phenom.json"),
    ),
    (
        "source-profiles/builtin/stepstone_de.json",
        include_str!("../../source-profiles/builtin/stepstone_de.json"),
    ),
    (
        "source-profiles/builtin/successfactors.json",
        include_str!("../../source-profiles/builtin/successfactors.json"),
    ),
    (
        "source-profiles/builtin/workday.json",
        include_str!("../../source-profiles/builtin/workday.json"),
    ),
];
pub const BUILTIN_SOURCE_JSON_FILES: &[EmbeddedSourceRegistryDocument<'static>] = &[
    (
        "sources/builtin/indeed_de.json",
        include_str!("../../sources/builtin/indeed_de.json"),
    ),
    (
        "sources/builtin/stepstone_de.json",
        include_str!("../../sources/builtin/stepstone_de.json"),
    ),
];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRegistryDocumentOrigin {
    BuiltIn,
    Custom,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRegistryDocumentKind {
    SourceProfile,
    Source,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRegistryDiagnosticCode {
    InvalidJson,
    InvalidShape,
    FilenameKeyMismatch,
    DuplicateKey,
    MissingProfileRef,
    MissingPathRef,
    ReadError,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRegistryDiagnostic {
    pub code: SourceRegistryDiagnosticCode,
    pub document_kind: SourceRegistryDocumentKind,
    pub origin: SourceRegistryDocumentOrigin,
    pub path: String,
    pub key: Option<String>,
    pub message: String,
}

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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceProfileKind {
    RecruitingSystem,
    JobPortal,
    WebsiteFamily,
    Generic,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DetectionPhase {
    Http,
    Browser,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetectionBlock {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phases: Vec<DetectionPhase>,
    pub required: Vec<Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceProfileIdentity {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key_candidates: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub name_candidates: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional_source_config: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvailabilityBlock {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_captures: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_config: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum BrowserInteraction {
    #[serde(rename = "waitFor")]
    WaitFor {
        selector: String,
        #[serde(rename = "timeoutMs", skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    #[serde(rename = "clickIfVisible")]
    ClickIfVisible {
        selector: String,
        #[serde(rename = "timeoutMs", skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    #[serde(rename = "clickUpToN")]
    ClickUpToN {
        selector: String,
        #[serde(rename = "maxClicks")]
        max_clicks: u64,
        #[serde(rename = "waitAfterClickMs", skip_serializing_if = "Option::is_none")]
        wait_after_click_ms: Option<u64>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileAccessPathDefinition {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub adapter_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_config_schema: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability: Option<AvailabilityBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inventory: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interactions: Option<Vec<BrowserInteraction>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manual_release: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceProfileDocument {
    pub schema_version: u64,
    pub key: String,
    pub name: String,
    pub kind: SourceProfileKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detect: Option<DetectionBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<SourceProfileIdentity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_config_schema: Option<Value>,
    pub access_paths: Vec<ProfileAccessPathDefinition>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceDocumentStatus {
    Draft,
    Active,
    Disabled,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SelectedAccessPath {
    #[serde(rename_all = "camelCase")]
    Profile {
        profile_key: String,
        path_key: String,
    },
    #[serde(rename_all = "camelCase")]
    SourceSpecific {
        adapter_key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_config_schema: Option<Value>,
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

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDocument {
    pub schema_version: u64,
    pub key: String,
    pub name: String,
    pub status: SourceDocumentStatus,
    pub source_config: Value,
    pub selected_access_path: SelectedAccessPath,
}

impl<'de> Deserialize<'de> for SourceDocument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        parse_source_document(value).map_err(de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for SelectedAccessPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        parse_selected_access_path(value).map_err(de::Error::custom)
    }
}

#[derive(Clone, Debug)]
struct RawRegistryDocument {
    kind: SourceRegistryDocumentKind,
    origin: SourceRegistryDocumentOrigin,
    path: String,
    contents: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawSourceDocument {
    schema_version: u64,
    key: String,
    name: String,
    status: SourceDocumentStatus,
    source_config: Value,
    selected_access_path: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawProfileSelectedAccessPath {
    #[serde(rename = "type")]
    path_type: String,
    profile_key: String,
    path_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawSourceSpecificSelectedAccessPath {
    #[serde(rename = "type")]
    path_type: String,
    adapter_key: String,
    source_config_schema: Option<Value>,
    query: Option<Value>,
    inventory: Option<Value>,
    interactions: Option<Vec<BrowserInteraction>>,
    manual_release: Option<Value>,
}

pub fn load_snapshot(app_data_dir: impl AsRef<Path>) -> SourceRegistrySnapshot {
    load_snapshot_with_builtins(
        app_data_dir,
        BUILTIN_SOURCE_PROFILE_JSON_FILES,
        BUILTIN_SOURCE_JSON_FILES,
    )
}

pub fn load_snapshot_with_builtins(
    app_data_dir: impl AsRef<Path>,
    builtin_source_profiles: &[EmbeddedSourceRegistryDocument<'_>],
    builtin_sources: &[EmbeddedSourceRegistryDocument<'_>],
) -> SourceRegistrySnapshot {
    let app_data_dir = app_data_dir.as_ref();
    let mut diagnostics = Vec::new();

    let mut profile_documents = embedded_documents(
        SourceRegistryDocumentKind::SourceProfile,
        builtin_source_profiles,
    );
    profile_documents.extend(custom_documents(
        SourceRegistryDocumentKind::SourceProfile,
        app_data_dir.join("source-profiles"),
        &mut diagnostics,
    ));

    let mut source_documents =
        embedded_documents(SourceRegistryDocumentKind::Source, builtin_sources);
    source_documents.extend(custom_documents(
        SourceRegistryDocumentKind::Source,
        app_data_dir.join("sources"),
        &mut diagnostics,
    ));

    let valid_profiles = load_profile_documents(profile_documents, &mut diagnostics);
    let candidate_sources = load_source_documents(source_documents, &mut diagnostics);
    let valid_sources =
        validate_source_references(candidate_sources, &valid_profiles, &mut diagnostics);

    SourceRegistrySnapshot {
        valid_profiles,
        valid_sources,
        diagnostics,
    }
}

fn embedded_documents(
    kind: SourceRegistryDocumentKind,
    documents: &[EmbeddedSourceRegistryDocument<'_>],
) -> Vec<RawRegistryDocument> {
    documents
        .iter()
        .map(|(path, contents)| RawRegistryDocument {
            kind,
            origin: SourceRegistryDocumentOrigin::BuiltIn,
            path: (*path).to_string(),
            contents: (*contents).to_string(),
        })
        .collect()
}

fn custom_documents(
    kind: SourceRegistryDocumentKind,
    directory: PathBuf,
    diagnostics: &mut Vec<SourceRegistryDiagnostic>,
) -> Vec<RawRegistryDocument> {
    let mut paths = Vec::new();
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            diagnostics.push(SourceRegistryDiagnostic {
                code: SourceRegistryDiagnosticCode::ReadError,
                document_kind: kind,
                origin: SourceRegistryDocumentOrigin::Custom,
                path: directory.display().to_string(),
                key: None,
                message: format!("could not read registry directory: {error}"),
            });
            return Vec::new();
        }
    };

    for entry in entries {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if path.is_file() && is_json_file(&path) {
                    paths.push(path);
                }
            }
            Err(error) => diagnostics.push(SourceRegistryDiagnostic {
                code: SourceRegistryDiagnosticCode::ReadError,
                document_kind: kind,
                origin: SourceRegistryDocumentOrigin::Custom,
                path: directory.display().to_string(),
                key: None,
                message: format!("could not read registry directory entry: {error}"),
            }),
        }
    }

    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| {
            let path_label = path.display().to_string();
            match fs::read_to_string(&path) {
                Ok(contents) => Some(RawRegistryDocument {
                    kind,
                    origin: SourceRegistryDocumentOrigin::Custom,
                    path: path_label,
                    contents,
                }),
                Err(error) => {
                    diagnostics.push(SourceRegistryDiagnostic {
                        code: SourceRegistryDiagnosticCode::ReadError,
                        document_kind: kind,
                        origin: SourceRegistryDocumentOrigin::Custom,
                        path: path_label,
                        key: None,
                        message: format!("could not read registry document: {error}"),
                    });
                    None
                }
            }
        })
        .collect()
}

fn is_json_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some(extension) if extension.eq_ignore_ascii_case("json")
    )
}

fn load_profile_documents(
    documents: Vec<RawRegistryDocument>,
    diagnostics: &mut Vec<SourceRegistryDiagnostic>,
) -> Vec<RegistrySourceProfile> {
    let mut valid_profiles = Vec::new();
    let mut seen_keys = HashMap::<String, (SourceRegistryDocumentOrigin, String)>::new();

    for document in documents {
        let Some(parsed) = parse_registry_document(&document, parse_profile_document, diagnostics)
        else {
            continue;
        };
        if !filename_matches_key(&document.path, &parsed.key) {
            diagnostics.push(filename_key_mismatch_diagnostic(&document, &parsed.key));
            continue;
        }
        if let Some((first_origin, first_path)) = seen_keys.get(&parsed.key) {
            diagnostics.push(duplicate_key_diagnostic(
                &document,
                &parsed.key,
                *first_origin,
                first_path,
            ));
            continue;
        }

        seen_keys.insert(parsed.key.clone(), (document.origin, document.path.clone()));
        valid_profiles.push(RegistrySourceProfile {
            origin: document.origin,
            path: document.path,
            document: parsed,
        });
    }

    valid_profiles
}

fn load_source_documents(
    documents: Vec<RawRegistryDocument>,
    diagnostics: &mut Vec<SourceRegistryDiagnostic>,
) -> Vec<RegistrySource> {
    let mut valid_sources = Vec::new();
    let mut seen_keys = HashMap::<String, (SourceRegistryDocumentOrigin, String)>::new();

    for document in documents {
        let Some(parsed) = parse_registry_document(&document, parse_source_document, diagnostics)
        else {
            continue;
        };
        if !filename_matches_key(&document.path, &parsed.key) {
            diagnostics.push(filename_key_mismatch_diagnostic(&document, &parsed.key));
            continue;
        }
        if let Some((first_origin, first_path)) = seen_keys.get(&parsed.key) {
            diagnostics.push(duplicate_key_diagnostic(
                &document,
                &parsed.key,
                *first_origin,
                first_path,
            ));
            continue;
        }

        seen_keys.insert(parsed.key.clone(), (document.origin, document.path.clone()));
        valid_sources.push(RegistrySource {
            origin: document.origin,
            path: document.path,
            document: parsed,
        });
    }

    valid_sources
}

fn parse_registry_document<T>(
    document: &RawRegistryDocument,
    parse: impl FnOnce(Value) -> Result<T, String>,
    diagnostics: &mut Vec<SourceRegistryDiagnostic>,
) -> Option<T> {
    let value = match serde_json::from_str::<Value>(&document.contents) {
        Ok(value) => value,
        Err(error) => {
            diagnostics.push(SourceRegistryDiagnostic {
                code: SourceRegistryDiagnosticCode::InvalidJson,
                document_kind: document.kind,
                origin: document.origin,
                path: document.path.clone(),
                key: None,
                message: format!("invalid JSON: {error}"),
            });
            return None;
        }
    };

    match parse(value) {
        Ok(parsed) => Some(parsed),
        Err(error) => {
            diagnostics.push(SourceRegistryDiagnostic {
                code: SourceRegistryDiagnosticCode::InvalidShape,
                document_kind: document.kind,
                origin: document.origin,
                path: document.path.clone(),
                key: None,
                message: error,
            });
            None
        }
    }
}

fn parse_profile_document(value: Value) -> Result<SourceProfileDocument, String> {
    let document = serde_json::from_value::<SourceProfileDocument>(value)
        .map_err(|error| format!("source profile document shape is invalid: {error}"))?;

    validate_schema_version(document.schema_version)?;
    validate_technical_key("key", &document.key)?;
    validate_required_text("name", &document.name)?;
    validate_json_object_option(document.source_config_schema.as_ref(), "sourceConfigSchema")?;
    if let Some(identity) = &document.identity {
        validate_profile_identity(identity)?;
    }
    if document.access_paths.is_empty() {
        return Err("accessPaths must contain at least one access path".to_string());
    }

    let mut access_path_keys = HashSet::new();
    for (index, access_path) in document.access_paths.iter().enumerate() {
        let path = format!("accessPaths[{index}]");
        validate_technical_key(&format!("{path}.key"), &access_path.key)?;
        if !access_path_keys.insert(access_path.key.clone()) {
            return Err(format!(
                "accessPaths contains duplicate key `{}`",
                access_path.key
            ));
        }
        if let Some(name) = &access_path.name {
            validate_required_text(&format!("{path}.name"), name)?;
        }
        validate_technical_key(&format!("{path}.adapterKey"), &access_path.adapter_key)?;
        validate_access_path_json_blocks(access_path, &path)?;
        if let Some(availability) = &access_path.availability {
            validate_availability_block(availability, &format!("{path}.availability"))?;
        }
    }

    Ok(document)
}

fn validate_profile_identity(identity: &SourceProfileIdentity) -> Result<(), String> {
    validate_non_empty_strings(&identity.key_candidates, "identity.keyCandidates")?;
    validate_non_empty_strings(&identity.name_candidates, "identity.nameCandidates")?;
    validate_json_object_option(
        identity.optional_source_config.as_ref(),
        "identity.optionalSourceConfig",
    )
}

fn validate_availability_block(availability: &AvailabilityBlock, path: &str) -> Result<(), String> {
    for (index, capture_key) in availability.required_captures.iter().enumerate() {
        validate_capture_key(&format!("{path}.requiredCaptures[{index}]"), capture_key)?;
    }
    validate_json_object_option(
        availability.source_config.as_ref(),
        &format!("{path}.sourceConfig"),
    )
}

fn validate_access_path_json_blocks(
    access_path: &ProfileAccessPathDefinition,
    path: &str,
) -> Result<(), String> {
    validate_json_object_option(
        access_path.source_config_schema.as_ref(),
        &format!("{path}.sourceConfigSchema"),
    )?;
    validate_json_object_option(access_path.query.as_ref(), &format!("{path}.query"))?;
    validate_json_object_option(access_path.inventory.as_ref(), &format!("{path}.inventory"))?;
    validate_json_object_option(
        access_path.manual_release.as_ref(),
        &format!("{path}.manualRelease"),
    )
}

fn parse_source_document(value: Value) -> Result<SourceDocument, String> {
    let raw = serde_json::from_value::<RawSourceDocument>(value)
        .map_err(|error| format!("source document shape is invalid: {error}"))?;

    validate_schema_version(raw.schema_version)?;
    validate_technical_key("key", &raw.key)?;
    validate_required_text("name", &raw.name)?;
    validate_json_object(&raw.source_config, "sourceConfig")?;
    let selected_access_path = parse_selected_access_path(raw.selected_access_path)?;

    Ok(SourceDocument {
        schema_version: raw.schema_version,
        key: raw.key,
        name: raw.name,
        status: raw.status,
        source_config: raw.source_config,
        selected_access_path,
    })
}

fn parse_selected_access_path(value: Value) -> Result<SelectedAccessPath, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "selectedAccessPath must be a JSON object".to_string())?;
    let Some(path_type) = object.get("type").and_then(Value::as_str) else {
        return Err("selectedAccessPath.type must be profile or source_specific".to_string());
    };

    match path_type {
        "profile" => {
            let raw = serde_json::from_value::<RawProfileSelectedAccessPath>(value)
                .map_err(|error| format!("selectedAccessPath profile shape is invalid: {error}"))?;
            if raw.path_type != "profile" {
                return Err("selectedAccessPath.type must be profile".to_string());
            }
            validate_technical_key("selectedAccessPath.profileKey", &raw.profile_key)?;
            validate_technical_key("selectedAccessPath.pathKey", &raw.path_key)?;
            Ok(SelectedAccessPath::Profile {
                profile_key: raw.profile_key,
                path_key: raw.path_key,
            })
        }
        "source_specific" => {
            if object.contains_key("availability") {
                return Err(
                    "selectedAccessPath.availability is not allowed for source_specific access paths"
                        .to_string(),
                );
            }
            let raw = serde_json::from_value::<RawSourceSpecificSelectedAccessPath>(value)
                .map_err(|error| {
                    format!("selectedAccessPath source_specific shape is invalid: {error}")
                })?;
            if raw.path_type != "source_specific" {
                return Err("selectedAccessPath.type must be source_specific".to_string());
            }
            validate_technical_key("selectedAccessPath.adapterKey", &raw.adapter_key)?;
            validate_json_object_option(
                raw.source_config_schema.as_ref(),
                "selectedAccessPath.sourceConfigSchema",
            )?;
            validate_json_object_option(raw.query.as_ref(), "selectedAccessPath.query")?;
            validate_json_object_option(raw.inventory.as_ref(), "selectedAccessPath.inventory")?;
            validate_json_object_option(
                raw.manual_release.as_ref(),
                "selectedAccessPath.manualRelease",
            )?;
            Ok(SelectedAccessPath::SourceSpecific {
                adapter_key: raw.adapter_key,
                source_config_schema: raw.source_config_schema,
                query: raw.query,
                inventory: raw.inventory,
                interactions: raw.interactions,
                manual_release: raw.manual_release,
            })
        }
        _ => Err(format!(
            "selectedAccessPath.type `{path_type}` must be profile or source_specific"
        )),
    }
}

fn validate_source_references(
    sources: Vec<RegistrySource>,
    profiles: &[RegistrySourceProfile],
    diagnostics: &mut Vec<SourceRegistryDiagnostic>,
) -> Vec<RegistrySource> {
    let profiles_by_key = profiles
        .iter()
        .map(|profile| (profile.document.key.as_str(), profile))
        .collect::<HashMap<_, _>>();
    let mut valid_sources = Vec::with_capacity(sources.len());

    for source in sources {
        let SelectedAccessPath::Profile {
            profile_key,
            path_key,
        } = &source.document.selected_access_path
        else {
            valid_sources.push(source);
            continue;
        };

        let Some(profile) = profiles_by_key.get(profile_key.as_str()) else {
            diagnostics.push(SourceRegistryDiagnostic {
                code: SourceRegistryDiagnosticCode::MissingProfileRef,
                document_kind: SourceRegistryDocumentKind::Source,
                origin: source.origin,
                path: source.path.clone(),
                key: Some(source.document.key.clone()),
                message: format!(
                    "source `{}` references missing profile `{profile_key}`",
                    source.document.key
                ),
            });
            continue;
        };

        if !profile
            .document
            .access_paths
            .iter()
            .any(|access_path| access_path.key == *path_key)
        {
            diagnostics.push(SourceRegistryDiagnostic {
                code: SourceRegistryDiagnosticCode::MissingPathRef,
                document_kind: SourceRegistryDocumentKind::Source,
                origin: source.origin,
                path: source.path.clone(),
                key: Some(source.document.key.clone()),
                message: format!(
                    "source `{}` references missing path `{path_key}` on profile `{profile_key}`",
                    source.document.key
                ),
            });
            continue;
        }

        valid_sources.push(source);
    }

    valid_sources
}

fn validate_schema_version(schema_version: u64) -> Result<(), String> {
    if schema_version == 1 {
        Ok(())
    } else {
        Err("schemaVersion must be 1".to_string())
    }
}

fn validate_required_text(path: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{path} must be a non-empty string"))
    } else {
        Ok(())
    }
}

fn validate_non_empty_strings(values: &[String], path: &str) -> Result<(), String> {
    for (index, value) in values.iter().enumerate() {
        validate_required_text(&format!("{path}[{index}]"), value)?;
    }
    Ok(())
}

fn validate_technical_key(path: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{path} must be a non-empty technical key"));
    }

    if value.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
    }) {
        Ok(())
    } else {
        Err(format!("{path} must match ^[a-z0-9_]+$"))
    }
}

fn validate_capture_key(path: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{path} must be a non-empty capture key"));
    }

    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        Ok(())
    } else {
        Err(format!("{path} must match ^[A-Za-z0-9_]+$"))
    }
}

fn validate_json_object_option(value: Option<&Value>, path: &str) -> Result<(), String> {
    match value {
        Some(value) => validate_json_object(value, path),
        None => Ok(()),
    }
}

fn validate_json_object(value: &Value, path: &str) -> Result<(), String> {
    if value.is_object() {
        Ok(())
    } else {
        Err(format!("{path} must be a JSON object"))
    }
}

fn filename_matches_key(path: &str, key: &str) -> bool {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem == key)
}

fn filename_key_mismatch_diagnostic(
    document: &RawRegistryDocument,
    key: &str,
) -> SourceRegistryDiagnostic {
    let filename = Path::new(&document.path)
        .file_name()
        .and_then(|filename| filename.to_str())
        .unwrap_or(document.path.as_str());
    SourceRegistryDiagnostic {
        code: SourceRegistryDiagnosticCode::FilenameKeyMismatch,
        document_kind: document.kind,
        origin: document.origin,
        path: document.path.clone(),
        key: Some(key.to_string()),
        message: format!("filename `{filename}` must be `{key}.json`"),
    }
}

fn duplicate_key_diagnostic(
    document: &RawRegistryDocument,
    key: &str,
    first_origin: SourceRegistryDocumentOrigin,
    first_path: &str,
) -> SourceRegistryDiagnostic {
    SourceRegistryDiagnostic {
        code: SourceRegistryDiagnosticCode::DuplicateKey,
        document_kind: document.kind,
        origin: document.origin,
        path: document.path.clone(),
        key: Some(key.to_string()),
        message: format!(
            "key `{key}` duplicates existing {} document `{first_path}`; duplicate ignored",
            origin_label(first_origin)
        ),
    }
}

fn origin_label(origin: SourceRegistryDocumentOrigin) -> &'static str {
    match origin {
        SourceRegistryDocumentOrigin::BuiltIn => "built-in",
        SourceRegistryDocumentOrigin::Custom => "custom",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn loads_migrated_builtin_source_registry_documents() {
        let temp_dir = tempfile::tempdir().unwrap();

        let snapshot = load_snapshot(temp_dir.path());

        assert!(
            snapshot.diagnostics.is_empty(),
            "built-in registry diagnostics: {:#?}",
            snapshot.diagnostics
        );
        assert_eq!(
            sorted_profile_keys(&snapshot),
            vec![
                "ashby",
                "greenhouse",
                "lever",
                "magnolia_esmp_job_search",
                "muz_global_jobboard",
                "personio",
                "phenom",
                "stepstone_de",
                "successfactors",
                "workday",
            ]
        );
        assert_eq!(
            sorted_source_keys(&snapshot),
            vec!["indeed_de", "stepstone_de"]
        );

        let stepstone = snapshot.source("stepstone_de").unwrap();
        assert_eq!(stepstone.origin, SourceRegistryDocumentOrigin::BuiltIn);
        assert_eq!(stepstone.document.status, SourceDocumentStatus::Active);
        assert!(matches!(
            &stepstone.document.selected_access_path,
            SelectedAccessPath::Profile { profile_key, path_key }
                if profile_key == "stepstone_de" && path_key == "browser_inventory"
        ));

        let indeed = snapshot.source("indeed_de").unwrap();
        assert_eq!(indeed.origin, SourceRegistryDocumentOrigin::BuiltIn);
        assert_eq!(indeed.document.status, SourceDocumentStatus::Active);
        assert!(matches!(
            &indeed.document.selected_access_path,
            SelectedAccessPath::SourceSpecific { adapter_key, .. } if adapter_key == "indeed_search"
        ));

        let stepstone_profile = snapshot.profile("stepstone_de").unwrap();
        assert_eq!(
            stepstone_profile.document.kind,
            SourceProfileKind::JobPortal
        );
        assert!(stepstone_profile
            .document
            .access_paths
            .iter()
            .any(|path| path.key == "browser_inventory"
                && path.adapter_key == "declarative_browser_inventory"));

        let greenhouse = snapshot.profile("greenhouse").unwrap();
        assert!(greenhouse
            .document
            .access_paths
            .iter()
            .any(|path| path.key == "endpoint_inventory"
                && path.adapter_key == "declarative_endpoint_inventory"));
    }

    #[test]
    fn loads_valid_profile_backed_and_source_specific_documents() {
        let temp_dir = tempfile::tempdir().unwrap();
        write_json(
            temp_dir.path().join("source-profiles/greenhouse.json"),
            &profile_json("greenhouse", &["boards_api"]),
        );
        write_json(
            temp_dir.path().join("sources/helsing.json"),
            &profile_source_json("helsing", "greenhouse", "boards_api"),
        );
        write_json(
            temp_dir.path().join("sources/example_company.json"),
            &source_specific_source_json("example_company"),
        );

        let snapshot = load_custom_only_snapshot(temp_dir.path());

        assert!(snapshot.diagnostics.is_empty());
        assert_eq!(snapshot.valid_profiles.len(), 1);
        assert_eq!(snapshot.valid_profiles[0].document.key, "greenhouse");
        assert_eq!(
            snapshot.valid_profiles[0].document.kind,
            SourceProfileKind::RecruitingSystem
        );
        assert_eq!(
            snapshot.valid_profiles[0].document.access_paths[0].key,
            "boards_api"
        );
        assert_eq!(snapshot.valid_sources.len(), 2);
        assert_eq!(snapshot.valid_sources[0].document.key, "example_company");
        assert!(matches!(
            snapshot.valid_sources[0].document.selected_access_path,
            SelectedAccessPath::SourceSpecific { .. }
        ));
        assert_eq!(snapshot.valid_sources[1].document.key, "helsing");
        assert!(matches!(
            snapshot.valid_sources[1].document.selected_access_path,
            SelectedAccessPath::Profile { .. }
        ));
    }

    #[test]
    fn reports_invalid_json_invalid_shape_and_does_not_create_missing_directories() {
        let temp_dir = tempfile::tempdir().unwrap();
        let app_data_dir = temp_dir.path().join("app-data");
        let invalid_json_path = app_data_dir.join("source-profiles/broken.json");
        let missing_kind_path = app_data_dir.join("source-profiles/missing_kind.json");
        write_raw(&invalid_json_path, "{not json");
        write_json(
            &missing_kind_path,
            &json!({
                "schemaVersion": 1,
                "key": "missing_kind",
                "name": "Missing Kind",
                "accessPaths": [{ "key": "api", "adapterKey": "declarative_endpoint_inventory" }]
            })
            .to_string(),
        );

        let snapshot = load_custom_only_snapshot(&app_data_dir);

        assert_eq!(snapshot.valid_profiles.len(), 0);
        assert_diagnostic_codes(
            &snapshot,
            &[
                SourceRegistryDiagnosticCode::InvalidJson,
                SourceRegistryDiagnosticCode::InvalidShape,
            ],
        );
        assert_eq!(
            std::fs::read_to_string(&invalid_json_path).unwrap(),
            "{not json"
        );

        let missing_app_data_dir = temp_dir.path().join("does-not-exist");
        let empty_snapshot = load_custom_only_snapshot(&missing_app_data_dir);
        assert!(empty_snapshot.diagnostics.is_empty());
        assert!(!missing_app_data_dir.exists());
    }

    #[test]
    fn reports_filename_key_mismatch_and_builtin_duplicate_keys() {
        let temp_dir = tempfile::tempdir().unwrap();
        write_json(
            temp_dir.path().join("source-profiles/greenhouse.json"),
            &profile_json("greenhouse", &["custom_api"]),
        );
        write_json(
            temp_dir.path().join("source-profiles/wrong_name.json"),
            &profile_json("right_name", &["api"]),
        );
        write_json(
            temp_dir.path().join("sources/stepstone_de.json"),
            &source_specific_source_json("stepstone_de"),
        );

        let snapshot = load_snapshot_with_builtins(
            temp_dir.path(),
            &[(
                "source-profiles/builtin/greenhouse.json",
                &profile_json("greenhouse", &["boards_api"]),
            )],
            &[(
                "sources/builtin/stepstone_de.json",
                &source_specific_source_json("stepstone_de"),
            )],
        );

        assert_eq!(snapshot.valid_profiles.len(), 1);
        assert_eq!(
            snapshot.valid_profiles[0].origin,
            SourceRegistryDocumentOrigin::BuiltIn
        );
        assert_eq!(
            snapshot.valid_profiles[0].document.access_paths[0].key,
            "boards_api"
        );
        assert_eq!(snapshot.valid_sources.len(), 1);
        assert_eq!(
            snapshot.valid_sources[0].origin,
            SourceRegistryDocumentOrigin::BuiltIn
        );
        assert_diagnostic_codes(
            &snapshot,
            &[
                SourceRegistryDiagnosticCode::DuplicateKey,
                SourceRegistryDiagnosticCode::FilenameKeyMismatch,
                SourceRegistryDiagnosticCode::DuplicateKey,
            ],
        );
        assert!(snapshot.diagnostics.iter().any(|diagnostic| diagnostic.code
            == SourceRegistryDiagnosticCode::DuplicateKey
            && diagnostic.document_kind == SourceRegistryDocumentKind::SourceProfile
            && diagnostic.key.as_deref() == Some("greenhouse")));
        assert!(snapshot.diagnostics.iter().any(|diagnostic| diagnostic.code
            == SourceRegistryDiagnosticCode::DuplicateKey
            && diagnostic.document_kind == SourceRegistryDocumentKind::Source
            && diagnostic.key.as_deref() == Some("stepstone_de")));
    }

    #[test]
    fn reports_missing_profile_and_missing_path_references() {
        let temp_dir = tempfile::tempdir().unwrap();
        write_json(
            temp_dir.path().join("source-profiles/greenhouse.json"),
            &profile_json("greenhouse", &["boards_api"]),
        );
        write_json(
            temp_dir.path().join("sources/missing_profile_source.json"),
            &profile_source_json("missing_profile_source", "unknown_profile", "boards_api"),
        );
        write_json(
            temp_dir.path().join("sources/missing_path_source.json"),
            &profile_source_json("missing_path_source", "greenhouse", "unknown_path"),
        );

        let snapshot = load_custom_only_snapshot(temp_dir.path());

        assert_eq!(snapshot.valid_sources.len(), 0);
        assert_diagnostic_codes(
            &snapshot,
            &[
                SourceRegistryDiagnosticCode::MissingPathRef,
                SourceRegistryDiagnosticCode::MissingProfileRef,
            ],
        );
        assert!(snapshot
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.key.as_deref() == Some("missing_profile_source")));
        assert!(snapshot
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.key.as_deref() == Some("missing_path_source")));
    }

    #[test]
    fn reports_invalid_selected_access_path_variants_and_source_specific_availability() {
        let temp_dir = tempfile::tempdir().unwrap();
        write_json(
            temp_dir.path().join("sources/invalid_variant.json"),
            &json!({
                "schemaVersion": 1,
                "key": "invalid_variant",
                "name": "Invalid Variant",
                "status": "draft",
                "sourceConfig": {},
                "selectedAccessPath": { "type": "browser", "adapterKey": "declarative_browser_inventory" }
            })
            .to_string(),
        );
        write_json(
            temp_dir
                .path()
                .join("sources/source_specific_with_availability.json"),
            &json!({
                "schemaVersion": 1,
                "key": "source_specific_with_availability",
                "name": "Source Specific With Availability",
                "status": "draft",
                "sourceConfig": {},
                "selectedAccessPath": {
                    "type": "source_specific",
                    "adapterKey": "declarative_browser_inventory",
                    "availability": { "requiredCaptures": [] }
                }
            })
            .to_string(),
        );

        let snapshot = load_custom_only_snapshot(temp_dir.path());

        assert!(snapshot.valid_sources.is_empty());
        assert_diagnostic_codes(
            &snapshot,
            &[
                SourceRegistryDiagnosticCode::InvalidShape,
                SourceRegistryDiagnosticCode::InvalidShape,
            ],
        );
        assert!(snapshot.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("must be profile or source_specific")));
        assert!(snapshot
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("availability is not allowed")));
    }

    #[test]
    fn reports_profile_access_paths_with_duplicate_keys_as_invalid_shape() {
        let temp_dir = tempfile::tempdir().unwrap();
        write_json(
            temp_dir.path().join("source-profiles/greenhouse.json"),
            &profile_json("greenhouse", &["boards_api", "boards_api"]),
        );

        let snapshot = load_custom_only_snapshot(temp_dir.path());

        assert!(snapshot.valid_profiles.is_empty());
        assert_diagnostic_codes(&snapshot, &[SourceRegistryDiagnosticCode::InvalidShape]);
        assert!(snapshot.diagnostics[0]
            .message
            .contains("accessPaths contains duplicate key `boards_api`"));
    }

    #[test]
    fn resolves_profile_backed_execution_plan_with_access_path_definition() {
        let temp_dir = tempfile::tempdir().unwrap();
        write_json(
            temp_dir.path().join("source-profiles/example_profile.json"),
            &profile_with_execution_plan_json(),
        );
        write_json(
            temp_dir.path().join("sources/example_source.json"),
            &json!({
                "schemaVersion": 1,
                "key": "example_source",
                "name": "Example Source",
                "status": "active",
                "sourceConfig": {
                    "tenant": "acme",
                    "startUrl": "https://example.test/jobs"
                },
                "selectedAccessPath": {
                    "type": "profile",
                    "profileKey": "example_profile",
                    "pathKey": "endpoint_inventory"
                }
            })
            .to_string(),
        );

        let snapshot = load_custom_only_snapshot(temp_dir.path());

        let plan = snapshot.resolve_source("example_source").unwrap();
        assert_eq!(plan.key, "example_source");
        assert_eq!(plan.name, "Example Source");
        assert_eq!(plan.adapter_key, "declarative_endpoint_inventory");
        assert_eq!(
            plan.source_config,
            json!({
                "tenant": "acme",
                "startUrl": "https://example.test/jobs"
            })
        );
        assert_eq!(
            plan.effective_source_config_schema,
            json!({
                "allOf": [
                    {
                        "type": "object",
                        "required": ["tenant"],
                        "properties": {
                            "tenant": { "type": "string" }
                        }
                    },
                    {
                        "type": "object",
                        "required": ["startUrl"],
                        "properties": {
                            "startUrl": { "type": "string", "format": "uri" }
                        }
                    }
                ]
            })
        );
        assert_eq!(
            plan.inventory(),
            Some(&json!({
                "fetch": { "url": "{{sourceConfig:startUrl}}" },
                "parse": { "as": "json" },
                "items": { "select": { "jsonPath": "$.jobs" } },
                "fields": {
                    "title": { "jsonPath": "$.title" },
                    "url": { "jsonPath": "$.url" },
                    "company": { "template": "{{sourceName}}" },
                    "locations": []
                }
            }))
        );
        assert_eq!(
            plan.query(),
            Some(&json!({
                "baseUrl": "{{sourceConfig:startUrl}}",
                "path": "/jobs",
                "params": []
            }))
        );
        assert!(matches!(
            &plan.selected_access_path,
            ResolvedSelectedAccessPath::Profile { profile_key, path_key, .. }
                if profile_key == "example_profile" && path_key == "endpoint_inventory"
        ));
    }

    #[test]
    fn resolves_source_specific_execution_plan_from_inline_selected_access_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        write_json(
            temp_dir.path().join("sources/example_company.json"),
            &json!({
                "schemaVersion": 1,
                "key": "example_company",
                "name": "Example Company",
                "status": "active",
                "sourceConfig": { "startUrl": "https://example.test/jobs" },
                "selectedAccessPath": {
                    "type": "source_specific",
                    "adapterKey": "declarative_browser_inventory",
                    "sourceConfigSchema": {
                        "type": "object",
                        "required": ["startUrl"],
                        "properties": {
                            "startUrl": { "type": "string", "format": "uri" }
                        }
                    },
                    "query": {
                        "baseUrl": "{{sourceConfig:startUrl}}",
                        "path": "/jobs",
                        "params": []
                    },
                    "interactions": [
                        { "type": "waitFor", "selector": ".job-card", "timeoutMs": 5000 }
                    ],
                    "inventory": {
                        "items": { "select": ".job-card" },
                        "fields": {
                            "title": { "selectorText": ".title" },
                            "company": { "selectorText": ".company" },
                            "url": {
                                "selectorAttribute": { "selector": "a", "attribute": "href" }
                            },
                            "locations": []
                        }
                    }
                }
            })
            .to_string(),
        );

        let snapshot = load_custom_only_snapshot(temp_dir.path());

        let plan = snapshot.resolve_source("example_company").unwrap();
        assert_eq!(plan.key, "example_company");
        assert_eq!(plan.adapter_key, "declarative_browser_inventory");
        assert_eq!(
            plan.effective_source_config_schema,
            json!({
                "type": "object",
                "required": ["startUrl"],
                "properties": {
                    "startUrl": { "type": "string", "format": "uri" }
                }
            })
        );
        assert_eq!(
            plan.query(),
            Some(&json!({
                "baseUrl": "{{sourceConfig:startUrl}}",
                "path": "/jobs",
                "params": []
            }))
        );
        assert_eq!(
            plan.inventory(),
            Some(&json!({
                "items": { "select": ".job-card" },
                "fields": {
                    "title": { "selectorText": ".title" },
                    "company": { "selectorText": ".company" },
                    "url": {
                        "selectorAttribute": { "selector": "a", "attribute": "href" }
                    },
                    "locations": []
                }
            }))
        );
        assert!(matches!(
            &plan.selected_access_path,
            ResolvedSelectedAccessPath::SourceSpecific { interactions, .. }
                if interactions.as_ref().is_some_and(|interactions| interactions.len() == 1)
        ));
    }

    fn load_custom_only_snapshot(app_data_dir: impl AsRef<Path>) -> SourceRegistrySnapshot {
        load_snapshot_with_builtins(app_data_dir, &[], &[])
    }

    fn sorted_profile_keys(snapshot: &SourceRegistrySnapshot) -> Vec<&str> {
        let mut keys = snapshot
            .valid_profiles
            .iter()
            .map(|profile| profile.document.key.as_str())
            .collect::<Vec<_>>();
        keys.sort_unstable();
        keys
    }

    fn sorted_source_keys(snapshot: &SourceRegistrySnapshot) -> Vec<&str> {
        let mut keys = snapshot
            .valid_sources
            .iter()
            .map(|source| source.document.key.as_str())
            .collect::<Vec<_>>();
        keys.sort_unstable();
        keys
    }

    fn profile_json(key: &str, access_path_keys: &[&str]) -> String {
        json!({
            "schemaVersion": 1,
            "key": key,
            "name": title_from_key(key),
            "kind": "recruiting_system",
            "detect": { "phases": ["http"], "required": [] },
            "identity": {
                "keyCandidates": ["{{capture:boardSlug|technicalKey}}"],
                "nameCandidates": ["{{capture:boardSlug|titleCase}}"]
            },
            "sourceConfigSchema": { "type": "object" },
            "accessPaths": access_path_keys.iter().map(|path_key| json!({
                "key": path_key,
                "adapterKey": "declarative_endpoint_inventory",
                "availability": {
                    "requiredCaptures": ["boardSlug"],
                    "checks": [],
                    "sourceConfig": { "boardSlug": "{{capture:boardSlug}}" }
                },
                "sourceConfigSchema": { "type": "object" },
                "inventory": {}
            })).collect::<Vec<_>>()
        })
        .to_string()
    }

    fn profile_source_json(key: &str, profile_key: &str, path_key: &str) -> String {
        json!({
            "schemaVersion": 1,
            "key": key,
            "name": title_from_key(key),
            "status": "draft",
            "sourceConfig": { "boardSlug": key },
            "selectedAccessPath": {
                "type": "profile",
                "profileKey": profile_key,
                "pathKey": path_key
            }
        })
        .to_string()
    }

    fn profile_with_execution_plan_json() -> String {
        json!({
            "schemaVersion": 1,
            "key": "example_profile",
            "name": "Example Profile",
            "kind": "recruiting_system",
            "sourceConfigSchema": {
                "type": "object",
                "required": ["tenant"],
                "properties": {
                    "tenant": { "type": "string" }
                }
            },
            "accessPaths": [
                {
                    "key": "endpoint_inventory",
                    "adapterKey": "declarative_endpoint_inventory",
                    "sourceConfigSchema": {
                        "type": "object",
                        "required": ["startUrl"],
                        "properties": {
                            "startUrl": { "type": "string", "format": "uri" }
                        }
                    },
                    "query": {
                        "baseUrl": "{{sourceConfig:startUrl}}",
                        "path": "/jobs",
                        "params": []
                    },
                    "inventory": {
                        "fetch": { "url": "{{sourceConfig:startUrl}}" },
                        "parse": { "as": "json" },
                        "items": { "select": { "jsonPath": "$.jobs" } },
                        "fields": {
                            "title": { "jsonPath": "$.title" },
                            "url": { "jsonPath": "$.url" },
                            "company": { "template": "{{sourceName}}" },
                            "locations": []
                        }
                    }
                }
            ]
        })
        .to_string()
    }

    fn source_specific_source_json(key: &str) -> String {
        json!({
            "schemaVersion": 1,
            "key": key,
            "name": title_from_key(key),
            "status": "draft",
            "sourceConfig": { "startUrl": "https://example.com/jobs" },
            "selectedAccessPath": {
                "type": "source_specific",
                "adapterKey": "declarative_browser_inventory",
                "sourceConfigSchema": { "type": "object" },
                "interactions": [
                    { "type": "waitFor", "selector": ".job-card", "timeoutMs": 1000 }
                ],
                "inventory": {}
            }
        })
        .to_string()
    }

    fn title_from_key(key: &str) -> String {
        key.split('_')
            .map(|part| {
                let mut characters = part.chars();
                match characters.next() {
                    Some(first) => format!("{}{}", first.to_ascii_uppercase(), characters.as_str()),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn write_json(path: impl AsRef<Path>, contents: &str) {
        write_raw(path, contents);
    }

    fn write_raw(path: impl AsRef<Path>, contents: &str) {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }

    fn assert_diagnostic_codes(
        snapshot: &SourceRegistrySnapshot,
        expected_codes: &[SourceRegistryDiagnosticCode],
    ) {
        let codes = snapshot
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>();
        assert_eq!(codes, expected_codes);
    }
}
