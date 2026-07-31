use std::{future::Future, pin::Pin, time::Duration};

use futures_util::StreamExt;
use source_profile_dsl::execution::{
    collect_profile_http_response, ProfileHttpClient, ProfileHttpError, ProfileHttpFailureKind,
    ProfileHttpRequest, ProfileHttpResponse, RuntimeExecutionContext,
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
                source_profile_dsl::definition::HttpMethod::Get => reqwest::Method::GET,
                source_profile_dsl::definition::HttpMethod::Post => reqwest::Method::POST,
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

#[cfg(test)]
mod tests {
    use super::*;
    use source_profile_dsl::definition::HttpMethod;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    fn request(url: String) -> ProfileHttpRequest {
        ProfileHttpRequest {
            method: HttpMethod::Get,
            url,
            headers: Vec::new(),
            body: None,
            timeout_ms: 5_000,
            authored_charset: None,
        }
    }

    #[test]
    fn preserves_redirect_non_success_repeated_raw_headers_and_exact_bytes() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for index in 0..2 {
                let (mut socket, _) = listener.accept().unwrap();
                let mut request = [0_u8; 2048];
                let read = socket.read(&mut request).unwrap();
                let line = String::from_utf8_lossy(&request[..read]);
                if index == 0 {
                    assert!(line.starts_with("GET /start "));
                    write!(socket, "HTTP/1.1 302 Found\r\nLocation: http://{address}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                } else {
                    assert!(line.starts_with("GET /final "));
                    let mut response = b"HTTP/1.1 418 Teapot\r\nContent-Type: text/plain; charset=windows-1252\r\nX-Repeat: first\r\nX-Repeat: ".to_vec();
                    response.push(0xff);
                    response.extend_from_slice(b"\r\nContent-Encoding: gzip\r\nContent-Length: 1\r\nConnection: close\r\n\r\n\x80");
                    socket.write_all(&response).unwrap();
                }
            }
        });
        let client = ReqwestProfileHttpClient::new();
        let response = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(client.fetch(
                request(format!("http://{address}/start")),
                RuntimeExecutionContext::uncancellable(),
            ))
            .unwrap_or_else(|_| panic!("adapter request failed"));
        server.join().unwrap();
        assert_eq!(response.status(), 418);
        assert_eq!(response.final_url(), format!("http://{address}/final"));
        assert_eq!(response.raw_body(), &[0x80]);
        assert_eq!(response.body, "€");
        let repeated = response
            .headers()
            .iter()
            .filter(|h| h.name() == "x-repeat")
            .map(|h| h.value())
            .collect::<Vec<_>>();
        assert_eq!(repeated, vec![b"first".as_slice(), &[0xff]]);
    }
}
