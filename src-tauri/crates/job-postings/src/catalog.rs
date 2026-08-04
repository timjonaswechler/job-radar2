mod queues;
mod sqlite;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::fmt;

/// Stable database identity of one persisted Job Posting.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Id(i64);

impl Id {
    pub fn new(value: i64) -> Self {
        Self(value)
    }
    pub fn get(self) -> i64 {
        self.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

macro_rules! stored_state {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }

        impl $name {
            pub(crate) fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $value),+ }
            }

            pub(crate) fn parse(value: &str) -> Option<Self> {
                match value { $($value => Some(Self::$variant),)+ _ => None }
            }
        }
    };
}

stored_state!(ReadState { Unread => "unread", Read => "read" });
stored_state!(InterestState {
    Undecided => "undecided", Interested => "interested", Dismissed => "dismissed"
});
stored_state!(PreparationState {
    NotStarted => "not_started", InProgress => "in_progress", Ready => "ready"
});
stored_state!(ApplicationState {
    NotApplied => "not_applied", Submitted => "submitted", InProcess => "in_process",
    RejectedByCompany => "rejected_by_company", WithdrawnByMe => "withdrawn_by_me",
    Accepted => "accepted"
});

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrimaryQueue {
    Inbox,
    Interested,
    Preparation,
    Applied,
    Archive,
}

/// Queue-scoped Catalog list selection. `All` is a list scope, never a Posting's primary queue.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Queue {
    Inbox,
    Interested,
    Preparation,
    Applied,
    Archive,
    All,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub inbox: i64,
    pub interested: i64,
    pub preparation: i64,
    pub applied: i64,
    pub archive: i64,
    pub all: i64,
    pub new_inbox: i64,
    pub review_inbox: i64,
}

/// Persisted identity projection. `kind` remains raw so Detail can diagnose one
/// corrupt occurrence and continue to a valid fallback instead of rejecting the whole Posting.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OccurrenceIdentity {
    pub kind: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Occurrence {
    pub id: i64,
    pub source_key: String,
    pub source_name_snapshot: String,
    pub identity: OccurrenceIdentity,
    pub provider_url: String,
    pub posting_meta: std::collections::BTreeMap<String, String>,
    pub first_seen_at: String,
    pub last_seen_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Posting {
    pub id: Id,
    pub title: String,
    pub company: String,
    pub locations: Vec<String>,
    pub description_text: Option<String>,
    pub read_state: ReadState,
    pub interest_state: InterestState,
    pub preparation_state: PreparationState,
    pub application_state: ApplicationState,
    pub primary_queue: PrimaryQueue,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub created_at: String,
    pub updated_at: String,
    pub primary_occurrence: Occurrence,
    pub occurrences: Vec<Occurrence>,
}

/// A nonempty partial update across the four independent Posting workflow axes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Change {
    pub(crate) read_state: Option<ReadState>,
    pub(crate) interest_state: Option<InterestState>,
    pub(crate) preparation_state: Option<PreparationState>,
    pub(crate) application_state: Option<ApplicationState>,
}

impl Change {
    pub fn new(
        read_state: Option<ReadState>,
        interest_state: Option<InterestState>,
        preparation_state: Option<PreparationState>,
        application_state: Option<ApplicationState>,
    ) -> Result<Self, Error> {
        if read_state.is_none()
            && interest_state.is_none()
            && preparation_state.is_none()
            && application_state.is_none()
        {
            return Err(Error::InvalidChange);
        }
        Ok(Self {
            read_state,
            interest_state,
            preparation_state,
            application_state,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("job posting {0} not found")]
    NotFound(Id),
    #[error("no state fields supplied")]
    InvalidChange,
    #[error("corrupt job posting {posting}: {message}")]
    Corrupt { posting: Id, message: String },
    #[error("job posting storage failed: {0}")]
    Storage(#[from] sqlx::Error),
}

impl Error {
    pub(crate) fn corrupt(posting: Id, message: impl Into<String>) -> Self {
        Self::Corrupt {
            posting,
            message: message.into(),
        }
    }
}

pub(crate) enum MarkReadError {
    Before(Error),
    After(Error),
}

/// Persistent Job Posting workflow catalog. Lists hydrate occurrences in one batch.
#[derive(Clone)]
pub struct Catalog {
    pool: SqlitePool,
}

impl Catalog {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self, queue: Queue) -> Result<Vec<Posting>, Error> {
        sqlite::list(&self.pool, queue).await
    }

    pub async fn counts(&self) -> Result<Counts, Error> {
        sqlite::counts(&self.pool).await
    }

    pub async fn change(&self, id: Id, change: Change) -> Result<Posting, Error> {
        sqlite::change(&self.pool, id, change).await
    }

    pub(crate) async fn mark_read(&self, id: Id) -> Result<Posting, MarkReadError> {
        sqlite::mark_read(&self.pool, id).await
    }

    pub(crate) async fn cache_description(
        &self,
        id: Id,
        description: &str,
    ) -> Result<Posting, Error> {
        sqlite::cache_description(&self.pool, id, description).await
    }
}
