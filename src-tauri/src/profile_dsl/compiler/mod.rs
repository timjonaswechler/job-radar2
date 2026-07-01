//! Profile Compiler skeleton for resolving concrete Sources into typed
//! Execution Plans. This module intentionally performs no runtime execution and
//! leaves semantic validation, boundedness, and security checks to later
//! compiler slices.

use serde::{Deserialize, Serialize};

use crate::profile_dsl::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};
use crate::profile_dsl::execution_plan::{
    ExecutionPlanAccessPath, ExecutionPlanSource, SourceExecutionPlan,
};
use crate::source::documents::{SelectedAccessPath, SourceDocument, SourceStatus};
use crate::source_profile::documents::SourceProfileDocument;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ProfileCompilerSnapshot {
    pub profiles: Vec<SourceProfileDocument>,
    pub sources: Vec<SourceDocument>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CompileSourceExecutionPlanResult {
    pub source_key: String,
    pub execution_plan: Option<SourceExecutionPlan>,
    pub diagnostics: Diagnostics,
}

pub fn compile_source_execution_plan(
    snapshot: &ProfileCompilerSnapshot,
    source_key: &str,
) -> CompileSourceExecutionPlanResult {
    let mut result = CompileSourceExecutionPlanResult {
        source_key: source_key.to_string(),
        execution_plan: None,
        diagnostics: Vec::new(),
    };

    let Some(source) = snapshot
        .sources
        .iter()
        .find(|source| source.key == source_key)
    else {
        result.diagnostics.push(compiler_error(
            "source_not_found",
            format!("Source `{source_key}` was not found in the compiler snapshot"),
            "",
            serde_json::json!({ "sourceKey": source_key }),
        ));
        return result;
    };

    if source.status != SourceStatus::Active {
        let status = serde_json::to_value(source.status)
            .expect("SourceStatus should serialize to a stable diagnostic value");
        result.diagnostics.push(compiler_error(
            "source_not_executable",
            format!(
                "Source `{}` has status `{}` and cannot be compiled into an executable plan",
                source.key,
                status.as_str().unwrap_or("unknown")
            ),
            "/status",
            serde_json::json!({
                "sourceKey": source.key,
                "status": status,
            }),
        ));
        return result;
    }

    result.execution_plan = match &source.selected_access_path {
        SelectedAccessPath::ProfileAccessPath {
            profile_key,
            path_key,
        } => {
            let Some(profile) = snapshot
                .profiles
                .iter()
                .find(|profile| profile.key == *profile_key)
            else {
                result.diagnostics.push(compiler_error(
                    "source_profile_not_found",
                    format!(
                        "Source `{}` references missing Source Profile `{profile_key}`",
                        source.key
                    ),
                    "/selectedAccessPath/profileKey",
                    serde_json::json!({
                        "sourceKey": source.key,
                        "profileKey": profile_key,
                    }),
                ));
                return result;
            };

            let Some(access_path) = profile
                .access_paths
                .iter()
                .find(|access_path| access_path.key == *path_key)
            else {
                result.diagnostics.push(compiler_error(
                    "access_path_not_found",
                    format!(
                        "Source `{}` references missing Access Path `{path_key}` on Source Profile `{profile_key}`",
                        source.key
                    ),
                    "/selectedAccessPath/pathKey",
                    serde_json::json!({
                        "sourceKey": source.key,
                        "profileKey": profile_key,
                        "pathKey": path_key,
                    }),
                ));
                return result;
            };

            Some(SourceExecutionPlan {
                source: ExecutionPlanSource {
                    key: source.key.clone(),
                    name: source.name.clone(),
                },
                selected_access_path: ExecutionPlanAccessPath::ProfileAccessPath {
                    profile_key: profile.key.clone(),
                    profile_name: profile.name.clone(),
                    path_key: access_path.key.clone(),
                    path_name: access_path.name.clone(),
                },
                source_config: source.source_config.clone(),
                source_overrides: source.source_overrides.clone(),
                posting_discovery: access_path.posting_discovery.clone(),
                posting_detail: access_path.posting_detail.clone(),
            })
        }
        SelectedAccessPath::SourceOwnedAccessPath {
            key,
            name,
            posting_discovery,
            posting_detail,
            ..
        } => Some(SourceExecutionPlan {
            source: ExecutionPlanSource {
                key: source.key.clone(),
                name: source.name.clone(),
            },
            selected_access_path: ExecutionPlanAccessPath::SourceOwnedAccessPath {
                key: key.clone(),
                name: name.clone(),
            },
            source_config: source.source_config.clone(),
            source_overrides: source.source_overrides.clone(),
            posting_discovery: posting_discovery.clone(),
            posting_detail: posting_detail.clone(),
        }),
    };

    result
}

fn compiler_error(
    code: impl Into<String>,
    message: impl Into<String>,
    path: impl Into<String>,
    details: serde_json::Value,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Compiler,
        code: code.into(),
        message: message.into(),
        severity: DiagnosticSeverity::Error,
        path: path.into(),
        strategy_key: None,
        details: Some(details),
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use crate::profile_dsl::compiler::{compile_source_execution_plan, ProfileCompilerSnapshot};
    use crate::profile_dsl::execution_plan::{ExecutionPlanAccessPath, SourceExecutionPlan};
    use crate::source::documents::SourceDocument;
    use crate::source_profile::documents::SourceProfileDocument;

    #[test]
    fn compiles_source_selecting_reusable_profile_access_path() {
        let profile: SourceProfileDocument =
            read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
        let source: SourceDocument = read_fixture(
            "tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json",
        );
        let snapshot = ProfileCompilerSnapshot {
            profiles: vec![profile],
            sources: vec![source],
        };

        let result = compile_source_execution_plan(&snapshot, "example_source");

        assert_eq!(result.source_key, "example_source");
        assert_eq!(result.diagnostics, Vec::new());

        let plan: SourceExecutionPlan = result
            .execution_plan
            .expect("active source with reusable access path should compile");
        assert_eq!(plan.source.key, "example_source");
        assert_eq!(plan.source.name, "Example Source");
        assert_eq!(
            plan.source_config["feedUrl"],
            "https://example.test/jobs.json"
        );
        assert!(plan.source_overrides.is_some());
        assert_eq!(plan.posting_discovery.strategies[0].key, "json_api");
        assert_eq!(
            plan.posting_detail.as_ref().unwrap().strategies[0].key,
            "detail_api"
        );
        assert_eq!(
            plan.selected_access_path,
            ExecutionPlanAccessPath::ProfileAccessPath {
                profile_key: "example_jobs".to_string(),
                profile_name: "Example Jobs".to_string(),
                path_key: "json_feed".to_string(),
                path_name: "JSON feed".to_string(),
            }
        );
    }

    #[test]
    fn compiles_source_with_source_owned_access_path() {
        let mut source: SourceDocument =
            read_fixture("tests/fixtures/source-profile-dsl/valid/source-owned-access-path.json");
        source.status = crate::source::documents::SourceStatus::Active;
        let snapshot = ProfileCompilerSnapshot {
            profiles: Vec::new(),
            sources: vec![source],
        };

        let result = compile_source_execution_plan(&snapshot, "owned_source");

        assert_eq!(result.diagnostics, Vec::new());
        let plan = result
            .execution_plan
            .expect("active source-owned access path should compile");
        assert_eq!(plan.source.key, "owned_source");
        assert_eq!(
            plan.source_config["startUrl"],
            "https://example.test/careers"
        );
        assert_eq!(plan.source_overrides, None);
        assert_eq!(plan.posting_discovery.strategies[0].key, "html_cards");
        assert_eq!(plan.posting_detail, None);
        assert_eq!(
            plan.selected_access_path,
            ExecutionPlanAccessPath::SourceOwnedAccessPath {
                key: "html_page".to_string(),
                name: "HTML page".to_string(),
            }
        );
    }

    #[test]
    fn missing_source_returns_structured_diagnostic() {
        let result = compile_source_execution_plan(&ProfileCompilerSnapshot::default(), "missing");

        assert_eq!(result.source_key, "missing");
        assert_eq!(result.execution_plan, None);
        assert_eq!(result.diagnostics.len(), 1);
        let diagnostic = &result.diagnostics[0];
        assert_eq!(
            diagnostic.category,
            crate::profile_dsl::diagnostics::DiagnosticCategory::Compiler
        );
        assert_eq!(diagnostic.code, "source_not_found");
        assert_eq!(
            diagnostic.severity,
            crate::profile_dsl::diagnostics::DiagnosticSeverity::Error
        );
        assert_eq!(diagnostic.path, "");
        assert_eq!(diagnostic.details.as_ref().unwrap()["sourceKey"], "missing");
    }

    #[test]
    fn missing_profile_and_access_path_return_structured_diagnostics() {
        let source: SourceDocument = read_fixture(
            "tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json",
        );
        let missing_profile_result = compile_source_execution_plan(
            &ProfileCompilerSnapshot {
                profiles: Vec::new(),
                sources: vec![source.clone()],
            },
            "example_source",
        );

        assert_eq!(missing_profile_result.execution_plan, None);
        assert_eq!(
            missing_profile_result.diagnostics[0].code,
            "source_profile_not_found"
        );
        assert_eq!(
            missing_profile_result.diagnostics[0].path,
            "/selectedAccessPath/profileKey"
        );
        assert_eq!(
            missing_profile_result.diagnostics[0]
                .details
                .as_ref()
                .unwrap()["profileKey"],
            "example_jobs"
        );

        let mut profile: SourceProfileDocument =
            read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
        profile.access_paths.clear();
        let missing_path_result = compile_source_execution_plan(
            &ProfileCompilerSnapshot {
                profiles: vec![profile],
                sources: vec![source],
            },
            "example_source",
        );

        assert_eq!(missing_path_result.execution_plan, None);
        assert_eq!(
            missing_path_result.diagnostics[0].code,
            "access_path_not_found"
        );
        assert_eq!(
            missing_path_result.diagnostics[0].path,
            "/selectedAccessPath/pathKey"
        );
        assert_eq!(
            missing_path_result.diagnostics[0].details.as_ref().unwrap()["pathKey"],
            "json_feed"
        );
    }

    #[test]
    fn draft_and_disabled_sources_do_not_produce_executable_plans() {
        for (status, expected) in [
            (crate::source::documents::SourceStatus::Draft, "draft"),
            (crate::source::documents::SourceStatus::Disabled, "disabled"),
        ] {
            let mut source: SourceDocument = read_fixture(
                "tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json",
            );
            source.status = status;
            let profile: SourceProfileDocument =
                read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
            let result = compile_source_execution_plan(
                &ProfileCompilerSnapshot {
                    profiles: vec![profile],
                    sources: vec![source],
                },
                "example_source",
            );

            assert_eq!(result.execution_plan, None);
            assert_eq!(result.diagnostics[0].code, "source_not_executable");
            assert_eq!(result.diagnostics[0].path, "/status");
            assert_eq!(
                result.diagnostics[0].details.as_ref().unwrap()["status"],
                expected
            );
        }
    }

    fn read_fixture<T>(relative_path: &str) -> T
    where
        T: serde::de::DeserializeOwned,
    {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
        let contents = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        serde_json::from_str(&contents)
            .unwrap_or_else(|error| panic!("failed to deserialize {}: {error}", path.display()))
    }
}
