use serde::Serialize;
use std::{ffi::OsString, path::PathBuf};

use crate::{
    app::paths::AppPaths,
    search::run::{default_search_run_result_path, DefaultSourceExecutor},
};

use super::{
    constants::{SMOKE_APP_DATA_DIR_ENV, SMOKE_COMMAND},
    run_schott_smoke,
    schott_source::ensure_schott_smoke_source,
    SearchRunSmokeSummary,
};

struct SmokeCliOptions {
    app_data_dir: Option<PathBuf>,
    ensure_schott_source: bool,
    help: bool,
}

pub fn run_dev_search_run_smoke_cli<I>(args: I) -> Result<(), String>
where
    I: IntoIterator<Item = OsString>,
{
    let options = parse_smoke_cli_args(args)?;
    if options.help {
        println!("{}", smoke_cli_help());
        return Ok(());
    }

    let app_data_dir = options
        .app_data_dir
        .or_else(|| std::env::var_os(SMOKE_APP_DATA_DIR_ENV).map(PathBuf::from))
        .ok_or_else(|| {
            format!(
                "missing --app-data-dir <path> or {SMOKE_APP_DATA_DIR_ENV}; see docs/dev-search-run-smoke.md"
            )
        })?;

    tauri::async_runtime::block_on(async move {
        let paths = AppPaths::from_app_data_dir(app_data_dir).map_err(|error| error.to_string())?;
        let state = crate::app::state::AppState::new(paths)
            .await
            .map_err(|error| error.to_string())?;

        if options.ensure_schott_source {
            ensure_schott_smoke_source(&state.paths.app_data_dir)?;
        }

        let result_path = default_search_run_result_path();
        let source_executor = DefaultSourceExecutor::new(state.paths.browser_runtime_dir.clone());
        let summary = run_schott_smoke(
            &state.db,
            &state.running_search_runs,
            &source_executor,
            result_path,
            state.paths.app_data_dir.clone(),
        )
        .await?;

        print_smoke_summary(&summary);
        Ok(())
    })
}

fn parse_smoke_cli_args<I>(args: I) -> Result<SmokeCliOptions, String>
where
    I: IntoIterator<Item = OsString>,
{
    let mut app_data_dir = None;
    let mut ensure_schott_source = false;
    let mut help = false;
    let mut args = args.into_iter().peekable();

    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            help = true;
            continue;
        }
        if arg == "--ensure-schott-source" {
            ensure_schott_source = true;
            continue;
        }
        if arg == "--app-data-dir" {
            let value = args
                .next()
                .ok_or_else(|| "--app-data-dir requires a path".to_string())?;
            app_data_dir = Some(PathBuf::from(value));
            continue;
        }

        let arg_string = arg.to_string_lossy();
        if let Some(value) = arg_string.strip_prefix("--app-data-dir=") {
            if value.is_empty() {
                return Err("--app-data-dir requires a path".to_string());
            }
            app_data_dir = Some(PathBuf::from(value));
            continue;
        }

        return Err(format!(
            "unknown {SMOKE_COMMAND} argument `{}`; use --help",
            arg_string
        ));
    }

    Ok(SmokeCliOptions {
        app_data_dir,
        ensure_schott_source,
        help,
    })
}

fn smoke_cli_help() -> &'static str {
    "Usage: cargo run --manifest-path src-tauri/Cargo.toml -- dev-search-run-smoke --app-data-dir <path> [--ensure-schott-source]\n\nRuns the network-dependent SCHOTT development smoke Search Run and overwrites search-run-result.json in the repository root. Use JOB_RADAR_SMOKE_APP_DATA_DIR instead of --app-data-dir if preferred."
}

fn print_smoke_summary(summary: &SearchRunSmokeSummary) {
    println!("Search-run smoke completed");
    println!("Search request ID: {}", summary.search_request_id);
    println!(
        "Search request: {}",
        if summary.search_request_created {
            "created"
        } else {
            "reused"
        }
    );
    println!("Result path: {}", summary.result_path);
    println!(
        "Overall status: {}",
        serialized_label(&summary.result.status)
    );
    println!("Postings: {}", summary.result.postings.len());
    println!("Source runs:");
    for source_run in &summary.result.source_runs {
        let error = source_run.error.as_deref().unwrap_or("-");
        println!(
            "- {}: status={}, candidates={}, matched={}, error={}",
            source_run.source_key,
            serialized_label(&source_run.status),
            source_run.candidate_count,
            source_run.matched_count,
            error
        );
    }
}

pub(super) fn serialized_label<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value)
        .map(|value| value.trim_matches('"').to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}
