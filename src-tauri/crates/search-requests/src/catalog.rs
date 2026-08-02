mod activity;
mod sqlite;
mod validation;

use std::{fmt, num::NonZeroI64, sync::Arc};

use activity::{Activity, Reservation};
use search_resolution::SearchRule;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// Authored Search Request catalog backed by one concrete SQLite pool.
///
/// Clones share the same process-local execution/mutation admission state. Hosts must construct
/// one root Catalog for a database and distribute its clones; independently constructed Catalogs
/// intentionally do not claim cross-process or distributed coordination.
#[derive(Clone)]
pub struct Catalog {
    pool: SqlitePool,
    activity: Arc<Activity>,
}

impl Catalog {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            activity: Arc::new(Activity::default()),
        }
    }
    pub async fn create(&self, input: Input) -> Result<Record, Error> {
        let (input, _) = validation::normalize(input)?;
        let id = sqlite::insert(&self.pool, &input).await?;
        sqlite::get(&self.pool, id).await
    }
    pub async fn list(&self) -> Result<Vec<Record>, Error> {
        sqlite::list(&self.pool).await
    }
    pub async fn get(&self, id: Id) -> Result<Record, Error> {
        sqlite::get(&self.pool, id).await
    }
    pub async fn update(&self, id: Id, input: Input) -> Result<Record, Error> {
        let _reservation = self.activity.reserve(id)?;
        let (input, _) = validation::normalize(input)?;
        sqlite::update(&self.pool, id, &input).await?;
        sqlite::get(&self.pool, id).await
    }
    pub async fn delete(&self, id: Id) -> Result<(), Error> {
        let _reservation = self.activity.reserve(id)?;
        sqlite::delete(&self.pool, id).await
    }
    pub async fn begin_execution(&self, id: Id) -> Result<Execution, Error> {
        let lease = self.activity.reserve(id)?;
        let record = sqlite::get(&self.pool, id).await?;
        if record.status != Status::Active {
            return Err(Error::InvalidInput {
                message: format!("search request {id} cannot run unless status is active"),
            });
        }
        if let Some(issue) = record.validation.issues.first() {
            return Err(Error::InvalidInput {
                message: format!(
                    "search request {id} cannot run: {} at {}",
                    issue.code, issue.path
                ),
            });
        }
        Ok(Execution {
            record,
            _lease: lease,
        })
    }
}

/// Positive identity of one persisted Search Request.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Id(NonZeroI64);

impl Id {
    pub fn new(value: i64) -> Result<Self, Error> {
        NonZeroI64::new(value)
            .filter(|value| value.get() > 0)
            .map(Self)
            .ok_or_else(|| Error::InvalidInput {
                message: "Search Request id must be greater than 0".to_string(),
            })
    }

    pub const fn get(self) -> i64 {
        self.0.get()
    }

    pub(crate) fn from_generated(value: i64) -> Result<Self, Error> {
        NonZeroI64::new(value)
            .filter(|value| value.get() > 0)
            .map(Self)
            .ok_or_else(|| Error::InternalInvariant {
                message: format!("SQLite generated invalid Search Request id {value}"),
            })
    }

    pub(crate) fn from_stored(value: i64) -> Result<Self, Error> {
        NonZeroI64::new(value)
            .filter(|value| value.get() > 0)
            .map(Self)
            .ok_or_else(|| Error::CorruptStoredRow {
                id: None,
                message: format!("invalid Search Request id {value}"),
            })
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = i64::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for Id {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Draft,
    Active,
    Disabled,
}
impl Status {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Disabled => "disabled",
        }
    }

    pub(crate) fn from_stored(value: &str, id: Id) -> Result<Self, Error> {
        match value {
            "draft" => Ok(Self::Draft),
            "active" => Ok(Self::Active),
            "disabled" => Ok(Self::Disabled),
            _ => Err(Error::CorruptStoredRow {
                id: Some(id),
                message: format!("unknown Search Request status: {value}"),
            }),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub status: Status,
    pub include_rules: Vec<SearchRule>,
    pub exclude_rules: Vec<SearchRule>,
    pub locations: Vec<String>,
    pub radius_km: Option<i64>,
    pub source_keys: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub id: Id,
    pub status: Status,
    pub include_rules: Vec<SearchRule>,
    pub exclude_rules: Vec<SearchRule>,
    pub locations: Vec<String>,
    pub radius_km: Option<i64>,
    pub source_keys: Vec<String>,
    pub validation: Validation,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Validation {
    pub issues: Vec<ValidationIssue>,
}
impl Validation {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationIssueCode {
    InvalidRegex,
    IncludeRuleRequired,
    SourceKeyRequired,
    DuplicateSourceKey,
    IssuesTruncated,
}
impl fmt::Display for ValidationIssueCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidRegex => "invalid_regex",
            Self::IncludeRuleRequired => "include_rule_required",
            Self::SourceKeyRequired => "source_key_required",
            Self::DuplicateSourceKey => "duplicate_source_key",
            Self::IssuesTruncated => "issues_truncated",
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub code: ValidationIssueCode,
    pub path: String,
    pub message: String,
}
impl ValidationIssue {
    fn new(code: ValidationIssueCode, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code,
            path: path.into(),
            message: message.into(),
        }
    }
}

/// Immutable admitted Search Request snapshot with an opaque process-local RAII lease.
///
/// The lease is released on drop, including cancellation and error returns. Execution is
/// deliberately not Clone so one admitted execution has one owner.
pub struct Execution {
    record: Record,
    _lease: Reservation,
}
impl Execution {
    pub fn snapshot(&self) -> &Record {
        &self.record
    }
    pub fn id(&self) -> Id {
        self.record.id
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    InvalidInput { message: String },
    NotFound { id: Id },
    Busy { id: Id },
    CorruptStoredRow { id: Option<Id>, message: String },
    StorageUnavailable { message: String },
    InternalInvariant { message: String },
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput { message } => f.write_str(message),
            Self::NotFound { id } => write!(f, "search request {id} not found"),
            Self::Busy { id } => write!(f, "search request {id} is busy"),
            Self::CorruptStoredRow { id, message } => match id {
                Some(id) => write!(f, "search request {id} has corrupt stored data: {message}"),
                None => write!(f, "stored Search Request row is corrupt: {message}"),
            },
            Self::StorageUnavailable { message } => {
                write!(f, "Search Request storage is unavailable: {message}")
            }
            Self::InternalInvariant { message } => {
                write!(f, "Search Request internal invariant failed: {message}")
            }
        }
    }
}
impl std::error::Error for Error {}
