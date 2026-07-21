use super::TransformDescriptor;
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegexReplace {
    pub pattern: String,
    pub replacement: String,
}
#[derive(Clone, Debug)]
pub struct RegexReplacePlan {
    pattern: String,
    replacement: String,
    regex: Regex,
}
impl PartialEq for RegexReplacePlan {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern && self.replacement == other.replacement
    }
}
impl Serialize for RegexReplacePlan {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        RegexReplace {
            pattern: self.pattern.clone(),
            replacement: self.replacement.clone(),
        }
        .serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for RegexReplacePlan {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let authored = RegexReplace::deserialize(deserializer)?;
        compile(&authored).map_err(serde::de::Error::custom)
    }
}
pub const DESCRIPTOR: TransformDescriptor = TransformDescriptor {
    key: "regex_replace",
};
pub(super) fn compile(value: &RegexReplace) -> Result<RegexReplacePlan, String> {
    let regex = Regex::new(&value.pattern)
        .map_err(|error| format!("regex_replace transform pattern is invalid: {error}"))?;
    Ok(RegexReplacePlan {
        pattern: value.pattern.clone(),
        replacement: value.replacement.clone(),
        regex,
    })
}
pub(super) fn execute(plan: &RegexReplacePlan, value: String) -> String {
    plan.regex
        .replace_all(&value, plan.replacement.as_str())
        .to_string()
}
