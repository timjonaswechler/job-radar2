use reqwest::Url;
use serde_json::Value;
use std::collections::HashMap;

use crate::{
    declarative::template::render_template,
    search_run_model::{
        BoxedSourceExecutionFuture, SourceCandidate, SourceExecutionError, SourceExecutionInput,
        SourceExecutor,
    },
};

use super::*;

pub(super) const DECLARATIVE_HTTP_ADAPTER_KEY: &str = "declarative_endpoint_inventory";

pub(super) const DECLARATIVE_SITEMAP_ADAPTER_KEY: &str = "declarative_sitemap_inventory";

pub(crate) struct DeclarativeInventoryExecutor<C = ReqwestInventoryHttpClient> {
    pub(super) client: C,
}

impl DeclarativeInventoryExecutor<ReqwestInventoryHttpClient> {
    pub(crate) fn new_reqwest() -> Self {
        Self {
            client: ReqwestInventoryHttpClient,
        }
    }
}

impl<C> DeclarativeInventoryExecutor<C> {
    #[cfg(test)]
    pub(super) fn new(client: C) -> Self {
        Self { client }
    }
}

impl<C> SourceExecutor for DeclarativeInventoryExecutor<C>
where
    C: InventoryHttpClient + Send + Sync,
{
    fn execute<'a>(&'a self, input: SourceExecutionInput<'a>) -> BoxedSourceExecutionFuture<'a> {
        Box::pin(async move { self.execute_source(input).await })
    }
}

impl<C> DeclarativeInventoryExecutor<C>
where
    C: InventoryHttpClient + Send + Sync,
{
    async fn execute_source(
        &self,
        input: SourceExecutionInput<'_>,
    ) -> Result<Vec<SourceCandidate>, SourceExecutionError> {
        let source = input.source;
        if !matches!(
            source.adapter_key.as_str(),
            DECLARATIVE_HTTP_ADAPTER_KEY | DECLARATIVE_SITEMAP_ADAPTER_KEY
        ) {
            return Err(SourceExecutionError::Failed(format!(
                "adapterKey {} is not supported by DeclarativeInventoryExecutor",
                source.adapter_key
            )));
        }

        let inventory = source
            .inventory()
            .and_then(Value::as_object)
            .ok_or_else(|| {
                SourceExecutionError::Failed(format!(
                    "executionPlan.inventory must be a JSON object for source {}",
                    source.key
                ))
            })?;
        let fetch = required_object_value(inventory, "fetch", "executionPlan.inventory.fetch")?;
        let fetch_url_template =
            required_string(fetch, "url", "executionPlan.inventory.fetch.url")?;
        let empty_captures = HashMap::new();
        let fetch_context = InventoryTemplateContext {
            source,
            item: None,
            captures: &empty_captures,
        };
        let fetch_url = render_template(fetch_url_template, &fetch_context).map_err(|error| {
            SourceExecutionError::Failed(format!(
                "executionPlan.inventory.fetch.url is invalid: {error}"
            ))
        })?;
        let fetch_url = parse_http_url(&fetch_url, "executionPlan.inventory.fetch.url")?;

        let parse = required_object_value(inventory, "parse", "executionPlan.inventory.parse")?;
        let parse_as = required_string(parse, "as", "executionPlan.inventory.parse.as")?;
        let items = required_object_value(inventory, "items", "executionPlan.inventory.items")?;
        let inventory_items = match parse_as {
            "xml" => {
                let body = self.fetch_inventory_text(fetch_url.clone()).await?;
                select_xml_items(&body, items)?
            }
            "json" => {
                self.select_json_inventory_items(fetch_url.clone(), fetch, items)
                    .await?
            }
            other => {
                return Err(SourceExecutionError::Failed(format!(
                    "executionPlan.inventory.parse.as `{other}` is not supported by this executor slice"
                )));
            }
        };

        let where_regexes =
            compile_regex_list(items.get("where"), "executionPlan.inventory.items.where")?;
        let capture_regexes = compile_regex_list(
            items.get("captures"),
            "executionPlan.inventory.items.captures",
        )?;
        let fields = required_object_value(inventory, "fields", "executionPlan.inventory.fields")?;

        let mut candidates = Vec::new();
        for inventory_item in inventory_items {
            let captures = match inventory_item.text() {
                Some(item_text) => {
                    if !where_regexes.iter().all(|regex| regex.is_match(item_text)) {
                        continue;
                    }
                    let Some(captures) = capture_item(&capture_regexes, item_text) else {
                        continue;
                    };
                    captures
                }
                None => {
                    if !where_regexes.is_empty() {
                        return Err(SourceExecutionError::Failed(
                            "executionPlan.inventory.items.where is only supported for text item selections"
                                .to_string(),
                        ));
                    }
                    if !capture_regexes.is_empty() {
                        return Err(SourceExecutionError::Failed(
                            "executionPlan.inventory.items.captures is only supported for text item selections"
                                .to_string(),
                        ));
                    }
                    HashMap::new()
                }
            };
            let context = InventoryTemplateContext {
                source,
                item: Some(&inventory_item),
                captures: &captures,
            };

            let title = render_required_field(fields, "title", &context)?;
            let raw_url = render_required_field(fields, "url", &context)?;
            let url = resolve_http_candidate_url(&raw_url, &fetch_url)
                .unwrap_or_else(|| raw_url.trim().to_string());
            let company = render_required_field(fields, "company", &context)?;
            let locations = render_locations(fields, &context)?;

            if title.trim().is_empty() || url.trim().is_empty() || company.trim().is_empty() {
                continue;
            }

            candidates.push(SourceCandidate {
                title,
                company,
                url,
                locations,
            });
        }

        Ok(candidates)
    }

    async fn fetch_inventory_text(&self, fetch_url: Url) -> Result<String, SourceExecutionError> {
        self.client
            .get_text(fetch_url.clone())
            .await
            .map_err(|error| {
                SourceExecutionError::Failed(format!(
                    "could not fetch inventory {}: {error}",
                    fetch_url.as_str()
                ))
            })
    }

    async fn select_json_inventory_items(
        &self,
        fetch_url: Url,
        fetch: &serde_json::Map<String, Value>,
        items: &serde_json::Map<String, Value>,
    ) -> Result<Vec<InventoryItem>, SourceExecutionError> {
        let Some(pagination_value) = fetch.get("pagination") else {
            let body = self.fetch_inventory_text(fetch_url).await?;
            return select_json_items(&body, items);
        };

        let pagination = parse_page_count_pagination(
            pagination_value,
            "executionPlan.inventory.fetch.pagination",
        )?;
        let first_url = page_count_pagination_url(&fetch_url, &pagination, pagination.first_page);
        let first_body = self.fetch_inventory_text(first_url).await?;
        let (mut inventory_items, first_root) = select_json_items_with_root(&first_body, items)?;
        let total = resolve_json_u64(
            &first_root,
            &pagination.total_path,
            "executionPlan.inventory.fetch.pagination.totalPath",
        )?;
        let page_count = total.div_ceil(pagination.size);
        if page_count <= 1 {
            return Ok(inventory_items);
        }

        let last_page = pagination.first_page + page_count - 1;
        for page in (pagination.first_page + 1)..=last_page {
            let page_url = page_count_pagination_url(&fetch_url, &pagination, page);
            let page_body = self.fetch_inventory_text(page_url).await?;
            inventory_items.extend(select_json_items(&page_body, items)?);
        }

        Ok(inventory_items)
    }
}
