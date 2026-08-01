use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckFingerprint {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

impl CheckFingerprint {
    pub fn new(kind: impl Into<String>, sha256: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            reference: None,
            sha256: Some(sha256.into()),
        }
    }

    pub fn with_reference(
        kind: impl Into<String>,
        reference: impl Into<String>,
        sha256: impl Into<String>,
    ) -> Self {
        Self {
            kind: kind.into(),
            reference: Some(reference.into()),
            sha256: Some(sha256.into()),
        }
    }
}
