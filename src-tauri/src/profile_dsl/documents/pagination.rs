use serde::{Deserialize, Serialize};

use crate::profile_dsl::documents::select::Select;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Pagination {
    Page {
        #[serde(rename = "pageParam")]
        page_param: String,
        #[serde(rename = "firstPage", skip_serializing_if = "Option::is_none")]
        first_page: Option<u64>,
        #[serde(rename = "pageSizeParam", skip_serializing_if = "Option::is_none")]
        page_size_param: Option<String>,
        #[serde(rename = "pageSize", skip_serializing_if = "Option::is_none")]
        page_size: Option<u64>,
        #[serde(rename = "totalPath", skip_serializing_if = "Option::is_none")]
        total_path: Option<String>,
        limits: PaginationLimits,
    },
    OffsetLimit {
        #[serde(rename = "offsetParam")]
        offset_param: String,
        #[serde(rename = "limitParam")]
        limit_param: String,
        #[serde(rename = "startOffset", skip_serializing_if = "Option::is_none")]
        start_offset: Option<u64>,
        limit: u64,
        #[serde(rename = "totalPath", skip_serializing_if = "Option::is_none")]
        total_path: Option<String>,
        limits: PaginationLimits,
    },
    Cursor {
        #[serde(rename = "cursorParam")]
        cursor_param: String,
        #[serde(rename = "nextCursorPath")]
        next_cursor_path: String,
        limits: PaginationLimits,
    },
    Sitemap {
        #[serde(
            rename = "childSitemapSelector",
            skip_serializing_if = "Option::is_none"
        )]
        child_sitemap_selector: Option<Select>,
        #[serde(rename = "postingUrlSelector", skip_serializing_if = "Option::is_none")]
        posting_url_selector: Option<Select>,
        limits: PaginationLimits,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PaginationLimits {
    pub max_requests: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_items: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<u64>,
}
