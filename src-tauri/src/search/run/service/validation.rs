use crate::search::request::SearchRequest;

pub(super) fn validate_executable_search_request(
    search_request: &SearchRequest,
) -> Result<(), String> {
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
    if search_request.source_keys.is_empty() {
        return Err(format!(
            "search request {} cannot run without selected sources",
            search_request.id
        ));
    }
    Ok(())
}
