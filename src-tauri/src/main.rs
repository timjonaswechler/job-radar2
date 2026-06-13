// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::ffi::OsStr;

fn main() {
    let mut args = std::env::args_os().collect::<Vec<_>>();
    let dev_smoke_requested = args
        .get(1)
        .is_some_and(|arg| arg == OsStr::new("dev-search-run-smoke"));

    if dev_smoke_requested {
        let smoke_args = args.drain(2..).collect::<Vec<_>>();
        if let Err(error) = job_radar_lib::run_dev_search_run_smoke_cli(smoke_args) {
            eprintln!("dev-search-run-smoke failed: {error}");
            std::process::exit(1);
        }
        return;
    }

    job_radar_lib::run()
}
