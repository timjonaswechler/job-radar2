use std::collections::HashSet;

use crate::search::{
    comparison::{comparison_key, comparison_tokens, token_containment, token_jaccard},
    normalization::normalized_text_key,
};

use super::super::{NormalizedPosting, PostingSource, SourceCandidate};

const TITLE_CONTAINMENT_THRESHOLD: f64 = 0.90;
const TITLE_JACCARD_THRESHOLD: f64 = 0.55;

#[derive(Clone, Debug, Eq, PartialEq)]
/// Treffer candidate that matched a Suchanfrage before final Stellenanzeige merging.
pub(super) struct Treffer {
    pub(super) candidate: SourceCandidate,
    pub(super) source: PostingSource,
}

pub(super) fn merge_postings(treffers: Vec<Treffer>) -> Vec<NormalizedPosting> {
    let mut postings = Vec::<NormalizedPosting>::new();

    for treffer in treffers {
        if let Some(existing) = postings
            .iter_mut()
            .find(|posting| can_merge(posting, &treffer.candidate))
        {
            merge_into_posting(existing, treffer);
        } else {
            postings.push(NormalizedPosting {
                title: treffer.candidate.title,
                company: treffer.candidate.company,
                url: treffer.candidate.url.clone(),
                locations: treffer.candidate.locations,
                sources: vec![PostingSource {
                    url: treffer.candidate.url,
                    ..treffer.source
                }],
            });
        }
    }

    postings
}

fn can_merge(posting: &NormalizedPosting, candidate: &SourceCandidate) -> bool {
    if comparison_key(&posting.company) != comparison_key(&candidate.company) {
        return false;
    }

    if !titles_compatible(&posting.title, &candidate.title) {
        return false;
    }

    if posting.locations.is_empty() || candidate.locations.is_empty() {
        return true;
    }

    locations_compatible(&posting.locations, &candidate.locations)
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

fn merge_into_posting(posting: &mut NormalizedPosting, treffer: Treffer) {
    let mut existing_location_keys = posting
        .locations
        .iter()
        .map(|location| normalized_text_key(location))
        .collect::<HashSet<_>>();
    for location in treffer.candidate.locations {
        if existing_location_keys.insert(normalized_text_key(&location)) {
            posting.locations.push(location);
        }
    }

    let source = PostingSource {
        url: treffer.candidate.url,
        ..treffer.source
    };
    if !posting.sources.iter().any(|existing| existing == &source) {
        posting.sources.push(source);
    }
}
