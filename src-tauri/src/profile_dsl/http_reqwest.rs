use std::{future::Future, pin::Pin, time::Duration};

use futures_util::StreamExt;
use source_profile_dsl::profile_dsl::runtime::{
    http::collect_profile_http_response, ProfileHttpClient, ProfileHttpError,
    ProfileHttpFailureKind, ProfileHttpRequest, ProfileHttpResponse, RuntimeExecutionContext,
};

#[derive(Clone)]
pub struct ReqwestProfileHttpClient {
    client: reqwest::Client,
}

impl ReqwestProfileHttpClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .build()
            .expect("static reqwest client configuration is valid");
        Self { client }
    }
}

impl Default for ReqwestProfileHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileHttpClient for ReqwestProfileHttpClient {
    fn fetch<'a>(
        &'a self,
        request: ProfileHttpRequest,
        context: RuntimeExecutionContext<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<ProfileHttpResponse, ProfileHttpError>> + Send + 'a>>
    {
        Box::pin(async move {
            let method = match request.method {
                source_profile_dsl::profile_dsl::documents::HttpMethod::Get => reqwest::Method::GET,
                source_profile_dsl::profile_dsl::documents::HttpMethod::Post => {
                    reqwest::Method::POST
                }
            };
            let mut builder = self
                .client
                .request(method, &request.url)
                .timeout(Duration::from_millis(request.timeout_ms));
            for (name, value) in &request.headers {
                let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
                    .map_err(|_| failure(ProfileHttpFailureKind::InvalidRequest, 0))?;
                let value = reqwest::header::HeaderValue::from_bytes(value)
                    .map_err(|_| failure(ProfileHttpFailureKind::InvalidRequest, 0))?;
                builder = builder.header(name, value);
            }
            if let Some(body) = &request.body {
                if let Some(content_type) = body.default_content_type() {
                    if !request
                        .headers
                        .iter()
                        .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
                    {
                        builder = builder.header("content-type", content_type);
                    }
                }
                builder = builder.body(body.bytes().to_vec());
            }
            let response = tokio::select! {
                biased;
                _ = context.cancelled() => return Err(failure(ProfileHttpFailureKind::Cancelled, 0)),
                response = builder.send() => response.map_err(|error| {
                    if error.is_timeout() { failure(ProfileHttpFailureKind::Timeout, 0) }
                    else if error.is_connect() { failure(ProfileHttpFailureKind::Connect, 0) }
                    else { failure(ProfileHttpFailureKind::InvalidRequest, 0) }
                })?,
                _ = context.deadline_reached() => {
                    context.mark_deadline();
                    return Err(failure(ProfileHttpFailureKind::Timeout, 0));
                },
            };
            let status = response.status().as_u16();
            let final_url = response.url().to_string();
            let mut headers = Vec::new();
            for name in response.headers().keys() {
                for value in response.headers().get_all(name).iter() {
                    headers.push((name.as_str().to_string(), value.as_bytes().to_vec()));
                }
            }
            let body_stream = response.bytes_stream().map(|item| {
                item.map(|bytes| bytes.to_vec())
                    .map_err(|_| ProfileHttpFailureKind::BodyStream)
            });
            collect_profile_http_response(
                body_stream,
                status,
                final_url,
                headers,
                request.authored_charset.as_deref(),
                context,
            )
            .await
        })
    }
}

fn failure(kind: ProfileHttpFailureKind, admitted_bytes: u64) -> ProfileHttpError {
    ProfileHttpError {
        kind,
        admitted_bytes,
    }
}
