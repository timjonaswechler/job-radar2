use job_radar_lib::{
    compile_source, CompileSourceOutcome, RegistrySourceProfile, SourceDocument,
    SourceExecutionPlan, SourceProfileDocument, SourceProfileRegistrySnapshot,
};

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
