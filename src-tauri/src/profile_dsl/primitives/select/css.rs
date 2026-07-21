use dom_query::Matcher;
use serde::{Deserialize, Serialize};

use super::{SelectedItem, SelectedSequence};

pub(super) const DESCRIPTOR: super::SelectDescriptor = super::SelectDescriptor { key: "css" };

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CssSelect {
    pub(super) selector: String,
}

#[derive(Clone, Debug)]
pub struct CssSelectPlan {
    selector: String,
    matcher: Matcher,
}

impl PartialEq for CssSelectPlan {
    fn eq(&self, other: &Self) -> bool {
        self.selector == other.selector
    }
}

impl Serialize for CssSelectPlan {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct Wire<'a> {
            selector: &'a str,
        }
        Wire {
            selector: &self.selector,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CssSelectPlan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            selector: String,
        }
        let wire = Wire::deserialize(deserializer)?;
        compile(&wire.selector).map_err(serde::de::Error::custom)
    }
}

pub(crate) fn compile(selector: &str) -> Result<CssSelectPlan, String> {
    let matcher =
        Matcher::new(selector).map_err(|error| format!("CSS selector is invalid: {error:?}"))?;
    Ok(CssSelectPlan {
        selector: selector.to_string(),
        matcher,
    })
}

pub(crate) fn execute<'doc>(
    plan: &CssSelectPlan,
    document: &'doc dom_query::Document,
) -> SelectedSequence<'doc, 'static> {
    SelectedSequence::new(
        document
            .select_matcher(&plan.matcher)
            .nodes()
            .iter()
            .cloned()
            .map(SelectedItem::Html)
            .collect(),
    )
}

pub(crate) fn execute_relative<'doc>(
    plan: &CssSelectPlan,
    node: &dom_query::NodeRef<'doc>,
) -> SelectedSequence<'doc, 'static> {
    SelectedSequence::new(
        dom_query::Selection::from(node.clone())
            .select_matcher(&plan.matcher)
            .nodes()
            .iter()
            .cloned()
            .map(SelectedItem::Html)
            .collect(),
    )
}
