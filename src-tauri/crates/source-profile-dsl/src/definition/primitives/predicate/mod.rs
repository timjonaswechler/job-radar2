use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::value::{
    evaluate_detail_output_value, evaluate_discovery_output_value, CompiledValue,
    CompiledValueResult, DetailMatchFilterOutputValueContext, DiscoveryFilterOutputValueContext,
    ValueCompileContext, ValueCompileError, ValueEvaluationError,
};

mod equal;
mod literal_contains;
mod non_empty;
mod regex;

pub use equal::{values_equal, Equal, EqualPlan};
pub use literal_contains::literal_contains;
pub use non_empty::{NonEmpty, NonEmptyPlan};
pub use regex::{compile_regex, CompiledRegex, RegexPredicate, RegexPredicatePlan};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Predicate {
    NonEmpty(NonEmpty),
    Regex(RegexPredicate),
    Equal(Equal),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PredicateKind {
    NonEmpty,
    Regex,
    Equal,
}
impl PredicateKind {
    pub const ALL: [Self; 3] = [Self::NonEmpty, Self::Regex, Self::Equal];
    pub const fn key(self) -> &'static str {
        match self {
            Self::NonEmpty => "non_empty",
            Self::Regex => "regex",
            Self::Equal => "equal",
        }
    }
}
pub fn deserialize_where_predicates<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<Predicate>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let predicates = Vec::<Predicate>::deserialize(deserializer)?;
    if predicates
        .iter()
        .all(|predicate| matches!(predicate, Predicate::NonEmpty(_) | Predicate::Regex(_)))
    {
        Ok(Some(predicates))
    } else {
        Err(serde::de::Error::custom(
            "where admits only non_empty and regex predicates",
        ))
    }
}

pub fn deserialize_detail_match<'de, D>(deserializer: D) -> Result<Option<Predicate>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let predicate = Predicate::deserialize(deserializer)?;
    if matches!(predicate, Predicate::Equal(_)) {
        Ok(Some(predicate))
    } else {
        Err(serde::de::Error::custom(
            "detail match admits only the equal predicate",
        ))
    }
}

impl Predicate {
    pub const fn kind(&self) -> PredicateKind {
        match self {
            Self::NonEmpty(_) => PredicateKind::NonEmpty,
            Self::Regex(_) => PredicateKind::Regex,
            Self::Equal(_) => PredicateKind::Equal,
        }
    }
    pub fn operands(&self) -> Vec<(&'static str, &crate::definition::documents::FieldExpression)> {
        match self {
            Self::NonEmpty(predicate) => vec![("field", &predicate.field)],
            Self::Regex(predicate) => vec![("field", &predicate.field)],
            Self::Equal(predicate) => {
                vec![("left", &predicate.left), ("right", &predicate.right)]
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PredicateDescriptor {
    pub key: &'static str,
}
const PREDICATE_DESCRIPTORS: [PredicateDescriptor; 3] =
    [non_empty::DESCRIPTOR, regex::DESCRIPTOR, equal::DESCRIPTOR];
pub fn predicate_descriptors() -> &'static [PredicateDescriptor] {
    &PREDICATE_DESCRIPTORS
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PredicateRegistryError {
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
pub fn validate_predicate_registration_keys(
    schema_keys: &[String],
    serde_keys: &[String],
    registration_keys: &[String],
) -> Result<(), PredicateRegistryError> {
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
            return Err(PredicateRegistryError::Duplicate {
                layer,
                keys: duplicates,
            });
        }
    }
    let expected = schema_keys.iter().cloned().collect::<BTreeSet<_>>();
    for (layer, keys) in [("serde", serde_keys), ("registration", registration_keys)] {
        let actual = keys.iter().cloned().collect::<BTreeSet<_>>();
        let missing = expected.difference(&actual).cloned().collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(PredicateRegistryError::Missing {
                layer,
                keys: missing,
            });
        }
        let extra = actual.difference(&expected).cloned().collect::<Vec<_>>();
        if !extra.is_empty() {
            return Err(PredicateRegistryError::Extra { layer, keys: extra });
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PredicatePlacement {
    Where,
    DetailMatch,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredicateCompileContext {
    pub placement: PredicatePlacement,
    pub value: ValueCompileContext,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PredicateCompileErrorKind {
    Placement,
    Value,
    OperandShape,
    InvalidRegex,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredicateCompileError {
    pub kind: PredicateCompileErrorKind,
    pub path: String,
    pub message: String,
    pub value_error: Option<ValueCompileError>,
}
impl std::fmt::Display for PredicateCompileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompiledPredicate {
    NonEmpty(NonEmptyPlan),
    Regex(RegexPredicatePlan),
    Equal(EqualPlan),
}
impl CompiledPredicate {
    pub const fn kind(&self) -> PredicateKind {
        match self {
            Self::NonEmpty(_) => PredicateKind::NonEmpty,
            Self::Regex(_) => PredicateKind::Regex,
            Self::Equal(_) => PredicateKind::Equal,
        }
    }
    pub fn references_source_name(&self) -> bool {
        match self {
            Self::NonEmpty(plan) => plan.references_source_name(),
            Self::Regex(plan) => plan.references_source_name(),
            Self::Equal(plan) => plan.references_source_name(),
        }
    }
}

fn value_error(operand: &str, error: ValueCompileError) -> PredicateCompileError {
    PredicateCompileError {
        kind: PredicateCompileErrorKind::Value,
        path: format!("/{operand}{}", error.path),
        message: error.message.clone(),
        value_error: Some(error),
    }
}
pub fn compile_predicate(
    predicate: &Predicate,
    context: &PredicateCompileContext,
) -> Result<CompiledPredicate, PredicateCompileError> {
    match predicate {
        Predicate::NonEmpty(predicate) => {
            non_empty::compile(predicate, context).map(CompiledPredicate::NonEmpty)
        }
        Predicate::Regex(predicate) => {
            regex::compile(predicate, context).map(CompiledPredicate::Regex)
        }
        Predicate::Equal(predicate) => {
            equal::compile(predicate, context).map(CompiledPredicate::Equal)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredicateEvaluationError {
    pub operand_path: &'static str,
    pub source: ValueEvaluationError,
}

pub fn evaluate_compiled_predicate<F>(
    predicate: &CompiledPredicate,
    mut evaluate: F,
) -> Result<bool, PredicateEvaluationError>
where
    F: FnMut(&CompiledValue) -> Result<CompiledValueResult, ValueEvaluationError>,
{
    match predicate {
        CompiledPredicate::NonEmpty(plan) => non_empty::execute(plan, &mut evaluate),
        CompiledPredicate::Regex(plan) => regex::execute(plan, &mut evaluate),
        CompiledPredicate::Equal(plan) => equal::execute(plan, &mut evaluate),
    }
}
pub fn evaluate_discovery_predicate(
    predicate: &CompiledPredicate,
    context: &DiscoveryFilterOutputValueContext<'_, '_, '_>,
) -> Result<bool, PredicateEvaluationError> {
    evaluate_compiled_predicate(predicate, |value| {
        evaluate_discovery_output_value(value, context)
    })
}
pub fn evaluate_detail_predicate(
    predicate: &CompiledPredicate,
    context: &DetailMatchFilterOutputValueContext<'_, '_, '_>,
) -> Result<bool, PredicateEvaluationError> {
    evaluate_compiled_predicate(predicate, |value| {
        evaluate_detail_output_value(value, context)
    })
}

pub fn completeness_serde_shapes() -> Vec<crate::definition::primitives::completeness::SerdeShape> {
    use crate::definition::primitives::completeness::{
        serde_shape,
        AuthoredShapeKind::{ParentOption, Tagged},
        Family::Predicate,
        PrimitiveContext::*,
    };
    vec![
        serde_shape(
            Predicate,
            "non_empty",
            &[
                DiscoveryWhere,
                DetailWhere,
                DetectionHttpContains,
                DetectionBrowserContains,
            ],
            Tagged,
            "src-tauri/crates/source-profile-dsl/src/definition/primitives/predicate/mod.rs",
        ),
        serde_shape(
            Predicate,
            "regex",
            &[
                DiscoveryWhere,
                DetailWhere,
                DetectionHttpRegex,
                DetectionBrowserRegex,
            ],
            Tagged,
            "src-tauri/crates/source-profile-dsl/src/definition/primitives/predicate/mod.rs",
        ),
        serde_shape(
            Predicate,
            "equal",
            &[DetailMatch, DetectionHttpStatus],
            Tagged,
            "src-tauri/crates/source-profile-dsl/src/definition/primitives/predicate/mod.rs",
        ),
        serde_shape(
            Predicate,
            "non_empty.field",
            &[DiscoveryWhere, DetailWhere],
            ParentOption,
            "src-tauri/crates/source-profile-dsl/src/definition/primitives/predicate/mod.rs",
        ),
        serde_shape(
            Predicate,
            "regex.field",
            &[DiscoveryWhere, DetailWhere],
            ParentOption,
            "src-tauri/crates/source-profile-dsl/src/definition/primitives/predicate/mod.rs",
        ),
        serde_shape(
            Predicate,
            "regex.pattern",
            &[DiscoveryWhere, DetailWhere],
            ParentOption,
            "src-tauri/crates/source-profile-dsl/src/definition/primitives/predicate/mod.rs",
        ),
        serde_shape(
            Predicate,
            "detail.match",
            &[DetailMatch],
            ParentOption,
            "src-tauri/crates/source-profile-dsl/src/definition/documents/detail.rs",
        ),
        serde_shape(
            Predicate,
            "detail.match.left",
            &[DetailMatch],
            ParentOption,
            "src-tauri/crates/source-profile-dsl/src/definition/documents/detail.rs",
        ),
        serde_shape(
            Predicate,
            "detail.match.right",
            &[DetailMatch],
            ParentOption,
            "src-tauri/crates/source-profile-dsl/src/definition/documents/detail.rs",
        ),
    ]
}

fn witness_non_empty() {
    fn check(value: &CompiledPredicate) {
        if let CompiledPredicate::NonEmpty(plan) = value {
            let _ = plan.field();
        }
    }
    let _ = check as fn(&CompiledPredicate);
}

fn witness_regex() {
    fn check(value: &CompiledPredicate) {
        if let CompiledPredicate::Regex(plan) = value {
            let _ = (plan.field(), plan.pattern());
        }
    }
    let _ = check as fn(&CompiledPredicate);
}

fn witness_equal() {
    fn check(value: &CompiledPredicate) {
        if let CompiledPredicate::Equal(plan) = value {
            let _ = (plan.left(), plan.right());
        }
    }
    let _ = check as fn(&CompiledPredicate);
}

fn witness_non_empty_field() {
    fn check(value: &CompiledPredicate) {
        if let CompiledPredicate::NonEmpty(plan) = value {
            let _ = plan.field();
        }
    }
    let _ = check as fn(&CompiledPredicate);
}
fn witness_regex_field() {
    fn check(value: &CompiledPredicate) {
        if let CompiledPredicate::Regex(plan) = value {
            let _ = plan.field();
        }
    }
    let _ = check as fn(&CompiledPredicate);
}
fn witness_regex_pattern() {
    fn check(value: &CompiledPredicate) {
        if let CompiledPredicate::Regex(plan) = value {
            let _ = plan.pattern();
        }
    }
    let _ = check as fn(&CompiledPredicate);
}
fn witness_detail_match() {
    fn check(value: &crate::definition::execution_plan::detail::ExecutionPlanDetailStrategy) {
        let _ = &value.field_match;
    }
    let _ = check as fn(&crate::definition::execution_plan::detail::ExecutionPlanDetailStrategy);
}
fn witness_detail_match_left() {
    fn check(value: &crate::definition::execution_plan::detail::ExecutionPlanDetailStrategy) {
        if let Some(CompiledPredicate::Equal(plan)) = &value.field_match {
            let _ = plan.left();
        }
    }
    let _ = check as fn(&crate::definition::execution_plan::detail::ExecutionPlanDetailStrategy);
}
fn witness_detail_match_right() {
    fn check(value: &crate::definition::execution_plan::detail::ExecutionPlanDetailStrategy) {
        if let Some(CompiledPredicate::Equal(plan)) = &value.field_match {
            let _ = plan.right();
        }
    }
    let _ = check as fn(&crate::definition::execution_plan::detail::ExecutionPlanDetailStrategy);
}

pub fn completeness_compiled_registrations(
) -> Vec<crate::definition::primitives::completeness::CompiledRegistration> {
    use crate::definition::primitives::completeness::{
        AuthoredShapeKind::{ParentOption, Tagged},
        CompiledRegistration,
        Family::Predicate,
        Owner::P07,
        PrimitiveContext::*,
    };
    vec![
        CompiledRegistration {
            family: Predicate,
            key: "non_empty",
            contexts: &[
                DiscoveryWhere,
                DetailWhere,
                DetectionHttpContains,
                DetectionBrowserContains,
            ],
            owner: P07,
            canonical_file: "src-tauri/crates/source-profile-dsl/src/definition/primitives/predicate/non_empty.rs",
            shape: Tagged,
            compiled_identity: "CompiledPredicate::NonEmpty",
            witness: witness_non_empty,
            behavior_bearing: false,
        },
        CompiledRegistration {
            family: Predicate,
            key: "regex",
            contexts: &[
                DiscoveryWhere,
                DetailWhere,
                DetectionHttpRegex,
                DetectionBrowserRegex,
            ],
            owner: P07,
            canonical_file: "src-tauri/crates/source-profile-dsl/src/definition/primitives/predicate/regex.rs",
            shape: Tagged,
            compiled_identity: "CompiledPredicate::Regex",
            witness: witness_regex,
            behavior_bearing: false,
        },
        CompiledRegistration {
            family: Predicate,
            key: "equal",
            contexts: &[DetailMatch, DetectionHttpStatus],
            owner: P07,
            canonical_file: "src-tauri/crates/source-profile-dsl/src/definition/primitives/predicate/equal.rs",
            shape: Tagged,
            compiled_identity: "CompiledPredicate::Equal",
            witness: witness_equal,
            behavior_bearing: false,
        },
        CompiledRegistration {
            family: Predicate,
            key: "non_empty.field",
            contexts: &[DiscoveryWhere, DetailWhere],
            owner: P07,
            canonical_file: "src-tauri/crates/source-profile-dsl/src/definition/primitives/predicate/non_empty.rs",
            shape: ParentOption,
            compiled_identity: "CompiledPredicate::NonEmpty.field",
            witness: witness_non_empty_field,
            behavior_bearing: false,
        },
        CompiledRegistration {
            family: Predicate,
            key: "regex.field",
            contexts: &[DiscoveryWhere, DetailWhere],
            owner: P07,
            canonical_file: "src-tauri/crates/source-profile-dsl/src/definition/primitives/predicate/regex.rs",
            shape: ParentOption,
            compiled_identity: "CompiledPredicate::Regex.field",
            witness: witness_regex_field,
            behavior_bearing: false,
        },
        CompiledRegistration {
            family: Predicate,
            key: "regex.pattern",
            contexts: &[DiscoveryWhere, DetailWhere],
            owner: P07,
            canonical_file: "src-tauri/crates/source-profile-dsl/src/definition/primitives/predicate/regex.rs",
            shape: ParentOption,
            compiled_identity: "CompiledPredicate::Regex.pattern",
            witness: witness_regex_pattern,
            behavior_bearing: false,
        },
        CompiledRegistration {
            family: Predicate,
            key: "detail.match",
            contexts: &[DetailMatch],
            owner: P07,
            canonical_file: "src-tauri/crates/source-profile-dsl/src/definition/primitives/predicate/mod.rs",
            shape: ParentOption,
            compiled_identity: "ExecutionPlanDetailStrategy.field_match::Equal",
            witness: witness_detail_match,
            behavior_bearing: false,
        },
        CompiledRegistration {
            family: Predicate,
            key: "detail.match.left",
            contexts: &[DetailMatch],
            owner: P07,
            canonical_file: "src-tauri/crates/source-profile-dsl/src/definition/primitives/predicate/mod.rs",
            shape: ParentOption,
            compiled_identity: "ExecutionPlanDetailStrategy.field_match::Equal.left",
            witness: witness_detail_match_left,
            behavior_bearing: false,
        },
        CompiledRegistration {
            family: Predicate,
            key: "detail.match.right",
            contexts: &[DetailMatch],
            owner: P07,
            canonical_file: "src-tauri/crates/source-profile-dsl/src/definition/primitives/predicate/mod.rs",
            shape: ParentOption,
            compiled_identity: "ExecutionPlanDetailStrategy.field_match::Equal.right",
            witness: witness_detail_match_right,
            behavior_bearing: false,
        },
    ]
}
