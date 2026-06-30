use dom_query::{Document, Matcher};
use reqwest::Url;
use serde_json::Value;
use std::{future::Future, pin::Pin, time::Duration};

use crate::declarative::template::{render_template, TemplateContext, TemplateError};

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
        if parse_as != "html" {
            return Err(PostingDetailError::Failed(format!(
                "postingDetail.parse.as `{parse_as}` is not supported by this extractor slice"
            )));
        }

        let fields = required_object(posting_detail, "fields", "postingDetail.fields")?;
        let description_text = required_object(
            fields,
            "descriptionText",
            "postingDetail.fields.descriptionText",
        )?;
        let selector = required_string(
            description_text,
            "selectorText",
            "postingDetail.fields.descriptionText.selectorText",
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
        extract_selector_text(
            &body,
            selector,
            "postingDetail.fields.descriptionText.selectorText",
        )
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
    let document = Document::from(html);
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
}
