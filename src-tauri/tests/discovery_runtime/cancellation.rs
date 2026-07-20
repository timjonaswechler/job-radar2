use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::Notify;

#[test]
fn discovery_cancellation_interrupts_an_active_http_fetch_without_fallback_failure() {
    block_on(async {
        let plan = compiled_json_discovery_plan(default_fields(), default_select());
        let fetcher = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/jobs.json".to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Gate("active-fetch".to_string())],
            content_length: None,
        }]);
        let browser = FakeBrowser::new([]);
        let cancellation = TestCancellation::default();
        let context = RuntimeExecutionContext::with_cancellation(&cancellation);

        let cancel_after_fetch_starts = async {
            while !fetcher.gate_is_waiting("active-fetch") {
                tokio::task::yield_now().await;
            }
            cancellation.cancel();
        };
        let execute = async {
            tokio::time::timeout(
                std::time::Duration::from_secs(1),
                execute_discovery(&plan, &fetcher, &browser, context),
            )
            .await
            .expect("cancellation should stop the active HTTP fetch promptly")
        };

        let (_, result) = tokio::join!(cancel_after_fetch_starts, execute);

        assert!(result.candidates.is_empty());
        assert_eq!(fetcher.request_count(), 1);
        assert_eq!(
            result
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == "runtime_execution_cancelled")
                .count(),
            1
        );
        assert!(result
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "fetch_failed"));
        assert!(result
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "fallback_exhausted"));
    });
}

#[test]
fn discovery_browser_cancellation_is_distinct_from_runtime_failure() {
    block_on(async {
        let plan = compiled_browser_discovery_plan(
            json!({ "type": "html" }),
            json!({ "type": "css", "selector": "article.posting" }),
            default_html_fields(),
            "https://example.test/jobs",
        );
        let fetcher = fake_fetcher([]);
        let browser = CancellationAwareBrowser::default();
        let cancellation = TestCancellation::default();
        let context = RuntimeExecutionContext::with_cancellation(&cancellation);

        let cancel_after_render_starts = async {
            browser.started.notified().await;
            cancellation.cancel();
        };
        let execute = async {
            tokio::time::timeout(
                std::time::Duration::from_secs(1),
                execute_discovery(&plan, &fetcher, &browser, context),
            )
            .await
            .expect("browser cancellation should be observed promptly at a safe point")
        };

        let (_, result) = tokio::join!(cancel_after_render_starts, execute);

        assert!(result.candidates.is_empty());
        assert_eq!(browser.render_count(), 1);
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "runtime_execution_cancelled"));
        assert!(result.diagnostics.iter().all(|diagnostic| {
            diagnostic.code != "browser_runtime_unavailable"
                && diagnostic.code != "fallback_exhausted"
        }));
    });
}

#[test]
fn discovery_cancellation_stops_page_pagination_before_the_next_request() {
    block_on(async {
        let plan = compiled_discovery_plan_with_strategy(
            json!({ "type": "json" }),
            default_select(),
            default_fields(),
            "https://example.test/jobs.json",
            serde_json::Map::from_iter([(
                "pagination".to_string(),
                json!({
                    "type": "page",
                    "pageParam": "page",
                    "firstPage": 1,
                    "limits": { "maxRequests": 1 }
                }),
            )]),
        );
        let cancellation = TestCancellation::default();
        let fetcher = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/jobs.json?page=1".to_string(),
            headers: Vec::new(),
            body: vec![
                ScriptedHttpBodyEvent::Chunk(b"{\"jobs\":[".to_vec()),
                ScriptedHttpBodyEvent::Gate("first-page-prefix".to_string()),
                ScriptedHttpBodyEvent::Chunk(b"]}".to_vec()),
            ],
            content_length: None,
        }]);
        let browser = FakeBrowser::new([]);

        let cancel = async {
            while !fetcher.gate_is_waiting("first-page-prefix") {
                tokio::task::yield_now().await;
            }
            cancellation.cancel();
        };
        let execute = execute_discovery(
            &plan,
            &fetcher,
            &browser,
            RuntimeExecutionContext::with_cancellation(&cancellation),
        );
        let (_, result) = tokio::join!(cancel, execute);

        assert!(result.candidates.is_empty());
        assert_eq!(fetcher.request_count(), 1);
        assert_eq!(
            result
                .report
                .as_ref()
                .expect("cancelled execution report")
                .usage
                .response_bytes,
            b"{\"jobs\":[".len() as u64
        );
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "runtime_execution_cancelled"));
        assert!(result.diagnostics.iter().all(|diagnostic| {
            diagnostic.code != "fallback_exhausted"
                && diagnostic.code != "pagination_max_requests_reached"
        }));
    });
}

#[derive(Default)]
struct TestCancellation {
    cancelled: AtomicBool,
}

impl TestCancellation {
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

impl RuntimeCancellation for TestCancellation {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

#[derive(Default)]
struct CancellationAwareBrowser {
    started: Arc<Notify>,
    render_count: std::sync::Mutex<usize>,
}

impl CancellationAwareBrowser {
    fn render_count(&self) -> usize {
        *self.render_count.lock().unwrap()
    }
}

impl ProfileBrowserClient for CancellationAwareBrowser {
    fn render<'a>(
        &'a self,
        _request: ProfileBrowserFetchRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProfileBrowserFetchResponse, ProfileBrowserFetchError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(
            async move { panic!("cancellation-aware runtime should call render_with_context") },
        )
    }

    fn render_with_context<'a>(
        &'a self,
        _request: ProfileBrowserFetchRequest,
        context: RuntimeExecutionContext<'a>,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProfileBrowserFetchResponse, ProfileBrowserFetchError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            *self.render_count.lock().unwrap() += 1;
            self.started.notify_one();
            context.cancelled().await;
            Err(ProfileBrowserFetchError::new(
                ProfileBrowserFetchErrorKind::Cancelled,
                "discovery cancelled",
            ))
        })
    }
}
