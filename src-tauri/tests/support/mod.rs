use job_radar_lib::{
    compile_source, execute_detail, execute_discovery, CompileSourceOutcome, DetailExecutionResult,
    DetailPostingOccurrence, DiscoveryExecutionResult, ProfileHttpClient, RegistrySourceProfile,
    RuntimeExecutionContext, SourceDocument, SourceExecutionPlan, SourceProfileDocument,
    SourceProfileRegistrySnapshot, UnavailableProfileBrowserClient,
};

#[allow(dead_code)]
pub async fn execute_discovery_test<F>(
    plan: &SourceExecutionPlan,
    fetcher: &F,
) -> DiscoveryExecutionResult
where
    F: ProfileHttpClient + Sync + ?Sized,
{
    execute_discovery(
        plan,
        fetcher,
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::uncancellable(),
    )
    .await
}

#[allow(dead_code)]
pub async fn execute_detail_test<F>(
    plan: &SourceExecutionPlan,
    posting: &DetailPostingOccurrence,
    fetcher: &F,
) -> DetailExecutionResult
where
    F: ProfileHttpClient + Sync + ?Sized,
{
    execute_detail(
        plan,
        posting,
        fetcher,
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::uncancellable(),
    )
    .await
}

pub fn compile_test_source(
    source: &SourceDocument,
    profile: Option<SourceProfileDocument>,
) -> CompileSourceOutcome {
    let registry = SourceProfileRegistrySnapshot {
        profiles: profile
            .into_iter()
            .map(|document| RegistrySourceProfile {
                origin: "test".into(),
                path: String::new(),
                document,
            })
            .collect(),
        sources: Vec::new(),
        diagnostics: Vec::new(),
    };
    compile_source(source, &registry)
}

pub fn unwrap_plan(outcome: CompileSourceOutcome) -> SourceExecutionPlan {
    match outcome {
        CompileSourceOutcome::Compiled {
            source,
            diagnostics,
        } if diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != job_radar_lib::DiagnosticSeverity::Error) =>
        {
            source.execution_plan
        }
        other => panic!("expected compiled Source, got {other:?}"),
    }
}
