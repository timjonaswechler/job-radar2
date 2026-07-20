//! Closed Strategy Policy used by every complete Discovery and Detail Strategy Set.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StrategyPolicy {
    #[serde(rename = "type")]
    policy_type: StrategyPolicyType,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum StrategyPolicyType {
    FirstAccepted,
}

impl StrategyPolicy {
    #[allow(non_upper_case_globals)]
    pub const FirstAccepted: Self = Self {
        policy_type: StrategyPolicyType::FirstAccepted,
    };
}
