use serde::{de::DeserializeOwned, Serialize};
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};

use super::{validation, Error, Id, Input, Record, Status};

const COLUMNS: &str = "id, status, include_rules_json, exclude_rules_json, locations_json, radius_km, source_keys_json, created_at, updated_at";

pub(super) async fn insert(pool: &SqlitePool, input: &Input) -> Result<Id, Error> {
    let result = sqlx::query("INSERT INTO search_requests (status, include_rules_json, exclude_rules_json, locations_json, radius_km, source_keys_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")
        .bind(input.status.as_str()).bind(json(&input.include_rules)?.0).bind(json(&input.exclude_rules)?.0).bind(json(&input.locations)?.0).bind(input.radius_km).bind(json(&input.source_keys)?.0)
        .execute(pool).await.map_err(storage)?;
    Id::from_generated(result.last_insert_rowid())
}

pub(super) async fn list(pool: &SqlitePool) -> Result<Vec<Record>, Error> {
    let query = format!("SELECT {COLUMNS} FROM search_requests ORDER BY id");
    sqlx::query(&query)
        .fetch_all(pool)
        .await
        .map_err(storage)?
        .into_iter()
        .map(row)
        .collect()
}

pub(super) async fn get(pool: &SqlitePool, id: Id) -> Result<Record, Error> {
    let query = format!("SELECT {COLUMNS} FROM search_requests WHERE id = ?1");
    let row = sqlx::query(&query)
        .bind(id.get())
        .fetch_optional(pool)
        .await
        .map_err(storage)?;
    row.map(self::row)
        .transpose()?
        .ok_or(Error::NotFound { id })
}

pub(super) async fn update(pool: &SqlitePool, id: Id, input: &Input) -> Result<(), Error> {
    let result = sqlx::query("UPDATE search_requests SET status=?1, include_rules_json=?2, exclude_rules_json=?3, locations_json=?4, radius_km=?5, source_keys_json=?6, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?7")
        .bind(input.status.as_str()).bind(json(&input.include_rules)?.0).bind(json(&input.exclude_rules)?.0).bind(json(&input.locations)?.0).bind(input.radius_km).bind(json(&input.source_keys)?.0).bind(id.get())
        .execute(pool).await.map_err(storage)?;
    if result.rows_affected() == 0 {
        return Err(Error::NotFound { id });
    }
    Ok(())
}

pub(super) async fn delete(pool: &SqlitePool, id: Id) -> Result<(), Error> {
    let result = sqlx::query("DELETE FROM search_requests WHERE id=?1")
        .bind(id.get())
        .execute(pool)
        .await
        .map_err(storage)?;
    if result.rows_affected() == 0 {
        return Err(Error::NotFound { id });
    }
    Ok(())
}

fn row(row: SqliteRow) -> Result<Record, Error> {
    let raw_id = row.try_get("id").map_err(corrupt)?;
    let id = Id::from_stored(raw_id)?;
    let status: String = row.try_get("status").map_err(corrupt)?;
    let status = Status::from_stored(&status, id)?;
    let include_rules: Vec<search_resolution::SearchRule> =
        from_json(&row, id, "include_rules_json")?;
    let exclude_rules: Vec<search_resolution::SearchRule> =
        from_json(&row, id, "exclude_rules_json")?;
    let source_keys: Vec<String> = from_json(&row, id, "source_keys_json")?;
    let validation = validation::derive(&include_rules, &exclude_rules, &source_keys);
    Ok(Record {
        id,
        status,
        include_rules,
        exclude_rules,
        locations: from_json(&row, id, "locations_json")?,
        radius_km: row.try_get("radius_km").map_err(corrupt)?,
        source_keys,
        validation,
        created_at: row.try_get("created_at").map_err(corrupt)?,
        updated_at: row.try_get("updated_at").map_err(corrupt)?,
    })
}

struct EncodedJson(String);

fn json<T: Serialize>(value: &T) -> Result<EncodedJson, Error> {
    serde_json::to_string(value)
        .map(EncodedJson)
        .map_err(|error| Error::InternalInvariant {
            message: format!("{error}"),
        })
}
fn from_json<T: DeserializeOwned>(row: &SqliteRow, id: Id, column: &str) -> Result<T, Error> {
    let value: String = row.try_get(column).map_err(corrupt)?;
    serde_json::from_str(&value).map_err(|error| Error::CorruptStoredRow {
        id: Some(id),
        message: format!("{column}: {error}"),
    })
}
fn storage(error: sqlx::Error) -> Error {
    Error::StorageUnavailable {
        message: error.to_string(),
    }
}
fn corrupt(error: sqlx::Error) -> Error {
    Error::CorruptStoredRow {
        id: None,
        message: error.to_string(),
    }
}
