//! Compiler security checks for Browser behavior.
//! Authored HTTP Fetch security is owned by `primitives::fetch::http`.

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::fetch::BrowserInteraction;
use crate::profile_dsl::documents::{DetailStep, DiscoveryStep, Fetch};

use super::compiler_error;

pub(super) fn validate_security(
    discovery: &DiscoveryStep,
    detail: Option<&DetailStep>,
    base_path: String,
    diagnostics: &mut Diagnostics,
) {
    for (index, strategy) in discovery.strategies.iter().enumerate() {
        validate_browser_security(
            &strategy.fetch,
            &format!("{base_path}/discovery/strategies/{index}/fetch"),
            &strategy.key,
            diagnostics,
        );
    }
    if let Some(detail) = detail {
        for (index, strategy) in detail.strategies.iter().enumerate() {
            validate_browser_security(
                &strategy.fetch,
                &format!("{base_path}/detail/strategies/{index}/fetch"),
                &strategy.key,
                diagnostics,
            );
        }
    }
}

fn validate_browser_security(
    fetch: &Fetch,
    path: &str,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    let Fetch::Browser { interactions, .. } = fetch else {
        return;
    };
    for (index, interaction) in interactions.as_deref().unwrap_or(&[]).iter().enumerate() {
        let behavior = match interaction {
            BrowserInteraction::ExecuteScript { .. } => Some("arbitrary_javascript"),
            BrowserInteraction::Eval { .. } => Some("eval"),
            BrowserInteraction::MutateDom { .. } => Some("dom_mutation"),
            BrowserInteraction::LoginFlow { .. } => Some("login_flow"),
            BrowserInteraction::CaptchaBypass { .. } => Some("captcha_bypass"),
            BrowserInteraction::ClickIfVisible { .. }
            | BrowserInteraction::ClickUntilGone { .. } => None,
        };
        if let Some(behavior) = behavior {
            let mut diagnostic = compiler_error(
                "prohibited_browser_behavior",
                format!("Browser behavior `{behavior}` is prohibited in Source Profiles"),
                format!("{path}/interactions/{index}"),
                serde_json::json!({ "behavior": behavior }),
            );
            diagnostic.strategy_key = Some(strategy_key.to_string());
            diagnostics.push(diagnostic);
        }
    }
}
