//! Shared normalization primitives used by Candidate Resolution and posting matching.

use std::collections::HashSet;

pub(crate) fn normalize_locations(locations: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized_locations = Vec::new();

    for location in locations {
        let location = collapse_whitespace(&location);
        if location.is_empty() {
            continue;
        }
        if seen.insert(normalized_text_key(&location)) {
            normalized_locations.push(location);
        }
    }

    normalized_locations
}

pub(crate) fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn normalized_text_key(value: &str) -> String {
    collapse_whitespace(value).to_lowercase()
}
