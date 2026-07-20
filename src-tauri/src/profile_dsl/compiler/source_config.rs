use crate::profile_dsl::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};
use crate::profile_dsl::documents::{JsonObject, JsonSchemaObject};
use crate::profile_dsl::source_config::{
    compile_contract, ContractViolation, EffectiveSourceConfigContract, SchemaLocation,
};
use crate::source::documents::SelectedAccessPath;

pub(super) fn compile_reusable_contract(
    profile_schema: Option<&JsonSchemaObject>,
    access_path_schema: Option<&JsonSchemaObject>,
    access_path_index: usize,
) -> Result<EffectiveSourceConfigContract, Vec<ContractViolation>> {
    let access_path = format!("/accessPaths/{access_path_index}/sourceConfigSchema");
    compile_contract(&[
        SchemaLocation {
            schema: profile_schema,
            path: "/sourceConfigSchema",
            title_allowed: true,
        },
        SchemaLocation {
            schema: access_path_schema,
            path: &access_path,
            title_allowed: true,
        },
    ])
}

pub(super) fn compile_source_owned_contract(
    schema: Option<&JsonSchemaObject>,
) -> Result<EffectiveSourceConfigContract, Vec<ContractViolation>> {
    compile_contract(&[SchemaLocation {
        schema,
        path: "/selectedAccessPath/sourceConfigSchema",
        title_allowed: false,
    }])
}

pub(super) fn validate_source_config_against_contract(
    contract: &EffectiveSourceConfigContract,
    source_config: &JsonObject,
    diagnostics: &mut Diagnostics,
) {
    push_value_violations(contract.validate_complete(source_config), diagnostics);
}

pub(super) fn push_definition_violations(
    violations: Vec<ContractViolation>,
    diagnostics: &mut Diagnostics,
) {
    for violation in violations {
        let diagnostic = diagnostic(violation, DiagnosticCategory::Compiler, "");
        if !diagnostics.contains(&diagnostic) {
            diagnostics.push(diagnostic);
        }
    }
}

fn push_value_violations(violations: Vec<ContractViolation>, diagnostics: &mut Diagnostics) {
    for violation in violations {
        diagnostics.push(diagnostic(
            violation,
            DiagnosticCategory::SourceValidation,
            "/sourceConfig",
        ));
    }
}

fn diagnostic(
    violation: ContractViolation,
    category: DiagnosticCategory,
    path_prefix: &str,
) -> Diagnostic {
    Diagnostic {
        category,
        code: violation.code.to_string(),
        message: violation.message,
        severity: DiagnosticSeverity::Error,
        path: format!("{path_prefix}{}", violation.path),
        strategy_key: None,
        details: Some(violation.details),
    }
}

pub(super) fn source_owned_access_path_schema(
    selected_access_path: &SelectedAccessPath,
) -> Option<&JsonSchemaObject> {
    match selected_access_path {
        SelectedAccessPath::SourceOwnedAccessPath {
            source_config_schema,
            ..
        } => source_config_schema.as_ref(),
        SelectedAccessPath::ProfileAccessPath { .. } => None,
    }
}
