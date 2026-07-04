use std::path::Path;

use sqlx::{sqlite::SqliteConnectOptions, Row, SqlitePool};

use super::{normalization::location_lookup_keys, GeoPoint, ResolvedLocation};

#[derive(Clone)]
pub struct GeoDbResolver {
    pool: SqlitePool,
}

impl GeoDbResolver {
    pub async fn connect(path: &Path) -> Result<Self, String> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(false)
            .read_only(true);
        let pool = SqlitePool::connect_with(options)
            .await
            .map_err(|error| format!("failed to open geo database {}: {error}", path.display()))?;

        Ok(Self { pool })
    }

    pub async fn resolve(&self, input: &str) -> Result<Vec<ResolvedLocation>, String> {
        let key = input.trim().to_lowercase();
        if key.is_empty() {
            return Ok(Vec::new());
        }

        if key.chars().all(|character| character.is_ascii_digit()) {
            let rows = sqlx::query(
                "SELECT p.place_name, pc.latitude, pc.longitude
                 FROM geo_postal_codes pc
                 JOIN geo_places p ON p.id = pc.place_id
                 WHERE pc.postal_code = ?1
                 ORDER BY p.id",
            )
            .bind(&key)
            .fetch_all(&self.pool)
            .await
            .map_err(|error| format!("failed to resolve geo location {input:?}: {error}"))?;

            return rows.into_iter().map(|row| resolved_location(input, row)).collect();
        }

        let mut resolved = Vec::new();
        for lookup_key in location_lookup_keys(input) {
            let rows = sqlx::query(
                "SELECT p.place_name, p.latitude, p.longitude
                 FROM geo_place_keys pk
                 JOIN geo_places p ON p.id = pk.place_id
                 WHERE pk.key = ?1
                 ORDER BY p.id",
            )
            .bind(&lookup_key)
            .fetch_all(&self.pool)
            .await
            .map_err(|error| format!("failed to resolve geo location {input:?}: {error}"))?;

            for row in rows {
                let location = resolved_location(input, row)?;
                if !resolved.contains(&location) {
                    resolved.push(location);
                }
            }
        }

        Ok(resolved)
    }
}

fn resolved_location(
    input: &str,
    row: sqlx::sqlite::SqliteRow,
) -> Result<ResolvedLocation, String> {
    Ok(ResolvedLocation {
        input: input.to_string(),
        label: row.try_get("place_name").map_err(db_error)?,
        point: GeoPoint {
            latitude: row.try_get("latitude").map_err(db_error)?,
            longitude: row.try_get("longitude").map_err(db_error)?,
        },
    })
}

fn db_error(error: sqlx::Error) -> String {
    error.to_string()
}
