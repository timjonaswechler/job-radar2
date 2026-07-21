use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::profile_dsl::{
    diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics},
    documents::strategy::Acceptance,
    occurrence::{DetailField, DetailPatch, PostingOccurrence, RequestedDetailFields},
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
pub(crate) struct StrategyAcceptanceFact {
    satisfied: bool,
}

impl StrategyAcceptanceFact {
    pub(crate) const fn is_satisfied(self) -> bool {
        self.satisfied
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FinalPhaseAcceptanceFact {
    satisfied: bool,
}

impl FinalPhaseAcceptanceFact {
    pub(crate) const fn is_satisfied(self) -> bool {
        self.satisfied
    }
}

pub(crate) fn evaluate_discovery_strategy_acceptance(
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

pub(crate) fn evaluate_discovery_final_acceptance(
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

pub(crate) fn evaluate_detail_strategy_acceptance(
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

pub(crate) fn evaluate_detail_final_acceptance(
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

pub(crate) fn validate_detail_acceptance_request<'a>(
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
