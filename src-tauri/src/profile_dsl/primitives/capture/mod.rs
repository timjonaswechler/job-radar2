use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::value::{
    CompiledValue, CompiledValueResult, ValueCompileContext, ValueCompileError,
    ValueEvaluationError, ValueShape,
};
mod named;

pub use named::{
    compile_named_capture_rule, compile_named_pattern, evaluate_named_pattern, CaptureRule,
    Captures, CompiledCaptureRule, CompiledNamedPattern,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureDescriptor {
    pub key: &'static str,
}

pub const CAPTURE_DESCRIPTOR: CaptureDescriptor = named::DESCRIPTOR;

pub fn capture_descriptors() -> &'static [CaptureDescriptor] {
    std::slice::from_ref(&CAPTURE_DESCRIPTOR)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CaptureRegistryError {
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

pub fn validate_capture_registration_keys(
    schema_keys: &[String],
    serde_keys: &[String],
    registration_keys: &[String],
) -> Result<(), CaptureRegistryError> {
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
            return Err(CaptureRegistryError::Duplicate {
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
            return Err(CaptureRegistryError::Missing {
                layer,
                keys: missing,
            });
        }
        let extra = actual.difference(&expected).cloned().collect::<Vec<_>>();
        if !extra.is_empty() {
            return Err(CaptureRegistryError::Extra { layer, keys: extra });
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureCompileErrorKind {
    Value,
    SourceShape,
    InvalidRegex,
    NamedGroupMissing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaptureCompileError {
    pub rule_index: usize,
    pub capture_key: String,
    pub kind: CaptureCompileErrorKind,
    pub path: String,
    pub message: String,
    pub value_error: Option<ValueCompileError>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompiledCapturePlan(Vec<CompiledCaptureRule>);

impl Serialize for CompiledCapturePlan {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CompiledCapturePlan {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let rules = Vec::<CompiledCaptureRule>::deserialize(deserializer)?;
        let mut keys = BTreeSet::new();
        if let Some(duplicate) = rules
            .iter()
            .map(|rule| rule.key())
            .find(|key| !keys.insert((*key).to_string()))
        {
            return Err(serde::de::Error::custom(format!(
                "compiled Capture plan contains duplicate key `{duplicate}`"
            )));
        }
        Ok(Self(rules))
    }
}

impl CompiledCapturePlan {
    pub fn rules(&self) -> &[CompiledCaptureRule] {
        &self.0
    }

    pub fn references_source_name(&self) -> bool {
        self.0
            .iter()
            .any(CompiledCaptureRule::references_source_name)
    }
}

pub fn compile_captures(
    captures: &Captures,
    context: &ValueCompileContext,
) -> Result<CompiledCapturePlan, CaptureCompileError> {
    captures
        .iter()
        .enumerate()
        .map(|(rule_index, (key, rule))| compile_named_capture_rule(rule_index, key, rule, context))
        .collect::<Result<Vec<_>, _>>()
        .map(CompiledCapturePlan)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaptureOutput {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureEvaluationErrorKind {
    Value,
    SourceMissing,
    PatternNotMatched,
    NamedGroupUnmatched,
    Empty,
}

impl CaptureEvaluationErrorKind {
    pub const fn diagnostic(self) -> (&'static str, &'static str) {
        match self {
            Self::Value => (
                "capture_value_failed",
                "Capture source Value evaluation failed",
            ),
            Self::SourceMissing => (
                "capture_source_missing",
                "Capture source did not resolve to text",
            ),
            Self::PatternNotMatched => (
                "capture_not_matched",
                "Capture pattern did not match runtime text",
            ),
            Self::NamedGroupUnmatched => (
                "capture_named_group_unmatched",
                "Capture named group was unmatched",
            ),
            Self::Empty => (
                "capture_empty",
                "Capture resolved to empty text after trimming",
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaptureEvaluationError {
    pub rule_index: usize,
    pub capture_key: String,
    pub kind: CaptureEvaluationErrorKind,
    pub value_error: Option<ValueEvaluationError>,
}

pub fn evaluate_compiled_captures<F>(
    plan: &CompiledCapturePlan,
    mut evaluate: F,
) -> Result<Vec<CaptureOutput>, Vec<CaptureEvaluationError>>
where
    F: FnMut(&CompiledValue) -> Result<CompiledValueResult, ValueEvaluationError>,
{
    let mut outputs = Vec::with_capacity(plan.0.len());
    let mut errors = Vec::new();

    for (rule_index, rule) in plan.0.iter().enumerate() {
        match rule.execute(&mut evaluate) {
            Ok(value) => outputs.push(CaptureOutput {
                key: rule.key().to_string(),
                value,
            }),
            Err((kind, value_error)) => errors.push(CaptureEvaluationError {
                rule_index,
                capture_key: rule.key().to_string(),
                kind,
                value_error,
            }),
        }
    }

    if errors.is_empty() {
        Ok(outputs)
    } else {
        Err(errors)
    }
}

fn compile_value_source(
    rule_index: usize,
    key: &str,
    rule: &CaptureRule,
    context: &ValueCompileContext,
) -> Result<CompiledValue, CaptureCompileError> {
    let value =
        super::value::compile_value(&rule.from, context).map_err(|error| CaptureCompileError {
            rule_index,
            capture_key: key.to_string(),
            kind: CaptureCompileErrorKind::Value,
            path: format!("/from{}", error.path),
            message: error.message.clone(),
            value_error: Some(error),
        })?;
    if value.shape() != ValueShape::Scalar {
        return Err(CaptureCompileError {
            rule_index,
            capture_key: key.to_string(),
            kind: CaptureCompileErrorKind::SourceShape,
            path: "/from".to_string(),
            message: "Capture source must resolve to a scalar Value".to_string(),
            value_error: None,
        });
    }
    Ok(value)
}
