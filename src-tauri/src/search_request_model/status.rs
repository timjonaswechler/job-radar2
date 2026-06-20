use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchRequestStatus {
    Draft,
    Active,
    Disabled,
    Invalid,
}

impl SearchRequestStatus {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Disabled => "disabled",
            Self::Invalid => "invalid",
        }
    }
}

impl TryFrom<&str> for SearchRequestStatus {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "draft" => Ok(Self::Draft),
            "active" => Ok(Self::Active),
            "disabled" => Ok(Self::Disabled),
            "invalid" => Ok(Self::Invalid),
            _ => Err(format!("unknown search request status: {value}")),
        }
    }
}
