use job_postings::identity::{merge_unique_locations, same, Comparison};
use search_resolution::Candidate;

use super::sqlite::{Posting, PostingSource};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MergeInput {
    title: String,
    company: String,
    locations: Vec<String>,
    source: PostingSource,
}

/// Sole productive conversion from Candidate Resolution's committed final value into merger input.
pub(super) fn finalized_merge_input(candidate: &Candidate, source_name: &str) -> MergeInput {
    MergeInput {
        title: candidate.title().to_string(),
        company: candidate.company().to_string(),
        locations: candidate.locations().to_vec(),
        source: PostingSource {
            source_key: candidate.source_key().to_string(),
            source_name: source_name.to_string(),
            identity: candidate.identity().clone(),
            provider_url: candidate.url().to_string(),
            posting_meta: candidate.posting_meta().clone(),
        },
    }
}

pub(super) fn merge_postings(inputs: Vec<MergeInput>) -> Vec<Posting> {
    let mut postings = Vec::<Posting>::new();
    for input in inputs {
        if let Some(existing) = postings
            .iter_mut()
            .find(|posting| can_merge(posting, &input))
        {
            merge_into_posting(existing, input);
        } else {
            postings.push(Posting {
                title: input.title,
                company: input.company,
                locations: input.locations,
                sources: vec![input.source],
            });
        }
    }
    postings
}

fn can_merge(posting: &Posting, input: &MergeInput) -> bool {
    same(
        Comparison {
            title: &posting.title,
            company: &posting.company,
            locations: &posting.locations,
        },
        Comparison {
            title: &input.title,
            company: &input.company,
            locations: &input.locations,
        },
    )
}

fn merge_into_posting(posting: &mut Posting, input: MergeInput) {
    posting.locations =
        merge_unique_locations(std::mem::take(&mut posting.locations), &input.locations);
    if !posting
        .sources
        .iter()
        .any(|existing| existing.identity == input.source.identity)
    {
        posting.sources.push(input.source);
    }
}
