//! Compiler security checks for declarative Profile DSL plans.
//!
//! These checks intentionally inspect only declared plan shape. They do not
//! execute network, browser, parser, selector, extractor, transform,
//! pagination, or runtime behavior.

use serde_json::Value;

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::fetch::BrowserInteraction;
use crate::profile_dsl::documents::{Fetch, PostingDetailStep, PostingDiscoveryStep, RequestBody};

use super::compiler_error;

const FORBIDDEN_HEADERS: &[&str] = &[
    "authorization",
    "cookie",
    "set-cookie",
    "x-api-key",
    "proxy-authorization",
];

pub(super) fn validate_security(
    posting_discovery: &PostingDiscoveryStep,
    posting_detail: Option<&PostingDetailStep>,
    base_path: String,
    diagnostics: &mut Diagnostics,
) {
    for (index, strategy) in posting_discovery.strategies.iter().enumerate() {
        validate_fetch_security(
            &strategy.fetch,
            &format!("{base_path}/postingDiscovery/strategies/{index}/fetch"),
            &strategy.key,
            diagnostics,
        );
    }

    if let Some(posting_detail) = posting_detail {
        for (index, strategy) in posting_detail.strategies.iter().enumerate() {
            validate_fetch_security(
                &strategy.fetch,
                &format!("{base_path}/postingDetail/strategies/{index}/fetch"),
                &strategy.key,
                diagnostics,
            );
        }
    }
}

fn validate_fetch_security(
    fetch: &Fetch,
    path: &str,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    match fetch {
        Fetch::Http { headers, body, .. } => {
            if let Some(headers) = headers {
                for header in headers.keys() {
                    if is_forbidden_header(header) {
                        push_security_diagnostic(
                            diagnostics,
                            "forbidden_request_header",
                            format!("HTTP header `{header}` is not allowed in Source Profiles"),
                            &format!("{path}/headers/{}", pointer_escape(header)),
                            strategy_key,
                            serde_json::json!({ "header": header }),
                        );
                    }
                }
            }

            if let Some(body) = body {
                validate_request_body_security(
                    body,
                    &format!("{path}/body"),
                    strategy_key,
                    diagnostics,
                );
            }
        }
        Fetch::Browser { interactions, .. } => {
            if let Some(interactions) = interactions {
                for (index, interaction) in interactions.iter().enumerate() {
                    validate_browser_interaction_security(
                        interaction,
                        &format!("{path}/interactions/{index}"),
                        strategy_key,
                        diagnostics,
                    );
                }
            }
        }
    }
}

fn validate_request_body_security(
    body: &RequestBody,
    path: &str,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    match body {
        RequestBody::Json { value } => {
            for (key, value) in value {
                validate_json_body_field_security(
                    key,
                    value,
                    &format!("{path}/value/{}", pointer_escape(key)),
                    strategy_key,
                    diagnostics,
                );
            }
        }
        RequestBody::Form { fields } => {
            for key in fields.keys() {
                if is_secret_like_key(key) {
                    push_secret_body_field_diagnostic(
                        key,
                        &format!("{path}/fields/{}", pointer_escape(key)),
                        strategy_key,
                        diagnostics,
                    );
                }
            }
        }
        RequestBody::Text { .. } => {}
    }
}

fn validate_json_body_field_security(
    key: &str,
    value: &Value,
    path: &str,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    if is_secret_like_key(key) {
        push_secret_body_field_diagnostic(key, path, strategy_key, diagnostics);
    }

    match value {
        Value::Object(object) => {
            for (child_key, child_value) in object {
                validate_json_body_field_security(
                    child_key,
                    child_value,
                    &format!("{path}/{}", pointer_escape(child_key)),
                    strategy_key,
                    diagnostics,
                );
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                if let Value::Object(object) = value {
                    for (child_key, child_value) in object {
                        validate_json_body_field_security(
                            child_key,
                            child_value,
                            &format!("{path}/{index}/{}", pointer_escape(child_key)),
                            strategy_key,
                            diagnostics,
                        );
                    }
                }
            }
        }
        _ => {}
    }
}

fn validate_browser_interaction_security(
    interaction: &BrowserInteraction,
    path: &str,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    let behavior = match interaction {
        BrowserInteraction::ExecuteScript { .. } => Some("arbitrary_javascript"),
        BrowserInteraction::Eval { .. } => Some("eval"),
        BrowserInteraction::MutateDom { .. } => Some("dom_mutation"),
        BrowserInteraction::LoginFlow { .. } => Some("login_flow"),
        BrowserInteraction::CaptchaBypass { .. } => Some("captcha_bypass"),
        BrowserInteraction::ClickIfVisible { .. } | BrowserInteraction::ClickUntilGone { .. } => {
            None
        }
    };

    if let Some(behavior) = behavior {
        push_security_diagnostic(
            diagnostics,
            "prohibited_browser_behavior",
            format!("Browser behavior `{behavior}` is prohibited in Source Profiles"),
            path,
            strategy_key,
            serde_json::json!({ "behavior": behavior }),
        );
    }
}

fn push_secret_body_field_diagnostic(
    key: &str,
    path: &str,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    push_security_diagnostic(
        diagnostics,
        "secret_like_request_body_field",
        format!("Request body field `{key}` looks like a secret or credential"),
        path,
        strategy_key,
        serde_json::json!({ "field": key }),
    );
}

fn push_security_diagnostic(
    diagnostics: &mut Diagnostics,
    code: &str,
    message: String,
    path: &str,
    strategy_key: &str,
    details: serde_json::Value,
) {
    let mut diagnostic = compiler_error(code, message, path, details);
    diagnostic.strategy_key = Some(strategy_key.to_string());
    diagnostics.push(diagnostic);
}

fn is_forbidden_header(header: &str) -> bool {
    let normalized = header.trim().to_ascii_lowercase();
    FORBIDDEN_HEADERS.contains(&normalized.as_str()) || is_secret_like_key(&normalized)
}

fn is_secret_like_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(|character| character.to_lowercase())
        .collect::<String>();

    matches!(
        normalized.as_str(),
        "password" | "token" | "apikey" | "auth" | "session" | "credential"
    ) || normalized.contains("password")
        || normalized.contains("token")
        || normalized.contains("apikey")
        || normalized.contains("auth")
        || normalized.contains("session")
        || normalized.contains("credential")
}

fn pointer_escape(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}
