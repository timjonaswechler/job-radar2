//! Shared candidate normalization for the Suchlauf funnel.
//!
//! Adapters may still perform parser-specific cleanup before returning
//! `SourceCandidate`s. The final generic shape used for matching, exclusion,
//! and Stellenanzeige merging belongs here.

use std::collections::HashSet;

use crate::search::run::SourceCandidate;

pub(crate) fn normalize_source_candidate(candidate: SourceCandidate) -> Option<SourceCandidate> {
    let title = collapse_whitespace(&candidate.title);
    let company = collapse_whitespace(&candidate.company);
    let url = candidate.url.trim().to_string();
    let locations = normalize_locations(candidate.locations);

    if title.is_empty() || company.is_empty() || url.is_empty() {
        return None;
    }

    Some(SourceCandidate {
        title,
        company,
        url,
        locations,
    })
}

pub(crate) fn normalize_locations(locations: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized_locations = Vec::new();

    for location in locations {
        let location = collapse_whitespace(&location);
        if location.is_empty() {
            continue;
        }
        if seen.insert(normalized_location_key(&location)) {
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

fn normalized_location_key(value: &str) -> String {
    normalized_text_key(value)
}
