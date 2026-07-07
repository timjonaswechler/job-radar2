use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Mutex};

use serde_json::json;

use crate::profile_dsl::{
    diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics},
    documents::HttpMethod,
    runtime::{
        PostingDetailFetchError, PostingDetailFetchRequest, PostingDetailFetchResponse,
        PostingDetailFetcher, PostingDiscoveryFetchError, PostingDiscoveryFetchRequest,
        PostingDiscoveryFetchResponse, PostingDiscoveryFetcher, ProfileBrowserClient,
        ProfileBrowserFetchError, ProfileBrowserFetchErrorKind, ProfileBrowserFetchRequest,
        ProfileBrowserFetchResponse,
    },
};

use super::{
    sha256_hex, CheckFingerprint, FixtureManifest, FixtureManifestRequestMethod, FixturePackSource,
};

#[derive(Debug)]
pub(super) struct FixtureReplaySetup {
    pub replay: FixtureReplay,
    pub diagnostics: Diagnostics,
}

#[derive(Debug)]
pub(super) struct FixtureReplay {
    responses: BTreeMap<FixtureReplayRequestKey, FixtureReplayResponse>,
    unmapped_requests: Mutex<Vec<FixtureReplayUnmappedRequest>>,
}

impl FixtureReplay {
    pub(super) fn from_manifest(
        fixture_pack_source: FixturePackSource<'_>,
        profile_key: &str,
        manifest_reference: &str,
        manifest: &FixtureManifest,
        fingerprints: &mut Vec<CheckFingerprint>,
    ) -> Result<FixtureReplaySetup, String> {
        let mut diagnostics = Vec::new();
        let mut responses = BTreeMap::new();

        for mapping in &manifest.requests {
            let body_content = fixture_pack_source.read_fixture_file(
                profile_key,
                &manifest.access_path_key,
                manifest_reference,
                &mapping.response.body_file,
            );
            diagnostics.extend(body_content.diagnostics);
            let Some(body_bytes) = body_content.bytes else {
                continue;
            };
            fingerprints.push(CheckFingerprint::with_reference(
                "fixture_file",
                &mapping.response.body_file,
                sha256_hex(&body_bytes),
            ));

            let key = match FixtureReplayRequestKey::new(
                fixture_request_method_label(mapping.request_match.method),
                &mapping.request_match.url,
            ) {
                Ok(key) => key,
                Err(error) => {
                    diagnostics.push(fixture_execution_failed_diagnostic(
                        profile_key,
                        &manifest.access_path_key,
                        manifest_reference,
                        format!(
                            "Fixture request mapping `{}` URL `{}` could not be normalized: {error}",
                            mapping.key, mapping.request_match.url
                        ),
                    ));
                    continue;
                }
            };

            let body = match String::from_utf8(body_bytes) {
                Ok(body) => body,
                Err(error) => {
                    diagnostics.push(fixture_execution_failed_diagnostic(
                        profile_key,
                        &manifest.access_path_key,
                        manifest_reference,
                        format!(
                            "Fixture response bodyFile `{}` is not valid UTF-8: {error}",
                            mapping.response.body_file
                        ),
                    ));
                    continue;
                }
            };

            responses.insert(
                key,
                FixtureReplayResponse {
                    status: mapping.response.status,
                    headers: mapping.response.headers.clone().unwrap_or_default(),
                    body,
                },
            );
        }

        Ok(FixtureReplaySetup {
            replay: FixtureReplay {
                responses,
                unmapped_requests: Mutex::new(Vec::new()),
            },
            diagnostics,
        })
    }

    pub fn take_unmapped_request_diagnostics(
        &self,
        profile_key: &str,
        access_path_key: &str,
    ) -> Diagnostics {
        let mut requests = self
            .unmapped_requests
            .lock()
            .expect("fixture replay unmapped request mutex should not be poisoned");
        std::mem::take(&mut *requests)
            .into_iter()
            .map(|request| unmapped_request_diagnostic(profile_key, access_path_key, request))
            .collect()
    }

    fn response_for(
        &self,
        method: &'static str,
        url: &str,
    ) -> Result<FixtureReplayResponse, String> {
        let key = match FixtureReplayRequestKey::new(method, url) {
            Ok(key) => key,
            Err(_) => {
                self.record_unmapped_request(method, url);
                return Err(format!(
                    "fixture request {method} {url} is not an absolute HTTP(S) URL"
                ));
            }
        };

        let Some(response) = self.responses.get(&key).cloned() else {
            self.record_unmapped_request(method, url);
            return Err(format!("fixture request {method} {url} is not mapped"));
        };

        if !(200..=299).contains(&response.status) {
            return Err(format!(
                "fixture response for {method} {url} returned HTTP status {}",
                response.status
            ));
        }

        Ok(response)
    }

    fn record_unmapped_request(&self, method: &'static str, url: &str) {
        let mut requests = self
            .unmapped_requests
            .lock()
            .expect("fixture replay unmapped request mutex should not be poisoned");
        requests.push(FixtureReplayUnmappedRequest {
            method: method.to_string(),
            url: url.to_string(),
        });
    }
}

impl PostingDiscoveryFetcher for FixtureReplay {
    fn fetch<'a>(
        &'a self,
        request: PostingDiscoveryFetchRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PostingDiscoveryFetchResponse, PostingDiscoveryFetchError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let method = http_method_label(request.method);
            self.response_for(method, &request.url)
                .map(|response| PostingDiscoveryFetchResponse {
                    body: response.body,
                })
                .map_err(PostingDiscoveryFetchError::new)
        })
    }
}

impl PostingDetailFetcher for FixtureReplay {
    fn fetch<'a>(
        &'a self,
        request: PostingDetailFetchRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PostingDetailFetchResponse, PostingDetailFetchError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let method = http_method_label(request.method);
            self.response_for(method, &request.url)
                .map(|response| PostingDetailFetchResponse {
                    body: response.body,
                })
                .map_err(PostingDetailFetchError::new)
        })
    }
}

impl ProfileBrowserClient for FixtureReplay {
    fn render<'a>(
        &'a self,
        request: ProfileBrowserFetchRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProfileBrowserFetchResponse, ProfileBrowserFetchError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.response_for("GET", &request.url)
                .map(|response| ProfileBrowserFetchResponse {
                    body: response.body,
                })
                .map_err(|message| {
                    ProfileBrowserFetchError::new(
                        ProfileBrowserFetchErrorKind::NavigationFailed,
                        message,
                    )
                })
        })
    }
}

#[derive(Clone, Debug)]
struct FixtureReplayResponse {
    status: u16,
    #[allow(dead_code)]
    headers: BTreeMap<String, String>,
    body: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FixtureReplayRequestKey {
    method: String,
    url: String,
}

impl FixtureReplayRequestKey {
    fn new(method: &str, url: &str) -> Result<Self, String> {
        Ok(Self {
            method: method.to_ascii_uppercase(),
            url: normalize_absolute_http_url(url)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FixtureReplayUnmappedRequest {
    method: String,
    url: String,
}

fn normalize_absolute_http_url(url: &str) -> Result<String, String> {
    let mut parsed = reqwest::Url::parse(url).map_err(|error| error.to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("URL scheme must be http or https".to_string());
    }

    parsed.set_fragment(None);
    let mut query_pairs = parsed
        .query_pairs()
        .map(|(name, value)| (name.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    query_pairs.sort();
    parsed.set_query(None);
    if !query_pairs.is_empty() {
        parsed.query_pairs_mut().extend_pairs(
            query_pairs
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str())),
        );
    }

    Ok(parsed.to_string())
}

fn http_method_label(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
    }
}

fn fixture_request_method_label(method: FixtureManifestRequestMethod) -> &'static str {
    match method {
        FixtureManifestRequestMethod::Get => "GET",
        FixtureManifestRequestMethod::Post => "POST",
        FixtureManifestRequestMethod::Put => "PUT",
        FixtureManifestRequestMethod::Patch => "PATCH",
        FixtureManifestRequestMethod::Delete => "DELETE",
        FixtureManifestRequestMethod::Head => "HEAD",
        FixtureManifestRequestMethod::Options => "OPTIONS",
    }
}

fn unmapped_request_diagnostic(
    profile_key: &str,
    access_path_key: &str,
    request: FixtureReplayUnmappedRequest,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Fixture,
        code: "fixture.unmapped_request".to_string(),
        message: format!(
            "Fixture Manifest does not map request {} {} for Source Profile `{profile_key}` Access Path `{access_path_key}`",
            request.method, request.url
        ),
        severity: DiagnosticSeverity::Error,
        path: "/requests".to_string(),
        strategy_key: None,
        details: Some(json!({
            "profileKey": profile_key,
            "accessPathKey": access_path_key,
            "method": request.method,
            "url": request.url,
        })),
    }
}

fn fixture_execution_failed_diagnostic(
    profile_key: &str,
    access_path_key: &str,
    reference: &str,
    cause: String,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Fixture,
        code: "fixture.execution_failed".to_string(),
        message: format!(
            "Fixture execution failed for Source Profile `{profile_key}` Access Path `{access_path_key}` Fixture Manifest `{reference}`"
        ),
        severity: DiagnosticSeverity::Error,
        path: "".to_string(),
        strategy_key: None,
        details: Some(json!({
            "profileKey": profile_key,
            "accessPathKey": access_path_key,
            "reference": reference,
            "cause": cause,
        })),
    }
}
