use regex::Regex;
use serde::Serialize;
use sqlx::SqlitePool;
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
};

use crate::{
    search_request_model::{
        RunningSearchRuns, SearchRequest, SearchRequestService, SearchRule, SearchRuleKind,
        SearchRuleTarget,
    },
    search_run::normalization::{normalize_source_candidate, normalized_text_key},
    source_model::{
        get_browser_profile, get_source, get_system_profile, BrowserProfile, Source, SourceStatus,
        SystemProfile,
    },
};

pub type BoxedSourceExecutionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<SourceCandidate>, SourceExecutionError>> + Send + 'a>>;

/// Public source-execution seam used by Suchläufe.
///
/// `SearchRunService` loads active profiles for sources that reference them and
/// passes them here. Adapters that require runtime profile policy, including
/// declarative inventory runtimes and the transitional `stepstone_search`, return
/// `SourceExecutionError::Failed` when the required profile or definition is
/// missing or invalid.
pub struct SourceExecutionInput<'a> {
    pub search_request: &'a SearchRequest,
    pub source: &'a Source,
    pub system_profile: Option<&'a SystemProfile>,
    pub browser_profile: Option<&'a BrowserProfile>,
}

pub trait SourceExecutor: Send + Sync {
    fn execute<'a>(&'a self, input: SourceExecutionInput<'a>) -> BoxedSourceExecutionFuture<'a>;
}

pub struct DefaultSourceExecutor {
    browser_runtime_dir: PathBuf,
}

impl DefaultSourceExecutor {
    pub fn new(browser_runtime_dir: impl Into<PathBuf>) -> Self {
        Self {
            browser_runtime_dir: browser_runtime_dir.into(),
        }
    }
}

impl SourceExecutor for DefaultSourceExecutor {
    fn execute<'a>(&'a self, input: SourceExecutionInput<'a>) -> BoxedSourceExecutionFuture<'a> {
        Box::pin(async move {
            match input.source.adapter_key.as_str() {
                "declarative_endpoint_inventory" | "declarative_sitemap_inventory" => {
                    let executor =
                        crate::declarative_inventory_executor::DeclarativeInventoryExecutor::new_reqwest();
                    executor.execute(input).await
                }
                "stepstone_search" => {
                    let executor =
                        crate::stepstone_source_executor::StepstoneSearchExecutor::new_managed(
                            self.browser_runtime_dir.clone(),
                        );
                    executor.execute(input).await
                }
                _ => Err(SourceExecutionError::Failed(format!(
                    "adapterKey {} has no search-run executor yet",
                    input.source.adapter_key
                ))),
            }
        })
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceExecutionError {
    Failed(String),
    Cancelled(String),
}

impl SourceExecutionError {
    fn status(&self) -> SourceRunStatus {
        match self {
            Self::Failed(_) => SourceRunStatus::Failed,
            Self::Cancelled(_) => SourceRunStatus::Cancelled,
        }
    }

    fn message(&self) -> String {
        match self {
            Self::Failed(message) | Self::Cancelled(message) => message.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCandidate {
    pub title: String,
    pub company: String,
    pub url: String,
    pub locations: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchRunStatus {
    Completed,
    CompletedWithErrors,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRunStatus {
    Completed,
    Failed,
    Cancelled,
}

/// Current-result Suchlauf written to `search-run-result.json`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRunResult {
    pub search_request_id: i64,
    pub status: SearchRunStatus,
    pub generated_at: String,
    pub source_runs: Vec<SourceRunResult>,
    pub postings: Vec<NormalizedPosting>,
}

/// Quellenlauf outcome for one selected Quelle.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRunResult {
    pub source_id: i64,
    pub source_key: String,
    pub source_name: String,
    pub status: SourceRunStatus,
    pub candidate_count: usize,
    pub matched_count: usize,
    pub error: Option<String>,
}

/// Normalized Stellenanzeige after Trefferregel/Ausschlussregel filtering and dedupe.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedPosting {
    pub title: String,
    pub company: String,
    pub url: String,
    pub locations: Vec<String>,
    pub sources: Vec<PostingSource>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostingSource {
    pub source_id: i64,
    pub source_key: String,
    pub source_name: String,
    pub url: String,
}

pub struct SearchRunService<'a> {
    pool: &'a SqlitePool,
    running_search_runs: &'a RunningSearchRuns,
    source_executor: &'a dyn SourceExecutor,
    result_path: PathBuf,
}

impl<'a> SearchRunService<'a> {
    pub fn new(
        pool: &'a SqlitePool,
        running_search_runs: &'a RunningSearchRuns,
        source_executor: &'a dyn SourceExecutor,
        result_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            pool,
            running_search_runs,
            source_executor,
            result_path: result_path.into(),
        }
    }

    pub async fn run(&self, search_request_id: i64) -> Result<SearchRunResult, String> {
        let _running_run = self.running_search_runs.begin(search_request_id)?;
        let search_request = SearchRequestService::new(self.pool, self.running_search_runs)
            .get(search_request_id)
            .await?;
        validate_executable_search_request(&search_request)?;

        let include_rules = compile_rules(&search_request.include_rules, "includeRules")?;
        let exclude_rules = compile_rules(&search_request.exclude_rules, "excludeRules")?;
        let sources = load_selected_sources(self.pool, &search_request.source_ids).await?;

        let mut source_runs = Vec::with_capacity(sources.len());
        let mut candidates = Vec::new();

        for source in &sources {
            let system_profile =
                match load_active_system_profile_for_source(self.pool, source).await {
                    Ok(system_profile) => system_profile,
                    Err(error) => {
                        source_runs.push(source_run_failed(source, error));
                        continue;
                    }
                };
            let browser_profile =
                match load_active_browser_profile_for_source(self.pool, source).await {
                    Ok(browser_profile) => browser_profile,
                    Err(error) => {
                        source_runs.push(source_run_failed(source, error));
                        continue;
                    }
                };
            let input = SourceExecutionInput {
                search_request: &search_request,
                source,
                system_profile: system_profile.as_ref(),
                browser_profile: browser_profile.as_ref(),
            };

            match self.source_executor.execute(input).await {
                Ok(source_candidates) => {
                    let candidate_count = source_candidates.len();
                    candidates.extend(source_candidates.into_iter().filter_map(|candidate| {
                        normalize_source_candidate(candidate).map(|candidate| Treffer {
                            candidate,
                            source: posting_source(source, None),
                        })
                    }));
                    source_runs.push(source_run_completed(source, candidate_count));
                }
                Err(error) => source_runs.push(source_run_failed(source, error)),
            }
        }

        let positive_matches = candidates
            .into_iter()
            .filter(|candidate| matches_any_rule(&include_rules, &candidate.candidate))
            .collect::<Vec<_>>();
        let treffers = positive_matches
            .into_iter()
            .filter(|candidate| !matches_any_rule(&exclude_rules, &candidate.candidate))
            .collect::<Vec<_>>();

        let mut matched_counts = HashMap::<i64, usize>::new();
        for treffer in &treffers {
            *matched_counts.entry(treffer.source.source_id).or_default() += 1;
        }
        for source_run in &mut source_runs {
            source_run.matched_count = matched_counts
                .get(&source_run.source_id)
                .copied()
                .unwrap_or_default();
        }

        let result = SearchRunResult {
            search_request_id,
            status: overall_status(&source_runs),
            generated_at: generated_at_timestamp(self.pool).await?,
            source_runs,
            postings: merge_postings(treffers),
        };

        write_search_run_result(&self.result_path, &result).await?;

        Ok(result)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Treffer candidate that matched a Suchanfrage before final Stellenanzeige merging.
struct Treffer {
    candidate: SourceCandidate,
    source: PostingSource,
}

struct CompiledRule {
    target: SearchRuleTarget,
    matcher: CompiledRuleMatcher,
}

enum CompiledRuleMatcher {
    Text(String),
    Regex(Regex),
}

pub fn default_search_run_result_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .unwrap_or(manifest_dir.as_path())
        .join("search-run-result.json")
}

async fn load_selected_sources(
    pool: &SqlitePool,
    source_ids: &[i64],
) -> Result<Vec<Source>, String> {
    let mut sources = Vec::with_capacity(source_ids.len());
    for source_id in source_ids {
        sources.push(get_source(pool, *source_id).await?);
    }
    Ok(sources)
}

async fn load_active_system_profile_for_source(
    pool: &SqlitePool,
    source: &Source,
) -> Result<Option<SystemProfile>, SourceExecutionError> {
    let Some(system_profile_id) = source.system_profile_id else {
        return Ok(None);
    };

    let system_profile = get_system_profile(pool, system_profile_id)
        .await
        .map_err(SourceExecutionError::Failed)?;
    if system_profile.status != SourceStatus::Active {
        return Err(SourceExecutionError::Failed(format!(
            "systemProfileId {system_profile_id} for source {} must reference an active system profile",
            source.key
        )));
    }

    Ok(Some(system_profile))
}

async fn load_active_browser_profile_for_source(
    pool: &SqlitePool,
    source: &Source,
) -> Result<Option<BrowserProfile>, SourceExecutionError> {
    let Some(browser_profile_id) = source.browser_profile_id else {
        return Ok(None);
    };

    let browser_profile = get_browser_profile(pool, browser_profile_id)
        .await
        .map_err(|error| {
            SourceExecutionError::Failed(format!(
                "browserProfileId {browser_profile_id} for source {} could not be loaded: {error}",
                source.key
            ))
        })?;
    if browser_profile.status != SourceStatus::Active {
        return Err(SourceExecutionError::Failed(format!(
            "browserProfileId {browser_profile_id} for source {} must reference an active browser profile",
            source.key
        )));
    }

    Ok(Some(browser_profile))
}

fn validate_executable_search_request(search_request: &SearchRequest) -> Result<(), String> {
    if let Some(validation_error) = &search_request.validation_error {
        return Err(format!(
            "search request {} cannot run with validationError: {validation_error}",
            search_request.id
        ));
    }
    if search_request.include_rules.is_empty() {
        return Err(format!(
            "search request {} cannot run without include rules",
            search_request.id
        ));
    }
    if search_request.source_ids.is_empty() {
        return Err(format!(
            "search request {} cannot run without selected sources",
            search_request.id
        ));
    }
    Ok(())
}

fn compile_rules(rules: &[SearchRule], field: &str) -> Result<Vec<CompiledRule>, String> {
    rules
        .iter()
        .enumerate()
        .map(|(index, rule)| {
            let matcher = match rule.kind {
                SearchRuleKind::Text => CompiledRuleMatcher::Text(rule.value.to_lowercase()),
                SearchRuleKind::Regex => {
                    CompiledRuleMatcher::Regex(Regex::new(&rule.value).map_err(|error| {
                        format!("{field}[{index}].value saved regex is invalid: {error}")
                    })?)
                }
            };
            Ok(CompiledRule {
                target: rule.target,
                matcher,
            })
        })
        .collect()
}

fn matches_any_rule(rules: &[CompiledRule], candidate: &SourceCandidate) -> bool {
    rules.iter().any(|rule| matches_rule(rule, candidate))
}

fn matches_rule(rule: &CompiledRule, candidate: &SourceCandidate) -> bool {
    let value = match rule.target {
        SearchRuleTarget::Title => candidate.title.as_str(),
    };

    match &rule.matcher {
        CompiledRuleMatcher::Text(needle) => value.to_lowercase().contains(needle),
        CompiledRuleMatcher::Regex(regex) => regex.is_match(value),
    }
}

fn posting_source(source: &Source, url: Option<String>) -> PostingSource {
    PostingSource {
        source_id: source.id,
        source_key: source.key.clone(),
        source_name: source.name.clone(),
        url: url.unwrap_or_default(),
    }
}

fn source_run_completed(source: &Source, candidate_count: usize) -> SourceRunResult {
    SourceRunResult {
        source_id: source.id,
        source_key: source.key.clone(),
        source_name: source.name.clone(),
        status: SourceRunStatus::Completed,
        candidate_count,
        matched_count: 0,
        error: None,
    }
}

fn source_run_failed(source: &Source, error: SourceExecutionError) -> SourceRunResult {
    SourceRunResult {
        source_id: source.id,
        source_key: source.key.clone(),
        source_name: source.name.clone(),
        status: error.status(),
        candidate_count: 0,
        matched_count: 0,
        error: Some(error.message()),
    }
}

fn overall_status(source_runs: &[SourceRunResult]) -> SearchRunStatus {
    if source_runs
        .iter()
        .all(|source_run| source_run.status == SourceRunStatus::Cancelled)
    {
        return SearchRunStatus::Cancelled;
    }

    let completed_count = source_runs
        .iter()
        .filter(|source_run| source_run.status == SourceRunStatus::Completed)
        .count();
    let failed_or_cancelled_count = source_runs.len().saturating_sub(completed_count);

    match (completed_count, failed_or_cancelled_count) {
        (0, _) => SearchRunStatus::Failed,
        (_, 0) => SearchRunStatus::Completed,
        _ => SearchRunStatus::CompletedWithErrors,
    }
}

fn merge_postings(treffers: Vec<Treffer>) -> Vec<NormalizedPosting> {
    let mut postings = Vec::<NormalizedPosting>::new();

    for treffer in treffers {
        if let Some(existing) = postings
            .iter_mut()
            .find(|posting| can_merge(posting, &treffer.candidate))
        {
            merge_into_posting(existing, treffer);
        } else {
            postings.push(NormalizedPosting {
                title: treffer.candidate.title,
                company: treffer.candidate.company,
                url: treffer.candidate.url.clone(),
                locations: treffer.candidate.locations,
                sources: vec![PostingSource {
                    url: treffer.candidate.url,
                    ..treffer.source
                }],
            });
        }
    }

    postings
}

fn can_merge(posting: &NormalizedPosting, candidate: &SourceCandidate) -> bool {
    if normalized_text_key(&posting.company) != normalized_text_key(&candidate.company)
        || normalized_text_key(&posting.title) != normalized_text_key(&candidate.title)
    {
        return false;
    }

    if posting.locations.is_empty() || candidate.locations.is_empty() {
        return true;
    }

    let existing_location_keys = posting
        .locations
        .iter()
        .map(|location| normalized_text_key(location))
        .collect::<HashSet<_>>();
    candidate
        .locations
        .iter()
        .any(|location| existing_location_keys.contains(&normalized_text_key(location)))
}

fn merge_into_posting(posting: &mut NormalizedPosting, treffer: Treffer) {
    let mut existing_location_keys = posting
        .locations
        .iter()
        .map(|location| normalized_text_key(location))
        .collect::<HashSet<_>>();
    for location in treffer.candidate.locations {
        if existing_location_keys.insert(normalized_text_key(&location)) {
            posting.locations.push(location);
        }
    }

    let source = PostingSource {
        url: treffer.candidate.url,
        ..treffer.source
    };
    if !posting.sources.iter().any(|existing| existing == &source) {
        posting.sources.push(source);
    }
}

async fn generated_at_timestamp(pool: &SqlitePool) -> Result<String, String> {
    sqlx::query_scalar::<_, String>("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')")
        .fetch_one(pool)
        .await
        .map_err(db_error)
}

async fn write_search_run_result(path: &Path, result: &SearchRunResult) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|error| error.to_string())?;
        }
    }

    let json = serde_json::to_string_pretty(result).map_err(|error| error.to_string())?;
    tokio::fs::write(path, json)
        .await
        .map_err(|error| error.to_string())
}

fn db_error(error: sqlx::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        search_request_model::{
            CreateSearchRequestInput, RunningSearchRuns, SearchRequestStatus, SearchRuleInput,
        },
        source_model::{
            create_browser_profile, create_source, BrowserProfile, CreateBrowserProfileInput,
            CreateSourceInput, SourceStatus,
        },
    };
    use serde_json::{json, Value};
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::sync::Mutex;

    struct FixtureSourceExecutor {
        responses: Mutex<HashMap<i64, Result<Vec<SourceCandidate>, SourceExecutionError>>>,
    }

    impl FixtureSourceExecutor {
        fn new(
            responses: impl IntoIterator<
                Item = (i64, Result<Vec<SourceCandidate>, SourceExecutionError>),
            >,
        ) -> Self {
            Self {
                responses: Mutex::new(responses.into_iter().collect()),
            }
        }
    }

    impl SourceExecutor for FixtureSourceExecutor {
        fn execute<'a>(
            &'a self,
            input: SourceExecutionInput<'a>,
        ) -> BoxedSourceExecutionFuture<'a> {
            Box::pin(async move {
                self.responses
                    .lock()
                    .unwrap()
                    .get(&input.source.id)
                    .cloned()
                    .unwrap_or_else(|| {
                        Err(SourceExecutionError::Failed(format!(
                            "missing fixture response for source {}",
                            input.source.id
                        )))
                    })
            })
        }
    }

    struct BrowserProfileCapturingExecutor {
        response: Result<Vec<SourceCandidate>, SourceExecutionError>,
        seen_browser_profiles: Mutex<Vec<(i64, Option<(i64, String)>)>>,
    }

    impl BrowserProfileCapturingExecutor {
        fn new(response: Result<Vec<SourceCandidate>, SourceExecutionError>) -> Self {
            Self {
                response,
                seen_browser_profiles: Mutex::new(Vec::new()),
            }
        }

        fn seen_browser_profiles(&self) -> Vec<(i64, Option<(i64, String)>)> {
            self.seen_browser_profiles.lock().unwrap().clone()
        }
    }

    impl SourceExecutor for BrowserProfileCapturingExecutor {
        fn execute<'a>(
            &'a self,
            input: SourceExecutionInput<'a>,
        ) -> BoxedSourceExecutionFuture<'a> {
            Box::pin(async move {
                self.seen_browser_profiles.lock().unwrap().push((
                    input.source.id,
                    input
                        .browser_profile
                        .map(|profile| (profile.id, profile.key.clone())),
                ));
                self.response.clone()
            })
        }
    }

    #[test]
    fn source_execution_input_includes_active_browser_profile_for_source() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let browser_profile =
                create_test_browser_profile(&pool, "manual_release_runtime", SourceStatus::Active)
                    .await;
            let source = create_stepstone_test_source(
                &pool,
                "browser_runtime_source",
                "Browser Runtime Source",
                browser_profile.id,
            )
            .await;
            let search_request = create_test_search_request(
                &pool,
                vec![source.id],
                vec![text_rule("engineer")],
                vec![],
            )
            .await;
            let temp_dir = tempfile::tempdir().unwrap();
            let executor = BrowserProfileCapturingExecutor::new(Ok(vec![candidate(
                "Browser Engineer",
                "ACME",
                "https://example.test/browser",
                &[],
            )]));
            let running_search_runs = RunningSearchRuns::default();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
            )
            .run(search_request.id)
            .await
            .unwrap();

            assert_eq!(result.status, SearchRunStatus::Completed);
            assert_eq!(result.source_runs[0].status, SourceRunStatus::Completed);
            assert_eq!(
                executor.seen_browser_profiles(),
                vec![(
                    source.id,
                    Some((browser_profile.id, "manual_release_runtime".to_string()))
                )]
            );
        });
    }

    #[test]
    fn missing_browser_profile_marks_source_run_failed_and_continues() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let browser_profile = create_test_browser_profile(
                &pool,
                "manual_release_before_missing",
                SourceStatus::Active,
            )
            .await;
            let missing_profile_source = create_stepstone_test_source(
                &pool,
                "missing_profile_source",
                "Missing Profile Source",
                browser_profile.id,
            )
            .await;
            let healthy_source = create_stepstone_test_source(
                &pool,
                "healthy_profile_source",
                "Healthy Profile Source",
                browser_profile.id,
            )
            .await;
            set_source_browser_profile_id_without_foreign_key_check(
                &pool,
                missing_profile_source.id,
                999_999,
            )
            .await;
            let search_request = create_test_search_request(
                &pool,
                vec![missing_profile_source.id, healthy_source.id],
                vec![text_rule("engineer")],
                vec![],
            )
            .await;
            let temp_dir = tempfile::tempdir().unwrap();
            let executor = FixtureSourceExecutor::new([(
                healthy_source.id,
                Ok(vec![candidate(
                    "Healthy Engineer",
                    "ACME",
                    "https://example.test/healthy",
                    &[],
                )]),
            )]);
            let running_search_runs = RunningSearchRuns::default();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
            )
            .run(search_request.id)
            .await
            .unwrap();

            assert_eq!(result.status, SearchRunStatus::CompletedWithErrors);
            assert_eq!(result.source_runs[0].source_key, "missing_profile_source");
            assert_eq!(result.source_runs[0].status, SourceRunStatus::Failed);
            let error = result.source_runs[0].error.as_deref().unwrap();
            assert!(error.contains("browserProfileId 999999"));
            assert!(error.contains("source missing_profile_source"));
            assert_eq!(result.source_runs[1].source_key, "healthy_profile_source");
            assert_eq!(result.source_runs[1].status, SourceRunStatus::Completed);
            assert_eq!(result.postings.len(), 1);
        });
    }

    #[test]
    fn inactive_browser_profile_marks_source_run_failed_with_clear_error() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let browser_profile = create_test_browser_profile(
                &pool,
                "manual_release_disabled",
                SourceStatus::Disabled,
            )
            .await;
            let source = create_stepstone_test_source(
                &pool,
                "disabled_profile_source",
                "Disabled Profile Source",
                browser_profile.id,
            )
            .await;
            let search_request = create_test_search_request(
                &pool,
                vec![source.id],
                vec![text_rule("engineer")],
                vec![],
            )
            .await;
            let temp_dir = tempfile::tempdir().unwrap();
            let executor = FixtureSourceExecutor::new([]);
            let running_search_runs = RunningSearchRuns::default();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
            )
            .run(search_request.id)
            .await
            .unwrap();

            assert_eq!(result.status, SearchRunStatus::Failed);
            assert_eq!(result.source_runs[0].status, SourceRunStatus::Failed);
            let expected_error = format!(
                "browserProfileId {} for source disabled_profile_source must reference an active browser profile",
                browser_profile.id
            );
            assert_eq!(
                result.source_runs[0].error.as_deref(),
                Some(expected_error.as_str())
            );
            assert!(result.postings.is_empty());
        });
    }

    #[test]
    fn matching_uses_or_semantics_and_excludes_after_positive_matching() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let source_ids = create_test_sources(&pool, &[("test_source", "Test Source")]).await;
            let search_request = create_test_search_request(
                &pool,
                source_ids.clone(),
                vec![text_rule("laser"), regex_rule("Optics\\s+Engineer")],
                vec![text_rule("praktikum"), regex_rule("Werkstudent")],
            )
            .await;
            let temp_dir = tempfile::tempdir().unwrap();
            let result_path = temp_dir.path().join("search-run-result.json");
            let executor = FixtureSourceExecutor::new([(
                source_ids[0],
                Ok(vec![
                    candidate(
                        "LASER Physicist",
                        "SCHOTT",
                        "https://example.test/1",
                        &["Mainz"],
                    ),
                    candidate(
                        "Senior Optics Engineer",
                        "SCHOTT",
                        "https://example.test/2",
                        &["Mainz"],
                    ),
                    candidate(
                        "Laser Praktikum",
                        "SCHOTT",
                        "https://example.test/3",
                        &["Mainz"],
                    ),
                    candidate(
                        "Werkstudent Optics Engineer",
                        "SCHOTT",
                        "https://example.test/4",
                        &["Mainz"],
                    ),
                    candidate("Chemist", "SCHOTT", "https://example.test/5", &["Mainz"]),
                ]),
            )]);
            let running_search_runs = RunningSearchRuns::default();

            let result =
                SearchRunService::new(&pool, &running_search_runs, &executor, result_path.clone())
                    .run(search_request.id)
                    .await
                    .unwrap();

            assert_eq!(result.status, SearchRunStatus::Completed);
            assert_eq!(result.source_runs[0].candidate_count, 5);
            assert_eq!(result.source_runs[0].matched_count, 2);
            assert_eq!(
                result
                    .postings
                    .iter()
                    .map(|posting| posting.title.as_str())
                    .collect::<Vec<_>>(),
                vec!["LASER Physicist", "Senior Optics Engineer"]
            );
            let result_json: Value = serde_json::from_str(
                &std::fs::read_to_string(result_path).expect("result JSON should be written"),
            )
            .unwrap();
            assert_eq!(result_json["status"], "completed");
            assert_eq!(result_json["sourceRuns"][0]["matchedCount"], 2);
        });
    }

    #[test]
    fn normalizes_source_candidates_before_matching_and_merging() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let source_ids = create_test_sources(&pool, &[("test_source", "Test Source")]).await;
            let search_request = create_test_search_request(
                &pool,
                source_ids.clone(),
                vec![text_rule("Senior Laser Engineer")],
                vec![],
            )
            .await;
            let temp_dir = tempfile::tempdir().unwrap();
            let executor = FixtureSourceExecutor::new([(
                source_ids[0],
                Ok(vec![
                    candidate(
                        "  Senior\n Laser   Engineer  ",
                        "\tACME   GmbH\n",
                        " https://example.test/laser ",
                        &[" Mainz ", "", "mainz", "  München\nNord  "],
                    ),
                    candidate(
                        "Senior Laser Engineer",
                        " ",
                        "https://example.test/empty-company",
                        &["Remote"],
                    ),
                ]),
            )]);
            let running_search_runs = RunningSearchRuns::default();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
            )
            .run(search_request.id)
            .await
            .unwrap();

            assert_eq!(result.status, SearchRunStatus::Completed);
            assert_eq!(result.source_runs[0].candidate_count, 2);
            assert_eq!(result.source_runs[0].matched_count, 1);
            assert_eq!(result.postings.len(), 1);
            let posting = &result.postings[0];
            assert_eq!(posting.title, "Senior Laser Engineer");
            assert_eq!(posting.company, "ACME GmbH");
            assert_eq!(posting.url, "https://example.test/laser");
            assert_eq!(posting.locations, vec!["Mainz", "München Nord"]);
            assert_eq!(posting.sources[0].url, "https://example.test/laser");
        });
    }

    #[test]
    fn dedupes_with_overlapping_locations_or_missing_locations_and_preserves_sources() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let source_ids = create_test_sources(
                &pool,
                &[("source_one", "Source One"), ("source_two", "Source Two")],
            )
            .await;
            let search_request = create_test_search_request(
                &pool,
                source_ids.clone(),
                vec![text_rule("engineer")],
                vec![],
            )
            .await;
            let temp_dir = tempfile::tempdir().unwrap();
            let executor = FixtureSourceExecutor::new([
                (
                    source_ids[0],
                    Ok(vec![
                        candidate(
                            "Laser Engineer",
                            "ACME",
                            "https://source-one.test/laser",
                            &["Mainz"],
                        ),
                        candidate(
                            "Remote Engineer",
                            "ACME",
                            "https://source-one.test/remote",
                            &[],
                        ),
                        candidate(
                            "Optics Engineer",
                            "ACME",
                            "https://source-one.test/optics-berlin",
                            &["Berlin"],
                        ),
                    ]),
                ),
                (
                    source_ids[1],
                    Ok(vec![
                        candidate(
                            "Laser Engineer",
                            "ACME",
                            "https://source-two.test/laser",
                            &[" mainz ", "Wiesbaden"],
                        ),
                        candidate(
                            "Remote Engineer",
                            "ACME",
                            "https://source-two.test/remote",
                            &["Berlin"],
                        ),
                        candidate(
                            "Optics Engineer",
                            "ACME",
                            "https://source-two.test/optics-hamburg",
                            &["Hamburg"],
                        ),
                    ]),
                ),
            ]);
            let running_search_runs = RunningSearchRuns::default();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
            )
            .run(search_request.id)
            .await
            .unwrap();

            assert_eq!(result.status, SearchRunStatus::Completed);
            assert_eq!(result.postings.len(), 4);

            let laser = result
                .postings
                .iter()
                .find(|posting| posting.title == "Laser Engineer")
                .unwrap();
            assert_eq!(laser.locations, vec!["Mainz", "Wiesbaden"]);
            assert_eq!(laser.sources.len(), 2);
            assert_eq!(
                laser
                    .sources
                    .iter()
                    .map(|source| source.source_key.as_str())
                    .collect::<Vec<_>>(),
                vec!["source_one", "source_two"]
            );

            let remote = result
                .postings
                .iter()
                .find(|posting| posting.title == "Remote Engineer")
                .unwrap();
            assert_eq!(remote.locations, vec!["Berlin"]);
            assert_eq!(remote.sources.len(), 2);

            let optics_postings = result
                .postings
                .iter()
                .filter(|posting| posting.title == "Optics Engineer")
                .collect::<Vec<_>>();
            assert_eq!(optics_postings.len(), 2);
            assert!(optics_postings
                .iter()
                .any(|posting| posting.locations == vec!["Berlin"]));
            assert!(optics_postings
                .iter()
                .any(|posting| posting.locations == vec!["Hamburg"]));
        });
    }

    #[test]
    fn partial_source_failure_completes_with_errors_and_records_failed_source_error() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let source_ids = create_test_sources(
                &pool,
                &[("source_one", "Source One"), ("source_two", "Source Two")],
            )
            .await;
            let search_request = create_test_search_request(
                &pool,
                source_ids.clone(),
                vec![text_rule("engineer")],
                vec![],
            )
            .await;
            let temp_dir = tempfile::tempdir().unwrap();
            let result_path = temp_dir.path().join("search-run-result.json");
            let executor = FixtureSourceExecutor::new([
                (
                    source_ids[0],
                    Ok(vec![candidate(
                        "Laser Engineer",
                        "ACME",
                        "https://source-one.test/laser",
                        &["Mainz"],
                    )]),
                ),
                (
                    source_ids[1],
                    Err(SourceExecutionError::Failed(
                        "fixture source failed".to_string(),
                    )),
                ),
            ]);
            let running_search_runs = RunningSearchRuns::default();

            let result =
                SearchRunService::new(&pool, &running_search_runs, &executor, result_path.clone())
                    .run(search_request.id)
                    .await
                    .unwrap();

            assert_eq!(result.status, SearchRunStatus::CompletedWithErrors);
            assert_eq!(result.postings.len(), 1);
            assert_eq!(result.source_runs[0].status, SourceRunStatus::Completed);
            assert_eq!(result.source_runs[1].status, SourceRunStatus::Failed);
            assert_eq!(
                result.source_runs[1].error.as_deref(),
                Some("fixture source failed")
            );

            let result_json: Value =
                serde_json::from_str(&std::fs::read_to_string(result_path).unwrap()).unwrap();
            assert_eq!(result_json["status"], "completed_with_errors");
            assert_eq!(
                result_json["sourceRuns"][1]["error"],
                "fixture source failed"
            );
        });
    }

    #[test]
    fn total_source_failure_produces_failed_result_without_postings() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let source_ids = create_test_sources(
                &pool,
                &[("source_one", "Source One"), ("source_two", "Source Two")],
            )
            .await;
            let search_request = create_test_search_request(
                &pool,
                source_ids.clone(),
                vec![text_rule("engineer")],
                vec![],
            )
            .await;
            let temp_dir = tempfile::tempdir().unwrap();
            let executor = FixtureSourceExecutor::new([
                (
                    source_ids[0],
                    Err(SourceExecutionError::Failed("first failed".to_string())),
                ),
                (
                    source_ids[1],
                    Err(SourceExecutionError::Failed("second failed".to_string())),
                ),
            ]);
            let running_search_runs = RunningSearchRuns::default();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
            )
            .run(search_request.id)
            .await
            .unwrap();

            assert_eq!(result.status, SearchRunStatus::Failed);
            assert!(result.postings.is_empty());
            assert!(result
                .source_runs
                .iter()
                .all(|source_run| source_run.status == SourceRunStatus::Failed));
        });
    }

    #[test]
    fn each_run_overwrites_search_run_result_json() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let source_ids = create_test_sources(&pool, &[("test_source", "Test Source")]).await;
            let search_request = create_test_search_request(
                &pool,
                source_ids.clone(),
                vec![text_rule("engineer")],
                vec![],
            )
            .await;
            let temp_dir = tempfile::tempdir().unwrap();
            let result_path = temp_dir.path().join("search-run-result.json");
            std::fs::write(&result_path, "stale result").unwrap();
            let running_search_runs = RunningSearchRuns::default();

            let first_executor = FixtureSourceExecutor::new([(
                source_ids[0],
                Ok(vec![candidate(
                    "First Engineer",
                    "ACME",
                    "https://example.test/first",
                    &[],
                )]),
            )]);
            SearchRunService::new(
                &pool,
                &running_search_runs,
                &first_executor,
                result_path.clone(),
            )
            .run(search_request.id)
            .await
            .unwrap();
            let first_contents = std::fs::read_to_string(&result_path).unwrap();
            assert!(first_contents.contains("First Engineer"));
            assert!(!first_contents.contains("stale result"));

            let second_executor = FixtureSourceExecutor::new([(
                source_ids[0],
                Ok(vec![candidate(
                    "Second Engineer",
                    "ACME",
                    "https://example.test/second",
                    &[],
                )]),
            )]);
            SearchRunService::new(
                &pool,
                &running_search_runs,
                &second_executor,
                result_path.clone(),
            )
            .run(search_request.id)
            .await
            .unwrap();

            let second_contents = std::fs::read_to_string(&result_path).unwrap();
            assert!(second_contents.contains("Second Engineer"));
            assert!(!second_contents.contains("First Engineer"));
            let result_json: Value = serde_json::from_str(&second_contents).unwrap();
            assert_eq!(result_json["postings"][0]["title"], "Second Engineer");
        });
    }

    async fn create_test_search_request(
        pool: &SqlitePool,
        source_ids: Vec<i64>,
        include_rules: Vec<SearchRuleInput>,
        exclude_rules: Vec<SearchRuleInput>,
    ) -> SearchRequest {
        let running_search_runs = RunningSearchRuns::default();
        SearchRequestService::new(pool, &running_search_runs)
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Active,
                include_rules,
                exclude_rules,
                locations: vec!["Mainz".to_string()],
                radius_km: Some(30),
                source_ids,
            })
            .await
            .unwrap()
    }

    async fn create_test_browser_profile(
        pool: &SqlitePool,
        key: &str,
        status: SourceStatus,
    ) -> BrowserProfile {
        create_browser_profile(
            pool,
            CreateBrowserProfileInput {
                key: key.to_string(),
                name: "Manuelle Freigabe".to_string(),
                description: None,
                name_i18n_key: None,
                description_i18n_key: None,
                definition_path: None,
                definition_hash: None,
                definition_schema_version: 1,
                definition: json!({}),
                source_config_schema: json!({ "type": "object" }),
                status,
                validation_error: None,
            },
        )
        .await
        .unwrap()
    }

    async fn create_stepstone_test_source(
        pool: &SqlitePool,
        key: &str,
        name: &str,
        browser_profile_id: i64,
    ) -> Source {
        create_source(
            pool,
            CreateSourceInput {
                key: key.to_string(),
                adapter_key: "stepstone_search".to_string(),
                system_profile_id: None,
                browser_profile_id: Some(browser_profile_id),
                name: name.to_string(),
                description: None,
                source_config: json!({}),
                status: SourceStatus::Active,
                validation_error: None,
            },
        )
        .await
        .unwrap()
    }

    async fn create_test_sources(pool: &SqlitePool, sources: &[(&str, &str)]) -> Vec<i64> {
        let browser_profile =
            create_test_browser_profile(pool, "manual_release", SourceStatus::Active).await;

        let mut source_ids = Vec::new();
        for (key, name) in sources {
            source_ids.push(
                create_stepstone_test_source(pool, key, name, browser_profile.id)
                    .await
                    .id,
            );
        }

        source_ids
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

    fn candidate(title: &str, company: &str, url: &str, locations: &[&str]) -> SourceCandidate {
        SourceCandidate {
            title: title.to_string(),
            company: company.to_string(),
            url: url.to_string(),
            locations: locations
                .iter()
                .map(|location| (*location).to_string())
                .collect(),
        }
    }

    async fn set_source_browser_profile_id_without_foreign_key_check(
        pool: &SqlitePool,
        source_id: i64,
        browser_profile_id: i64,
    ) {
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("UPDATE sources SET browser_profile_id = ?1 WHERE id = ?2")
            .bind(browser_profile_id)
            .bind(source_id)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(pool)
            .await
            .unwrap();
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
