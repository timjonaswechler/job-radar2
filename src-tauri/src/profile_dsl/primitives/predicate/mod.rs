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
impl Predicate {
    pub const fn kind(&self) -> PredicateKind {
        match self {
            Self::NonEmpty(_) => PredicateKind::NonEmpty,
            Self::Regex(_) => PredicateKind::Regex,
            Self::Equal(_) => PredicateKind::Equal,
        }
    }
    pub(crate) fn operands(
        &self,
    ) -> Vec<(
        &'static str,
        &crate::profile_dsl::documents::FieldExpression,
    )> {
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
