use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use serde::{de::Visitor, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::json;

use super::select::{SelectedItem, SelectedSequence};
use crate::profile_dsl::diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity};

mod all;
mod first;
mod one;
mod optional;

pub use all::{All, AllPlan};
pub use first::{First, FirstPlan};
pub use one::{One, OnePlan};
pub use optional::{Optional, OptionalPlan};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cardinality {
    One(One),
    First(First),
    Optional(Optional),
    All(All),
}

impl Cardinality {
    pub const ALL: [Self; 4] = [
        Self::One(One),
        Self::First(First),
        Self::Optional(Optional),
        Self::All(All),
    ];

    pub const fn key(self) -> &'static str {
        match self {
            Self::One(_) => one::DESCRIPTOR.key,
            Self::First(_) => first::DESCRIPTOR.key,
            Self::Optional(_) => optional::DESCRIPTOR.key,
            Self::All(_) => all::DESCRIPTOR.key,
        }
    }
}

impl Default for Cardinality {
    fn default() -> Self {
        Self::One(One)
    }
}

impl Serialize for Cardinality {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.key())
    }
}

impl<'de> Deserialize<'de> for Cardinality {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CardinalityVisitor;
        impl Visitor<'_> for CardinalityVisitor {
            type Value = Cardinality;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("one, first, optional, or all")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "one" => Ok(Cardinality::One(One)),
                    "first" => Ok(Cardinality::First(First)),
                    "optional" => Ok(Cardinality::Optional(Optional)),
                    "all" => Ok(Cardinality::All(All)),
                    _ => Err(E::unknown_variant(
                        value,
                        &["one", "first", "optional", "all"],
                    )),
                }
            }
        }
        deserializer.deserialize_str(CardinalityVisitor)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CardinalityDescriptor {
    pub key: &'static str,
}

const CARDINALITY_DESCRIPTORS: [CardinalityDescriptor; 4] = [
    one::DESCRIPTOR,
    first::DESCRIPTOR,
    optional::DESCRIPTOR,
    all::DESCRIPTOR,
];

pub fn cardinality_descriptors() -> &'static [CardinalityDescriptor] {
    &CARDINALITY_DESCRIPTORS
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CardinalityRegistryError {
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

pub fn validate_cardinality_registration_keys(
    schema_keys: &[String],
    serde_keys: &[String],
    registration_keys: &[String],
) -> Result<(), CardinalityRegistryError> {
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
            return Err(CardinalityRegistryError::Duplicate {
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
            return Err(CardinalityRegistryError::Missing {
                layer,
                keys: missing,
            });
        }
        let extra = actual.difference(&schema).cloned().collect::<Vec<_>>();
        if !extra.is_empty() {
            return Err(CardinalityRegistryError::Extra { layer, keys: extra });
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompiledCardinality {
    One(OnePlan),
    First(FirstPlan),
    Optional(OptionalPlan),
    All(AllPlan),
}

impl Default for CompiledCardinality {
    fn default() -> Self {
        compile_cardinality(Cardinality::default())
    }
}

impl CompiledCardinality {
    const fn key(self) -> &'static str {
        match self {
            Self::One(_) => one::DESCRIPTOR.key,
            Self::First(_) => first::DESCRIPTOR.key,
            Self::Optional(_) => optional::DESCRIPTOR.key,
            Self::All(_) => all::DESCRIPTOR.key,
        }
    }

    pub fn execute<T, S>(self, values: S) -> Result<CardinalityOutcome<T>, CardinalityError>
    where
        S: CardinalitySequence<T>,
    {
        let values = values.into_values();
        match self {
            Self::One(plan) => one::execute(plan, values),
            Self::First(plan) => first::execute(plan, values),
            Self::Optional(plan) => optional::execute(plan, values),
            Self::All(plan) => all::execute(plan, values),
        }
    }
}

impl Serialize for CompiledCardinality {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.key())
    }
}

impl<'de> Deserialize<'de> for CompiledCardinality {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let authored = Cardinality::deserialize(deserializer)?;
        Ok(compile_cardinality(authored))
    }
}

pub const fn compile_cardinality(authored: Cardinality) -> CompiledCardinality {
    match authored {
        Cardinality::One(authored) => CompiledCardinality::One(one::compile(authored)),
        Cardinality::First(authored) => CompiledCardinality::First(first::compile(authored)),
        Cardinality::Optional(authored) => {
            CompiledCardinality::Optional(optional::compile(authored))
        }
        Cardinality::All(authored) => CompiledCardinality::All(all::compile(authored)),
    }
}

pub trait CardinalitySequence<T> {
    fn into_values(self) -> Vec<T>;
}

impl<T> CardinalitySequence<T> for Vec<T> {
    fn into_values(self) -> Vec<T> {
        self
    }
}

impl<'doc, 'body> CardinalitySequence<SelectedItem<'doc, 'body>> for SelectedSequence<'doc, 'body> {
    fn into_values(self) -> Vec<SelectedItem<'doc, 'body>> {
        self.into_vec()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CardinalityOutcome<T> {
    Scalar(Option<T>),
    Sequence(Vec<T>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CardinalityError {
    pub expected: String,
    pub actual_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CardinalityDiagnosticContext<'a> {
    pub path: &'a str,
    pub strategy_key: Option<&'a str>,
    pub item_index: Option<usize>,
}

impl CardinalityError {
    pub fn into_diagnostic(self, context: CardinalityDiagnosticContext<'_>) -> Diagnostic {
        let mut details = json!({
            "expectedCardinality": &self.expected,
            "actualCount": self.actual_count,
        });
        if let Some(item_index) = context.item_index {
            details["itemIndex"] = json!(item_index);
        }
        Diagnostic {
            category: DiagnosticCategory::Runtime,
            code: "field_cardinality_mismatch".to_string(),
            message: format!(
                "Field cardinality `{}` did not match {} resolved values",
                self.expected, self.actual_count
            ),
            severity: DiagnosticSeverity::Error,
            path: context.path.to_string(),
            strategy_key: context.strategy_key.map(str::to_string),
            details: Some(details),
        }
    }
}

pub(crate) fn mismatch(expected: &'static str, actual_count: usize) -> CardinalityError {
    CardinalityError {
        expected: expected.to_string(),
        actual_count,
    }
}
