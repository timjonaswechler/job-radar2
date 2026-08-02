use crate::search::request::{SearchRequest, SearchRequestStatus};

pub(super) fn validate_executable_search_request(
    search_request: &SearchRequest,
) -> Result<(), String> {
    if search_request.status != SearchRequestStatus::Active {
        return Err(format!(
            "search request {} cannot run unless status is active",
            search_request.id
        ));
    }

    if let Some(issue) = search_request.validation_issues.first() {
        return Err(format!(
            "search request {} cannot run: {} at {}",
            search_request.id, issue.code, issue.path
        ));
    }
    Ok(())
}
