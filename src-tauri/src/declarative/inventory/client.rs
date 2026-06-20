use reqwest::Url;
use std::{future::Future, pin::Pin, time::Duration};

pub(super) type BoxedTextFuture<'a> =
    Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>>;

pub(crate) trait InventoryHttpClient {
    fn get_text(&self, url: Url) -> BoxedTextFuture<'_>;
}

pub(crate) struct ReqwestInventoryHttpClient;

impl InventoryHttpClient for ReqwestInventoryHttpClient {
    fn get_text(&self, url: Url) -> BoxedTextFuture<'_> {
        Box::pin(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .user_agent("JobRadarDeclarativeInventoryExecutor/0.1")
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
