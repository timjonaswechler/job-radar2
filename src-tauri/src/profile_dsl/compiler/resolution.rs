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

use super::boundedness::validate_boundedness;
use super::capabilities::validate_capability_compatibility;
use super::compiler_error;
use super::has_error_diagnostics;
use super::keys::{
    access_path_index, validate_detail_strategy_keys, validate_discovery_strategy_keys,
    validate_reusable_access_path_keys,
};
use super::source_config::{
    compile_reusable_contract, compile_source_owned_contract, push_definition_violations,
    source_owned_access_path_schema, validate_source_config_against_contract,
};
use super::support::validate_support_metadata;
use super::templates::validate_template_variables;
use super::values::validate_value_context_foundation;
use super::{SourceRuntimeBinding, SourceRuntimeBindingDependencies};

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
    if profile.detection.is_some() {
        if let Err(detection_diagnostics) =
            crate::source_profile::detection::compile_detection_plan(profile)
        {
            diagnostics.extend(detection_diagnostics);
        }
    }
    let mut total_value_nodes = 0usize;

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
                access_path_step_path(Some(path_index), "discovery"),
                diagnostics,
            );
            if let Some(detail) = &access_path.detail {
                validate_detail_strategy_keys(
                    detail,
                    access_path_step_path(Some(path_index), "detail"),
                    diagnostics,
                );
            }

            let access_path_base = access_path_base_path(Some(path_index));
            let source_config_keys = contract
                .as_ref()
                .map(EffectiveSourceConfigContract::property_keys)
                .unwrap_or_default();
            let mut runtime_binding_dependencies = validate_template_variables(
                &access_path.discovery,
                access_path.detail.as_ref(),
                source_config_keys.iter().cloned().collect(),
                access_path_base.clone(),
                diagnostics,
            );
            if validate_value_context_foundation(
                &access_path.discovery,
                access_path.detail.as_ref(),
                source_config_keys.into_iter().collect(),
                &access_path_base,
                &mut total_value_nodes,
                diagnostics,
            ) {
                runtime_binding_dependencies.insert(SourceRuntimeBinding::Name);
            }
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
            ValidatedAccessPath {
                contract,
                runtime_binding_dependencies,
            }
        })
        .collect()
}

pub(super) fn compile_materialized_profile_access_path(
    source: &SourceDocument,
    effective_profile: &SourceProfileDocument,
    profile_key: &str,
    path_key: &str,
    diagnostics: &mut Diagnostics,
) -> Option<ResolvedSourceExecutionPlan> {
    let contracts = validate_source_profile_document_with_contracts(effective_profile, diagnostics);
    if has_error_diagnostics(diagnostics) {
        return None;
    }

    let path_index = access_path_index(effective_profile, path_key);
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
        effective_profile,
        profile_key,
        path_key,
        diagnostics,
    )?;
    if has_error_diagnostics(diagnostics) {
        return None;
    }

    let source_config_keys = path_index
        .map(|index| {
            contracts[index]
                .contract
                .as_ref()
                .expect("validated contract")
                .property_keys()
                .into_iter()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let posting_meta_keys = posting_meta_keys(&access_path.discovery);
    let discovery = compile_discovery_step(
        &access_path.discovery,
        &access_path_step_path(path_index, "discovery"),
        &source_config_keys,
    )
    .map_err(|error| push_compiled_plan_invariant(error, diagnostics))
    .ok()?;
    let detail = access_path
        .detail
        .as_ref()
        .map(|detail| {
            compile_detail_step(
                detail,
                &access_path_step_path(path_index, "detail"),
                &source_config_keys,
                &posting_meta_keys,
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

pub(super) fn compile_source_owned_access_path(
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

    let source_config_keys = compile_source_owned_contract(source_owned_access_path_schema(
        &source.selected_access_path,
    ))
    .expect("validated Source-owned Source Config contract must compile")
    .property_keys()
    .into_iter()
    .collect::<Vec<_>>();
    let posting_meta_keys = posting_meta_keys(discovery);
    let compiled_discovery = compile_discovery_step(
        discovery,
        "/selectedAccessPath/discovery",
        &source_config_keys,
    )
    .map_err(|error| push_compiled_plan_invariant(error, diagnostics))
    .ok()?;
    let compiled_detail = detail
        .as_ref()
        .map(|detail| {
            compile_detail_step(
                detail,
                "/selectedAccessPath/detail",
                &source_config_keys,
                &posting_meta_keys,
            )
        })
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
    let source_config_contract = compile_source_owned_contract(source_owned_access_path_schema(
        &source.selected_access_path,
    ));
    let source_config_keys = match source_config_contract {
        Ok(contract) => {
            let keys = contract
                .property_keys()
                .into_iter()
                .collect::<std::collections::HashSet<_>>();
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
        "/selectedAccessPath/discovery".to_string(),
        diagnostics,
    );
    if let Some(detail) = detail {
        validate_detail_strategy_keys(
            detail,
            "/selectedAccessPath/detail".to_string(),
            diagnostics,
        );
    }
    let mut runtime_binding_dependencies = validate_template_variables(
        discovery,
        detail,
        source_config_keys.clone(),
        "/selectedAccessPath".to_string(),
        diagnostics,
    );
    let mut total_value_nodes = 0usize;
    if validate_value_context_foundation(
        discovery,
        detail,
        source_config_keys.into_iter().collect(),
        "/selectedAccessPath",
        &mut total_value_nodes,
        diagnostics,
    ) {
        runtime_binding_dependencies.insert(SourceRuntimeBinding::Name);
    }
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
    runtime_binding_dependencies
}

fn push_compiled_plan_invariant(error: ExecutionPlanBuildError, diagnostics: &mut Diagnostics) {
    diagnostics.push(compiler_error(
        error.code,
        format!(
            "Validated Profile DSL could not be converted into a strict Execution Plan: {}",
            error.message
        ),
        error.path,
        error.details,
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

fn posting_meta_keys(discovery: &DiscoveryStep) -> Vec<String> {
    discovery
        .strategies
        .iter()
        .filter_map(|strategy| strategy.extract.posting_meta.as_ref())
        .flat_map(|values| values.keys().cloned())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}
