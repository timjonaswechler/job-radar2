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
