use reqwest::Url;
use serde_json::Value;

use crate::{search::run::SourceExecutionError, simple_json_path::resolve_simple_json_path};

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PageCountPagination {
    pub(super) page_param: String,
    pub(super) size_param: String,
    pub(super) size: u64,
    pub(super) first_page: u64,
    pub(super) total_path: String,
}

pub(super) fn parse_page_count_pagination(
    value: &Value,
    path: &str,
) -> Result<PageCountPagination, SourceExecutionError> {
    let object = value
        .as_object()
        .ok_or_else(|| SourceExecutionError::Failed(format!("{path} must be a JSON object")))?;
    let pagination_type = required_string(object, "type", &format!("{path}.type"))?;
    if pagination_type != "page_count" {
        return Err(SourceExecutionError::Failed(format!(
            "{path}.type `{pagination_type}` is not supported by this executor slice"
        )));
    }

    let page_param = required_string(object, "pageParam", &format!("{path}.pageParam"))?;
    let size_param = required_string(object, "sizeParam", &format!("{path}.sizeParam"))?;
    let size = required_u64(object, "size", &format!("{path}.size"))?;
    if size == 0 {
        return Err(SourceExecutionError::Failed(format!(
            "{path}.size must be greater than 0"
        )));
    }
    let first_page = optional_u64(object, "firstPage", &format!("{path}.firstPage"))?.unwrap_or(1);
    let total_path = required_string(object, "totalPath", &format!("{path}.totalPath"))?;

    Ok(PageCountPagination {
        page_param: page_param.to_string(),
        size_param: size_param.to_string(),
        size,
        first_page,
        total_path: total_path.to_string(),
    })
}

pub(super) fn page_count_pagination_url(
    base_url: &Url,
    pagination: &PageCountPagination,
    page: u64,
) -> Url {
    let overrides = [
        (pagination.size_param.as_str(), pagination.size.to_string()),
        (pagination.page_param.as_str(), page.to_string()),
    ];
    query_param_override_url(base_url, &overrides)
}

pub(super) fn query_param_override_url(base_url: &Url, overrides: &[(&str, String)]) -> Url {
    let mut url = base_url.clone();
    let existing_pairs = url
        .query_pairs()
        .filter(|(key, _value)| {
            !overrides
                .iter()
                .any(|(override_key, _)| key == *override_key)
        })
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();

    url.set_query(None);
    {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in existing_pairs {
            pairs.append_pair(&key, &value);
        }
        for (key, value) in overrides {
            pairs.append_pair(key, value);
        }
    }
    url
}

pub(super) fn resolve_json_u64(
    root: &Value,
    json_path: &str,
    path: &str,
) -> Result<u64, SourceExecutionError> {
    let selected = resolve_simple_json_path(root, json_path)
        .map_err(|error| simple_json_path_execution_error(path, error))?
        .ok_or_else(|| {
            SourceExecutionError::Failed(format!(
                "{path} `{json_path}` must resolve to a non-negative integer, but no value was found"
            ))
        })?;
    selected.as_u64().ok_or_else(|| {
        SourceExecutionError::Failed(format!(
            "{path} `{json_path}` must resolve to a non-negative integer, but resolved to {}",
            json_type_label(selected)
        ))
    })
}
