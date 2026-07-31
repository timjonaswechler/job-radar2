use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    definition::{
        diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics},
        documents::strategy::Acceptance,
    },
    execution::occurrence::{DetailField, DetailPatch, PostingOccurrence, RequestedDetailFields},
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptancePhase {
    Discovery,
    Detail,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptanceDescriptor {
    pub key: &'static str,
    pub phases: &'static [AcceptancePhase],
}

mod min_description_length;
mod min_results;
mod required_fields;

const ACCEPTANCE_DESCRIPTORS: [AcceptanceDescriptor; 3] = [
    required_fields::DESCRIPTOR,
    min_description_length::DESCRIPTOR,
    min_results::DESCRIPTOR,
];

pub fn acceptance_descriptors() -> &'static [AcceptanceDescriptor] {
    &ACCEPTANCE_DESCRIPTORS
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptanceRegistryError {
    Duplicate {
        layer: &'static str,
        keys: Vec<String>,
    },
    Missing {
        layer: &'static str,
        keys: Vec<String>,
    },
    Extra {
        layer: &'static str,
        keys: Vec<String>,
    },
}

pub fn validate_acceptance_registration_keys(
    schema_keys: &[String],
    serde_keys: &[String],
    registration_keys: &[String],
) -> Result<(), AcceptanceRegistryError> {
    for (layer, keys) in [
        ("schema", schema_keys),
        ("serde", serde_keys),
        ("registration", registration_keys),
    ] {
        let mut counts = BTreeMap::new();
        for key in keys {
            *counts.entry(key.clone()).or_insert(0usize) += 1;
        }
        let duplicates = counts
            .into_iter()
            .filter_map(|(key, count)| (count > 1).then_some(key))
            .collect::<Vec<_>>();
        if !duplicates.is_empty() {
            return Err(AcceptanceRegistryError::Duplicate {
                layer,
                keys: duplicates,
            });
        }
    }
    let schema = schema_keys.iter().cloned().collect::<BTreeSet<_>>();
    for (layer, keys) in [("serde", serde_keys), ("registration", registration_keys)] {
        let actual = keys.iter().cloned().collect::<BTreeSet<_>>();
        let missing = schema.difference(&actual).cloned().collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(AcceptanceRegistryError::Missing {
                layer,
                keys: missing,
            });
        }
        let extra = actual.difference(&schema).cloned().collect::<Vec<_>>();
        if !extra.is_empty() {
            return Err(AcceptanceRegistryError::Extra { layer, keys: extra });
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AcceptanceContextRegistration {
    pub key: &'static str,
    pub phase: AcceptancePhase,
}

impl AcceptanceContextRegistration {
    pub const ALL: [Self; 5] = [
        Self {
            key: "requiredFields",
            phase: AcceptancePhase::Discovery,
        },
        Self {
            key: "requiredFields",
            phase: AcceptancePhase::Detail,
        },
        Self {
            key: "minDescriptionLength",
            phase: AcceptancePhase::Discovery,
        },
        Self {
            key: "minDescriptionLength",
            phase: AcceptancePhase::Detail,
        },
        Self {
            key: "minResults",
            phase: AcceptancePhase::Discovery,
        },
    ];
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptanceContextRegistryError {
    Duplicate { registrations: Vec<String> },
    Missing { registrations: Vec<String> },
    Extra { registrations: Vec<String> },
}

pub fn acceptance_context_registrations() -> Vec<AcceptanceContextRegistration> {
    acceptance_descriptors()
        .iter()
        .flat_map(|descriptor| {
            descriptor
                .phases
                .iter()
                .copied()
                .map(|phase| AcceptanceContextRegistration {
                    key: descriptor.key,
                    phase,
                })
        })
        .collect()
}

pub fn validate_acceptance_context_registrations(
    registrations: &[AcceptanceContextRegistration],
) -> Result<(), AcceptanceContextRegistryError> {
    let label = |registration: &AcceptanceContextRegistration| {
        format!("{}:{:?}", registration.key, registration.phase)
    };
    let mut counts = BTreeMap::new();
    for registration in registrations {
        *counts.entry(*registration).or_insert(0usize) += 1;
    }
    let duplicates = counts
        .into_iter()
        .filter_map(|(registration, count)| (count > 1).then(|| label(&registration)))
        .collect::<Vec<_>>();
    if !duplicates.is_empty() {
        return Err(AcceptanceContextRegistryError::Duplicate {
            registrations: duplicates,
        });
    }
    let expected = AcceptanceContextRegistration::ALL
        .into_iter()
        .collect::<BTreeSet<_>>();
    let actual = registrations.iter().copied().collect::<BTreeSet<_>>();
    let missing = expected.difference(&actual).map(label).collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(AcceptanceContextRegistryError::Missing {
            registrations: missing,
        });
    }
    let extra = actual.difference(&expected).map(label).collect::<Vec<_>>();
    if !extra.is_empty() {
        return Err(AcceptanceContextRegistryError::Extra {
            registrations: extra,
        });
    }
    Ok(())
}

fn key_is_admitted(key: &str, phase: AcceptancePhase) -> bool {
    acceptance_descriptors()
        .iter()
        .find(|descriptor| descriptor.key == key)
        .is_some_and(|descriptor| descriptor.phases.contains(&phase))
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", content = "key", rename_all = "snake_case")]
pub enum AcceptanceField {
    Url,
    Title,
    Company,
    Locations,
    DescriptionText,
    PostingMeta(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompiledAcceptance {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_fields: Vec<AcceptanceField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_description_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_results: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptanceCompileContext {
    pub phase: AcceptancePhase,
    posting_meta_keys: BTreeSet<String>,
}

impl AcceptanceCompileContext {
    pub fn discovery(keys: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            phase: AcceptancePhase::Discovery,
            posting_meta_keys: keys.into_iter().map(Into::into).collect(),
        }
    }
    pub fn detail() -> Self {
        Self {
            phase: AcceptancePhase::Detail,
            posting_meta_keys: BTreeSet::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptanceCompileError {
    pub phase: AcceptancePhase,
    pub key: &'static str,
    pub field: Option<String>,
    pub message: String,
}

pub fn compile_acceptance(
    authored: &Acceptance,
    context: &AcceptanceCompileContext,
) -> Result<CompiledAcceptance, AcceptanceCompileError> {
    min_results::validate_placement(authored.min_results, context.phase)?;
    Ok(CompiledAcceptance {
        required_fields: required_fields::compile(authored.required_fields.as_deref(), context)?,
        min_description_length: authored.min_description_length,
        min_results: authored.min_results,
    })
}

struct EffectiveRule<'a, T> {
    value: T,
    owner_path: &'a str,
}

fn required_rules<'a>(
    phase: Option<&'a CompiledAcceptance>,
    strategy: Option<&'a CompiledAcceptance>,
    phase_path: &'a str,
    strategy_path: &'a str,
) -> Vec<EffectiveRule<'a, &'a AcceptanceField>> {
    let mut rules = Vec::new();
    if let Some(plan) = phase {
        rules.extend(plan.required_fields.iter().map(|value| EffectiveRule {
            value,
            owner_path: phase_path,
        }));
    }
    if let Some(plan) = strategy {
        for value in &plan.required_fields {
            if !rules.iter().any(|rule| rule.value == value) {
                rules.push(EffectiveRule {
                    value,
                    owner_path: strategy_path,
                });
            }
        }
    }
    rules
}

fn stricter<'a>(
    phase: Option<u64>,
    strategy: Option<u64>,
    phase_path: &'a str,
    strategy_path: &'a str,
) -> Option<EffectiveRule<'a, u64>> {
    match (phase, strategy) {
        (Some(a), Some(b)) if b >= a => Some(EffectiveRule {
            value: b,
            owner_path: strategy_path,
        }),
        (Some(a), _) => Some(EffectiveRule {
            value: a,
            owner_path: phase_path,
        }),
        (None, Some(b)) => Some(EffectiveRule {
            value: b,
            owner_path: strategy_path,
        }),
        (None, None) => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrategyAcceptanceFact {
    satisfied: bool,
}

impl StrategyAcceptanceFact {
    pub const fn is_satisfied(self) -> bool {
        self.satisfied
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalPhaseAcceptanceFact {
    satisfied: bool,
}

impl FinalPhaseAcceptanceFact {
    pub const fn is_satisfied(self) -> bool {
        self.satisfied
    }
}

pub fn evaluate_discovery_strategy_acceptance(
    candidates: &[PostingOccurrence],
    phase: Option<&CompiledAcceptance>,
    strategy: Option<&CompiledAcceptance>,
    strategy_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> StrategyAcceptanceFact {
    StrategyAcceptanceFact {
        satisfied: discovery_acceptance_satisfied(
            candidates,
            phase,
            strategy,
            strategy_path,
            strategy_key,
            diagnostics,
        ),
    }
}

pub fn evaluate_discovery_final_acceptance(
    candidates: &[PostingOccurrence],
    phase: Option<&CompiledAcceptance>,
    diagnostics: &mut Diagnostics,
) -> FinalPhaseAcceptanceFact {
    FinalPhaseAcceptanceFact {
        satisfied: discovery_acceptance_satisfied(
            candidates,
            phase,
            None,
            "/discovery",
            None,
            diagnostics,
        ),
    }
}

fn discovery_acceptance_satisfied(
    candidates: &[PostingOccurrence],
    phase: Option<&CompiledAcceptance>,
    strategy: Option<&CompiledAcceptance>,
    strategy_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> bool {
    required_fields::evaluate_discovery(
        candidates,
        phase,
        strategy,
        strategy_path,
        strategy_key,
        diagnostics,
    ) && min_description_length::evaluate_discovery(
        candidates,
        phase,
        strategy,
        strategy_path,
        strategy_key,
        diagnostics,
    ) && min_results::evaluate_discovery(
        candidates,
        phase,
        strategy,
        strategy_path,
        strategy_key,
        diagnostics,
    )
}

pub fn evaluate_detail_strategy_acceptance(
    patch: &DetailPatch,
    phase: Option<&CompiledAcceptance>,
    strategy: Option<&CompiledAcceptance>,
    strategy_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> StrategyAcceptanceFact {
    StrategyAcceptanceFact {
        satisfied: detail_acceptance_satisfied(
            patch,
            phase,
            strategy,
            strategy_path,
            strategy_key,
            diagnostics,
        ),
    }
}

pub fn evaluate_detail_final_acceptance(
    patch: &DetailPatch,
    phase: Option<&CompiledAcceptance>,
    diagnostics: &mut Diagnostics,
) -> FinalPhaseAcceptanceFact {
    FinalPhaseAcceptanceFact {
        satisfied: detail_acceptance_satisfied(patch, phase, None, "/detail", None, diagnostics),
    }
}

fn detail_acceptance_satisfied(
    patch: &DetailPatch,
    phase: Option<&CompiledAcceptance>,
    strategy: Option<&CompiledAcceptance>,
    strategy_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> bool {
    required_fields::evaluate_detail(
        patch,
        phase,
        strategy,
        strategy_path,
        strategy_key,
        diagnostics,
    ) && min_description_length::evaluate_detail(
        patch,
        phase,
        strategy,
        strategy_path,
        strategy_key,
        diagnostics,
    )
}

pub fn validate_detail_acceptance_request<'a>(
    phase: Option<&'a CompiledAcceptance>,
    strategies: impl IntoIterator<Item = (String, String, Option<&'a CompiledAcceptance>)>,
    requested: &RequestedDetailFields,
) -> Option<Diagnostic> {
    let mut plans = Vec::new();
    if let Some(plan) = phase {
        plans.push(("/detail".to_string(), None, plan));
    }
    plans.extend(
        strategies
            .into_iter()
            .filter_map(|(path, key, plan)| plan.map(|plan| (path, Some(key), plan))),
    );
    for (path, strategy_key, plan) in plans {
        if let Some(diagnostic) = required_fields::validate_detail_request(
            plan,
            &path,
            strategy_key.as_deref(),
            requested,
        ) {
            return Some(diagnostic);
        }
        if let Some(diagnostic) = min_description_length::validate_detail_request(
            plan,
            &path,
            strategy_key.as_deref(),
            requested,
        ) {
            return Some(diagnostic);
        }
    }
    None
}

fn acceptance_diagnostic(
    code: &str,
    message: &str,
    path: String,
    strategy_key: Option<&str>,
    details: serde_json::Value,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: code.into(),
        message: message.into(),
        severity: DiagnosticSeverity::Error,
        path,
        strategy_key: strategy_key.map(str::to_string),
        details: Some(details),
    }
}

pub fn completeness_serde_shapes() -> Vec<crate::definition::primitives::completeness::SerdeShape> {
    use crate::definition::primitives::completeness::{
        serde_shape,
        AuthoredShapeKind::ParentOption,
        Family::Acceptance,
        PrimitiveContext::{Detail, Discovery},
    };
    vec![
        serde_shape(
            Acceptance,
            "requiredFields",
            &[Discovery, Detail],
            ParentOption,
            "src-tauri/crates/source-profile-dsl/src/definition/documents/strategy.rs",
        ),
        serde_shape(
            Acceptance,
            "minDescriptionLength",
            &[Discovery, Detail],
            ParentOption,
            "src-tauri/crates/source-profile-dsl/src/definition/documents/strategy.rs",
        ),
        serde_shape(
            Acceptance,
            "minResults",
            &[Discovery],
            ParentOption,
            "src-tauri/crates/source-profile-dsl/src/definition/documents/strategy.rs",
        ),
    ]
}

fn witness_acceptance_required() {
    fn check(v: &CompiledAcceptance) {
        let _ = &v.required_fields;
    }
    let _ = check as fn(&CompiledAcceptance);
}
fn witness_acceptance_description() {
    fn check(v: &CompiledAcceptance) {
        let _ = &v.min_description_length;
    }
    let _ = check as fn(&CompiledAcceptance);
}
fn witness_acceptance_results() {
    fn check(v: &CompiledAcceptance) {
        let _ = &v.min_results;
    }
    let _ = check as fn(&CompiledAcceptance);
}

pub fn completeness_compiled_registrations(
) -> Vec<crate::definition::primitives::completeness::CompiledRegistration> {
    use crate::definition::primitives::completeness::{
        AuthoredShapeKind::ParentOption,
        CompiledRegistration,
        Family::Acceptance,
        Owner::P11,
        PrimitiveContext::{Detail, Discovery},
    };
    vec![
        CompiledRegistration {
            family: Acceptance,
            key: "requiredFields",
            contexts: &[Discovery, Detail],
            owner: P11,
            canonical_file:
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/acceptance/mod.rs",
            shape: ParentOption,
            compiled_identity: "CompiledAcceptance.required_fields",
            witness: witness_acceptance_required,
            behavior_bearing: false,
        },
        CompiledRegistration {
            family: Acceptance,
            key: "minDescriptionLength",
            contexts: &[Discovery, Detail],
            owner: P11,
            canonical_file:
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/acceptance/mod.rs",
            shape: ParentOption,
            compiled_identity: "CompiledAcceptance.min_description_length",
            witness: witness_acceptance_description,
            behavior_bearing: false,
        },
        CompiledRegistration {
            family: Acceptance,
            key: "minResults",
            contexts: &[Discovery],
            owner: P11,
            canonical_file:
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/acceptance/mod.rs",
            shape: ParentOption,
            compiled_identity: "CompiledAcceptance.min_results",
            witness: witness_acceptance_results,
            behavior_bearing: false,
        },
    ]
}
