use std::collections::HashSet;

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::{DetailStep, DiscoveryStep};
use crate::source_profile::documents::SourceProfileDocument;

use super::compiler_error;

pub(super) fn validate_reusable_access_path_keys(
    profile: &SourceProfileDocument,
    diagnostics: &mut Diagnostics,
) {
    let mut seen = HashSet::new();
    for (index, access_path) in profile.access_paths.iter().enumerate() {
        if !seen.insert(access_path.key.as_str()) {
            diagnostics.push(compiler_error(
                "duplicate_access_path_key",
                format!(
                    "Source Profile `{}` declares duplicate Access Path key `{}`",
                    profile.key, access_path.key
                ),
                format!("/accessPaths/{index}/key"),
                serde_json::json!({
                    "sourceProfileKey": profile.key,
                    "accessPathKey": access_path.key,
                }),
            ));
        }
    }
}

pub(super) fn validate_discovery_strategy_keys(
    step: &DiscoveryStep,
    step_path: String,
    diagnostics: &mut Diagnostics,
) {
    let mut seen = HashSet::new();
    for (index, strategy) in step.strategies.iter().enumerate() {
        if !seen.insert(strategy.key.as_str()) {
            let mut diagnostic = compiler_error(
                "duplicate_strategy_key",
                format!(
                    "postingDiscovery declares duplicate Strategy key `{}`",
                    strategy.key
                ),
                format!("{step_path}/strategies/{index}/key"),
                serde_json::json!({
                    "step": "postingDiscovery",
                    "strategyKey": strategy.key,
                }),
            );
            diagnostic.strategy_key = Some(strategy.key.clone());
            diagnostics.push(diagnostic);
        }
    }
}

pub(super) fn validate_detail_strategy_keys(
    step: &DetailStep,
    step_path: String,
    diagnostics: &mut Diagnostics,
) {
    let mut seen = HashSet::new();
    for (index, strategy) in step.strategies.iter().enumerate() {
        if !seen.insert(strategy.key.as_str()) {
            let mut diagnostic = compiler_error(
                "duplicate_strategy_key",
                format!(
                    "postingDetail declares duplicate Strategy key `{}`",
                    strategy.key
                ),
                format!("{step_path}/strategies/{index}/key"),
                serde_json::json!({
                    "step": "postingDetail",
                    "strategyKey": strategy.key,
                }),
            );
            diagnostic.strategy_key = Some(strategy.key.clone());
            diagnostics.push(diagnostic);
        }
    }
}

pub(super) fn access_path_index(profile: &SourceProfileDocument, key: &str) -> Option<usize> {
    profile
        .access_paths
        .iter()
        .position(|access_path| access_path.key == key)
}
