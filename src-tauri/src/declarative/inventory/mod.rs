mod client;
mod executor;
mod json;
mod pagination;
mod rendering;
#[cfg(test)]
mod tests;
mod xml;

pub(crate) use self::client::{InventoryHttpClient, ReqwestInventoryHttpClient};
pub(crate) use self::executor::DeclarativeInventoryExecutor;

use self::json::{
    json_type_label, select_json_items, select_json_items_with_root,
    simple_json_path_execution_error, InventoryItem,
};
use self::pagination::{page_count_pagination_url, parse_page_count_pagination, resolve_json_u64};
use self::rendering::{
    capture_item, compile_regex_list, optional_u64, parse_http_url, render_locations,
    render_posting_meta, render_required_field, required_object_value, required_string,
    required_u64, resolve_http_candidate_url, InventoryTemplateContext,
};
use self::xml::select_xml_items;

#[cfg(test)]
use self::client::BoxedTextFuture;
#[cfg(test)]
use self::executor::{DECLARATIVE_HTTP_ADAPTER_KEY, DECLARATIVE_SITEMAP_ADAPTER_KEY};
#[cfg(test)]
use self::xml::{parse_xml_element_values, parse_xml_text_values};
