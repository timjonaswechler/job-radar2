//! Closed Strategy Policy used by every complete Discovery and Detail Strategy Set.

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StrategyPolicy {
    FirstAccepted,
    AllRequired,
    AtLeast {
        count: usize,
    },
    CollectAll {
        #[serde(rename = "minAccepted")]
        min_accepted: usize,
    },
}

impl StrategyPolicy {
    pub(crate) fn reports_final_rejection(self) -> bool {
        match self {
            Self::FirstAccepted => false,
            Self::AllRequired | Self::AtLeast { .. } | Self::CollectAll { .. } => true,
        }
    }
}

impl<'de> Deserialize<'de> for StrategyPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum PolicyObject {
            FirstAccepted(EmptyPolicy),
            AllRequired(EmptyPolicy),
            AtLeast(AtLeastPolicy),
            CollectAll(CollectAllPolicy),
        }

        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct EmptyPolicy {}

        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct AtLeastPolicy {
            count: std::num::NonZeroUsize,
        }

        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct CollectAllPolicy {
            #[serde(rename = "minAccepted")]
            min_accepted: std::num::NonZeroUsize,
        }

        Ok(match PolicyObject::deserialize(deserializer)? {
            PolicyObject::FirstAccepted(_) => Self::FirstAccepted,
            PolicyObject::AllRequired(_) => Self::AllRequired,
            PolicyObject::AtLeast(policy) => Self::AtLeast {
                count: policy.count.get(),
            },
            PolicyObject::CollectAll(policy) => Self::CollectAll {
                min_accepted: policy.min_accepted.get(),
            },
        })
    }
}
