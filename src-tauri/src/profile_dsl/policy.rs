//! Closed Strategy Policy used by every complete Discovery and Detail Strategy Set.

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StrategyPolicy {
    FirstAccepted,
    AllRequired,
}

impl<'de> Deserialize<'de> for StrategyPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct PolicyObject {
            #[serde(rename = "type")]
            policy_type: PolicyType,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum PolicyType {
            FirstAccepted,
            AllRequired,
        }

        Ok(match PolicyObject::deserialize(deserializer)?.policy_type {
            PolicyType::FirstAccepted => Self::FirstAccepted,
            PolicyType::AllRequired => Self::AllRequired,
        })
    }
}
