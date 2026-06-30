use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobPosting {
    pub id: i64,
    pub title: String,
    pub company: String,
    pub locations: Vec<String>,
    pub description_text: Option<String>,
    pub read_state: ReadState,
    pub interest_state: InterestState,
    pub preparation_state: PreparationState,
    pub application_state: ApplicationState,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub created_at: String,
    pub updated_at: String,
    pub primary_source: Option<JobPostingSource>,
    pub sources: Vec<JobPostingSource>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobPostingSource {
    pub id: i64,
    pub source_key: String,
    pub source_name_snapshot: String,
    pub url: String,
    #[serde(skip)]
    pub(crate) posting_meta: BTreeMap<String, String>,
    pub first_seen_at: String,
    pub last_seen_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobPostingDetail {
    #[serde(flatten)]
    pub posting: JobPosting,
    pub description_state: PostingDescriptionState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum PostingDescriptionState {
    Loaded { text: String },
    Unsupported { message: String },
    Failed { message: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobPostingQueueCounts {
    pub inbox: i64,
    pub interested: i64,
    pub preparation: i64,
    pub applied: i64,
    pub archive: i64,
    pub all: i64,
    pub new_inbox: i64,
    pub review_inbox: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobPostingQueueId {
    Inbox,
    Interested,
    Preparation,
    Applied,
    Archive,
    All,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadState {
    Unread,
    Read,
}

impl ReadState {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Unread => "unread",
            Self::Read => "read",
        }
    }
}

impl TryFrom<&str> for ReadState {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "unread" => Ok(Self::Unread),
            "read" => Ok(Self::Read),
            _ => Err(format!("unknown job posting read state: {value}")),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InterestState {
    Undecided,
    Interested,
    Dismissed,
}

impl InterestState {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Undecided => "undecided",
            Self::Interested => "interested",
            Self::Dismissed => "dismissed",
        }
    }
}

impl TryFrom<&str> for InterestState {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "undecided" => Ok(Self::Undecided),
            "interested" => Ok(Self::Interested),
            "dismissed" => Ok(Self::Dismissed),
            _ => Err(format!("unknown job posting interest state: {value}")),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreparationState {
    NotStarted,
    InProgress,
    Ready,
}

impl PreparationState {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::NotStarted => "not_started",
            Self::InProgress => "in_progress",
            Self::Ready => "ready",
        }
    }
}

impl TryFrom<&str> for PreparationState {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "not_started" => Ok(Self::NotStarted),
            "in_progress" => Ok(Self::InProgress),
            "ready" => Ok(Self::Ready),
            _ => Err(format!("unknown job posting preparation state: {value}")),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationState {
    NotApplied,
    Submitted,
    InProcess,
    RejectedByCompany,
    WithdrawnByMe,
    Accepted,
}

impl ApplicationState {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::NotApplied => "not_applied",
            Self::Submitted => "submitted",
            Self::InProcess => "in_process",
            Self::RejectedByCompany => "rejected_by_company",
            Self::WithdrawnByMe => "withdrawn_by_me",
            Self::Accepted => "accepted",
        }
    }
}

impl TryFrom<&str> for ApplicationState {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "not_applied" => Ok(Self::NotApplied),
            "submitted" => Ok(Self::Submitted),
            "in_process" => Ok(Self::InProcess),
            "rejected_by_company" => Ok(Self::RejectedByCompany),
            "withdrawn_by_me" => Ok(Self::WithdrawnByMe),
            "accepted" => Ok(Self::Accepted),
            _ => Err(format!("unknown job posting application state: {value}")),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateJobPostingStateInput {
    pub read_state: Option<ReadState>,
    pub interest_state: Option<InterestState>,
    pub preparation_state: Option<PreparationState>,
    pub application_state: Option<ApplicationState>,
}
