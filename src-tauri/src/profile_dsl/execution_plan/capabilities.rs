use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::profile_dsl::documents::fetch::{BrowserInteraction, BrowserWait};
use crate::profile_dsl::documents::{
    Fetch, HttpMethod, Pagination, PaginationParameterLocation, Parse, RequestBody, Select,
};
use crate::profile_dsl::template::{
    compile_template, descriptor_for_placement, json_pointer_segment, CompiledTemplate,
    TemplateAdmissionKeys, TemplateDescriptor, TemplatePlacement,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ExecutionPlanFetch {
    Http {
        #[serde(skip_serializing_if = "Option::is_none")]
        method: Option<HttpMethod>,
        url: CompiledTemplate,
        #[serde(skip_serializing_if = "Option::is_none")]
        headers: Option<BTreeMap<String, CompiledTemplate>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        body: Option<ExecutionPlanRequestBody>,
        #[serde(rename = "timeoutMs")]
        timeout_ms: u64,
    },
    Browser {
        url: CompiledTemplate,
        #[serde(rename = "timeoutMs")]
        timeout_ms: u64,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        waits: Vec<ExecutionPlanBrowserWait>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        interactions: Vec<ExecutionPlanBrowserInteraction>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionPlanBrowserWait {
    Selector {
        #[serde(skip_serializing_if = "Option::is_none")]
        selector: Option<String>,
        #[serde(rename = "timeoutMs")]
        timeout_ms: u64,
    },
    NetworkIdle {
        #[serde(skip_serializing_if = "Option::is_none")]
        selector: Option<String>,
        #[serde(rename = "timeoutMs")]
        timeout_ms: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionPlanBrowserInteraction {
    ClickIfVisible {
        selector: String,
        #[serde(rename = "maxCount")]
        max_count: u64,
        #[serde(rename = "waitAfterMs", skip_serializing_if = "Option::is_none")]
        wait_after_ms: Option<u64>,
    },
    ClickUntilGone {
        selector: String,
        #[serde(rename = "maxCount")]
        max_count: u64,
        #[serde(rename = "waitAfterMs", skip_serializing_if = "Option::is_none")]
        wait_after_ms: Option<u64>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionPlanPagination {
    Page {
        #[serde(rename = "pageParam")]
        page_param: String,
        #[serde(rename = "parameterLocation")]
        parameter_location: PaginationParameterLocation,
        #[serde(rename = "firstPage", skip_serializing_if = "Option::is_none")]
        first_page: Option<u64>,
        #[serde(rename = "pageSizeParam", skip_serializing_if = "Option::is_none")]
        page_size_param: Option<String>,
        #[serde(rename = "pageSize", skip_serializing_if = "Option::is_none")]
        page_size: Option<u64>,
        #[serde(rename = "totalPath", skip_serializing_if = "Option::is_none")]
        total_path: Option<String>,
        limits: ExecutionPlanPaginationLimits,
    },
    OffsetLimit {
        #[serde(rename = "offsetParam")]
        offset_param: String,
        #[serde(rename = "limitParam")]
        limit_param: String,
        #[serde(rename = "parameterLocation")]
        parameter_location: PaginationParameterLocation,
        #[serde(rename = "startOffset", skip_serializing_if = "Option::is_none")]
        start_offset: Option<u64>,
        limit: u64,
        #[serde(rename = "totalPath", skip_serializing_if = "Option::is_none")]
        total_path: Option<String>,
        limits: ExecutionPlanPaginationLimits,
    },
    Cursor {
        #[serde(rename = "cursorParam")]
        cursor_param: String,
        #[serde(rename = "parameterLocation")]
        parameter_location: PaginationParameterLocation,
        #[serde(rename = "nextCursorPath")]
        next_cursor_path: String,
        limits: ExecutionPlanPaginationLimits,
    },
    Sitemap {
        #[serde(
            rename = "childSitemapSelector",
            skip_serializing_if = "Option::is_none"
        )]
        child_sitemap_selector: Option<Select>,
        #[serde(rename = "postingUrlSelector", skip_serializing_if = "Option::is_none")]
        posting_url_selector: Option<Select>,
        limits: ExecutionPlanPaginationLimits,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanPaginationLimits {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_requests: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_items: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionPlanRequestBody {
    Json {
        value: BTreeMap<String, ExecutionPlanJsonValue>,
    },
    Text {
        value: CompiledTemplate,
    },
    Form {
        fields: BTreeMap<String, CompiledTemplate>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ExecutionPlanJsonValue {
    Template(CompiledTemplate),
    Array(Vec<ExecutionPlanJsonValue>),
    Object(BTreeMap<String, ExecutionPlanJsonValue>),
    Scalar(serde_json::Value),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExecutionPlanBuildError {
    pub path: String,
    pub message: String,
}

impl ExecutionPlanBuildError {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

pub(crate) fn compile_fetch(
    fetch: &Fetch,
    path: &str,
    placement: TemplatePlacement,
    keys: &TemplateAdmissionKeys,
) -> Result<ExecutionPlanFetch, ExecutionPlanBuildError> {
    let descriptor = descriptor_for_placement(placement, keys);
    let (header_descriptor, body_descriptor) = match placement {
        TemplatePlacement::DiscoveryHttpUrl => (
            descriptor_for_placement(TemplatePlacement::DiscoveryHttpHeader, keys),
            descriptor_for_placement(TemplatePlacement::DiscoveryHttpBody, keys),
        ),
        TemplatePlacement::DetailHttpUrl => (
            descriptor_for_placement(TemplatePlacement::DetailHttpHeader, keys),
            descriptor_for_placement(TemplatePlacement::DetailHttpBody, keys),
        ),
        _ => (descriptor.clone(), descriptor.clone()),
    };
    match fetch {
        Fetch::Http {
            method,
            url,
            headers,
            body,
            timeout_ms,
        } => Ok(ExecutionPlanFetch::Http {
            method: *method,
            url: compile_template(url, &descriptor).map_err(|error| {
                ExecutionPlanBuildError::new(format!("{path}/url"), error.to_string())
            })?,
            headers: headers
                .as_ref()
                .map(|headers| {
                    headers
                        .iter()
                        .map(|(name, value)| {
                            Ok((
                                name.clone(),
                                compile_template(value, &header_descriptor).map_err(|error| {
                                    ExecutionPlanBuildError::new(
                                        format!("{path}/headers/{}", json_pointer_segment(name)),
                                        error.to_string(),
                                    )
                                })?,
                            ))
                        })
                        .collect::<Result<_, ExecutionPlanBuildError>>()
                })
                .transpose()?,
            body: body
                .as_ref()
                .map(|body| compile_request_body(body, &body_descriptor, &format!("{path}/body")))
                .transpose()?,
            timeout_ms: require_positive(*timeout_ms, &format!("{path}/timeoutMs"))?,
        }),
        Fetch::Browser {
            url,
            timeout_ms,
            waits,
            interactions,
        } => Ok(ExecutionPlanFetch::Browser {
            url: compile_template(url, &descriptor).map_err(|error| {
                ExecutionPlanBuildError::new(format!("{path}/url"), error.to_string())
            })?,
            timeout_ms: require_positive(*timeout_ms, &format!("{path}/timeoutMs"))?,
            waits: waits
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .enumerate()
                .map(|(index, wait)| compile_browser_wait(wait, &format!("{path}/waits/{index}")))
                .collect::<Result<Vec<_>, _>>()?,
            interactions: interactions
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .enumerate()
                .map(|(index, interaction)| {
                    compile_browser_interaction(
                        interaction,
                        &format!("{path}/interactions/{index}"),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        }),
    }
}

fn compile_browser_wait(
    wait: &BrowserWait,
    path: &str,
) -> Result<ExecutionPlanBrowserWait, ExecutionPlanBuildError> {
    match wait {
        BrowserWait::Selector {
            selector,
            timeout_ms,
        } => Ok(ExecutionPlanBrowserWait::Selector {
            selector: selector.clone(),
            timeout_ms: require_positive(*timeout_ms, &format!("{path}/timeoutMs"))?,
        }),
        BrowserWait::NetworkIdle {
            selector,
            timeout_ms,
        } => Ok(ExecutionPlanBrowserWait::NetworkIdle {
            selector: selector.clone(),
            timeout_ms: require_positive(*timeout_ms, &format!("{path}/timeoutMs"))?,
        }),
    }
}

fn compile_browser_interaction(
    interaction: &BrowserInteraction,
    path: &str,
) -> Result<ExecutionPlanBrowserInteraction, ExecutionPlanBuildError> {
    match interaction {
        BrowserInteraction::ClickIfVisible {
            selector,
            max_count,
            wait_after_ms,
        } => Ok(ExecutionPlanBrowserInteraction::ClickIfVisible {
            selector: selector.clone(),
            max_count: require_positive(*max_count, &format!("{path}/maxCount"))?,
            wait_after_ms: *wait_after_ms,
        }),
        BrowserInteraction::ClickUntilGone {
            selector,
            max_count,
            wait_after_ms,
        } => Ok(ExecutionPlanBrowserInteraction::ClickUntilGone {
            selector: selector.clone(),
            max_count: require_positive(*max_count, &format!("{path}/maxCount"))?,
            wait_after_ms: *wait_after_ms,
        }),
        BrowserInteraction::ExecuteScript { .. }
        | BrowserInteraction::Eval { .. }
        | BrowserInteraction::MutateDom { .. }
        | BrowserInteraction::LoginFlow { .. }
        | BrowserInteraction::CaptchaBypass { .. } => Err(ExecutionPlanBuildError::new(
            path,
            "prohibited browser behavior cannot be compiled into an Execution Plan",
        )),
    }
}

pub(crate) fn compile_pagination(
    pagination: &Pagination,
    path: &str,
) -> Result<ExecutionPlanPagination, ExecutionPlanBuildError> {
    match pagination {
        Pagination::Page {
            page_param,
            parameter_location,
            first_page,
            page_size_param,
            page_size,
            total_path,
            limits,
        } => Ok(ExecutionPlanPagination::Page {
            page_param: page_param.clone(),
            parameter_location: parameter_location.unwrap_or(PaginationParameterLocation::Query),
            first_page: *first_page,
            page_size_param: page_size_param.clone(),
            page_size: *page_size,
            total_path: total_path.clone(),
            limits: compile_pagination_limits(limits.as_ref(), &format!("{path}/limits"))?,
        }),
        Pagination::OffsetLimit {
            offset_param,
            limit_param,
            parameter_location,
            start_offset,
            limit,
            total_path,
            limits,
        } => Ok(ExecutionPlanPagination::OffsetLimit {
            offset_param: offset_param.clone(),
            limit_param: limit_param.clone(),
            parameter_location: parameter_location.unwrap_or(PaginationParameterLocation::Query),
            start_offset: *start_offset,
            limit: *limit,
            total_path: total_path.clone(),
            limits: compile_pagination_limits(limits.as_ref(), &format!("{path}/limits"))?,
        }),
        Pagination::Cursor {
            cursor_param,
            parameter_location,
            next_cursor_path,
            limits,
        } => Ok(ExecutionPlanPagination::Cursor {
            cursor_param: cursor_param.clone(),
            parameter_location: parameter_location.unwrap_or(PaginationParameterLocation::Query),
            next_cursor_path: next_cursor_path.clone(),
            limits: compile_pagination_limits(limits.as_ref(), &format!("{path}/limits"))?,
        }),
        Pagination::Sitemap {
            child_sitemap_selector,
            posting_url_selector,
            limits,
        } => Ok(ExecutionPlanPagination::Sitemap {
            child_sitemap_selector: child_sitemap_selector.clone(),
            posting_url_selector: posting_url_selector.clone(),
            limits: compile_pagination_limits(limits.as_ref(), &format!("{path}/limits"))?,
        }),
    }
}

fn compile_pagination_limits(
    limits: Option<&crate::profile_dsl::documents::PaginationLimits>,
    path: &str,
) -> Result<ExecutionPlanPaginationLimits, ExecutionPlanBuildError> {
    let limits = limits.ok_or_else(|| {
        ExecutionPlanBuildError::new(path, "pagination limits are required in an Execution Plan")
    })?;

    let compiled = ExecutionPlanPaginationLimits {
        max_requests: limits.max_requests,
        max_items: limits.max_items,
        max_depth: limits.max_depth,
    };

    if compiled.max_requests.filter(|value| *value > 0).is_none()
        && compiled.max_items.filter(|value| *value > 0).is_none()
        && compiled.max_depth.is_none()
    {
        return Err(ExecutionPlanBuildError::new(
            path,
            "pagination limits must include at least one stop rule",
        ));
    }

    Ok(compiled)
}

pub(crate) fn clone_parse(parse: &Parse) -> Parse {
    parse.clone()
}

pub(crate) fn clone_select(select: &Select) -> Select {
    select.clone()
}

fn compile_request_body(
    body: &RequestBody,
    descriptor: &TemplateDescriptor,
    path: &str,
) -> Result<ExecutionPlanRequestBody, ExecutionPlanBuildError> {
    Ok(match body {
        RequestBody::Text { value } => ExecutionPlanRequestBody::Text {
            value: compile_template(value, descriptor)
                .map_err(|error| ExecutionPlanBuildError::new(path, error.to_string()))?,
        },
        RequestBody::Form { fields } => ExecutionPlanRequestBody::Form {
            fields: fields
                .iter()
                .map(|(key, value)| {
                    Ok((
                        key.clone(),
                        compile_template(value, descriptor).map_err(|error| {
                            ExecutionPlanBuildError::new(
                                format!("{path}/{}", json_pointer_segment(key)),
                                error.to_string(),
                            )
                        })?,
                    ))
                })
                .collect::<Result<_, ExecutionPlanBuildError>>()?,
        },
        RequestBody::Json { value } => ExecutionPlanRequestBody::Json {
            value: value
                .iter()
                .map(|(key, value)| {
                    Ok((
                        key.clone(),
                        compile_json_value(
                            value,
                            descriptor,
                            &format!("{path}/{}", json_pointer_segment(key)),
                        )?,
                    ))
                })
                .collect::<Result<_, ExecutionPlanBuildError>>()?,
        },
    })
}

fn compile_json_value(
    value: &serde_json::Value,
    descriptor: &TemplateDescriptor,
    path: &str,
) -> Result<ExecutionPlanJsonValue, ExecutionPlanBuildError> {
    Ok(match value {
        serde_json::Value::String(value) => ExecutionPlanJsonValue::Template(
            compile_template(value, descriptor)
                .map_err(|error| ExecutionPlanBuildError::new(path, error.to_string()))?,
        ),
        serde_json::Value::Array(values) => ExecutionPlanJsonValue::Array(
            values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    compile_json_value(value, descriptor, &format!("{path}/{index}"))
                })
                .collect::<Result<_, _>>()?,
        ),
        serde_json::Value::Object(values) => ExecutionPlanJsonValue::Object(
            values
                .iter()
                .map(|(key, value)| {
                    Ok((
                        key.clone(),
                        compile_json_value(
                            value,
                            descriptor,
                            &format!("{path}/{}", json_pointer_segment(key)),
                        )?,
                    ))
                })
                .collect::<Result<_, ExecutionPlanBuildError>>()?,
        ),
        _ => ExecutionPlanJsonValue::Scalar(value.clone()),
    })
}

fn require_positive(value: Option<u64>, path: &str) -> Result<u64, ExecutionPlanBuildError> {
    value
        .filter(|value| *value > 0)
        .ok_or_else(|| ExecutionPlanBuildError::new(path, "positive bound is required"))
}
