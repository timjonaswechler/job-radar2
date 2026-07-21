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
pub struct CompiledCaptureRule {
    key: String,
    from: CompiledValue,
    pattern: String,
    regex: Regex,
}

impl CompiledCaptureRule {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn from(&self) -> &CompiledValue {
        &self.from
    }

    pub fn pattern(&self) -> &str {
        &self.pattern
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
        let Some(captures) = self.regex.captures(&source) else {
            return Err((CaptureEvaluationErrorKind::PatternNotMatched, None));
        };
        let Some(value) = captures.name(&self.key) else {
            return Err((CaptureEvaluationErrorKind::NamedGroupUnmatched, None));
        };
        let value = value.as_str().trim();
        if value.is_empty() {
            return Err((CaptureEvaluationErrorKind::Empty, None));
        }
        Ok(value.to_string())
    }
}

impl PartialEq for CompiledCaptureRule {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.from == other.from && self.pattern == other.pattern
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
            pattern: self.pattern.clone(),
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
        let regex = Regex::new(&value.pattern).map_err(serde::de::Error::custom)?;
        if !regex
            .capture_names()
            .flatten()
            .any(|name| name == value.key)
        {
            return Err(serde::de::Error::custom(
                "compiled Capture pattern does not declare its selected named group",
            ));
        }
        Ok(Self {
            key: value.key,
            from: value.from,
            pattern: value.pattern,
            regex,
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
    let regex = Regex::new(&rule.pattern).map_err(|_| CaptureCompileError {
        rule_index,
        capture_key: key.to_string(),
        kind: CaptureCompileErrorKind::InvalidRegex,
        path: "/pattern".to_string(),
        message: "Capture pattern is invalid Rust regex syntax".to_string(),
        value_error: None,
    })?;
    if !regex.capture_names().flatten().any(|name| name == key) {
        return Err(CaptureCompileError {
            rule_index,
            capture_key: key.to_string(),
            kind: CaptureCompileErrorKind::NamedGroupMissing,
            path: "/pattern".to_string(),
            message: "Capture pattern must declare a named group matching its Capture key"
                .to_string(),
            value_error: None,
        });
    }
    Ok(CompiledCaptureRule {
        key: key.to_string(),
        from,
        pattern: rule.pattern.clone(),
        regex,
    })
}
