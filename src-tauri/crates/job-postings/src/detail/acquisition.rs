use super::{diagnostics, Description};
use crate::catalog::{Posting, Source};
use source_engine::execution::{
    DetailField, PostingOccurrence, RequestedDetailFields, RequestedFieldDisposition,
    RuntimeExecutionContext, SourceDetailExecution, SourceDetailOutcome, SourceDetailRequest,
};
use sources::installed::Snapshot;

pub(super) async fn load<E>(posting: &Posting, snapshot: &Snapshot, execution: &E) -> Description
where
    E: SourceDetailExecution + ?Sized,
{
    let mut collected = Vec::new();
    let mut attempted = false;

    for source in ordered_sources(posting) {
        let Some(installed) = snapshot.source(&source.source_key) else {
            diagnostics::push(
                &mut collected,
                diagnostics::source(
                    source,
                    "source_not_found",
                    format!(
                        "Persisted posting source `{}` was not found in the Source Profile registry snapshot",
                        source.source_key
                    ),
                    "",
                    serde_json::json!({ "sourceKey": source.source_key }),
                ),
            );
            continue;
        };
        if !installed.validation().can_compile {
            diagnostics::append(
                &mut collected,
                diagnostics::contextualize(installed.validation().diagnostics.clone(), source),
            );
            continue;
        }
        let Some(compiled) = installed.compiled() else {
            diagnostics::append(
                &mut collected,
                diagnostics::contextualize(installed.preparation_diagnostics().to_vec(), source),
            );
            continue;
        };
        diagnostics::append(
            &mut collected,
            diagnostics::contextualize(installed.preparation_diagnostics().to_vec(), source),
        );
        if !compiled
            .detail_capabilities()
            .contains(DetailField::DescriptionText)
        {
            diagnostics::push(
                &mut collected,
                diagnostics::source(
                    source,
                    "detail_missing",
                    format!(
                        "Source `{}` does not provide descriptionText Detail capability",
                        installed.document().key
                    ),
                    "/detail",
                    serde_json::json!({ "sourceKey": installed.document().key }),
                ),
            );
            continue;
        }
        let occurrence = match posting_occurrence(posting, source) {
            Ok(occurrence) => occurrence,
            Err(message) => {
                diagnostics::push(
                    &mut collected,
                    diagnostics::source(
                        source,
                        "posting_occurrence_invalid",
                        message.to_string(),
                        "",
                        serde_json::json!({}),
                    ),
                );
                continue;
            }
        };
        attempted = true;
        let result = execution
            .execute(SourceDetailRequest {
                compiled_source: compiled,
                occurrence: &occurrence,
                requested_fields: RequestedDetailFields::description_text(),
                context: RuntimeExecutionContext::uncancellable(),
            })
            .await;
        let completed_without_description = matches!(
            &result,
            Ok(SourceDetailOutcome::Completed { dispositions, .. })
                if dispositions.iter().any(|disposition| matches!(
                    disposition,
                    RequestedFieldDisposition::Unavailable {
                        field: DetailField::DescriptionText
                    }
                ))
        );
        let (description, evidence) = match result {
            Ok(SourceDetailOutcome::Completed {
                fields,
                dispositions,
                phase_evidence,
            }) => {
                let usable = dispositions.iter().any(|disposition| {
                    matches!(
                        disposition,
                        RequestedFieldDisposition::Reused {
                            field: DetailField::DescriptionText
                        } | RequestedFieldDisposition::Produced {
                            field: DetailField::DescriptionText
                        }
                    )
                });
                (
                    usable.then_some(fields.description_text).flatten(),
                    phase_evidence
                        .map(|evidence| evidence.diagnostics)
                        .unwrap_or_default(),
                )
            }
            Ok(outcome) => (None, outcome.diagnostics().cloned().unwrap_or_default()),
            Err(cancelled) => (None, cancelled.diagnostics),
        };
        diagnostics::append(&mut collected, diagnostics::contextualize(evidence, source));
        if let Some(text) = description {
            return Description::Loaded {
                text,
                diagnostics: collected,
            };
        }
        if completed_without_description {
            diagnostics::push(
                &mut collected,
                diagnostics::source(
                    source,
                    "description_empty",
                    "Completed Source Detail did not provide the requested descriptionText",
                    "/detail/fields/descriptionText",
                    serde_json::json!({}),
                ),
            );
        }
    }

    if attempted {
        Description::Failed {
            message: diagnostics::summary(
                &collected,
                "description loading failed for all detail-capable persisted posting sources",
            ),
            diagnostics: collected,
        }
    } else {
        if collected.is_empty() {
            collected.push(diagnostics::posting(
                posting.id.get(),
                "detail_source_missing",
                format!(
                    "Job Posting {} has no persisted posting source that can provide compiled Detail",
                    posting.id.get()
                ),
            ));
        }
        Description::Unsupported {
            message: diagnostics::summary(
                &collected,
                "job posting has no persisted posting source that can provide compiled Detail",
            ),
            diagnostics: collected,
        }
    }
}

fn ordered_sources(posting: &Posting) -> Vec<&Source> {
    let mut sources = Vec::with_capacity(posting.sources.len());
    sources.push(&posting.primary_source);
    sources.extend(
        posting
            .sources
            .iter()
            .filter(|source| source.id != posting.primary_source.id),
    );
    sources
}

struct OccurrenceError(String);

impl std::fmt::Display for OccurrenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn posting_occurrence(
    posting: &Posting,
    source: &Source,
) -> Result<PostingOccurrence, OccurrenceError> {
    let provider_posting_id = match source.identity_kind.as_str() {
        "provider_posting_id" => Some(source.identity_value.clone()),
        "normalized_url" => None,
        kind => {
            return Err(OccurrenceError(format!(
                "persisted posting source has invalid identity kind: {kind}"
            )))
        }
    };
    let (reference, identity) = source_engine::execution::validate_posting_reference(
        &source.source_key,
        &source.url,
        provider_posting_id,
    )
    .map_err(|_| {
        OccurrenceError("persisted posting source has an invalid provider reference".to_string())
    })?;
    let persisted_identity = match source.identity_kind.as_str() {
        "provider_posting_id" => {
            source_engine::execution::PostingOccurrenceIdentity::ProviderPostingId {
                source_key: source.source_key.clone(),
                provider_posting_id: source.identity_value.clone(),
            }
        }
        "normalized_url" => source_engine::execution::PostingOccurrenceIdentity::NormalizedUrl {
            source_key: source.source_key.clone(),
            normalized_url: source.identity_value.clone(),
        },
        _ => unreachable!("identity kind checked above"),
    };
    if identity != persisted_identity {
        return Err(OccurrenceError(
            "persisted posting source identity does not match its provider reference".to_string(),
        ));
    }
    let posting_meta = serde_json::from_str(&source.posting_meta_json).map_err(|error| {
        OccurrenceError(format!(
            "persisted posting source has invalid postingMeta: {error}"
        ))
    })?;
    Ok(PostingOccurrence {
        identity,
        reference,
        provider_values: source_engine::execution::ProviderValues {
            title: Some(posting.title.clone()),
            company: Some(posting.company.clone()),
            locations: posting.locations.clone(),
            description_text: posting.description_text.clone(),
        },
        hints: Default::default(),
        posting_meta,
    })
}
