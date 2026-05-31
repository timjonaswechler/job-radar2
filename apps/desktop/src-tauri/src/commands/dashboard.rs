use serde::Serialize;
use tauri::State;

use crate::app_state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStats {
    scanned_sources_today: i64,
    listed_postings: i64,
    saved_applications: MetricWithDelta,
    created_applications: MetricWithDelta,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricWithDelta {
    total: i64,
    this_week_delta: i64,
}

#[tauri::command]
pub async fn get_dashboard_stats(state: State<'_, AppState>) -> Result<DashboardStats, String> {
    let listed_postings = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM postings")
        .fetch_one(&state.db)
        .await
        .map_err(|error| error.to_string())?;

    let applications_total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM applications")
        .fetch_one(&state.db)
        .await
        .map_err(|error| error.to_string())?;

    let applications_this_week = sqlx::query_scalar::<_, i64>(
        r#"
           SELECT COUNT(*)
           FROM applications
           WHERE date(created_at) >= date(
             'now',
             '-' || ((cast(strftime('%w', 'now') as integer) + 6) % 7) || ' days'
           )
           "#,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|error| error.to_string())?;

    let scanned_sources_today = sqlx::query_scalar::<_, i64>(
        r#"
           SELECT COUNT(DISTINCT findings.job_source_id)
           FROM findings
           JOIN search_runs ON search_runs.id = findings.search_run_id
           WHERE date(search_runs.started_at) = date('now')
           "#,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|error| error.to_string())?;

    Ok(DashboardStats {
        scanned_sources_today,
        listed_postings,
        saved_applications: MetricWithDelta {
            total: applications_total,
            this_week_delta: applications_this_week,
        },
        created_applications: MetricWithDelta {
            total: applications_total,
            this_week_delta: applications_this_week,
        },
    })
}
