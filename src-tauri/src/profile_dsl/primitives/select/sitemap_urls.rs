use regex::Regex;
use serde::{Deserialize, Serialize};

use super::{SelectedItem, SelectedSequence};

pub(super) const DESCRIPTOR: super::SelectDescriptor = super::SelectDescriptor {
    key: "sitemap_urls",
};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SitemapUrlsSelect {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) url_pattern: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SitemapUrlsSelectPlan {
    url_pattern: Option<String>,
    pattern: Option<Regex>,
}

impl PartialEq for SitemapUrlsSelectPlan {
    fn eq(&self, other: &Self) -> bool {
        self.url_pattern == other.url_pattern
    }
}

impl Serialize for SitemapUrlsSelectPlan {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Wire<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            url_pattern: Option<&'a str>,
        }
        Wire {
            url_pattern: self.url_pattern.as_deref(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SitemapUrlsSelectPlan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Wire {
            url_pattern: Option<String>,
        }
        let wire = Wire::deserialize(deserializer)?;
        compile(wire.url_pattern.as_deref()).map_err(serde::de::Error::custom)
    }
}

pub(super) fn compile(url_pattern: Option<&str>) -> Result<SitemapUrlsSelectPlan, String> {
    let pattern = url_pattern
        .map(Regex::new)
        .transpose()
        .map_err(|error| format!("sitemap URL pattern is invalid: {error}"))?;
    Ok(SitemapUrlsSelectPlan {
        url_pattern: url_pattern.map(str::to_string),
        pattern,
    })
}

pub(super) fn execute<'doc, 'body>(
    plan: &SitemapUrlsSelectPlan,
    root: roxmltree::Node<'doc, 'body>,
) -> SelectedSequence<'doc, 'body> {
    SelectedSequence::new(
        super::xml_element::descendant_elements(root, "loc")
            .into_iter()
            .map(super::xml_text::node_text)
            .map(|url| url.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|url| !url.is_empty())
            .filter(|url| {
                plan.pattern
                    .as_ref()
                    .is_none_or(|pattern| pattern.is_match(url))
            })
            .map(SelectedItem::Text)
            .collect(),
    )
}
