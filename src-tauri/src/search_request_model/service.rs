use sqlx::SqlitePool;

use super::{
    persistence::{db_error, json_to_string, search_request_from_row},
    validation::validate_search_request_input,
    CreateSearchRequestInput, RunningSearchRuns, SearchRequest, UpdateSearchRequestInput,
};

pub struct SearchRequestService<'a> {
    pool: &'a SqlitePool,
    running_search_runs: &'a RunningSearchRuns,
}

impl<'a> SearchRequestService<'a> {
    pub fn new(pool: &'a SqlitePool, running_search_runs: &'a RunningSearchRuns) -> Self {
        Self {
            pool,
            running_search_runs,
        }
    }

    pub async fn create(&self, input: CreateSearchRequestInput) -> Result<SearchRequest, String> {
        let input = validate_search_request_input(
            self.pool,
            input.status,
            input.include_rules,
            input.exclude_rules,
            input.locations,
            input.radius_km,
            input.source_keys,
        )
        .await?;
        let include_rules_json = json_to_string(&input.include_rules)?;
        let exclude_rules_json = json_to_string(&input.exclude_rules)?;
        let locations_json = json_to_string(&input.locations)?;
        let source_keys_json = json_to_string(&input.source_keys)?;

        let result = sqlx::query(
            "INSERT INTO search_requests (
               status, include_rules_json, exclude_rules_json, locations_json,
               radius_km, source_keys_json, validation_error
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(input.status.as_str())
        .bind(include_rules_json)
        .bind(exclude_rules_json)
        .bind(locations_json)
        .bind(input.radius_km)
        .bind(source_keys_json)
        .bind(input.validation_error.as_deref())
        .execute(self.pool)
        .await
        .map_err(db_error)?;

        self.get(result.last_insert_rowid()).await
    }

    pub async fn list(&self) -> Result<Vec<SearchRequest>, String> {
        let rows = sqlx::query(
            "SELECT id, status, include_rules_json, exclude_rules_json, locations_json,
                    radius_km, source_keys_json, validation_error, created_at, updated_at
             FROM search_requests
             ORDER BY id",
        )
        .fetch_all(self.pool)
        .await
        .map_err(db_error)?;

        rows.into_iter().map(search_request_from_row).collect()
    }

    pub async fn get(&self, id: i64) -> Result<SearchRequest, String> {
        let row = sqlx::query(
            "SELECT id, status, include_rules_json, exclude_rules_json, locations_json,
                    radius_km, source_keys_json, validation_error, created_at, updated_at
             FROM search_requests
             WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await
        .map_err(db_error)?;

        row.map(search_request_from_row)
            .transpose()?
            .ok_or_else(|| format!("search request {id} not found"))
    }

    pub async fn update(
        &self,
        id: i64,
        input: UpdateSearchRequestInput,
    ) -> Result<SearchRequest, String> {
        self.get(id).await?;
        self.ensure_not_running(id)?;

        let input = validate_search_request_input(
            self.pool,
            input.status,
            input.include_rules,
            input.exclude_rules,
            input.locations,
            input.radius_km,
            input.source_keys,
        )
        .await?;
        let include_rules_json = json_to_string(&input.include_rules)?;
        let exclude_rules_json = json_to_string(&input.exclude_rules)?;
        let locations_json = json_to_string(&input.locations)?;
        let source_keys_json = json_to_string(&input.source_keys)?;

        sqlx::query(
            "UPDATE search_requests
             SET status = ?1,
                 include_rules_json = ?2,
                 exclude_rules_json = ?3,
                 locations_json = ?4,
                 radius_km = ?5,
                 source_keys_json = ?6,
                 validation_error = ?7,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?8",
        )
        .bind(input.status.as_str())
        .bind(include_rules_json)
        .bind(exclude_rules_json)
        .bind(locations_json)
        .bind(input.radius_km)
        .bind(source_keys_json)
        .bind(input.validation_error.as_deref())
        .bind(id)
        .execute(self.pool)
        .await
        .map_err(db_error)?;

        self.get(id).await
    }

    pub async fn delete(&self, id: i64) -> Result<(), String> {
        self.get(id).await?;
        self.ensure_not_running(id)?;

        sqlx::query("DELETE FROM search_requests WHERE id = ?1")
            .bind(id)
            .execute(self.pool)
            .await
            .map_err(db_error)?;

        Ok(())
    }

    fn ensure_not_running(&self, id: i64) -> Result<(), String> {
        if self.running_search_runs.is_running(id)? {
            return Err(format!(
                "search request {id} has a currently running search run"
            ));
        }

        Ok(())
    }
}
