#[test]
fn desktop_exports_are_the_canonical_core_types() {
    fn accepts_core_source(_: source_profile_dsl::SourceDocument) {}
    fn accepts_core_profile(_: source_profile_dsl::SourceProfileDocument) {}
    fn accepts_core_plan(_: source_profile_dsl::SourceExecutionPlan) {}

    let source: job_radar_lib::SourceDocument = serde_json::from_str(include_str!(
        "fixtures/source-profile-dsl/valid/source-owned-access-path.json"
    ))
    .expect("valid Source fixture");
    let profile: job_radar_lib::SourceProfileDocument = serde_json::from_str(include_str!(
        "fixtures/source-profile-dsl/valid/simple-source-profile.json"
    ))
    .expect("valid Source Profile fixture");
    let plan: Option<job_radar_lib::SourceExecutionPlan> = None;

    accepts_core_source(source);
    accepts_core_profile(profile);
    if let Some(plan) = plan {
        accepts_core_plan(plan);
    }
}
