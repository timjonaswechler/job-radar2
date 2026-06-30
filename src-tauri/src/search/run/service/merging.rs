use std::collections::HashSet;

use crate::search::{normalization::normalized_text_key, posting::matching::same_job_posting};

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
                    posting_meta: treffer.candidate.posting_meta,
                    ..treffer.source
                }],
            });
        }
    }

    postings
}

fn can_merge(posting: &NormalizedPosting, candidate: &SourceCandidate) -> bool {
    same_job_posting(
        &posting.title,
        &posting.company,
        &posting.locations,
        &candidate.title,
        &candidate.company,
        &candidate.locations,
    )
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
        posting_meta: treffer.candidate.posting_meta,
        ..treffer.source
    };
    if !posting
        .sources
        .iter()
        .any(|existing| same_source_row(existing, &source))
    {
        posting.sources.push(source);
    }
}

fn same_source_row(left: &PostingSource, right: &PostingSource) -> bool {
    left.source_key == right.source_key && left.url == right.url
}
