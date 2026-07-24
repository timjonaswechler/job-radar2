use super::super::SearchRunService;

#[test]
fn search_run_service_entrypoint_accepts_only_typed_service_state() {
    fn typecheck(service: &SearchRunService<'_>) {
        let future = service.run_with_cancellation(1, None);
        drop(future);
    }
    let _ = typecheck as fn(&SearchRunService<'_>);
}
