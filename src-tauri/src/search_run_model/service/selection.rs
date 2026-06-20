use crate::source_registry::SourceRegistrySnapshot;

use super::{super::SourceExecutionSource, SourceExecutionError};

#[derive(Clone, Debug, PartialEq)]
pub(super) enum SelectedSearchRunSource {
    Resolved(SourceExecutionSource),
    Missing {
        source_key: String,
        error: SourceExecutionError,
    },
}

pub(super) fn resolve_selected_sources(
    snapshot: &SourceRegistrySnapshot,
    source_keys: &[String],
) -> Vec<SelectedSearchRunSource> {
    source_keys
        .iter()
        .map(|source_key| match snapshot.resolve_source(source_key) {
            Ok(source) => SelectedSearchRunSource::Resolved(source),
            Err(message) => SelectedSearchRunSource::Missing {
                source_key: source_key.clone(),
                error: SourceExecutionError::Failed(message),
            },
        })
        .collect()
}
