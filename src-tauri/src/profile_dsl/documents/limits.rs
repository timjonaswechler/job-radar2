use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

/// Authored tighten-only safety ceilings for one Discovery or Detail invocation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PhaseLimits {
    pub max_strategy_attempts: u64,
    pub max_requests: u64,
    pub max_produced_items: u64,
    pub max_duration_ms: u64,
    pub max_pages: u64,
    pub max_browser_actions: u64,
    pub max_fan_out: u64,
    pub max_response_bytes: u64,
    pub max_browser_rendered_bytes: u64,
}

impl PhaseLimits {
    pub const BACKEND: Self = Self {
        max_strategy_attempts: 50,
        max_requests: 1_000,
        max_produced_items: 100_000,
        max_duration_ms: 120_000,
        max_pages: 1_000,
        max_browser_actions: 50,
        max_fan_out: 100_000,
        max_response_bytes: 67_108_864,
        max_browser_rendered_bytes: 67_108_864,
    };

    pub const fn minimum(self, other: Self) -> Self {
        Self {
            max_strategy_attempts: if self.max_strategy_attempts < other.max_strategy_attempts {
                self.max_strategy_attempts
            } else {
                other.max_strategy_attempts
            },
            max_requests: if self.max_requests < other.max_requests {
                self.max_requests
            } else {
                other.max_requests
            },
            max_produced_items: if self.max_produced_items < other.max_produced_items {
                self.max_produced_items
            } else {
                other.max_produced_items
            },
            max_duration_ms: if self.max_duration_ms < other.max_duration_ms {
                self.max_duration_ms
            } else {
                other.max_duration_ms
            },
            max_pages: if self.max_pages < other.max_pages {
                self.max_pages
            } else {
                other.max_pages
            },
            max_browser_actions: if self.max_browser_actions < other.max_browser_actions {
                self.max_browser_actions
            } else {
                other.max_browser_actions
            },
            max_fan_out: if self.max_fan_out < other.max_fan_out {
                self.max_fan_out
            } else {
                other.max_fan_out
            },
            max_response_bytes: if self.max_response_bytes < other.max_response_bytes {
                self.max_response_bytes
            } else {
                other.max_response_bytes
            },
            max_browser_rendered_bytes: if self.max_browser_rendered_bytes
                < other.max_browser_rendered_bytes
            {
                self.max_browser_rendered_bytes
            } else {
                other.max_browser_rendered_bytes
            },
        }
    }

    pub const fn all_positive(self) -> bool {
        self.max_strategy_attempts > 0
            && self.max_requests > 0
            && self.max_produced_items > 0
            && self.max_duration_ms > 0
            && self.max_pages > 0
            && self.max_browser_actions > 0
            && self.max_fan_out > 0
            && self.max_response_bytes > 0
            && self.max_browser_rendered_bytes > 0
    }

    pub const fn within(self, inherited: Self) -> bool {
        self.max_strategy_attempts <= inherited.max_strategy_attempts
            && self.max_requests <= inherited.max_requests
            && self.max_produced_items <= inherited.max_produced_items
            && self.max_duration_ms <= inherited.max_duration_ms
            && self.max_pages <= inherited.max_pages
            && self.max_browser_actions <= inherited.max_browser_actions
            && self.max_fan_out <= inherited.max_fan_out
            && self.max_response_bytes <= inherited.max_response_bytes
            && self.max_browser_rendered_bytes <= inherited.max_browser_rendered_bytes
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseLimitsFragment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_strategy_attempts: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_requests: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_produced_items: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_pages: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_browser_actions: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_fan_out: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_response_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_browser_rendered_bytes: Option<u64>,
}

impl<'de> Deserialize<'de> for PhaseLimitsFragment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let object = value
            .as_object()
            .ok_or_else(|| D::Error::custom("phase limits fragment must be an object"))?;
        const FIELDS: &[&str] = &[
            "maxStrategyAttempts",
            "maxRequests",
            "maxProducedItems",
            "maxDurationMs",
            "maxPages",
            "maxBrowserActions",
            "maxFanOut",
            "maxResponseBytes",
            "maxBrowserRenderedBytes",
        ];
        if object.is_empty() {
            return Err(D::Error::custom(
                "phase limits fragment must tighten at least one field",
            ));
        }
        if let Some(unknown) = object.keys().find(|key| !FIELDS.contains(&key.as_str())) {
            return Err(D::Error::custom(format!(
                "unknown phase limits field `{unknown}`"
            )));
        }
        let field = |name: &str| -> Result<Option<u64>, D::Error> {
            object
                .get(name)
                .map(|value| serde_json::from_value::<u64>(value.clone()).map_err(D::Error::custom))
                .transpose()
        };
        Ok(Self {
            max_strategy_attempts: field("maxStrategyAttempts")?,
            max_requests: field("maxRequests")?,
            max_produced_items: field("maxProducedItems")?,
            max_duration_ms: field("maxDurationMs")?,
            max_pages: field("maxPages")?,
            max_browser_actions: field("maxBrowserActions")?,
            max_fan_out: field("maxFanOut")?,
            max_response_bytes: field("maxResponseBytes")?,
            max_browser_rendered_bytes: field("maxBrowserRenderedBytes")?,
        })
    }
}
