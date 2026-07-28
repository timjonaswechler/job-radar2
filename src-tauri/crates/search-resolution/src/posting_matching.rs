use std::collections::HashSet;

use crate::{
    comparison::{comparison_key, comparison_tokens, token_containment, token_jaccard},
    normalization::normalized_text_key,
};

const TITLE_CONTAINMENT_THRESHOLD: f64 = 0.90;
const TITLE_JACCARD_THRESHOLD: f64 = 0.55;

#[derive(Clone, Copy, Debug)]
pub struct PostingComparison<'a> {
    pub title: &'a str,
    pub company: &'a str,
    pub locations: &'a [String],
}

pub fn same_job_posting(existing: PostingComparison<'_>, candidate: PostingComparison<'_>) -> bool {
    if comparison_key(existing.company) != comparison_key(candidate.company) {
        return false;
    }

    if !titles_compatible(existing.title, candidate.title) {
        return false;
    }

    if existing.locations.is_empty() || candidate.locations.is_empty() {
        return true;
    }

    locations_compatible(existing.locations, candidate.locations)
}

/// Extends finalized locations without adding canonical duplicates.
pub fn merge_unique_locations(mut existing: Vec<String>, incoming: &[String]) -> Vec<String> {
    let mut keys = existing
        .iter()
        .map(|location| normalized_text_key(location))
        .collect::<HashSet<_>>();
    for location in incoming {
        if keys.insert(normalized_text_key(location)) {
            existing.push(location.clone());
        }
    }
    existing
}

fn titles_compatible(existing_title: &str, candidate_title: &str) -> bool {
    let existing_tokens = comparison_tokens(existing_title);
    let candidate_tokens = comparison_tokens(candidate_title);

    token_containment(&existing_tokens, &candidate_tokens) >= TITLE_CONTAINMENT_THRESHOLD
        && token_jaccard(&existing_tokens, &candidate_tokens) >= TITLE_JACCARD_THRESHOLD
}

fn locations_compatible(existing_locations: &[String], candidate_locations: &[String]) -> bool {
    existing_locations.iter().any(|existing_location| {
        candidate_locations
            .iter()
            .any(|candidate_location| location_compatible(existing_location, candidate_location))
    })
}

fn location_compatible(existing_location: &str, candidate_location: &str) -> bool {
    if comparison_key(existing_location) == comparison_key(candidate_location) {
        return true;
    }

    let existing_tokens = comparison_tokens(existing_location);
    let candidate_tokens = comparison_tokens(candidate_location);
    location_token_prefix_matches(&existing_tokens, &candidate_tokens)
}

fn location_token_prefix_matches(existing_tokens: &[String], candidate_tokens: &[String]) -> bool {
    if existing_tokens.is_empty() || candidate_tokens.is_empty() {
        return false;
    }

    let (shorter, longer) = if existing_tokens.len() <= candidate_tokens.len() {
        (existing_tokens, candidate_tokens)
    } else {
        (candidate_tokens, existing_tokens)
    };

    longer.starts_with(shorter)
}
