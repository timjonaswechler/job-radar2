mod comparison;

use std::{collections::BTreeSet, fmt};

use comparison::{containment, jaccard, key, normalized_text_key, tokens};

const TITLE_CONTAINMENT_THRESHOLD: f64 = 0.90;
const TITLE_JACCARD_THRESHOLD: f64 = 0.55;

#[derive(Clone, Copy, Debug)]
pub struct Comparison<'a> {
    pub title: &'a str,
    pub company: &'a str,
    pub locations: &'a [String],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Conflict {
    posting_ids: Vec<i64>,
}

impl Conflict {
    pub fn posting_ids(&self) -> &[i64] {
        &self.posting_ids
    }
}

impl fmt::Display for Conflict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "exact Posting Occurrence identities resolve to different Job Postings: {:?}",
            self.posting_ids
        )
    }
}

impl std::error::Error for Conflict {}

/// Chooses a durable Job Posting only after all exact identity hits are collected.
pub fn decide(exact_ids: &[i64], semantic_ids: &[i64]) -> Result<Option<i64>, Conflict> {
    let exact = exact_ids.iter().copied().collect::<BTreeSet<_>>();
    if exact.len() > 1 {
        return Err(Conflict {
            posting_ids: exact.into_iter().collect(),
        });
    }
    Ok(exact
        .into_iter()
        .next()
        .or_else(|| semantic_ids.iter().copied().min()))
}

pub fn same(existing: Comparison<'_>, candidate: Comparison<'_>) -> bool {
    if key(existing.company) != key(candidate.company) {
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
        .collect::<BTreeSet<_>>();
    for location in incoming {
        if keys.insert(normalized_text_key(location)) {
            existing.push(location.clone());
        }
    }
    existing
}

fn titles_compatible(existing_title: &str, candidate_title: &str) -> bool {
    let existing_tokens = tokens(existing_title);
    let candidate_tokens = tokens(candidate_title);
    containment(&existing_tokens, &candidate_tokens) >= TITLE_CONTAINMENT_THRESHOLD
        && jaccard(&existing_tokens, &candidate_tokens) >= TITLE_JACCARD_THRESHOLD
}

fn locations_compatible(existing_locations: &[String], candidate_locations: &[String]) -> bool {
    existing_locations.iter().any(|existing| {
        candidate_locations
            .iter()
            .any(|candidate| location_compatible(existing, candidate))
    })
}

fn location_compatible(existing: &str, candidate: &str) -> bool {
    if key(existing) == key(candidate) {
        return true;
    }
    let existing = tokens(existing);
    let candidate = tokens(candidate);
    if existing.is_empty() || candidate.is_empty() {
        return false;
    }
    let (shorter, longer) = if existing.len() <= candidate.len() {
        (&existing, &candidate)
    } else {
        (&candidate, &existing)
    };
    longer.starts_with(shorter)
}
