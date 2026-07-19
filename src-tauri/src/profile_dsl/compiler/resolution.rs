use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::{PostingDetailStep, PostingDiscoveryStep};
use crate::profile_dsl::execution_plan::capabilities::ExecutionPlanBuildError;
use crate::profile_dsl::execution_plan::posting_detail::compile_posting_detail_step;
use crate::profile_dsl::execution_plan::posting_discovery::compile_posting_discovery_step;
use crate::profile_dsl::execution_plan::{
    ExecutionPlanAccessPath, ExecutionPlanSource, SourceExecutionPlan,
};
use crate::source::documents::{SelectedAccessPath, SourceDocument};
use crate::source_profile::documents::SourceProfileDocument;
use crate::source_profile::registry::SourceProfileRegistrySnapshot;

use super::boundedness::validate_boundedness;
use super::capabilities::validate_capability_compatibility;
use super::compiler_error;
use super::has_error_diagnostics;
use super::keys::{
    access_path_index, validate_posting_detail_strategy_keys,
    validate_posting_discovery_strategy_keys, validate_reusable_access_path_keys,
};
use super::overrides::apply_source_overrides;
use super::security::validate_security;
use super::source_config::{
    source_config_schema_keys, source_owned_access_path_schema, validate_source_config,
    validate_source_config_against_validated_contract, validate_source_config_contract,
};
use super::support::validate_support_metadata;
use super::templates::validate_template_variables;
use super::{CompiledSource, CompiledSourceAccess, EffectiveSourceProfile, SourceOwnedAccessPath};

pub(super) fn validate_source_profile_document(
    profile: &SourceProfileDocument,
    diagnostics: &mut Diagnostics,
) {
    validate_support_metadata(
        &profile.support,
        "/support",
        serde_json::json!({ "sourceProfileKey": profile.key }),
        diagnostics,
    );
    validate_reusable_access_path_keys(profile, diagnostics);

    for (path_index, access_path) in profile.access_paths.iter().enumerate() {
        validate_source_config_contract(
            profile.source_config_schema.as_ref(),
            access_path.source_config_schema.as_ref(),
            Some(path_index),
            diagnostics,
        );
        validate_posting_discovery_strategy_keys(
            &access_path.posting_discovery,
            access_path_step_path(Some(path_index), "postingDiscovery"),
            diagnostics,
        );
        if let Some(posting_detail) = &access_path.posting_detail {
            validate_posting_detail_strategy_keys(
                posting_detail,
                access_path_step_path(Some(path_index), "postingDetail"),
                diagnostics,
            );
        }

        let access_path_base = access_path_base_path(Some(path_index));
        validate_template_variables(
            &access_path.posting_discovery,
            access_path.posting_detail.as_ref(),
            source_config_schema_keys(
                profile.source_config_schema.as_ref(),
                access_path.source_config_schema.as_ref(),
            ),
            access_path_base.clone(),
            diagnostics,
        );
        validate_capability_compatibility(
            &access_path.posting_discovery,
            access_path.posting_detail.as_ref(),
            access_path_base.clone(),
            diagnostics,
        );
        validate_boundedness(
            &access_path.posting_discovery,
            access_path.posting_detail.as_ref(),
            access_path_base.clone(),
            diagnostics,
        );
        validate_security(
            &access_path.posting_discovery,
            access_path.posting_detail.as_ref(),
            access_path_base.clone(),
            diagnostics,
        );

        if !has_error_diagnostics(diagnostics) {
            let _ = compile_posting_discovery_step(
                &access_path.posting_discovery,
                &access_path_step_path(Some(path_index), "postingDiscovery"),
            )
            .map_err(|error| push_compiled_plan_invariant(error, diagnostics));
            if let Some(posting_detail) = &access_path.posting_detail {
                let _ = compile_posting_detail_step(
                    posting_detail,
                    &access_path_step_path(Some(path_index), "postingDetail"),
                )
                .map_err(|error| push_compiled_plan_invariant(error, diagnostics));
            }
        }
    }
}

pub(super) fn compile_authoritative_source(
    source: &SourceDocument,
    registry: &SourceProfileRegistrySnapshot,
    diagnostics: &mut Diagnostics,
) -> Option<CompiledSource> {
    match &source.selected_access_path {
        SelectedAccessPath::ProfileAccessPath {
            profile_key,
            path_key,
        } => compile_profile_access_path(registry, source, profile_key, path_key, diagnostics),
        SelectedAccessPath::SourceOwnedAccessPath { .. } => {
            let execution_plan = compile_source_owned_access_path(source, diagnostics)?;
            if has_error_diagnostics(diagnostics) {
                return None;
            }
            Some(CompiledSource {
                access: CompiledSourceAccess::SourceOwned {
                    access_path: source_owned_access_path(source),
                },
                execution_plan,
            })
        }
    }
}

fn compile_profile_access_path(
    registry: &SourceProfileRegistrySnapshot,
    source: &SourceDocument,
    profile_key: &str,
    path_key: &str,
    diagnostics: &mut Diagnostics,
) -> Option<CompiledSource> {
    let base_profile = resolve_profile(registry, source, profile_key, diagnostics)?;
    let mut effective_profile = base_profile.clone();

    if let Some(access_path) = effective_profile
        .access_paths
        .iter_mut()
        .find(|access_path| access_path.key == path_key)
    {
        let effective_steps = apply_source_overrides(
            source.source_overrides.as_ref(),
            &access_path.posting_discovery,
            access_path.posting_detail.as_ref(),
            diagnostics,
        );
        access_path.posting_discovery = effective_steps.posting_discovery;
        access_path.posting_detail = effective_steps.posting_detail;
    }

    validate_source_profile_document(&effective_profile, diagnostics);

    let path_index = access_path_index(&effective_profile, path_key);
    let selected_source_config_schema = path_index
        .and_then(|index| effective_profile.access_paths.get(index))
        .and_then(|access_path| access_path.source_config_schema.as_ref());
    validate_source_config_against_validated_contract(
        effective_profile.source_config_schema.as_ref(),
        selected_source_config_schema,
        &source.source_config,
        diagnostics,
    );

    let access_path = resolve_access_path(
        source,
        &effective_profile,
        profile_key,
        path_key,
        diagnostics,
    )?;
    if has_error_diagnostics(diagnostics) {
        return None;
    }

    let posting_discovery = compile_posting_discovery_step(
        &access_path.posting_discovery,
        &access_path_step_path(path_index, "postingDiscovery"),
    )
    .map_err(|error| push_compiled_plan_invariant(error, diagnostics))
    .ok()?;
    let posting_detail = access_path
        .posting_detail
        .as_ref()
        .map(|posting_detail| {
            compile_posting_detail_step(
                posting_detail,
                &access_path_step_path(path_index, "postingDetail"),
            )
        })
        .transpose()
        .map_err(|error| push_compiled_plan_invariant(error, diagnostics))
        .ok()?;

    let execution_plan = SourceExecutionPlan {
        source: ExecutionPlanSource {
            key: source.key.clone(),
            name: source.name.clone(),
        },
        selected_access_path: ExecutionPlanAccessPath::ProfileAccessPath {
            profile_key: effective_profile.key.clone(),
            profile_name: effective_profile.name.clone(),
            path_key: access_path.key.clone(),
            path_name: access_path.name.clone(),
        },
        source_config: source.source_config.clone(),
        posting_discovery,
        posting_detail,
    };

    Some(CompiledSource {
        access: CompiledSourceAccess::Profile {
            effective_profile: EffectiveSourceProfile {
                document: effective_profile,
            },
        },
        execution_plan,
    })
}

fn resolve_profile<'a>(
    registry: &'a SourceProfileRegistrySnapshot,
    source: &SourceDocument,
    profile_key: &str,
    diagnostics: &mut Diagnostics,
) -> Option<&'a SourceProfileDocument> {
    let profile = registry
        .profile(profile_key)
        .map(|profile| &profile.document);
    if profile.is_none() {
        diagnostics.push(compiler_error(
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
    }
    profile
}

fn resolve_access_path<'a>(
    source: &SourceDocument,
    profile: &'a SourceProfileDocument,
    profile_key: &str,
    path_key: &str,
    diagnostics: &mut Diagnostics,
) -> Option<&'a crate::profile_dsl::documents::ReusableAccessPathDocument> {
    let access_path = profile
        .access_paths
        .iter()
        .find(|access_path| access_path.key == path_key);
    if access_path.is_none() {
        diagnostics.push(compiler_error(
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
    }
    access_path
}

fn compile_source_owned_access_path(
    source: &SourceDocument,
    diagnostics: &mut Diagnostics,
) -> Option<SourceExecutionPlan> {
    let SelectedAccessPath::SourceOwnedAccessPath {
        key,
        name,
        posting_discovery,
        posting_detail,
        ..
    } = &source.selected_access_path
    else {
        unreachable!("caller only passes Source-owned Access Paths")
    };

    validate_source_owned_access_path(
        source,
        posting_discovery,
        posting_detail.as_ref(),
        diagnostics,
    );
    if has_error_diagnostics(diagnostics) {
        return None;
    }

    let compiled_posting_discovery =
        compile_posting_discovery_step(posting_discovery, "/selectedAccessPath/postingDiscovery")
            .map_err(|error| push_compiled_plan_invariant(error, diagnostics))
            .ok()?;
    let compiled_posting_detail = posting_detail
        .as_ref()
        .map(|posting_detail| {
            compile_posting_detail_step(posting_detail, "/selectedAccessPath/postingDetail")
        })
        .transpose()
        .map_err(|error| push_compiled_plan_invariant(error, diagnostics))
        .ok()?;

    Some(SourceExecutionPlan {
        source: ExecutionPlanSource {
            key: source.key.clone(),
            name: source.name.clone(),
        },
        selected_access_path: ExecutionPlanAccessPath::SourceOwnedAccessPath {
            key: key.clone(),
            name: name.clone(),
        },
        source_config: source.source_config.clone(),
        posting_discovery: compiled_posting_discovery,
        posting_detail: compiled_posting_detail,
    })
}

fn source_owned_access_path(source: &SourceDocument) -> SourceOwnedAccessPath {
    let SelectedAccessPath::SourceOwnedAccessPath {
        key,
        name,
        description,
        source_config_schema,
        posting_discovery,
        posting_detail,
        diagnostics,
    } = &source.selected_access_path
    else {
        unreachable!("caller only passes Source-owned Access Paths")
    };

    SourceOwnedAccessPath {
        key: key.clone(),
        name: name.clone(),
        description: description.clone(),
        source_config_schema: source_config_schema.clone(),
        posting_discovery: posting_discovery.clone(),
        posting_detail: posting_detail.clone(),
        diagnostics: diagnostics.clone(),
    }
}

fn validate_source_owned_access_path(
    source: &SourceDocument,
    posting_discovery: &PostingDiscoveryStep,
    posting_detail: Option<&PostingDetailStep>,
    diagnostics: &mut Diagnostics,
) {
    match &source.source_support {
        Some(source_support) => validate_support_metadata(
            source_support,
            "/sourceSupport",
            serde_json::json!({ "sourceKey": source.key }),
            diagnostics,
        ),
        None => diagnostics.push(compiler_error(
            "missing_source_support",
            format!(
                "Source `{}` uses a Source-owned Access Path but does not declare sourceSupport metadata",
                source.key
            ),
            "/sourceSupport",
            serde_json::json!({ "sourceKey": source.key }),
        )),
    }
    if source.source_overrides.is_some() {
        diagnostics.push(compiler_error(
            "source_overrides_not_supported_for_source_owned_access_path",
            format!(
                "Source `{}` uses a Source-owned Access Path, so sourceOverrides cannot be applied",
                source.key
            ),
            "/sourceOverrides",
            serde_json::json!({ "sourceKey": source.key }),
        ));
    }
    validate_source_config(
        None,
        source_owned_access_path_schema(&source.selected_access_path),
        &source.source_config,
        None,
        diagnostics,
    );
    validate_posting_discovery_strategy_keys(
        posting_discovery,
        "/selectedAccessPath/postingDiscovery".to_string(),
        diagnostics,
    );
    if let Some(posting_detail) = posting_detail {
        validate_posting_detail_strategy_keys(
            posting_detail,
            "/selectedAccessPath/postingDetail".to_string(),
            diagnostics,
        );
    }
    validate_template_variables(
        posting_discovery,
        posting_detail,
        source_config_schema_keys(
            None,
            source_owned_access_path_schema(&source.selected_access_path),
        ),
        "/selectedAccessPath".to_string(),
        diagnostics,
    );
    validate_capability_compatibility(
        posting_discovery,
        posting_detail,
        "/selectedAccessPath".to_string(),
        diagnostics,
    );
    validate_boundedness(
        posting_discovery,
        posting_detail,
        "/selectedAccessPath".to_string(),
        diagnostics,
    );
    validate_security(
        posting_discovery,
        posting_detail,
        "/selectedAccessPath".to_string(),
        diagnostics,
    );
}

fn push_compiled_plan_invariant(error: ExecutionPlanBuildError, diagnostics: &mut Diagnostics) {
    diagnostics.push(compiler_error(
        "compiled_execution_plan_invariant_violation",
        format!(
            "Validated Profile DSL could not be converted into a strict Execution Plan: {}",
            error.message
        ),
        error.path,
        serde_json::json!({ "invariant": "strict_execution_plan" }),
    ));
}

fn access_path_base_path(path_index: Option<usize>) -> String {
    path_index
        .map(|index| format!("/accessPaths/{index}"))
        .unwrap_or_else(|| "/accessPaths".to_string())
}

fn access_path_step_path(path_index: Option<usize>, step: &str) -> String {
    format!("{}/{step}", access_path_base_path(path_index))
}
