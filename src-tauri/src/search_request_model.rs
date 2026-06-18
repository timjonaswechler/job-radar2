use regex::Regex;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};
use std::{
    collections::HashSet,
    sync::{Mutex, MutexGuard},
};

#[derive(Default)]
pub struct RunningSearchRuns {
    search_request_ids: Mutex<HashSet<i64>>,
}

impl RunningSearchRuns {
    #[allow(dead_code)]
    pub fn begin(&self, search_request_id: i64) -> Result<RunningSearchRun<'_>, String> {
        let mut search_request_ids = self.lock_search_request_ids()?;
        if !search_request_ids.insert(search_request_id) {
            return Err(format!(
                "search request {search_request_id} already has a running search run"
            ));
        }

        Ok(RunningSearchRun {
            registry: self,
            search_request_id,
        })
    }

    fn is_running(&self, search_request_id: i64) -> Result<bool, String> {
        Ok(self.lock_search_request_ids()?.contains(&search_request_id))
    }

    #[allow(dead_code)]
    fn finish(&self, search_request_id: i64) {
        if let Ok(mut search_request_ids) = self.search_request_ids.lock() {
            search_request_ids.remove(&search_request_id);
        }
    }

    fn lock_search_request_ids(&self) -> Result<MutexGuard<'_, HashSet<i64>>, String> {
        self.search_request_ids
            .lock()
            .map_err(|_| "running search run state is unavailable".to_string())
    }
}

#[allow(dead_code)]
pub struct RunningSearchRun<'a> {
    registry: &'a RunningSearchRuns,
    search_request_id: i64,
}

impl Drop for RunningSearchRun<'_> {
    fn drop(&mut self) {
        self.registry.finish(self.search_request_id);
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchRequestStatus {
    Draft,
    Active,
    Disabled,
    Invalid,
}

impl SearchRequestStatus {
    fn as_str(self) -> &'static str {
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchRuleTarget {
    Title,
}

impl TryFrom<&str> for SearchRuleTarget {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "title" => Ok(Self::Title),
            _ => Err("must be title".to_string()),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchRuleKind {
    Text,
    Regex,
}

impl TryFrom<&str> for SearchRuleKind {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "text" => Ok(Self::Text),
            "regex" => Ok(Self::Regex),
            _ => Err("must be text or regex".to_string()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRule {
    pub target: SearchRuleTarget,
    pub kind: SearchRuleKind,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRuleInput {
    pub target: String,
    pub kind: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    pub id: i64,
    pub status: SearchRequestStatus,
    pub include_rules: Vec<SearchRule>,
    pub exclude_rules: Vec<SearchRule>,
    pub locations: Vec<String>,
    pub radius_km: Option<i64>,
    pub source_ids: Vec<i64>,
    pub validation_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSearchRequestInput {
    pub status: SearchRequestStatus,
    pub include_rules: Vec<SearchRuleInput>,
    pub exclude_rules: Vec<SearchRuleInput>,
    pub locations: Vec<String>,
    pub radius_km: Option<i64>,
    pub source_ids: Vec<i64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSearchRequestInput {
    pub status: SearchRequestStatus,
    pub include_rules: Vec<SearchRuleInput>,
    pub exclude_rules: Vec<SearchRuleInput>,
    pub locations: Vec<String>,
    pub radius_km: Option<i64>,
    pub source_ids: Vec<i64>,
}

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
            input.source_ids,
        )
        .await?;
        let include_rules_json = json_to_string(&input.include_rules)?;
        let exclude_rules_json = json_to_string(&input.exclude_rules)?;
        let locations_json = json_to_string(&input.locations)?;
        let source_ids_json = json_to_string(&input.source_ids)?;

        let result = sqlx::query(
            "INSERT INTO search_requests (
               status, include_rules_json, exclude_rules_json, locations_json,
               radius_km, source_ids_json, validation_error
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(input.status.as_str())
        .bind(include_rules_json)
        .bind(exclude_rules_json)
        .bind(locations_json)
        .bind(input.radius_km)
        .bind(source_ids_json)
        .bind(input.validation_error.as_deref())
        .execute(self.pool)
        .await
        .map_err(db_error)?;

        self.get(result.last_insert_rowid()).await
    }

    pub async fn list(&self) -> Result<Vec<SearchRequest>, String> {
        let rows = sqlx::query(
            "SELECT id, status, include_rules_json, exclude_rules_json, locations_json,
                    radius_km, source_ids_json, validation_error, created_at, updated_at
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
                    radius_km, source_ids_json, validation_error, created_at, updated_at
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
            input.source_ids,
        )
        .await?;
        let include_rules_json = json_to_string(&input.include_rules)?;
        let exclude_rules_json = json_to_string(&input.exclude_rules)?;
        let locations_json = json_to_string(&input.locations)?;
        let source_ids_json = json_to_string(&input.source_ids)?;

        sqlx::query(
            "UPDATE search_requests
             SET status = ?1,
                 include_rules_json = ?2,
                 exclude_rules_json = ?3,
                 locations_json = ?4,
                 radius_km = ?5,
                 source_ids_json = ?6,
                 validation_error = ?7,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?8",
        )
        .bind(input.status.as_str())
        .bind(include_rules_json)
        .bind(exclude_rules_json)
        .bind(locations_json)
        .bind(input.radius_km)
        .bind(source_ids_json)
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

struct NormalizedSearchRequestInput {
    status: SearchRequestStatus,
    include_rules: Vec<SearchRule>,
    exclude_rules: Vec<SearchRule>,
    locations: Vec<String>,
    radius_km: Option<i64>,
    source_ids: Vec<i64>,
    validation_error: Option<String>,
}

async fn validate_search_request_input(
    _pool: &SqlitePool,
    status: SearchRequestStatus,
    include_rules: Vec<SearchRuleInput>,
    exclude_rules: Vec<SearchRuleInput>,
    locations: Vec<String>,
    radius_km: Option<i64>,
    source_ids: Vec<i64>,
) -> Result<NormalizedSearchRequestInput, String> {
    let (include_rules, mut validation_errors) = normalize_rules(include_rules, "includeRules")?;
    let (exclude_rules, exclude_validation_errors) =
        normalize_rules(exclude_rules, "excludeRules")?;
    validation_errors.extend(exclude_validation_errors);

    if let Some(radius_km) = radius_km {
        if radius_km < 0 {
            return Err("radiusKm must be greater than or equal to 0".to_string());
        }
    }

    validate_source_id_values(&source_ids)?;

    if status == SearchRequestStatus::Active {
        if include_rules.is_empty() {
            return Err(
                "active/executable search requests require at least one include rule".to_string(),
            );
        }
        if source_ids.is_empty() {
            return Err(
                "active/executable search requests require at least one sourceId".to_string(),
            );
        }
    }

    let validation_error = if validation_errors.is_empty() {
        None
    } else {
        Some(validation_errors.join("; "))
    };

    if status == SearchRequestStatus::Active {
        if let Some(validation_error) = &validation_error {
            return Err(format!(
                "active/executable search requests cannot have validationError: {validation_error}"
            ));
        }
    }

    Ok(NormalizedSearchRequestInput {
        status,
        include_rules,
        exclude_rules,
        locations: normalize_locations(locations),
        radius_km,
        source_ids,
        validation_error,
    })
}

fn normalize_rules(
    rules: Vec<SearchRuleInput>,
    field: &str,
) -> Result<(Vec<SearchRule>, Vec<String>), String> {
    let mut normalized_rules = Vec::with_capacity(rules.len());
    let mut validation_errors = Vec::new();

    for (index, rule) in rules.into_iter().enumerate() {
        let path = format!("{field}[{index}]");
        let target = SearchRuleTarget::try_from(rule.target.as_str())
            .map_err(|error| format!("{path}.target {error}"))?;
        let kind = SearchRuleKind::try_from(rule.kind.as_str())
            .map_err(|error| format!("{path}.kind {error}"))?;
        let value = rule.value.trim().to_string();
        if value.is_empty() {
            return Err(format!("{path}.value must not be empty"));
        }

        if kind == SearchRuleKind::Regex {
            if let Err(error) = Regex::new(&value) {
                validation_errors.push(format!("{path}.value is invalid regex: {error}"));
            }
        }

        normalized_rules.push(SearchRule {
            target,
            kind,
            value,
        });
    }

    Ok((normalized_rules, validation_errors))
}

fn normalize_locations(locations: Vec<String>) -> Vec<String> {
    locations
        .into_iter()
        .map(|location| location.trim().to_string())
        .filter(|location| !location.is_empty())
        .collect()
}

fn validate_source_id_values(source_ids: &[i64]) -> Result<(), String> {
    for source_id in source_ids {
        if *source_id < 1 {
            return Err(format!("sourceIds contains invalid source id {source_id}"));
        }
    }

    Ok(())
}

fn search_request_from_row(row: SqliteRow) -> Result<SearchRequest, String> {
    let status: String = row.try_get("status").map_err(db_error)?;

    Ok(SearchRequest {
        id: row.try_get("id").map_err(db_error)?,
        status: SearchRequestStatus::try_from(status.as_str())?,
        include_rules: json_from_row(&row, "include_rules_json")?,
        exclude_rules: json_from_row(&row, "exclude_rules_json")?,
        locations: json_from_row(&row, "locations_json")?,
        radius_km: row.try_get("radius_km").map_err(db_error)?,
        source_ids: json_from_row(&row, "source_ids_json")?,
        validation_error: row.try_get("validation_error").map_err(db_error)?,
        created_at: row.try_get("created_at").map_err(db_error)?,
        updated_at: row.try_get("updated_at").map_err(db_error)?,
    })
}

fn json_to_string<T>(value: &T) -> Result<String, String>
where
    T: Serialize,
{
    serde_json::to_string(value).map_err(|error| error.to_string())
}

fn json_from_row<T>(row: &SqliteRow, column: &str) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let json: String = row.try_get(column).map_err(db_error)?;
    serde_json::from_str(&json).map_err(|error| error.to_string())
}

fn db_error(error: sqlx::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    #[test]
    fn search_request_crud_round_trips_without_name() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let running_search_runs = RunningSearchRuns::default();
            let service = SearchRequestService::new(&pool, &running_search_runs);
            let source_id = 1;

            let created = service
                .create(CreateSearchRequestInput {
                    status: SearchRequestStatus::Active,
                    include_rules: vec![text_rule(" Physik ")],
                    exclude_rules: vec![text_rule("Praktikum")],
                    locations: vec![" Mainz ".to_string(), "".to_string()],
                    radius_km: Some(30),
                    source_ids: vec![source_id],
                })
                .await
                .unwrap();

            assert_eq!(created.status, SearchRequestStatus::Active);
            assert_eq!(created.include_rules[0].value, "Physik");
            assert_eq!(created.exclude_rules[0].value, "Praktikum");
            assert_eq!(created.locations, vec!["Mainz"]);
            assert_eq!(created.radius_km, Some(30));
            assert_eq!(created.source_ids, vec![source_id]);
            assert!(created.validation_error.is_none());
            assert!(!created.created_at.is_empty());
            assert!(!created.updated_at.is_empty());
            assert!(serde_json::to_value(&created)
                .unwrap()
                .get("name")
                .is_none());

            let listed = service.list().await.unwrap();
            assert_eq!(listed, vec![created.clone()]);
            assert_eq!(service.get(created.id).await.unwrap(), created);

            let updated = service
                .update(
                    created.id,
                    UpdateSearchRequestInput {
                        status: SearchRequestStatus::Draft,
                        include_rules: vec![regex_rule("Laser|Optik")],
                        exclude_rules: vec![],
                        locations: vec!["Berlin".to_string()],
                        radius_km: None,
                        source_ids: vec![],
                    },
                )
                .await
                .unwrap();

            assert_eq!(updated.status, SearchRequestStatus::Draft);
            assert_eq!(updated.include_rules[0].kind, SearchRuleKind::Regex);
            assert_eq!(updated.locations, vec!["Berlin"]);
            assert_eq!(updated.radius_km, None);
            assert!(updated.source_ids.is_empty());

            service.delete(created.id).await.unwrap();
            assert!(service.get(created.id).await.is_err());
        });
    }

    #[test]
    fn invalid_regex_is_persisted_as_validation_error_only_when_not_active() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let running_search_runs = RunningSearchRuns::default();
            let service = SearchRequestService::new(&pool, &running_search_runs);
            let source_id = 1;

            let draft = service
                .create(CreateSearchRequestInput {
                    status: SearchRequestStatus::Draft,
                    include_rules: vec![regex_rule("[")],
                    exclude_rules: vec![],
                    locations: vec![],
                    radius_km: None,
                    source_ids: vec![],
                })
                .await
                .unwrap();
            assert_eq!(draft.status, SearchRequestStatus::Draft);
            assert!(draft
                .validation_error
                .as_deref()
                .unwrap()
                .contains("includeRules[0].value is invalid regex"));

            let active_error = service
                .create(CreateSearchRequestInput {
                    status: SearchRequestStatus::Active,
                    include_rules: vec![regex_rule("[")],
                    exclude_rules: vec![],
                    locations: vec![],
                    radius_km: None,
                    source_ids: vec![source_id],
                })
                .await
                .unwrap_err();
            assert!(active_error.contains("validationError"));
            assert!(active_error.contains("invalid regex"));
        });
    }

    #[test]
    fn search_requests_do_not_query_removed_source_domain_table() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let running_search_runs = RunningSearchRuns::default();
            let service = SearchRequestService::new(&pool, &running_search_runs);

            let created = service
                .create(CreateSearchRequestInput {
                    status: SearchRequestStatus::Active,
                    include_rules: vec![text_rule("Physik")],
                    exclude_rules: vec![],
                    locations: vec![],
                    radius_km: None,
                    source_ids: vec![999],
                })
                .await
                .unwrap();

            assert_eq!(created.source_ids, vec![999]);
            assert_eq!(service.list().await.unwrap(), vec![created]);

            let invalid_source_id = service
                .create(CreateSearchRequestInput {
                    status: SearchRequestStatus::Draft,
                    include_rules: vec![text_rule("Physik")],
                    exclude_rules: vec![],
                    locations: vec![],
                    radius_km: None,
                    source_ids: vec![0],
                })
                .await
                .unwrap_err();
            assert!(invalid_source_id.contains("sourceIds contains invalid source id 0"));
        });
    }

    #[test]
    fn active_search_requests_require_include_rule_and_source_id() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let running_search_runs = RunningSearchRuns::default();
            let service = SearchRequestService::new(&pool, &running_search_runs);
            let source_id = 1;

            let missing_rule = service
                .create(CreateSearchRequestInput {
                    status: SearchRequestStatus::Active,
                    include_rules: vec![],
                    exclude_rules: vec![],
                    locations: vec![],
                    radius_km: None,
                    source_ids: vec![source_id],
                })
                .await
                .unwrap_err();
            assert!(missing_rule.contains("at least one include rule"));

            let missing_source = service
                .create(CreateSearchRequestInput {
                    status: SearchRequestStatus::Active,
                    include_rules: vec![text_rule("Physik")],
                    exclude_rules: vec![],
                    locations: vec![],
                    radius_km: None,
                    source_ids: vec![],
                })
                .await
                .unwrap_err();
            assert!(missing_source.contains("at least one sourceId"));
        });
    }

    #[test]
    fn unsupported_rules_and_empty_rule_values_are_rejected() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let running_search_runs = RunningSearchRuns::default();
            let service = SearchRequestService::new(&pool, &running_search_runs);

            let unsupported_target = service
                .create(CreateSearchRequestInput {
                    status: SearchRequestStatus::Draft,
                    include_rules: vec![SearchRuleInput {
                        target: "company".to_string(),
                        kind: "text".to_string(),
                        value: "Acme".to_string(),
                    }],
                    exclude_rules: vec![],
                    locations: vec![],
                    radius_km: None,
                    source_ids: vec![],
                })
                .await
                .unwrap_err();
            assert!(unsupported_target.contains("includeRules[0].target must be title"));

            let unsupported_kind = service
                .create(CreateSearchRequestInput {
                    status: SearchRequestStatus::Draft,
                    include_rules: vec![SearchRuleInput {
                        target: "title".to_string(),
                        kind: "glob".to_string(),
                        value: "Physik".to_string(),
                    }],
                    exclude_rules: vec![],
                    locations: vec![],
                    radius_km: None,
                    source_ids: vec![],
                })
                .await
                .unwrap_err();
            assert!(unsupported_kind.contains("includeRules[0].kind must be text or regex"));

            let empty_value = service
                .create(CreateSearchRequestInput {
                    status: SearchRequestStatus::Draft,
                    include_rules: vec![text_rule("   ")],
                    exclude_rules: vec![],
                    locations: vec![],
                    radius_km: None,
                    source_ids: vec![],
                })
                .await
                .unwrap_err();
            assert!(empty_value.contains("includeRules[0].value must not be empty"));
        });
    }

    #[test]
    fn update_and_delete_are_rejected_while_search_request_has_running_run() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let running_search_runs = RunningSearchRuns::default();
            let service = SearchRequestService::new(&pool, &running_search_runs);

            let created = service
                .create(CreateSearchRequestInput {
                    status: SearchRequestStatus::Draft,
                    include_rules: vec![text_rule("Physik")],
                    exclude_rules: vec![],
                    locations: vec![],
                    radius_km: None,
                    source_ids: vec![],
                })
                .await
                .unwrap();

            let running_run = running_search_runs.begin(created.id).unwrap();

            let update_error = service
                .update(
                    created.id,
                    UpdateSearchRequestInput {
                        status: SearchRequestStatus::Draft,
                        include_rules: vec![text_rule("Laser")],
                        exclude_rules: vec![],
                        locations: vec![],
                        radius_km: None,
                        source_ids: vec![],
                    },
                )
                .await
                .unwrap_err();
            assert!(update_error.contains("currently running search run"));

            let delete_error = service.delete(created.id).await.unwrap_err();
            assert!(delete_error.contains("currently running search run"));

            drop(running_run);
            service.delete(created.id).await.unwrap();
        });
    }

    fn text_rule(value: &str) -> SearchRuleInput {
        SearchRuleInput {
            target: "title".to_string(),
            kind: "text".to_string(),
            value: value.to_string(),
        }
    }

    fn regex_rule(value: &str) -> SearchRuleInput {
        SearchRuleInput {
            target: "title".to_string(),
            kind: "regex".to_string(),
            value: value.to_string(),
        }
    }

    async fn migrated_pool() -> SqlitePool {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();

        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        pool
    }
}
