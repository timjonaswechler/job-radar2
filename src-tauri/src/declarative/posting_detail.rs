use dom_query::{Document as HtmlDocument, Matcher};
use reqwest::Url;
use roxmltree::{Document as XmlDocument, Node as XmlNode};
use serde_json::Value;
use std::{future::Future, pin::Pin, time::Duration};

use crate::{
    declarative::template::{render_template, TemplateContext, TemplateError},
    simple_json_path::resolve_simple_json_path,
    source::registry::ResolvedSourceExecutionPlan,
};

pub(crate) type BoxedPostingDetailTextFuture<'a> =
    Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>>;

pub(crate) trait PostingDetailHttpClient {
    fn get_text(&self, url: Url) -> BoxedPostingDetailTextFuture<'_>;
}

pub(crate) struct ReqwestPostingDetailHttpClient;

impl PostingDetailHttpClient for ReqwestPostingDetailHttpClient {
    fn get_text(&self, url: Url) -> BoxedPostingDetailTextFuture<'_> {
        Box::pin(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .user_agent("JobRadarPostingDetailExtractor/0.1")
                .build()
                .map_err(|error| error.to_string())?;
            let response = client
                .get(url.clone())
                .send()
                .await
                .map_err(|error| error.to_string())?;
            if !response.status().is_success() {
                return Err(format!("HTTP {}", response.status()));
            }
            response.text().await.map_err(|error| error.to_string())
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PostingDetail {
    pub(crate) description_text: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PostingDetailSource<'a> {
    pub(crate) source_key: &'a str,
    pub(crate) url: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PostingDetailError {
    Unsupported(String),
    Failed(String),
}

impl std::fmt::Display for PostingDetailError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(message) | Self::Failed(message) => formatter.write_str(message),
        }
    }
}

pub(crate) struct PostingDetailExtractor<C = ReqwestPostingDetailHttpClient> {
    client: C,
}

impl PostingDetailExtractor<ReqwestPostingDetailHttpClient> {
    #[allow(dead_code)]
    pub(crate) fn new_reqwest() -> Self {
        Self {
            client: ReqwestPostingDetailHttpClient,
        }
    }
}

impl<C> PostingDetailExtractor<C> {
    pub(crate) fn new(client: C) -> Self {
        Self { client }
    }
}

impl<C> PostingDetailExtractor<C>
where
    C: PostingDetailHttpClient + Send + Sync,
{
    pub(crate) async fn load_source_description_text(
        &self,
        source: &ResolvedSourceExecutionPlan,
        posting_source: PostingDetailSource<'_>,
    ) -> Result<PostingDetail, PostingDetailError> {
        if posting_source.source_key != source.key {
            return Err(PostingDetailError::Failed(format!(
                "posting source key `{}` does not match selected execution source `{}`",
                posting_source.source_key, source.key
            )));
        }

        self.load_description_text(source.posting_detail(), posting_source.url)
            .await
    }

    pub(crate) async fn load_description_text(
        &self,
        posting_detail: Option<&Value>,
        posting_url: &str,
    ) -> Result<PostingDetail, PostingDetailError> {
        let posting_detail = posting_detail.ok_or_else(|| {
            PostingDetailError::Unsupported(
                "selected access path has no postingDetail extraction".to_string(),
            )
        })?;
        let posting_detail = json_object(posting_detail, "postingDetail")?;
        let fetch = required_object(posting_detail, "fetch", "postingDetail.fetch")?;
        let fetch_url_template = required_string(fetch, "url", "postingDetail.fetch.url")?;
        let context = PostingDetailTemplateContext { posting_url };
        let fetch_url = render_template(fetch_url_template, &context).map_err(|error| {
            PostingDetailError::Failed(format!("postingDetail.fetch.url is invalid: {error}"))
        })?;
        let fetch_url = parse_http_url(&fetch_url, "postingDetail.fetch.url")?;

        let parse = required_object(posting_detail, "parse", "postingDetail.parse")?;
        let parse_as = required_string(parse, "as", "postingDetail.parse.as")?;
        let fields = required_object(posting_detail, "fields", "postingDetail.fields")?;
        let description_text = required_object(
            fields,
            "descriptionText",
            "postingDetail.fields.descriptionText",
        )?;

        let body = self
            .client
            .get_text(fetch_url.clone())
            .await
            .map_err(|error| {
                PostingDetailError::Failed(format!(
                    "could not fetch posting detail {}: {error}",
                    fetch_url.as_str()
                ))
            })?;

        match parse_as {
            "html" => {
                let selector = required_string(
                    description_text,
                    "selectorText",
                    "postingDetail.fields.descriptionText.selectorText",
                )?;
                extract_selector_text(
                    &body,
                    selector,
                    "postingDetail.fields.descriptionText.selectorText",
                )
            }
            "json" => extract_json_description_text(&body, description_text),
            "xml" => extract_xml_description_text(&body, description_text),
            _ => Err(PostingDetailError::Failed(format!(
                "postingDetail.parse.as `{parse_as}` is not supported by this extractor slice"
            ))),
        }
    }
}

struct PostingDetailTemplateContext<'a> {
    posting_url: &'a str,
}

impl TemplateContext for PostingDetailTemplateContext<'_> {
    fn resolve_variable(&self, variable: &str) -> Result<Option<String>, TemplateError> {
        if variable == "posting:url" {
            Ok(Some(self.posting_url.to_string()))
        } else {
            Err(TemplateError::Invalid(format!(
                "unsupported postingDetail template variable `{variable}`"
            )))
        }
    }
}

fn extract_selector_text(
    html: &str,
    selector: &str,
    path: &str,
) -> Result<PostingDetail, PostingDetailError> {
    let matcher = Matcher::new(selector).map_err(|error| {
        PostingDetailError::Failed(format!(
            "{path} must be a valid CSS selector for the postingDetail language: {error:?}"
        ))
    })?;
    let document = HtmlDocument::from(html);
    let description_text = document
        .select_matcher(&matcher)
        .iter()
        .map(|selection| normalize_description_text(&selection.text().to_string()))
        .find(|text| !text.is_empty())
        .ok_or_else(|| {
            PostingDetailError::Failed(format!(
                "{path} did not match non-empty posting detail text"
            ))
        })?;

    Ok(PostingDetail { description_text })
}

fn extract_json_description_text(
    body: &str,
    description_text: &serde_json::Map<String, Value>,
) -> Result<PostingDetail, PostingDetailError> {
    let document = serde_json::from_str::<Value>(body).map_err(|error| {
        PostingDetailError::Failed(format!(
            "could not parse postingDetail JSON document: {error}"
        ))
    })?;
    let (key, raw_kind, json_path) = json_description_path(description_text)?;
    let path = format!("postingDetail.fields.descriptionText.{key}");
    let value = resolve_simple_json_path(&document, json_path)
        .map_err(|error| PostingDetailError::Failed(format!("{path} {error}")))?;
    let raw_description = json_description_value_to_string(value, &path)?;

    normalize_raw_description(&raw_description, raw_kind, &path)
}

fn json_description_path<'a>(
    description_text: &'a serde_json::Map<String, Value>,
) -> Result<(&'static str, RawDescriptionKind, &'a str), PostingDetailError> {
    if description_text.contains_key("jsonPath") {
        return Ok((
            "jsonPath",
            RawDescriptionKind::Text,
            required_string(
                description_text,
                "jsonPath",
                "postingDetail.fields.descriptionText.jsonPath",
            )?,
        ));
    }
    if description_text.contains_key("jsonPathHtml") {
        return Ok((
            "jsonPathHtml",
            RawDescriptionKind::Html,
            required_string(
                description_text,
                "jsonPathHtml",
                "postingDetail.fields.descriptionText.jsonPathHtml",
            )?,
        ));
    }
    Err(PostingDetailError::Failed(
        "postingDetail.fields.descriptionText must contain jsonPath or jsonPathHtml for JSON postingDetail extraction".to_string(),
    ))
}

fn json_description_value_to_string(
    value: Option<&Value>,
    path: &str,
) -> Result<String, PostingDetailError> {
    match value {
        None | Some(Value::Null) => Err(PostingDetailError::Failed(format!(
            "{path} did not match a posting detail value"
        ))),
        Some(Value::String(value)) => Ok(value.clone()),
        Some(Value::Bool(value)) => Ok(value.to_string()),
        Some(Value::Number(value)) => Ok(value.to_string()),
        Some(Value::Array(_) | Value::Object(_)) => Err(PostingDetailError::Failed(format!(
            "{path} must resolve to a string, number, boolean, or null"
        ))),
    }
}

fn extract_xml_description_text(
    body: &str,
    description_text: &serde_json::Map<String, Value>,
) -> Result<PostingDetail, PostingDetailError> {
    let document = XmlDocument::parse(body).map_err(|error| {
        PostingDetailError::Failed(format!(
            "could not parse postingDetail XML document: {error}"
        ))
    })?;
    let (key, raw_kind, element_name) = xml_description_selector(description_text)?;
    let path = format!("postingDetail.fields.descriptionText.{key}");

    for node in document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == element_name)
    {
        let raw_description = match key {
            "xmlText" | "xmlTextHtml" => xml_immediate_text(node, &path)?,
            "xmlElement" => xml_descendant_text(node),
            _ => unreachable!("validated XML description key"),
        };
        let normalized = match raw_kind {
            RawDescriptionKind::Text => normalize_description_text(&raw_description),
            RawDescriptionKind::Html => normalize_html_description_text(&raw_description),
        };
        if !normalized.is_empty() {
            return Ok(PostingDetail {
                description_text: normalized,
            });
        }
    }

    Err(PostingDetailError::Failed(format!(
        "{path} did not match non-empty posting detail text"
    )))
}

fn xml_description_selector<'a>(
    description_text: &'a serde_json::Map<String, Value>,
) -> Result<(&'static str, RawDescriptionKind, &'a str), PostingDetailError> {
    if description_text.contains_key("xmlText") {
        return Ok((
            "xmlText",
            RawDescriptionKind::Text,
            required_string(
                description_text,
                "xmlText",
                "postingDetail.fields.descriptionText.xmlText",
            )?,
        ));
    }
    if description_text.contains_key("xmlTextHtml") {
        return Ok((
            "xmlTextHtml",
            RawDescriptionKind::Html,
            required_string(
                description_text,
                "xmlTextHtml",
                "postingDetail.fields.descriptionText.xmlTextHtml",
            )?,
        ));
    }
    if description_text.contains_key("xmlElement") {
        return Ok((
            "xmlElement",
            RawDescriptionKind::Text,
            required_string(
                description_text,
                "xmlElement",
                "postingDetail.fields.descriptionText.xmlElement",
            )?,
        ));
    }
    Err(PostingDetailError::Failed(
        "postingDetail.fields.descriptionText must contain xmlText, xmlTextHtml, or xmlElement for XML postingDetail extraction".to_string(),
    ))
}

fn xml_immediate_text(node: XmlNode<'_, '_>, path: &str) -> Result<String, PostingDetailError> {
    if node.children().any(|child| child.is_element()) {
        return Err(PostingDetailError::Failed(format!(
            "{path} matched nested XML; use xmlElement when nested element text should be normalized"
        )));
    }
    Ok(node
        .children()
        .filter(|child| child.is_text())
        .filter_map(|text| text.text())
        .collect::<Vec<_>>()
        .join(" "))
}

fn xml_descendant_text(node: XmlNode<'_, '_>) -> String {
    node.descendants()
        .filter(|descendant| descendant.is_text())
        .filter_map(|text| text.text())
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RawDescriptionKind {
    Text,
    Html,
}

fn normalize_raw_description(
    raw_description: &str,
    raw_kind: RawDescriptionKind,
    path: &str,
) -> Result<PostingDetail, PostingDetailError> {
    let description_text = match raw_kind {
        RawDescriptionKind::Text => normalize_description_text(raw_description),
        RawDescriptionKind::Html => normalize_html_description_text(raw_description),
    };
    if description_text.is_empty() {
        return Err(PostingDetailError::Failed(format!(
            "{path} did not resolve to non-empty posting detail text"
        )));
    }
    Ok(PostingDetail { description_text })
}

fn normalize_html_description_text(value: &str) -> String {
    normalize_description_text(&HtmlDocument::fragment(value).formatted_text().to_string())
}

fn normalize_description_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn required_object<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a serde_json::Map<String, Value>, PostingDetailError> {
    object
        .get(key)
        .ok_or_else(|| PostingDetailError::Failed(format!("{path} is required")))
        .and_then(|value| json_object(value, path))
}

fn json_object<'a>(
    value: &'a Value,
    path: &str,
) -> Result<&'a serde_json::Map<String, Value>, PostingDetailError> {
    value
        .as_object()
        .ok_or_else(|| PostingDetailError::Failed(format!("{path} must be a JSON object")))
}

fn required_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a str, PostingDetailError> {
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| PostingDetailError::Failed(format!("{path} must be a non-empty string")))?;
    if value.trim().is_empty() {
        return Err(PostingDetailError::Failed(format!(
            "{path} must be a non-empty string"
        )));
    }
    Ok(value)
}

fn parse_http_url(value: &str, path: &str) -> Result<Url, PostingDetailError> {
    let url = Url::parse(value.trim()).map_err(|error| {
        PostingDetailError::Failed(format!("{path} must be an absolute HTTP(S) URL: {error}"))
    })?;
    if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() {
        Ok(url)
    } else {
        Err(PostingDetailError::Failed(format!(
            "{path} must be an absolute HTTP(S) URL"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::registry::{ResolvedSelectedAccessPath, ResolvedSourceExecutionPlan};
    use reqwest::Url;
    use serde_json::json;
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    struct FakePostingDetailHttpClient {
        responses: HashMap<String, String>,
        requested_urls: Arc<Mutex<Vec<String>>>,
    }

    impl PostingDetailHttpClient for FakePostingDetailHttpClient {
        fn get_text(&self, url: Url) -> BoxedPostingDetailTextFuture<'_> {
            let requested_urls = Arc::clone(&self.requested_urls);
            Box::pin(async move {
                let url = url.to_string();
                requested_urls.lock().unwrap().push(url.clone());
                self.responses
                    .get(&url)
                    .cloned()
                    .ok_or_else(|| format!("missing fake response for {url}"))
            })
        }
    }

    #[test]
    fn missing_posting_detail_returns_unsupported_error() {
        tauri::async_runtime::block_on(async {
            let client = FakePostingDetailHttpClient {
                responses: HashMap::new(),
                requested_urls: Arc::new(Mutex::new(Vec::new())),
            };
            let extractor = PostingDetailExtractor::new(client);

            let error = extractor
                .load_description_text(None, "https://example.test/jobs/42")
                .await
                .unwrap_err();

            assert!(matches!(error, PostingDetailError::Unsupported(_)));
            assert_eq!(
                error.to_string(),
                "selected access path has no postingDetail extraction"
            );
        });
    }

    #[test]
    fn source_detail_pairing_uses_selected_source_access_path_and_posting_source_url() {
        tauri::async_runtime::block_on(async {
            let requested_urls = Arc::new(Mutex::new(Vec::new()));
            let client = FakePostingDetailHttpClient {
                responses: HashMap::from([(
                    "https://source-a.example/jobs/42".to_string(),
                    r#"<section class="source-a-description">Source A description</section>"#
                        .to_string(),
                )]),
                requested_urls: Arc::clone(&requested_urls),
            };
            let extractor = PostingDetailExtractor::new(client);
            let source = resolved_source_plan(
                "source_a",
                Some(json!({
                    "fetch": { "url": "{{posting:url}}" },
                    "parse": { "as": "html" },
                    "fields": {
                        "descriptionText": { "selectorText": ".source-a-description" }
                    }
                })),
            );

            let detail = extractor
                .load_source_description_text(
                    &source,
                    PostingDetailSource {
                        source_key: "source_a",
                        url: "https://source-a.example/jobs/42",
                    },
                )
                .await
                .unwrap();

            assert_eq!(detail.description_text, "Source A description");
            assert_eq!(
                *requested_urls.lock().unwrap(),
                vec!["https://source-a.example/jobs/42".to_string()]
            );
        });
    }

    #[test]
    fn source_detail_pairing_rejects_mismatched_source_key_without_fetching() {
        tauri::async_runtime::block_on(async {
            let requested_urls = Arc::new(Mutex::new(Vec::new()));
            let client = FakePostingDetailHttpClient {
                responses: HashMap::new(),
                requested_urls: Arc::clone(&requested_urls),
            };
            let extractor = PostingDetailExtractor::new(client);
            let source = resolved_source_plan(
                "source_a",
                Some(json!({
                    "fetch": { "url": "{{posting:url}}" },
                    "parse": { "as": "html" },
                    "fields": {
                        "descriptionText": { "selectorText": ".source-a-description" }
                    }
                })),
            );

            let error = extractor
                .load_source_description_text(
                    &source,
                    PostingDetailSource {
                        source_key: "source_b",
                        url: "https://source-b.example/jobs/99",
                    },
                )
                .await
                .unwrap_err();

            assert!(matches!(error, PostingDetailError::Failed(_)));
            assert_eq!(
                error.to_string(),
                "posting source key `source_b` does not match selected execution source `source_a`"
            );
            assert!(requested_urls.lock().unwrap().is_empty());
        });
    }

    #[test]
    fn unsupported_profile_access_path_returns_unsupported_error() {
        tauri::async_runtime::block_on(async {
            let requested_urls = Arc::new(Mutex::new(Vec::new()));
            let client = FakePostingDetailHttpClient {
                responses: HashMap::new(),
                requested_urls: Arc::clone(&requested_urls),
            };
            let extractor = PostingDetailExtractor::new(client);
            let source = resolved_source_plan("source_without_detail", None);

            let error = extractor
                .load_source_description_text(
                    &source,
                    PostingDetailSource {
                        source_key: "source_without_detail",
                        url: "https://example.test/jobs/42",
                    },
                )
                .await
                .unwrap_err();

            assert!(matches!(error, PostingDetailError::Unsupported(_)));
            assert_eq!(
                error.to_string(),
                "selected access path has no postingDetail extraction"
            );
            assert!(requested_urls.lock().unwrap().is_empty());
        });
    }

    #[test]
    fn posting_detail_uses_selected_posting_url_template_and_extracts_description_text() {
        tauri::async_runtime::block_on(async {
            let requested_urls = Arc::new(Mutex::new(Vec::new()));
            let client = FakePostingDetailHttpClient {
                responses: HashMap::from([(
                    "https://example.test/jobs/42".to_string(),
                    r#"
                    <html>
                      <body>
                        <main>
                          <div class="job__description">
                            <p>First paragraph.</p>
                            <p>Second paragraph.</p>
                          </div>
                        </main>
                      </body>
                    </html>
                    "#
                    .to_string(),
                )]),
                requested_urls: Arc::clone(&requested_urls),
            };
            let extractor = PostingDetailExtractor::new(client);
            let posting_detail = json!({
                "fetch": { "url": "{{posting:url}}" },
                "parse": { "as": "html" },
                "fields": {
                    "descriptionText": { "selectorText": ".job__description" }
                }
            });

            let detail = extractor
                .load_description_text(Some(&posting_detail), "https://example.test/jobs/42")
                .await
                .unwrap();

            assert_eq!(
                detail.description_text,
                "First paragraph. Second paragraph."
            );
            assert_eq!(
                *requested_urls.lock().unwrap(),
                vec!["https://example.test/jobs/42".to_string()]
            );
        });
    }

    #[test]
    fn posting_detail_extracts_direct_json_text_description() {
        tauri::async_runtime::block_on(async {
            let requested_urls = Arc::new(Mutex::new(Vec::new()));
            let client = FakePostingDetailHttpClient {
                responses: HashMap::from([(
                    "https://example.test/jobs/42.json".to_string(),
                    r#"{ "description": "First paragraph.\n\nSecond paragraph." }"#.to_string(),
                )]),
                requested_urls: Arc::clone(&requested_urls),
            };
            let extractor = PostingDetailExtractor::new(client);
            let posting_detail = json!({
                "fetch": { "url": "{{posting:url}}" },
                "parse": { "as": "json" },
                "fields": {
                    "descriptionText": { "jsonPath": "$.description" }
                }
            });

            let detail = extractor
                .load_description_text(Some(&posting_detail), "https://example.test/jobs/42.json")
                .await
                .unwrap();

            assert_eq!(
                detail.description_text,
                "First paragraph. Second paragraph."
            );
            assert_eq!(
                *requested_urls.lock().unwrap(),
                vec!["https://example.test/jobs/42.json".to_string()]
            );
        });
    }

    #[test]
    fn posting_detail_extracts_direct_json_html_description() {
        tauri::async_runtime::block_on(async {
            let client = FakePostingDetailHttpClient {
                responses: HashMap::from([(
                    "https://example.test/jobs/42.json".to_string(),
                    r#"{ "description_html": "<p>First paragraph.</p><p>Second paragraph.</p>" }"#
                        .to_string(),
                )]),
                requested_urls: Arc::new(Mutex::new(Vec::new())),
            };
            let extractor = PostingDetailExtractor::new(client);
            let posting_detail = json!({
                "fetch": { "url": "{{posting:url}}" },
                "parse": { "as": "json" },
                "fields": {
                    "descriptionText": { "jsonPathHtml": "$.description_html" }
                }
            });

            let detail = extractor
                .load_description_text(Some(&posting_detail), "https://example.test/jobs/42.json")
                .await
                .unwrap();

            assert_eq!(
                detail.description_text,
                "First paragraph. Second paragraph."
            );
        });
    }

    #[test]
    fn malformed_json_detail_returns_actionable_error() {
        tauri::async_runtime::block_on(async {
            let client = FakePostingDetailHttpClient {
                responses: HashMap::from([(
                    "https://example.test/jobs/42.json".to_string(),
                    "{ not json".to_string(),
                )]),
                requested_urls: Arc::new(Mutex::new(Vec::new())),
            };
            let extractor = PostingDetailExtractor::new(client);
            let posting_detail = json!({
                "fetch": { "url": "{{posting:url}}" },
                "parse": { "as": "json" },
                "fields": {
                    "descriptionText": { "jsonPath": "$.description" }
                }
            });

            let error = extractor
                .load_description_text(Some(&posting_detail), "https://example.test/jobs/42.json")
                .await
                .unwrap_err();

            assert!(matches!(error, PostingDetailError::Failed(_)));
            assert!(error
                .to_string()
                .contains("could not parse postingDetail JSON document"));
        });
    }

    #[test]
    fn missing_json_description_value_returns_actionable_error() {
        tauri::async_runtime::block_on(async {
            let client = FakePostingDetailHttpClient {
                responses: HashMap::from([(
                    "https://example.test/jobs/42.json".to_string(),
                    r#"{ "title": "Engineer" }"#.to_string(),
                )]),
                requested_urls: Arc::new(Mutex::new(Vec::new())),
            };
            let extractor = PostingDetailExtractor::new(client);
            let posting_detail = json!({
                "fetch": { "url": "{{posting:url}}" },
                "parse": { "as": "json" },
                "fields": {
                    "descriptionText": { "jsonPath": "$.description" }
                }
            });

            let error = extractor
                .load_description_text(Some(&posting_detail), "https://example.test/jobs/42.json")
                .await
                .unwrap_err();

            assert_eq!(
                error.to_string(),
                "postingDetail.fields.descriptionText.jsonPath did not match a posting detail value"
            );
        });
    }

    #[test]
    fn json_object_or_array_description_value_returns_actionable_error() {
        tauri::async_runtime::block_on(async {
            for (url, response) in [
                (
                    "https://example.test/jobs/object.json",
                    r#"{ "description": { "text": "Nested" } }"#,
                ),
                (
                    "https://example.test/jobs/array.json",
                    r#"{ "description": ["First", "Second"] }"#,
                ),
            ] {
                let client = FakePostingDetailHttpClient {
                    responses: HashMap::from([(url.to_string(), response.to_string())]),
                    requested_urls: Arc::new(Mutex::new(Vec::new())),
                };
                let extractor = PostingDetailExtractor::new(client);
                let posting_detail = json!({
                    "fetch": { "url": "{{posting:url}}" },
                    "parse": { "as": "json" },
                    "fields": {
                        "descriptionText": { "jsonPath": "$.description" }
                    }
                });

                let error = extractor
                    .load_description_text(Some(&posting_detail), url)
                    .await
                    .unwrap_err();

                assert_eq!(
                    error.to_string(),
                    "postingDetail.fields.descriptionText.jsonPath must resolve to a string, number, boolean, or null"
                );
            }
        });
    }

    #[test]
    fn posting_detail_extracts_direct_xml_text_description() {
        tauri::async_runtime::block_on(async {
            let requested_urls = Arc::new(Mutex::new(Vec::new()));
            let client = FakePostingDetailHttpClient {
                responses: HashMap::from([(
                    "https://example.test/jobs/42.xml".to_string(),
                    r#"<job><description>First paragraph.

Second paragraph.</description></job>"#
                        .to_string(),
                )]),
                requested_urls: Arc::clone(&requested_urls),
            };
            let extractor = PostingDetailExtractor::new(client);
            let posting_detail = json!({
                "fetch": { "url": "{{posting:url}}" },
                "parse": { "as": "xml" },
                "fields": {
                    "descriptionText": { "xmlText": "description" }
                }
            });

            let detail = extractor
                .load_description_text(Some(&posting_detail), "https://example.test/jobs/42.xml")
                .await
                .unwrap();

            assert_eq!(
                detail.description_text,
                "First paragraph. Second paragraph."
            );
            assert_eq!(
                *requested_urls.lock().unwrap(),
                vec!["https://example.test/jobs/42.xml".to_string()]
            );
        });
    }

    #[test]
    fn posting_detail_extracts_direct_xml_cdata_html_description() {
        tauri::async_runtime::block_on(async {
            let client = FakePostingDetailHttpClient {
                responses: HashMap::from([(
                    "https://example.test/jobs/42.xml".to_string(),
                    r#"<job><description><![CDATA[<p>First paragraph.</p><p>Second paragraph.</p>]]></description></job>"#.to_string(),
                )]),
                requested_urls: Arc::new(Mutex::new(Vec::new())),
            };
            let extractor = PostingDetailExtractor::new(client);
            let posting_detail = json!({
                "fetch": { "url": "{{posting:url}}" },
                "parse": { "as": "xml" },
                "fields": {
                    "descriptionText": { "xmlTextHtml": "description" }
                }
            });

            let detail = extractor
                .load_description_text(Some(&posting_detail), "https://example.test/jobs/42.xml")
                .await
                .unwrap();

            assert_eq!(
                detail.description_text,
                "First paragraph. Second paragraph."
            );
        });
    }

    #[test]
    fn posting_detail_extracts_direct_xml_nested_element_description() {
        tauri::async_runtime::block_on(async {
            let client = FakePostingDetailHttpClient {
                responses: HashMap::from([(
                    "https://example.test/jobs/42.xml".to_string(),
                    r#"<job><description><p>First paragraph.</p><p>Second paragraph.</p></description></job>"#.to_string(),
                )]),
                requested_urls: Arc::new(Mutex::new(Vec::new())),
            };
            let extractor = PostingDetailExtractor::new(client);
            let posting_detail = json!({
                "fetch": { "url": "{{posting:url}}" },
                "parse": { "as": "xml" },
                "fields": {
                    "descriptionText": { "xmlElement": "description" }
                }
            });

            let detail = extractor
                .load_description_text(Some(&posting_detail), "https://example.test/jobs/42.xml")
                .await
                .unwrap();

            assert_eq!(
                detail.description_text,
                "First paragraph. Second paragraph."
            );
        });
    }

    #[test]
    fn malformed_xml_detail_returns_actionable_error() {
        tauri::async_runtime::block_on(async {
            let client = FakePostingDetailHttpClient {
                responses: HashMap::from([(
                    "https://example.test/jobs/42.xml".to_string(),
                    "<job><description>Unclosed".to_string(),
                )]),
                requested_urls: Arc::new(Mutex::new(Vec::new())),
            };
            let extractor = PostingDetailExtractor::new(client);
            let posting_detail = json!({
                "fetch": { "url": "{{posting:url}}" },
                "parse": { "as": "xml" },
                "fields": {
                    "descriptionText": { "xmlText": "description" }
                }
            });

            let error = extractor
                .load_description_text(Some(&posting_detail), "https://example.test/jobs/42.xml")
                .await
                .unwrap_err();

            assert!(matches!(error, PostingDetailError::Failed(_)));
            assert!(error
                .to_string()
                .contains("could not parse postingDetail XML document"));
        });
    }

    #[test]
    fn missing_xml_description_value_returns_actionable_error() {
        tauri::async_runtime::block_on(async {
            let client = FakePostingDetailHttpClient {
                responses: HashMap::from([(
                    "https://example.test/jobs/42.xml".to_string(),
                    "<job><title>Engineer</title></job>".to_string(),
                )]),
                requested_urls: Arc::new(Mutex::new(Vec::new())),
            };
            let extractor = PostingDetailExtractor::new(client);
            let posting_detail = json!({
                "fetch": { "url": "{{posting:url}}" },
                "parse": { "as": "xml" },
                "fields": {
                    "descriptionText": { "xmlText": "description" }
                }
            });

            let error = extractor
                .load_description_text(Some(&posting_detail), "https://example.test/jobs/42.xml")
                .await
                .unwrap_err();

            assert_eq!(
                error.to_string(),
                "postingDetail.fields.descriptionText.xmlText did not match non-empty posting detail text"
            );
        });
    }

    #[test]
    fn xml_text_description_with_nested_elements_returns_actionable_error() {
        tauri::async_runtime::block_on(async {
            let client = FakePostingDetailHttpClient {
                responses: HashMap::from([(
                    "https://example.test/jobs/42.xml".to_string(),
                    r#"<job><description><p>Nested paragraph.</p></description></job>"#.to_string(),
                )]),
                requested_urls: Arc::new(Mutex::new(Vec::new())),
            };
            let extractor = PostingDetailExtractor::new(client);
            let posting_detail = json!({
                "fetch": { "url": "{{posting:url}}" },
                "parse": { "as": "xml" },
                "fields": {
                    "descriptionText": { "xmlText": "description" }
                }
            });

            let error = extractor
                .load_description_text(Some(&posting_detail), "https://example.test/jobs/42.xml")
                .await
                .unwrap_err();

            assert_eq!(
                error.to_string(),
                "postingDetail.fields.descriptionText.xmlText matched nested XML; use xmlElement when nested element text should be normalized"
            );
        });
    }

    #[test]
    fn fetch_failure_returns_failed_error_with_requested_url() {
        tauri::async_runtime::block_on(async {
            let requested_urls = Arc::new(Mutex::new(Vec::new()));
            let client = FakePostingDetailHttpClient {
                responses: HashMap::new(),
                requested_urls: Arc::clone(&requested_urls),
            };
            let extractor = PostingDetailExtractor::new(client);
            let posting_detail = json!({
                "fetch": { "url": "{{posting:url}}" },
                "parse": { "as": "html" },
                "fields": {
                    "descriptionText": { "selectorText": ".job__description" }
                }
            });

            let error = extractor
                .load_description_text(Some(&posting_detail), "https://example.test/jobs/404")
                .await
                .unwrap_err();

            assert!(matches!(error, PostingDetailError::Failed(_)));
            assert_eq!(
                error.to_string(),
                "could not fetch posting detail https://example.test/jobs/404: missing fake response for https://example.test/jobs/404"
            );
            assert_eq!(
                *requested_urls.lock().unwrap(),
                vec!["https://example.test/jobs/404".to_string()]
            );
        });
    }

    #[test]
    fn invalid_selector_returns_failed_error() {
        tauri::async_runtime::block_on(async {
            let requested_urls = Arc::new(Mutex::new(Vec::new()));
            let client = FakePostingDetailHttpClient {
                responses: HashMap::from([(
                    "https://example.test/jobs/42".to_string(),
                    "<div>Body</div>".to_string(),
                )]),
                requested_urls: Arc::clone(&requested_urls),
            };
            let extractor = PostingDetailExtractor::new(client);
            let posting_detail = json!({
                "fetch": { "url": "{{posting:url}}" },
                "parse": { "as": "html" },
                "fields": {
                    "descriptionText": { "selectorText": "[" }
                }
            });

            let error = extractor
                .load_description_text(Some(&posting_detail), "https://example.test/jobs/42")
                .await
                .unwrap_err();

            assert!(matches!(error, PostingDetailError::Failed(_)));
            assert!(error.to_string().contains(
                "postingDetail.fields.descriptionText.selectorText must be a valid CSS selector"
            ));
        });
    }

    fn resolved_source_plan(
        key: &str,
        posting_detail: Option<Value>,
    ) -> ResolvedSourceExecutionPlan {
        ResolvedSourceExecutionPlan {
            key: key.to_string(),
            name: key.to_string(),
            adapter_key: "declarative_endpoint_inventory".to_string(),
            source_config: json!({}),
            effective_source_config_schema: json!({}),
            selected_access_path: ResolvedSelectedAccessPath::Profile {
                profile_key: "example_profile".to_string(),
                path_key: "endpoint_inventory".to_string(),
                query: None,
                inventory: None,
                posting_detail,
                interactions: None,
                manual_release: None,
            },
        }
    }
}
