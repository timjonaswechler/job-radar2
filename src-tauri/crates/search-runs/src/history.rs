use std::{collections::HashMap, fmt};

use search_requests::Id;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use crate::Status;

// Stay below SQLite's host-parameter limit while retaining one batched read per chunk.
const SQLITE_PROJECTION_CHUNK: usize = 500;

/// Latest Search Run projection reader backed by one concrete SQLite pool.
#[derive(Clone)]
pub struct History {
    pool: SqlitePool,
}

impl History {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn latest(&self, request: Id) -> Result<Latest, Error> {
        Ok(self
            .latest_many(&[request])
            .await?
            .remove(&request)
            .unwrap_or_default())
    }

    pub async fn latest_many(&self, requests: &[Id]) -> Result<HashMap<Id, Latest>, Error> {
        let mut latest = HashMap::with_capacity(requests.len());
        for ids in requests.chunks(SQLITE_PROJECTION_CHUNK) {
            let mut query = QueryBuilder::<Sqlite>::new(
                "SELECT id, last_run_at, last_run_status, last_run_error \
                 FROM search_requests WHERE id IN (",
            );
            let mut separated = query.separated(", ");
            for id in ids {
                separated.push_bind(id.get());
            }
            separated.push_unseparated(")");
            let rows = query
                .build()
                .fetch_all(&self.pool)
                .await
                .map_err(Error::storage)?;
            for row in rows {
                let raw_id: i64 = row.try_get("id").map_err(Error::corrupt_unknown)?;
                let id = Id::new(raw_id).map_err(|error| Error::CorruptStoredRow {
                    id: None,
                    message: error.to_string(),
                })?;
                let raw_status: Option<String> = row
                    .try_get("last_run_status")
                    .map_err(|error| Error::corrupt(Some(id), error))?;
                latest.insert(
                    id,
                    Latest {
                        at: row
                            .try_get("last_run_at")
                            .map_err(|error| Error::corrupt(Some(id), error))?,
                        status: raw_status
                            .as_deref()
                            .map(Status::try_from)
                            .transpose()
                            .map_err(|message| Error::CorruptStoredRow {
                                id: Some(id),
                                message,
                            })?,
                        error: row
                            .try_get("last_run_error")
                            .map_err(|error| Error::corrupt(Some(id), error))?,
                    },
                );
            }
        }
        Ok(latest)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Latest {
    pub at: Option<String>,
    pub status: Option<Status>,
    pub error: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    CorruptStoredRow { id: Option<Id>, message: String },
    StorageUnavailable { message: String },
}

impl Error {
    fn storage(error: sqlx::Error) -> Self {
        Self::StorageUnavailable {
            message: error.to_string(),
        }
    }

    fn corrupt(id: Option<Id>, error: sqlx::Error) -> Self {
        Self::CorruptStoredRow {
            id,
            message: error.to_string(),
        }
    }

    fn corrupt_unknown(error: sqlx::Error) -> Self {
        Self::corrupt(None, error)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CorruptStoredRow { id, message } => match id {
                Some(id) => write!(
                    formatter,
                    "Search Request {id} has a corrupt latest Search Run projection: {message}"
                ),
                None => write!(
                    formatter,
                    "a latest Search Run projection row is corrupt: {message}"
                ),
            },
            Self::StorageUnavailable { message } => {
                write!(
                    formatter,
                    "Search Run history storage is unavailable: {message}"
                )
            }
        }
    }
}

impl std::error::Error for Error {}
