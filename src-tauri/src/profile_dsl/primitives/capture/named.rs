use indexmap::IndexMap;
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{
    compile_value_source, CaptureCompileError, CaptureCompileErrorKind, CaptureDescriptor,
    CaptureEvaluationErrorKind,
};
use crate::profile_dsl::{
    documents::FieldExpression,
    primitives::value::{
        CompiledValue, CompiledValueResult, ValueCompileContext, ValueEvaluationError, ValueShape,
    },
};

pub const DESCRIPTOR: CaptureDescriptor = CaptureDescriptor { key: "capture" };

pub type Captures = IndexMap<String, CaptureRule>;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureRule {
    pub from: FieldExpression,
    pub pattern: String,
}

impl CaptureRule {
    pub const PRIMITIVE_KEY: &'static str = DESCRIPTOR.key;
}

#[derive(Clone, Debug)]
pub struct CompiledNamedPattern {
    pattern: String,
    keys: Vec<String>,
    regex: Regex,
}

impl PartialEq for CompiledNamedPattern {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern && self.keys == other.keys
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NamedPatternCompileError {
    InvalidRegex,
    DuplicateKey(String),
    NamedGroupMissing(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NamedPatternEvaluationError {
    NamedGroupUnmatched(String),
    Empty(String),
}

pub fn compile_named_pattern(
    pattern: &str,
    keys: &[String],
) -> Result<CompiledNamedPattern, NamedPatternCompileError> {
    let regex = Regex::new(pattern).map_err(|_| NamedPatternCompileError::InvalidRegex)?;
    let mut unique = std::collections::BTreeSet::new();
    for key in keys {
        if !unique.insert(key.clone()) {
            return Err(NamedPatternCompileError::DuplicateKey(key.clone()));
        }
        if !regex.capture_names().flatten().any(|name| name == key) {
            return Err(NamedPatternCompileError::NamedGroupMissing(key.clone()));
        }
    }
    Ok(CompiledNamedPattern {
        pattern: pattern.to_string(),
        keys: keys.to_vec(),
        regex,
    })
}

pub fn evaluate_named_pattern(
    pattern: &CompiledNamedPattern,
    value: &str,
) -> Result<Option<Vec<super::CaptureOutput>>, NamedPatternEvaluationError> {
    let Some(captures) = pattern.regex.captures(value) else {
        return Ok(None);
    };
    pattern
        .keys
        .iter()
        .map(|key| {
            let value = captures
                .name(key)
                .ok_or_else(|| NamedPatternEvaluationError::NamedGroupUnmatched(key.clone()))?
                .as_str()
                .trim();
            if value.is_empty() {
                return Err(NamedPatternEvaluationError::Empty(key.clone()));
            }
            Ok(super::CaptureOutput {
                key: key.clone(),
                value: value.to_string(),
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

#[derive(Clone, Debug)]
pub struct CompiledCaptureRule {
    key: String,
    from: CompiledValue,
    named_pattern: CompiledNamedPattern,
}

impl CompiledCaptureRule {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn from(&self) -> &CompiledValue {
        &self.from
    }

    pub fn pattern(&self) -> &str {
        &self.named_pattern.pattern
    }

    pub(crate) fn references_source_name(&self) -> bool {
        self.from.references_source_name()
    }

    pub(super) fn execute<F>(
        &self,
        evaluate: &mut F,
    ) -> Result<String, (CaptureEvaluationErrorKind, Option<ValueEvaluationError>)>
    where
        F: FnMut(&CompiledValue) -> Result<CompiledValueResult, ValueEvaluationError>,
    {
        let source = match evaluate(&self.from) {
            Ok(CompiledValueResult::Scalar(Some(value))) => value,
            Ok(CompiledValueResult::Scalar(None)) => {
                return Err((CaptureEvaluationErrorKind::SourceMissing, None));
            }
            Ok(CompiledValueResult::Sequence(_)) => {
                return Err((CaptureEvaluationErrorKind::SourceMissing, None));
            }
            Err(error) => return Err((CaptureEvaluationErrorKind::Value, Some(error))),
        };
        match evaluate_named_pattern(&self.named_pattern, &source) {
            Ok(Some(mut outputs)) => Ok(outputs.remove(0).value),
            Ok(None) => Err((CaptureEvaluationErrorKind::PatternNotMatched, None)),
            Err(NamedPatternEvaluationError::NamedGroupUnmatched(_)) => {
                Err((CaptureEvaluationErrorKind::NamedGroupUnmatched, None))
            }
            Err(NamedPatternEvaluationError::Empty(_)) => {
                Err((CaptureEvaluationErrorKind::Empty, None))
            }
        }
    }
}

impl PartialEq for CompiledCaptureRule {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.from == other.from
            && self.named_pattern == other.named_pattern
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SerializedCompiledCaptureRule {
    key: String,
    from: CompiledValue,
    pattern: String,
}

impl Serialize for CompiledCaptureRule {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        SerializedCompiledCaptureRule {
            key: self.key.clone(),
            from: self.from.clone(),
            pattern: self.named_pattern.pattern.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CompiledCaptureRule {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = SerializedCompiledCaptureRule::deserialize(deserializer)?;
        if value.from.shape() != ValueShape::Scalar {
            return Err(serde::de::Error::custom(
                "compiled Capture source must be scalar",
            ));
        }
        let named_pattern = compile_named_pattern(&value.pattern, std::slice::from_ref(&value.key))
            .map_err(|error| match error {
                NamedPatternCompileError::InvalidRegex => serde::de::Error::custom(
                    "compiled Capture pattern is invalid Rust regex syntax",
                ),
                NamedPatternCompileError::DuplicateKey(_)
                | NamedPatternCompileError::NamedGroupMissing(_) => serde::de::Error::custom(
                    "compiled Capture pattern must declare its selected named group",
                ),
            })?;
        Ok(Self {
            key: value.key,
            from: value.from,
            named_pattern,
        })
    }
}

pub fn compile_named_capture_rule(
    rule_index: usize,
    key: &str,
    rule: &CaptureRule,
    context: &ValueCompileContext,
) -> Result<CompiledCaptureRule, CaptureCompileError> {
    let from = compile_value_source(rule_index, key, rule, context)?;
    let named_pattern =
        compile_named_pattern(&rule.pattern, &[key.to_string()]).map_err(|error| {
            CaptureCompileError {
                rule_index,
                capture_key: key.to_string(),
                kind: match &error {
                    NamedPatternCompileError::InvalidRegex => CaptureCompileErrorKind::InvalidRegex,
                    NamedPatternCompileError::DuplicateKey(_)
                    | NamedPatternCompileError::NamedGroupMissing(_) => {
                        CaptureCompileErrorKind::NamedGroupMissing
                    }
                },
                path: "/pattern".to_string(),
                message: match &error {
                    NamedPatternCompileError::InvalidRegex => {
                        "Capture pattern is invalid Rust regex syntax".to_string()
                    }
                    NamedPatternCompileError::DuplicateKey(_)
                    | NamedPatternCompileError::NamedGroupMissing(_) => {
                        "Capture pattern must declare a named group matching its Capture key"
                            .to_string()
                    }
                },
                value_error: None,
            }
        })?;
    Ok(CompiledCaptureRule {
        key: key.to_string(),
        from,
        named_pattern,
    })
}
