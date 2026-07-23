use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{
    value_error, PredicateCompileContext, PredicateCompileError, PredicateCompileErrorKind,
    PredicateDescriptor, PredicateEvaluationError, PredicatePlacement,
};
use crate::profile_dsl::{
    documents::FieldExpression,
    primitives::value::{
        compile_value, CompiledValue, CompiledValueResult, ValueEvaluationError, ValueShape,
    },
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegexPredicate {
    pub field: FieldExpression,
    pub pattern: String,
}

pub const DESCRIPTOR: PredicateDescriptor = PredicateDescriptor { key: "regex" };

#[derive(Clone, Debug)]
pub struct CompiledRegex {
    pattern: String,
    regex: Regex,
}

impl CompiledRegex {
    pub fn pattern(&self) -> &str {
        &self.pattern
    }
    pub fn is_match(&self, value: &str) -> bool {
        !value.is_empty() && self.regex.is_match(value)
    }
}

impl PartialEq for CompiledRegex {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegexCompileError;

pub fn compile_regex(pattern: &str) -> Result<CompiledRegex, RegexCompileError> {
    Ok(CompiledRegex {
        pattern: pattern.to_string(),
        regex: Regex::new(pattern).map_err(|_| RegexCompileError)?,
    })
}

#[derive(Clone, Debug)]
pub struct RegexPredicatePlan {
    field: CompiledValue,
    regex: CompiledRegex,
}
impl RegexPredicatePlan {
    pub fn field(&self) -> &CompiledValue {
        &self.field
    }
    pub fn pattern(&self) -> &str {
        self.regex.pattern()
    }
    pub(super) fn references_source_name(&self) -> bool {
        self.field.references_source_name()
    }
}
impl PartialEq for RegexPredicatePlan {
    fn eq(&self, other: &Self) -> bool {
        self.field == other.field && self.regex == other.regex
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SerializedRegexPredicatePlan {
    field: CompiledValue,
    pattern: String,
}
impl Serialize for RegexPredicatePlan {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        SerializedRegexPredicatePlan {
            field: self.field.clone(),
            pattern: self.regex.pattern.clone(),
        }
        .serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for RegexPredicatePlan {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = SerializedRegexPredicatePlan::deserialize(deserializer)?;
        let regex =
            compile_regex(&value.pattern).map_err(|_| serde::de::Error::custom("invalid regex"))?;
        Ok(Self {
            field: value.field,
            regex,
        })
    }
}

pub(super) fn compile(
    predicate: &RegexPredicate,
    context: &PredicateCompileContext,
) -> Result<RegexPredicatePlan, PredicateCompileError> {
    if context.placement != PredicatePlacement::Where {
        return Err(PredicateCompileError {
            kind: PredicateCompileErrorKind::Placement,
            path: String::new(),
            message: format!(
                "regex predicate is unavailable in {:?} placement",
                context.placement
            ),
            value_error: None,
        });
    }
    let field = compile_value(&predicate.field, &context.value)
        .map_err(|error| value_error("field", error))?;
    if field.shape() != ValueShape::Scalar {
        return Err(PredicateCompileError {
            kind: PredicateCompileErrorKind::OperandShape,
            path: "/field".to_string(),
            message: "regex predicate field must resolve to a scalar Value".to_string(),
            value_error: None,
        });
    }
    let regex = compile_regex(&predicate.pattern).map_err(|_| PredicateCompileError {
        kind: PredicateCompileErrorKind::InvalidRegex,
        path: "/pattern".to_string(),
        message: "regex predicate pattern is invalid Rust regex syntax".to_string(),
        value_error: None,
    })?;
    Ok(RegexPredicatePlan { field, regex })
}

pub(super) fn execute<F>(
    plan: &RegexPredicatePlan,
    evaluate: &mut F,
) -> Result<bool, PredicateEvaluationError>
where
    F: FnMut(&CompiledValue) -> Result<CompiledValueResult, ValueEvaluationError>,
{
    let result = evaluate(&plan.field).map_err(|source| PredicateEvaluationError {
        operand_path: "/field",
        source,
    })?;
    Ok(match result {
        CompiledValueResult::Scalar(Some(value)) => plan.regex.is_match(&value),
        CompiledValueResult::Scalar(None) => false,
        CompiledValueResult::Sequence(_) => false,
    })
}
