use sqlx::{sqlite::SqliteRow, Row, SqlitePool};
use std::path::Path;

use crate::{
    profile_dsl::{
        compiler::{compile_source, CompileSourceOutcome},
        diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics},
        runtime::{
            execute_detail, ManagedProfileBrowserClient, PhaseCompletion, PostingOccurrence,
            ProfileBrowserClient, ProfileHttpClient, RequestedDetailFields,
            ReqwestProfileHttpClient, RuntimeExecutionContext,
        },
    },
    source_profile::registry::SourceProfileRegistrySnapshot,
};

use super::{
    ApplicationState, InterestState, JobPosting, JobPostingQueueCounts, JobPostingQueueId,
    JobPostingSource, JobPostingView, PostingDescriptionState, PreparationState, ReadState,
    UpdateJobPostingStateInput,
};

const ARCHIVE_QUEUE_CONDITION: &str = "(interest_state = 'dismissed'
    OR application_state IN ('rejected_by_company', 'withdrawn_by_me', 'accepted'))";
const APPLIED_QUEUE_CONDITION: &str = "(NOT (interest_state = 'dismissed'
    OR application_state IN ('rejected_by_company', 'withdrawn_by_me', 'accepted'))
    AND application_state IN ('submitted', 'in_process'))";
const INBOX_QUEUE_CONDITION: &str = "(NOT (interest_state = 'dismissed'
    OR application_state IN ('rejected_by_company', 'withdrawn_by_me', 'accepted'))
    AND interest_state = 'undecided'
    AND application_state = 'not_applied')";
const NEW_INBOX_CONDITION: &str = "(NOT (interest_state = 'dismissed'
    OR application_state IN ('rejected_by_company', 'withdrawn_by_me', 'accepted'))
    AND read_state = 'unread'
    AND interest_state = 'undecided'
    AND application_state = 'not_applied')";
const REVIEW_INBOX_CONDITION: &str = "(NOT (interest_state = 'dismissed'
    OR application_state IN ('rejected_by_company', 'withdrawn_by_me', 'accepted'))
    AND read_state = 'read'
    AND interest_state = 'undecided'
    AND application_state = 'not_applied')";
const INTERESTED_QUEUE_CONDITION: &str = "(interest_state = 'interested'
    AND preparation_state = 'not_started'
    AND application_state = 'not_applied')";
const PREPARATION_QUEUE_CONDITION: &str = "(interest_state = 'interested'
    AND application_state = 'not_applied'
    AND preparation_state IN ('in_progress', 'ready'))";

pub struct JobPostingService<'a> {
    pool: &'a SqlitePool,
}

impl<'a> JobPostingService<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<JobPosting>, String> {
        self.list_for_queue(JobPostingQueueId::All).await
    }

    pub async fn list_for_queue(
        &self,
        queue_id: JobPostingQueueId,
    ) -> Result<Vec<JobPosting>, String> {
        let where_clause = queue_condition(queue_id)
            .map(|condition| format!("WHERE {condition}"))
            .unwrap_or_default();
        let sql = format!(
            "SELECT id, title, company, locations_json, description_text, primary_source_id,
                    read_state, interest_state, preparation_state, application_state,
                    first_seen_at, last_seen_at, created_at, updated_at
             FROM job_postings
             {where_clause}
             ORDER BY last_seen_at DESC, id DESC"
        );
        let rows = sqlx::query(&sql)
            .fetch_all(self.pool)
            .await
            .map_err(db_error)?;

        let mut postings = Vec::with_capacity(rows.len());
        for row in rows {
            postings.push(self.posting_from_row(row).await?);
        }

        Ok(postings)
    }

    pub async fn queue_counts(&self) -> Result<JobPostingQueueCounts, String> {
        let sql = format!(
            "SELECT
                COUNT(*) AS all_count,
                COALESCE(SUM(CASE WHEN {ARCHIVE_QUEUE_CONDITION}
                    THEN 1 ELSE 0 END), 0) AS archive_count,
                COALESCE(SUM(CASE WHEN {APPLIED_QUEUE_CONDITION}
                    THEN 1 ELSE 0 END), 0) AS applied_count,
                COALESCE(SUM(CASE WHEN {INBOX_QUEUE_CONDITION}
                    THEN 1 ELSE 0 END), 0) AS inbox_count,
                COALESCE(SUM(CASE WHEN {NEW_INBOX_CONDITION}
                    THEN 1 ELSE 0 END), 0) AS new_inbox_count,
                COALESCE(SUM(CASE WHEN {REVIEW_INBOX_CONDITION}
                    THEN 1 ELSE 0 END), 0) AS review_inbox_count,
                COALESCE(SUM(CASE WHEN {INTERESTED_QUEUE_CONDITION}
                    THEN 1 ELSE 0 END), 0) AS interested_count,
                COALESCE(SUM(CASE WHEN {PREPARATION_QUEUE_CONDITION}
                    THEN 1 ELSE 0 END), 0) AS preparation_count
             FROM job_postings"
        );
        let row = sqlx::query(&sql)
            .fetch_one(self.pool)
            .await
            .map_err(db_error)?;

        Ok(JobPostingQueueCounts {
            inbox: row.try_get("inbox_count").map_err(db_error)?,
            interested: row.try_get("interested_count").map_err(db_error)?,
            preparation: row.try_get("preparation_count").map_err(db_error)?,
            applied: row.try_get("applied_count").map_err(db_error)?,
            archive: row.try_get("archive_count").map_err(db_error)?,
            all: row.try_get("all_count").map_err(db_error)?,
            new_inbox: row.try_get("new_inbox_count").map_err(db_error)?,
            review_inbox: row.try_get("review_inbox_count").map_err(db_error)?,
        })
    }

    pub async fn get_job_posting(
        &self,
        id: i64,
        app_data_dir: impl AsRef<Path>,
        browser_runtime_dir: impl Into<std::path::PathBuf>,
    ) -> Result<JobPostingView, String> {
        let snapshot = crate::source_profile::registry::load_snapshot(app_data_dir);
        let fetcher = ReqwestProfileHttpClient::new();
        let browser = ManagedProfileBrowserClient::new(browser_runtime_dir);
        self.get_job_posting_with_clients(id, &snapshot, &fetcher, &browser)
            .await
    }

    pub(crate) async fn get_job_posting_with_clients<F, B>(
        &self,
        id: i64,
        snapshot: &SourceProfileRegistrySnapshot,
        fetcher: &F,
        browser: &B,
    ) -> Result<JobPostingView, String>
    where
        F: ProfileHttpClient + Sync + ?Sized,
        B: ProfileBrowserClient + Sync + ?Sized,
    {
        let mut posting = self.get(id).await?;
        if posting.read_state != ReadState::Read {
            sqlx::query(
                "UPDATE job_postings
                 SET read_state = 'read',
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?1",
            )
            .bind(id)
            .execute(self.pool)
            .await
            .map_err(db_error)?;
            posting = self.get(id).await?;
        }

        if let Some(text) = posting.description_text.clone() {
            return Ok(JobPostingView {
                posting,
                description_state: PostingDescriptionState::Loaded {
                    text,
                    diagnostics: Vec::new(),
                },
            });
        }

        let mut diagnostics = Vec::new();
        let mut attempted_detail_capable_source = false;

        for posting_source in ordered_posting_sources(&posting) {
            let Some(source) = snapshot.source(&posting_source.source_key) else {
                diagnostics.push(detail_source_diagnostic(
                    &posting_source,
                    "source_not_found",
                    format!(
                        "Persisted posting source `{}` was not found in the Source Profile registry snapshot",
                        posting_source.source_key
                    ),
                    "",
                    serde_json::json!({ "sourceKey": posting_source.source_key }),
                ));
                continue;
            };

            if !source.validation_state.can_compile {
                diagnostics.extend(with_posting_source_context(
                    source.validation_state.diagnostics.clone(),
                    &posting_source,
                ));
                continue;
            }

            let (execution_plan, compile_diagnostics) =
                match compile_source(&source.document, snapshot) {
                    CompileSourceOutcome::Compiled {
                        source: compiled,
                        diagnostics,
                    } if !has_error_diagnostics(&diagnostics) => {
                        (compiled.execution_plan, diagnostics)
                    }
                    CompileSourceOutcome::Compiled {
                        diagnostics: compile_diagnostics,
                        ..
                    }
                    | CompileSourceOutcome::Rejected {
                        diagnostics: compile_diagnostics,
                    } => {
                        diagnostics.extend(with_posting_source_context(
                            compile_diagnostics,
                            &posting_source,
                        ));
                        continue;
                    }
                };

            diagnostics.extend(with_posting_source_context(
                compile_diagnostics,
                &posting_source,
            ));
            if execution_plan.detail.is_none() {
                diagnostics.push(detail_source_diagnostic(
                    &posting_source,
                    "detail_missing",
                    format!(
                        "Source `{}` compiled successfully but does not provide Detail",
                        source.document.key
                    ),
                    "/detail",
                    serde_json::json!({ "sourceKey": source.document.key }),
                ));
                continue;
            }

            attempted_detail_capable_source = true;
            let occurrence = posting_occurrence(&posting, &posting_source)?;
            let result = execute_detail(
                &execution_plan,
                &source.document.source_config,
                &occurrence,
                RequestedDetailFields::description_text(),
                fetcher,
                browser,
                RuntimeExecutionContext::uncancellable(),
            )
            .await;
            let result_diagnostics =
                with_posting_source_context(result.diagnostics, &posting_source);

            if matches!(
                result.report.as_ref().map(|report| &report.completion),
                Some(PhaseCompletion::Accepted)
            ) {
                let Some(description_text) = result.patch.description_text else {
                    diagnostics.extend(result_diagnostics);
                    diagnostics.push(detail_source_diagnostic(
                        &posting_source,
                        "description_empty",
                        "Accepted Detail response did not provide the requested descriptionText",
                        "/detail/fields/descriptionText",
                        serde_json::json!({}),
                    ));
                    continue;
                };
                diagnostics.extend(result_diagnostics);
                sqlx::query(
                    "UPDATE job_postings
                     SET description_text = ?1,
                         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                     WHERE id = ?2",
                )
                .bind(&description_text)
                .bind(id)
                .execute(self.pool)
                .await
                .map_err(db_error)?;
                let posting = self.get(id).await?;
                return Ok(JobPostingView {
                    posting,
                    description_state: PostingDescriptionState::Loaded {
                        text: description_text,
                        diagnostics,
                    },
                });
            }

            diagnostics.extend(result_diagnostics);
        }

        if attempted_detail_capable_source {
            Ok(JobPostingView {
                posting,
                description_state: PostingDescriptionState::Failed {
                    message: diagnostic_summary(
                        &diagnostics,
                        "description loading failed for all detail-capable persisted posting sources",
                    ),
                    diagnostics,
                },
            })
        } else {
            if diagnostics.is_empty() {
                diagnostics.push(Diagnostic {
                    category: DiagnosticCategory::SourceValidation,
                    code: "detail_source_missing".to_string(),
                    message: format!(
                        "Job Posting {id} has no persisted posting source that can provide compiled Detail"
                    ),
                    severity: DiagnosticSeverity::Error,
                    path: "".to_string(),
                    strategy_key: None,
                    details: Some(serde_json::json!({ "postingId": id })),
                });
            }
            Ok(JobPostingView {
                posting,
                description_state: PostingDescriptionState::Unsupported {
                    message: diagnostic_summary(
                        &diagnostics,
                        "job posting has no persisted posting source that can provide compiled Detail",
                    ),
                    diagnostics,
                },
            })
        }
    }

    pub async fn update_state(
        &self,
        id: i64,
        input: UpdateJobPostingStateInput,
    ) -> Result<JobPosting, String> {
        if input.read_state.is_none()
            && input.interest_state.is_none()
            && input.preparation_state.is_none()
            && input.application_state.is_none()
        {
            return Err("no state fields supplied".to_string());
        }

        let current = self.get(id).await?;
        let read_state = input.read_state.unwrap_or(current.read_state);
        let interest_state = input.interest_state.unwrap_or(current.interest_state);
        let preparation_state = input.preparation_state.unwrap_or(current.preparation_state);
        let application_state = input.application_state.unwrap_or(current.application_state);

        sqlx::query(
            "UPDATE job_postings
             SET read_state = ?1,
                 interest_state = ?2,
                 preparation_state = ?3,
                 application_state = ?4,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?5",
        )
        .bind(read_state.as_str())
        .bind(interest_state.as_str())
        .bind(preparation_state.as_str())
        .bind(application_state.as_str())
        .bind(id)
        .execute(self.pool)
        .await
        .map_err(db_error)?;

        self.get(id).await
    }

    async fn get(&self, id: i64) -> Result<JobPosting, String> {
        let row = sqlx::query(
            "SELECT id, title, company, locations_json, description_text, primary_source_id,
                    read_state, interest_state, preparation_state, application_state,
                    first_seen_at, last_seen_at, created_at, updated_at
             FROM job_postings
             WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await
        .map_err(db_error)?;

        row.map(|row| self.posting_from_row(row))
            .ok_or_else(|| format!("job posting {id} not found"))?
            .await
    }

    async fn posting_from_row(&self, row: SqliteRow) -> Result<JobPosting, String> {
        let posting_id = row.try_get::<i64, _>("id").map_err(db_error)?;
        let primary_source_id = row
            .try_get::<Option<i64>, _>("primary_source_id")
            .map_err(db_error)?;
        let sources = self.sources_for_posting(posting_id).await?;
        let primary_source =
            primary_source_id.and_then(|id| sources.iter().find(|source| source.id == id).cloned());
        let read_state = row.try_get::<String, _>("read_state").map_err(db_error)?;
        let interest_state = row
            .try_get::<String, _>("interest_state")
            .map_err(db_error)?;
        let preparation_state = row
            .try_get::<String, _>("preparation_state")
            .map_err(db_error)?;
        let application_state = row
            .try_get::<String, _>("application_state")
            .map_err(db_error)?;

        Ok(JobPosting {
            id: posting_id,
            title: row.try_get("title").map_err(db_error)?,
            company: row.try_get("company").map_err(db_error)?,
            locations: locations_from_json(
                posting_id,
                &row.try_get::<String, _>("locations_json")
                    .map_err(db_error)?,
            )?,
            description_text: row.try_get("description_text").map_err(db_error)?,
            read_state: ReadState::try_from(read_state.as_str())?,
            interest_state: InterestState::try_from(interest_state.as_str())?,
            preparation_state: PreparationState::try_from(preparation_state.as_str())?,
            application_state: ApplicationState::try_from(application_state.as_str())?,
            first_seen_at: row.try_get("first_seen_at").map_err(db_error)?,
            last_seen_at: row.try_get("last_seen_at").map_err(db_error)?,
            created_at: row.try_get("created_at").map_err(db_error)?,
            updated_at: row.try_get("updated_at").map_err(db_error)?,
            primary_source,
            sources,
        })
    }

    async fn sources_for_posting(&self, posting_id: i64) -> Result<Vec<JobPostingSource>, String> {
        let rows = sqlx::query(
            "SELECT id, source_key, source_name_snapshot, url, posting_meta_json,
                    first_seen_at, last_seen_at
             FROM job_posting_sources
             WHERE posting_id = ?1
             ORDER BY id",
        )
        .bind(posting_id)
        .fetch_all(self.pool)
        .await
        .map_err(db_error)?;

        rows.into_iter().map(source_from_row).collect()
    }
}

fn queue_condition(queue_id: JobPostingQueueId) -> Option<&'static str> {
    match queue_id {
        JobPostingQueueId::All => None,
        JobPostingQueueId::Archive => Some(ARCHIVE_QUEUE_CONDITION),
        JobPostingQueueId::Applied => Some(APPLIED_QUEUE_CONDITION),
        JobPostingQueueId::Inbox => Some(INBOX_QUEUE_CONDITION),
        JobPostingQueueId::Interested => Some(INTERESTED_QUEUE_CONDITION),
        JobPostingQueueId::Preparation => Some(PREPARATION_QUEUE_CONDITION),
    }
}

fn ordered_posting_sources(posting: &JobPosting) -> Vec<JobPostingSource> {
    let mut sources = Vec::new();
    if let Some(primary_source) = &posting.primary_source {
        sources.push(primary_source.clone());
    }

    for source in &posting.sources {
        if posting
            .primary_source
            .as_ref()
            .is_some_and(|primary_source| primary_source.id == source.id)
        {
            continue;
        }
        sources.push(source.clone());
    }

    sources
}

fn posting_occurrence(
    posting: &JobPosting,
    posting_source: &JobPostingSource,
) -> Result<PostingOccurrence, String> {
    let (reference, identity) = crate::profile_dsl::occurrence::validate_posting_reference(
        &posting_source.source_key,
        &posting_source.url,
        None,
    )
    .map_err(|_| "persisted posting source has an invalid provider URL".to_string())?;
    Ok(PostingOccurrence {
        identity,
        reference,
        provider_values: crate::profile_dsl::occurrence::ProviderValues {
            title: Some(posting.title.clone()),
            company: Some(posting.company.clone()),
            locations: posting.locations.clone(),
            description_text: posting.description_text.clone(),
        },
        hints: Default::default(),
        posting_meta: posting_source.posting_meta.clone(),
    })
}

fn has_error_diagnostics(diagnostics: &Diagnostics) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
}

fn with_posting_source_context(
    diagnostics: Diagnostics,
    posting_source: &JobPostingSource,
) -> Diagnostics {
    diagnostics
        .into_iter()
        .map(|diagnostic| with_posting_source_context_one(diagnostic, posting_source))
        .collect()
}

fn with_posting_source_context_one(
    mut diagnostic: Diagnostic,
    posting_source: &JobPostingSource,
) -> Diagnostic {
    let original_details = diagnostic.details.take();
    let mut details = original_details
        .as_ref()
        .and_then(|details| details.as_object().cloned())
        .unwrap_or_default();
    if details.is_empty() {
        if let Some(original_details) = original_details.filter(|details| !details.is_object()) {
            details.insert("originalDetails".to_string(), original_details);
        }
    }
    details.insert(
        "postingSourceId".to_string(),
        serde_json::json!(posting_source.id),
    );
    details.insert(
        "postingSourceKey".to_string(),
        serde_json::json!(posting_source.source_key),
    );
    details.insert(
        "postingUrl".to_string(),
        serde_json::json!(posting_source.url),
    );
    diagnostic.details = Some(serde_json::Value::Object(details));
    diagnostic
}

fn detail_source_diagnostic(
    posting_source: &JobPostingSource,
    code: impl Into<String>,
    message: impl Into<String>,
    path: impl Into<String>,
    details: serde_json::Value,
) -> Diagnostic {
    with_posting_source_context_one(
        Diagnostic {
            category: DiagnosticCategory::SourceValidation,
            code: code.into(),
            message: message.into(),
            severity: DiagnosticSeverity::Error,
            path: path.into(),
            strategy_key: None,
            details: Some(details),
        },
        posting_source,
    )
}

fn diagnostic_summary(diagnostics: &Diagnostics, fallback: &str) -> String {
    diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .or_else(|| diagnostics.first())
        .map(|diagnostic| diagnostic.message.clone())
        .unwrap_or_else(|| fallback.to_string())
}

fn source_from_row(row: SqliteRow) -> Result<JobPostingSource, String> {
    let id = row.try_get("id").map_err(db_error)?;
    let posting_meta_json = row
        .try_get::<String, _>("posting_meta_json")
        .map_err(db_error)?;
    let posting_meta = serde_json::from_str(&posting_meta_json).map_err(|error| {
        format!("invalid posting_meta_json for job posting source {id}: {error}")
    })?;

    Ok(JobPostingSource {
        id,
        source_key: row.try_get("source_key").map_err(db_error)?,
        source_name_snapshot: row.try_get("source_name_snapshot").map_err(db_error)?,
        url: row.try_get("url").map_err(db_error)?,
        posting_meta,
        first_seen_at: row.try_get("first_seen_at").map_err(db_error)?,
        last_seen_at: row.try_get("last_seen_at").map_err(db_error)?,
    })
}

fn locations_from_json(posting_id: i64, json: &str) -> Result<Vec<String>, String> {
    serde_json::from_str(json)
        .map_err(|error| format!("invalid locations_json for job posting {posting_id}: {error}"))
}

fn db_error(error: sqlx::Error) -> String {
    error.to_string()
}
