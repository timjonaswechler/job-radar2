use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStats {
    new_postings_this_week: PeriodMetric,
    interesting_postings: InterestingPostingsMetric,
    applications_sent_this_week: PeriodMetric,
    due_follow_ups: DueFollowUpsMetric,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodMetric {
    count: i64,
    previous_week_count: i64,
    delta: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterestingPostingsMetric {
    total: i64,
    new_this_week: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DueFollowUpsMetric {
    total: i64,
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DashboardPosting {
    id: String,
    title: String,
    company: String,
    primary_location: Option<String>,
    region: Option<String>,
    work_model: String,
    status: String,
    description_excerpt: String,
    created_at: String,
    updated_at: String,
    finding_count: i64,
    last_found_at: Option<String>,
    latest_result_url: Option<String>,
    latest_source_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStatsRange {
    current_week_start: String,
    current_week_end: String,
    previous_week_start: String,
    previous_week_end: String,
    current_week_start_date: String,
    current_week_end_date: String,
    previous_week_start_date: String,
    previous_week_end_date: String,
    due_until: String,
}

#[tauri::command]
pub async fn get_dashboard_stats(
    state: State<'_, AppState>,
    range: DashboardStatsRange,
) -> Result<DashboardStats, String> {
    let new_postings_this_week = sqlx::query_scalar::<_, i64>(
        r#"
           SELECT COUNT(*)
           FROM postings
           WHERE created_at >= ?1
             AND created_at < ?2
           "#,
    )
    .bind(&range.current_week_start)
    .bind(&range.current_week_end)
    .fetch_one(&state.db)
    .await
    .map_err(|error| error.to_string())?;

    let new_postings_previous_week = sqlx::query_scalar::<_, i64>(
        r#"
           SELECT COUNT(*)
           FROM postings
           WHERE created_at >= ?1
             AND created_at < ?2
           "#,
    )
    .bind(&range.previous_week_start)
    .bind(&range.previous_week_end)
    .fetch_one(&state.db)
    .await
    .map_err(|error| error.to_string())?;

    let interesting_postings_total = sqlx::query_scalar::<_, i64>(
        r#"
           SELECT COUNT(*)
           FROM postings
           WHERE status IN ('interesting', 'review_later')
           "#,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|error| error.to_string())?;

    let interesting_postings_new_this_week = sqlx::query_scalar::<_, i64>(
        r#"
           SELECT COUNT(*)
           FROM postings
           WHERE status IN ('interesting', 'review_later')
             AND created_at >= ?1
             AND created_at < ?2
           "#,
    )
    .bind(&range.current_week_start)
    .bind(&range.current_week_end)
    .fetch_one(&state.db)
    .await
    .map_err(|error| error.to_string())?;

    let applications_sent_this_week = sqlx::query_scalar::<_, i64>(
        r#"
           SELECT COUNT(*)
           FROM applications
           WHERE applied_on IS NOT NULL
             AND date(applied_on) >= date(?1)
             AND date(applied_on) < date(?2)
           "#,
    )
    .bind(&range.current_week_start_date)
    .bind(&range.current_week_end_date)
    .fetch_one(&state.db)
    .await
    .map_err(|error| error.to_string())?;

    let applications_sent_previous_week = sqlx::query_scalar::<_, i64>(
        r#"
           SELECT COUNT(*)
           FROM applications
           WHERE applied_on IS NOT NULL
             AND date(applied_on) >= date(?1)
             AND date(applied_on) < date(?2)
           "#,
    )
    .bind(&range.previous_week_start_date)
    .bind(&range.previous_week_end_date)
    .fetch_one(&state.db)
    .await
    .map_err(|error| error.to_string())?;

    let due_follow_ups = sqlx::query_scalar::<_, i64>(
        r#"
           SELECT COUNT(*)
           FROM reminders
           WHERE done_at IS NULL
             AND due_at <= ?1
           "#,
    )
    .bind(&range.due_until)
    .fetch_one(&state.db)
    .await
    .map_err(|error| error.to_string())?;

    Ok(DashboardStats {
        new_postings_this_week: PeriodMetric {
            count: new_postings_this_week,
            previous_week_count: new_postings_previous_week,
            delta: new_postings_this_week - new_postings_previous_week,
        },
        interesting_postings: InterestingPostingsMetric {
            total: interesting_postings_total,
            new_this_week: interesting_postings_new_this_week,
        },
        applications_sent_this_week: PeriodMetric {
            count: applications_sent_this_week,
            previous_week_count: applications_sent_previous_week,
            delta: applications_sent_this_week - applications_sent_previous_week,
        },
        due_follow_ups: DueFollowUpsMetric {
            total: due_follow_ups,
        },
    })
}

#[tauri::command]
pub async fn get_dashboard_postings(
    state: State<'_, AppState>,
) -> Result<Vec<DashboardPosting>, String> {
    sqlx::query_as::<_, DashboardPosting>(
        r#"
        WITH posting_descriptions AS (
          SELECT
            p.id,
            p.title,
            p.company,
            p.primary_location,
            p.region,
            p.work_model,
            p.status,
            p.created_at,
            p.updated_at,
            trim(replace(replace(p.description_plain_text, char(13), ' '), char(10), ' ')) AS normalized_description
          FROM postings p
        )
        SELECT
          p.id,
          p.title,
          p.company,
          p.primary_location,
          p.region,
          p.work_model,
          p.status,
          CASE
            WHEN length(p.normalized_description) > 360
              THEN substr(p.normalized_description, 1, 360) || '…'
            ELSE p.normalized_description
          END AS description_excerpt,
          p.created_at,
          p.updated_at,
          COUNT(f.id) AS finding_count,
          MAX(f.found_at) AS last_found_at,
          (
            SELECT f2.result_url
            FROM findings f2
            WHERE f2.posting_id = p.id
            ORDER BY f2.found_at DESC, f2.created_at DESC
            LIMIT 1
          ) AS latest_result_url,
          (
            SELECT js.name
            FROM findings f3
            JOIN job_sources js ON js.id = f3.job_source_id
            WHERE f3.posting_id = p.id
            ORDER BY f3.found_at DESC, f3.created_at DESC
            LIMIT 1
          ) AS latest_source_name
        FROM posting_descriptions p
        LEFT JOIN findings f ON f.posting_id = p.id
        GROUP BY
          p.id,
          p.title,
          p.company,
          p.primary_location,
          p.region,
          p.work_model,
          p.status,
          p.normalized_description,
          p.created_at,
          p.updated_at
        ORDER BY COALESCE(MAX(f.found_at), p.created_at) DESC, p.created_at DESC
        LIMIT 100
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|error| error.to_string())
}
