use search_resolution::{
    merge_unique_locations, same_job_posting, FinalizedCandidate, PostingComparison,
};

use super::super::{NormalizedPosting, PostingSource};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FinalizedMergeInput {
    title: String,
    company: String,
    url: String,
    locations: Vec<String>,
    source: PostingSource,
}

/// Sole productive conversion from Q01's committed final value into merger input.
pub(super) fn finalized_merge_input(
    candidate: &FinalizedCandidate,
    source_name: &str,
) -> FinalizedMergeInput {
    FinalizedMergeInput {
        title: candidate.title().to_string(),
        company: candidate.company().to_string(),
        url: candidate.url().to_string(),
        locations: candidate.locations().to_vec(),
        source: PostingSource {
            source_key: candidate.source_key().to_string(),
            source_name: source_name.to_string(),
            url: candidate.url().to_string(),
        },
    }
}

pub(super) fn merge_postings(inputs: Vec<FinalizedMergeInput>) -> Vec<NormalizedPosting> {
    let mut postings = Vec::<NormalizedPosting>::new();
    for input in inputs {
        if let Some(existing) = postings
            .iter_mut()
            .find(|posting| can_merge(posting, &input))
        {
            merge_into_posting(existing, input);
        } else {
            postings.push(NormalizedPosting {
                title: input.title,
                company: input.company,
                url: input.url,
                locations: input.locations,
                sources: vec![input.source],
            });
        }
    }
    postings
}

fn can_merge(posting: &NormalizedPosting, input: &FinalizedMergeInput) -> bool {
    same_job_posting(
        PostingComparison {
            title: &posting.title,
            company: &posting.company,
            locations: &posting.locations,
        },
        PostingComparison {
            title: &input.title,
            company: &input.company,
            locations: &input.locations,
        },
    )
}

fn merge_into_posting(posting: &mut NormalizedPosting, input: FinalizedMergeInput) {
    posting.locations =
        merge_unique_locations(std::mem::take(&mut posting.locations), &input.locations);
    if !posting.sources.iter().any(|existing| {
        existing.source_key == input.source.source_key && existing.url == input.source.url
    }) {
        posting.sources.push(input.source);
    }
}
