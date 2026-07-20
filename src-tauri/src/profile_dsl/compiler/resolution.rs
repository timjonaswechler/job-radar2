use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::{DetailStep, DiscoveryStep};
use crate::profile_dsl::execution_plan::capabilities::ExecutionPlanBuildError;
use crate::profile_dsl::execution_plan::detail::compile_detail_step;
use crate::profile_dsl::execution_plan::discovery::compile_discovery_step;
use crate::profile_dsl::execution_plan::{
    ExecutionPlanAccessPath, ExecutionPlanSource, SourceExecutionPlan,
};
use crate::profile_dsl::source_config::EffectiveSourceConfigContract;
use crate::source::documents::{SelectedAccessPath, SourceDocument};
use crate::source_profile::documents::SourceProfileDocument;
use crate::source_profile::registry::SourceProfileRegistrySnapshot;

use super::boundedness::validate_boundedness;
use super::capabilities::validate_capability_compatibility;
use super::compiler_error;
use super::has_error_diagnostics;
use super::keys::{
    access_path_index, validate_detail_strategy_keys, validate_discovery_strategy_keys,
    validate_reusable_access_path_keys,
};
use super::overrides::apply_source_overrides;
use super::security::validate_security;
use super::source_config::{
    compile_reusable_contract, compile_source_owned_contract, push_definition_violations,
    source_owned_access_path_schema, validate_source_config_against_contract,
};
use super::specialization::specialize_profile;
use super::support::validate_support_metadata;
use super::templates::validate_template_variables;
use super::SourceRuntimeBindingDependencies;

pub(super) struct ResolvedSourceExecutionPlan {
    pub(super) execution_plan: SourceExecutionPlan,
    pub(super) runtime_binding_dependencies: SourceRuntimeBindingDependencies,
}

struct ValidatedAccessPath {
    contract: Option<EffectiveSourceConfigContract>,
    runtime_binding_dependencies: SourceRuntimeBindingDependencies,
}

pub(super) fn validate_source_profile_document(
    profile: &SourceProfileDocument,
    diagnostics: &mut Diagnostics,
) {
    let _ = validate_source_profile_document_with_contracts(profile, diagnostics);
}

fn validate_source_profile_document_with_contracts(
    profile: &SourceProfileDocument,
    diagnostics: &mut Diagnostics,
) -> Vec<ValidatedAccessPath> {
    validate_support_metadata(
        &profile.support,
        "/support",
        serde_json::json!({ "sourceProfileKey": profile.key }),
        diagnostics,
    );
    validate_reusable_access_path_keys(profile, diagnostics);

    profile
        .access_paths
        .iter()
        .enumerate()
        .map(|(path_index, access_path)| {
            let contract = match compile_reusable_contract(
                profile.source_config_schema.as_ref(),
                access_path.source_config_schema.as_ref(),
                path_index,
            ) {
                Ok(contract) => Some(contract),
                Err(violations) => {
                    push_definition_violations(violations, diagnostics);
                    None
                }
            };
            validate_discovery_strategy_keys(
                &access_path.discovery,
                access_path_step_path(Some(path_index), "postingDiscovery"),
                diagnostics,
            );
            if let Some(detail) = &access_path.detail {
                validate_detail_strategy_keys(
                    detail,
                    access_path_step_path(Some(path_index), "postingDetail"),
                    diagnostics,
                );
            }

            let access_path_base = access_path_base_path(Some(path_index));
            let runtime_binding_dependencies = validate_template_variables(
                &access_path.discovery,
                access_path.detail.as_ref(),
                contract
                    .as_ref()
                    .map(EffectiveSourceConfigContract::property_keys)
                    .unwrap_or_default()
                    .into_iter()
                    .collect(),
                access_path_base.clone(),
                diagnostics,
            );
            validate_capability_compatibility(
                &access_path.discovery,
                access_path.detail.as_ref(),
                access_path_base.clone(),
                diagnostics,
            );
            validate_boundedness(
                &access_path.discovery,
                access_path.detail.as_ref(),
                access_path_base.clone(),
                diagnostics,
            );
            validate_security(
                &access_path.discovery,
                access_path.detail.as_ref(),
                access_path_base,
                diagnostics,
            );
            ValidatedAccessPath {
                contract,
                runtime_binding_dependencies,
            }
        })
        .collect()
}

pub(super) fn compile_source_execution_plan(
    source: &SourceDocument,
    registry: &SourceProfileRegistrySnapshot,
    diagnostics: &mut Diagnostics,
) -> Option<ResolvedSourceExecutionPlan> {
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
            Some(execution_plan)
        }
    }
}

fn compile_profile_access_path(
    registry: &SourceProfileRegistrySnapshot,
    source: &SourceDocument,
    profile_key: &str,
    path_key: &str,
    diagnostics: &mut Diagnostics,
) -> Option<ResolvedSourceExecutionPlan> {
    let base_profile = resolve_profile(registry, source, profile_key, diagnostics)?;
    if source.access_paths.is_some() && source.source_overrides.is_some() {
        diagnostics.push(compiler_error(
            "conflicting_source_specialization_models",
            format!(
                "Source `{}` cannot combine direct Access Path fragments with legacy sourceOverrides",
                source.key
            ),
            "/accessPaths",
            serde_json::json!({ "sourceKey": source.key }),
        ));
        return None;
    }

    let mut effective_profile =
        specialize_profile(base_profile, source.access_paths.as_deref(), diagnostics)?;

    // A01 owns migration and deletion of the productive schema-v2 Source
    // Overrides route. It remains isolated from the final keyed merger.
    if source.access_paths.is_none() {
        if let Some(access_path) = effective_profile
            .access_paths
            .iter_mut()
            .find(|access_path| access_path.key == path_key)
        {
            let effective_steps = apply_source_overrides(
                source.source_overrides.as_ref(),
                &access_path.discovery,
                access_path.detail.as_ref(),
                diagnostics,
            );
            access_path.discovery = effective_steps.discovery;
            access_path.detail = effective_steps.detail;
        }
    }

    let contracts =
        validate_source_profile_document_with_contracts(&effective_profile, diagnostics);
    if has_error_diagnostics(diagnostics) {
        return None;
    }

    let path_index = access_path_index(&effective_profile, path_key);
    if let Some(index) = path_index {
        let contract = contracts[index]
            .contract
            .as_ref()
            .expect("error-free whole-profile validation must retain every compiled contract");
        validate_source_config_against_contract(contract, &source.source_config, diagnostics);
        if has_error_diagnostics(diagnostics) {
            return None;
        }
    }

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

    let discovery = compile_discovery_step(
        &access_path.discovery,
        &access_path_step_path(path_index, "postingDiscovery"),
    )
    .map_err(|error| push_compiled_plan_invariant(error, diagnostics))
    .ok()?;
    let detail = access_path
        .detail
        .as_ref()
        .map(|detail| {
            compile_detail_step(detail, &access_path_step_path(path_index, "postingDetail"))
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
        discovery,
        detail,
    };

    let runtime_binding_dependencies = path_index
        .map(|index| contracts[index].runtime_binding_dependencies.clone())
        .expect("successful Access Path resolution must retain selected validation dependencies");
    Some(ResolvedSourceExecutionPlan {
        execution_plan,
        runtime_binding_dependencies,
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
) -> Option<ResolvedSourceExecutionPlan> {
    let SelectedAccessPath::SourceOwnedAccessPath {
        key,
        name,
        discovery,
        detail,
        ..
    } = &source.selected_access_path
    else {
        unreachable!("caller only passes Source-owned Access Paths")
    };

    let runtime_binding_dependencies =
        validate_source_owned_access_path(source, discovery, detail.as_ref(), diagnostics);
    if has_error_diagnostics(diagnostics) {
        return None;
    }

    let compiled_discovery =
        compile_discovery_step(discovery, "/selectedAccessPath/postingDiscovery")
            .map_err(|error| push_compiled_plan_invariant(error, diagnostics))
            .ok()?;
    let compiled_detail = detail
        .as_ref()
        .map(|detail| compile_detail_step(detail, "/selectedAccessPath/postingDetail"))
        .transpose()
        .map_err(|error| push_compiled_plan_invariant(error, diagnostics))
        .ok()?;

    Some(ResolvedSourceExecutionPlan {
        execution_plan: SourceExecutionPlan {
            source: ExecutionPlanSource {
                key: source.key.clone(),
                name: source.name.clone(),
            },
            selected_access_path: ExecutionPlanAccessPath::SourceOwnedAccessPath {
                key: key.clone(),
                name: name.clone(),
            },
            source_config: source.source_config.clone(),
            discovery: compiled_discovery,
            detail: compiled_detail,
        },
        runtime_binding_dependencies,
    })
}

fn validate_source_owned_access_path(
    source: &SourceDocument,
    discovery: &DiscoveryStep,
    detail: Option<&DetailStep>,
    diagnostics: &mut Diagnostics,
) -> SourceRuntimeBindingDependencies {
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
    if source.access_paths.is_some() {
        diagnostics.push(compiler_error(
            "profile_fragments_not_supported_for_source_owned_access_path",
            format!(
                "Source `{}` uses a Source-owned Access Path, so direct Access Path fragments cannot be applied",
                source.key
            ),
            "/accessPaths",
            serde_json::json!({ "sourceKey": source.key }),
        ));
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
    let source_config_contract = compile_source_owned_contract(source_owned_access_path_schema(
        &source.selected_access_path,
    ));
    let source_config_keys = match source_config_contract {
        Ok(contract) => {
            let keys = contract.property_keys().into_iter().collect();
            validate_source_config_against_contract(&contract, &source.source_config, diagnostics);
            keys
        }
        Err(violations) => {
            push_definition_violations(violations, diagnostics);
            Default::default()
        }
    };
    validate_discovery_strategy_keys(
        discovery,
        "/selectedAccessPath/postingDiscovery".to_string(),
        diagnostics,
    );
    if let Some(detail) = detail {
        validate_detail_strategy_keys(
            detail,
            "/selectedAccessPath/postingDetail".to_string(),
            diagnostics,
        );
    }
    let runtime_binding_dependencies = validate_template_variables(
        discovery,
        detail,
        source_config_keys,
        "/selectedAccessPath".to_string(),
        diagnostics,
    );
    validate_capability_compatibility(
        discovery,
        detail,
        "/selectedAccessPath".to_string(),
        diagnostics,
    );
    validate_boundedness(
        discovery,
        detail,
        "/selectedAccessPath".to_string(),
        diagnostics,
    );
    validate_security(
        discovery,
        detail,
        "/selectedAccessPath".to_string(),
        diagnostics,
    );
    runtime_binding_dependencies
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
