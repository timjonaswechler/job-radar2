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

    pub(super) fn is_running(&self, search_request_id: i64) -> Result<bool, String> {
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
