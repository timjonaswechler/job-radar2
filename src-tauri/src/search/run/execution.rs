use std::path::PathBuf;

use crate::{
    background_tasks::CancellationToken,
    browser_runtime::ManagedBrowserAcquisition,
    profile_dsl::{
        compiler::CompiledSource,
        documents::PhaseLimits,
        runtime::{
            ProfileDslSourceDetailExecution, ReqwestProfileHttpClient, RuntimeCancellation,
            SourceDetailExecution,
        },
    },
    search::candidate_resolution::{
        resolve_source_candidates, CompiledSearchRequirements, ResolutionCeilings, SourceDiscovery,
        SourceResolution, SourceResolutionError, SourceResolutionRequest,
    },
};

/// Sealed Search Run runtime. Both productive and deterministic modes enter Q01 through the
/// single `resolve_with_dependencies` call below; callers cannot inject a prebuilt Resolution.
pub struct SearchRunResolutionRuntime {
    mode: ResolutionRuntimeMode,
}

enum ResolutionRuntimeMode {
    Production {
        browser_runtime_dir: PathBuf,
    },
    #[cfg(test)]
    Scripted(std::collections::HashMap<String, ScriptedResolutionSource>),
}

#[cfg(test)]
pub(crate) struct ScriptedResolutionSource {
    pub(crate) discovery: crate::search::candidate_resolution::ScriptedSourceDiscoveryExecution,
    pub(crate) detail: crate::profile_dsl::runtime::ScriptedSourceDetailExecution,
}

impl SearchRunResolutionRuntime {
    pub fn production(browser_runtime_dir: impl Into<PathBuf>) -> Self {
        Self {
            mode: ResolutionRuntimeMode::Production {
                browser_runtime_dir: browser_runtime_dir.into(),
            },
        }
    }

    #[cfg(test)]
    pub(crate) fn scripted(
        sources: impl IntoIterator<Item = (String, ScriptedResolutionSource)>,
    ) -> Self {
        Self {
            mode: ResolutionRuntimeMode::Scripted(sources.into_iter().collect()),
        }
    }

    pub(crate) async fn resolve(
        &self,
        compiled_source: &CompiledSource,
        requirements: &CompiledSearchRequirements<'_>,
        cancellation: &dyn RuntimeCancellation,
    ) -> Result<SourceResolution, SourceResolutionError> {
        match &self.mode {
            ResolutionRuntimeMode::Production {
                browser_runtime_dir,
            } => {
                let fetcher = ReqwestProfileHttpClient::new();
                let acquisition = ManagedBrowserAcquisition::new(browser_runtime_dir.clone());
                let detail = ProfileDslSourceDetailExecution::new(&fetcher, &acquisition);
                resolve_with_dependencies(
                    compiled_source,
                    requirements,
                    cancellation,
                    SourceDiscovery::profile_dsl(&fetcher, &acquisition),
                    &detail,
                )
                .await
            }
            #[cfg(test)]
            ResolutionRuntimeMode::Scripted(sources) => {
                let key = &compiled_source.execution_plan.source.key;
                let Some(source) = sources.get(key) else {
                    return Err(SourceResolutionError::Failed {
                        failure: crate::search::candidate_resolution::ResolutionFailure::DiscoveryExecution,
                        diagnostics: Vec::new(),
                    });
                };
                resolve_with_dependencies(
                    compiled_source,
                    requirements,
                    cancellation,
                    SourceDiscovery::scripted(&source.discovery),
                    &source.detail,
                )
                .await
            }
        }
    }
}

async fn resolve_with_dependencies(
    compiled_source: &CompiledSource,
    requirements: &CompiledSearchRequirements<'_>,
    cancellation: &dyn RuntimeCancellation,
    discovery: SourceDiscovery<'_>,
    detail: &dyn SourceDetailExecution,
) -> Result<SourceResolution, SourceResolutionError> {
    resolve_source_candidates(SourceResolutionRequest {
        compiled_source,
        requirements,
        ceilings: production_resolution_ceilings(),
        cancellation,
        discovery,
        detail,
    })
    .await
}

pub(crate) fn production_resolution_ceilings() -> ResolutionCeilings {
    let phase = PhaseLimits::BACKEND;
    ResolutionCeilings {
        max_batch_size: phase.max_produced_items,
        max_discovery_batches: phase.max_pages,
        max_discovered_items: phase.max_produced_items,
        max_detail_candidates: phase.max_fan_out,
        phase,
    }
}

pub(crate) struct NeverCancelled;
impl RuntimeCancellation for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}

pub(crate) fn cancellation_or_default<'a>(
    token: Option<&'a CancellationToken>,
    fallback: &'a NeverCancelled,
) -> &'a dyn RuntimeCancellation {
    token.map_or(fallback as &dyn RuntimeCancellation, |token| token)
}
