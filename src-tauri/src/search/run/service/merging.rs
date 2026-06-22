use std::collections::HashSet;

use crate::search::normalization::normalized_text_key;

use super::super::{NormalizedPosting, PostingSource, SourceCandidate};

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
    if normalized_text_key(&posting.company) != normalized_text_key(&candidate.company)
        || normalized_text_key(&posting.title) != normalized_text_key(&candidate.title)
    {
        return false;
    }

    if posting.locations.is_empty() || candidate.locations.is_empty() {
        return true;
    }

    let existing_location_keys = posting
        .locations
        .iter()
        .map(|location| normalized_text_key(location))
        .collect::<HashSet<_>>();
    candidate
        .locations
        .iter()
        .any(|location| existing_location_keys.contains(&normalized_text_key(location)))
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
