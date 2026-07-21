use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::Arc,
    thread,
    time::Duration,
};

use job_radar_lib::{
    HttpMethod, PhaseLimits, ProfileHttpClient, ProfileHttpFailureKind, ProfileHttpRequest,
    ReqwestProfileHttpClient, RuntimeExecutionContext, ScriptedHttpBodyEvent, ScriptedHttpEvent,
    ScriptedProfileHttpClient,
};

fn request(url: String, authored_charset: Option<&str>) -> ProfileHttpRequest {
    ProfileHttpRequest {
        method: HttpMethod::Get,
        url,
        headers: Vec::new(),
        body: None,
        timeout_ms: 5_000,
        authored_charset: authored_charset.map(ToString::to_string),
    }
}

#[test]
fn scripted_adapter_preserves_bytes_metadata_and_strictly_decodes_declared_charset() {
    let client = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 418,
        final_url: "https://example.test/final?secret=hidden".to_string(),
        headers: vec![
            (
                "Content-Type".to_string(),
                b"text/plain; charset=windows-1252".to_vec(),
            ),
            ("X-Repeat".to_string(), b"first".to_vec()),
            ("X-Repeat".to_string(), vec![0xff]),
        ],
        body: vec![ScriptedHttpBodyEvent::Chunk(vec![0x80])],
        content_length: None,
    }]);

    let response = block_on(client.fetch(
        request("https://example.test/start".to_string(), Some("cp1252")),
        RuntimeExecutionContext::uncancellable(),
    ))
    .unwrap_or_else(|_| panic!("aliases of the same WHATWG encoding should decode strictly"));

    assert_eq!(response.status(), 418);
    assert_eq!(response.content_type(), Some("text/plain"));
    assert_eq!(response.raw_body(), &[0x80]);
    assert_eq!(response.body, "€");
    let repeated = response
        .headers()
        .iter()
        .filter(|h| h.name() == "x-repeat")
        .map(|h| h.value())
        .collect::<Vec<_>>();
    assert_eq!(repeated, vec![b"first".as_slice(), &[0xff]]);
    assert_eq!(client.request_count(), 1);
}

#[test]
fn scripted_adapter_handles_bom_precedence_aliases_and_strict_eof() {
    let utf16 = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/".to_string(),
        headers: vec![(
            "content-type".to_string(),
            b"text/plain; charset=unicodefeff".to_vec(),
        )],
        body: vec![ScriptedHttpBodyEvent::Chunk(vec![
            0xff, 0xfe, b'O', 0, b'K', 0,
        ])],
        content_length: None,
    }]);
    let response = block_on(utf16.fetch(
        request("https://example.test/".to_string(), Some("utf-16le")),
        RuntimeExecutionContext::uncancellable(),
    ))
    .unwrap_or_else(|_| panic!("canonical aliases and a compatible BOM must agree"));
    assert_eq!(response.body, "OK");
    assert_eq!(response.raw_body(), &[0xff, 0xfe, b'O', 0, b'K', 0]);

    let incomplete = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/".to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Chunk(vec![0xff, 0xfe, b'O'])],
        content_length: None,
    }]);
    let error = block_on(incomplete.fetch(
        request("https://example.test/".to_string(), None),
        RuntimeExecutionContext::uncancellable(),
    ))
    .err()
    .expect("incomplete UTF-16 input must fail without replacement text");
    assert_eq!(error.kind, ProfileHttpFailureKind::MalformedText);
    assert_eq!(error.admitted_bytes, 3);
}

#[test]
fn scripted_adapter_validates_every_charset_declaration_and_rejects_malformed_syntax() {
    for header in [
        b"text/plain; charset=unsupported-secret-label".to_vec(),
        b"text/plain; charset".to_vec(),
        b"text/plain; charset:utf-16le".to_vec(),
        b"text/plain; charset = \"utf-8".to_vec(),
    ] {
        let client = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/".to_string(),
            headers: vec![("content-type".to_string(), header)],
            body: vec![ScriptedHttpBodyEvent::Chunk(b"ok".to_vec())],
            content_length: None,
        }]);
        let error = block_on(client.fetch(
            request("https://example.test/".to_string(), Some("utf-8")),
            RuntimeExecutionContext::uncancellable(),
        ))
        .err()
        .expect("every malformed or unsupported lower-precedence declaration must fail");
        assert_eq!(error.kind, ProfileHttpFailureKind::InvalidCharset);
    }

    let bom_conflict = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/".to_string(),
        headers: vec![(
            "content-type".to_string(),
            b"text/plain; charset=utf-16le".to_vec(),
        )],
        body: vec![ScriptedHttpBodyEvent::Chunk(
            b"\xef\xbb\xbfconflict".to_vec(),
        )],
        content_length: None,
    }]);
    let error = block_on(bom_conflict.fetch(
        request("https://example.test/".to_string(), None),
        RuntimeExecutionContext::uncancellable(),
    ))
    .err()
    .expect("conflicting BOM and HTTP declarations must fail");
    assert_eq!(error.kind, ProfileHttpFailureKind::InvalidCharset);
}

#[test]
fn scripted_adapter_rejects_conflicts_malformed_text_and_known_oversize_without_text() {
    let conflict = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/".to_string(),
        headers: vec![(
            "content-type".to_string(),
            b"text/plain; charset=utf-16le".to_vec(),
        )],
        body: vec![ScriptedHttpBodyEvent::Chunk(b"ok".to_vec())],
        content_length: None,
    }]);
    let error = block_on(conflict.fetch(
        request("https://example.test/".to_string(), Some("utf-8")),
        RuntimeExecutionContext::uncancellable(),
    ))
    .err()
    .expect("conflicting declarations fail");
    assert_eq!(error.kind, ProfileHttpFailureKind::InvalidCharset);

    let malformed = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/".to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Chunk(vec![0xff])],
        content_length: None,
    }]);
    let error = block_on(malformed.fetch(
        request("https://example.test/".to_string(), None),
        RuntimeExecutionContext::uncancellable(),
    ))
    .err()
    .expect("malformed UTF-8 fails without replacement");
    assert_eq!(error.kind, ProfileHttpFailureKind::MalformedText);

    let oversized = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/".to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Chunk(
            b"must not be admitted".to_vec(),
        )],
        content_length: Some(PhaseLimits::BACKEND.max_response_bytes + 1),
    }]);
    let error = block_on(oversized.fetch(
        request("https://example.test/".to_string(), None),
        RuntimeExecutionContext::uncancellable(),
    ))
    .err()
    .expect("trusted oversized length is rejected");
    assert_eq!(error.kind, ProfileHttpFailureKind::ResponseBytesExceeded);
    assert_eq!(error.admitted_bytes, 0);
}

#[test]
fn scripted_adapter_validates_order_waits_on_named_gate_and_enforces_failure_prefix() {
    let expected = request("https://example.test/ordered".to_string(), None);
    let client = Arc::new(ScriptedProfileHttpClient::expecting(
        [expected],
        [ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/ordered".to_string(),
            headers: Vec::new(),
            body: vec![
                ScriptedHttpBodyEvent::Gate("body-ready".to_string()),
                ScriptedHttpBodyEvent::Chunk(b"ok".to_vec()),
            ],
            content_length: Some(1), // deliberately inaccurate low evidence
        }],
    ));
    let releaser = Arc::clone(&client);
    let release = thread::spawn(move || {
        for _ in 0..100 {
            if releaser.release_gate("body-ready") {
                return;
            }
            thread::sleep(Duration::from_millis(1));
        }
        panic!("scripted gate was not registered");
    });
    let response = block_on(client.fetch(
        request("https://example.test/ordered".to_string(), None),
        RuntimeExecutionContext::uncancellable(),
    ))
    .unwrap_or_else(|_| panic!("released scripted response should complete"));
    release.join().unwrap();
    assert_eq!(response.body, "ok");

    let mismatch = ScriptedProfileHttpClient::expecting(
        [request("https://example.test/expected".to_string(), None)],
        [ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/expected".to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Chunk(b"ignored".to_vec())],
            content_length: None,
        }],
    );
    let error = block_on(mismatch.fetch(
        request(
            "https://example.test/unexpected?secret=redacted".to_string(),
            None,
        ),
        RuntimeExecutionContext::uncancellable(),
    ))
    .err()
    .expect("ordered request mismatch fails safely");
    assert_eq!(error.kind, ProfileHttpFailureKind::InvalidRequest);

    let failure = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/failure".to_string(),
        headers: Vec::new(),
        body: vec![
            ScriptedHttpBodyEvent::Chunk(b"prefix".to_vec()),
            ScriptedHttpBodyEvent::Failure(ProfileHttpFailureKind::BodyStream),
        ],
        content_length: None,
    }]);
    let error = block_on(failure.fetch(
        request("https://example.test/failure".to_string(), None),
        RuntimeExecutionContext::uncancellable(),
    ))
    .err()
    .expect("scripted stream failure is typed");
    assert_eq!(error.kind, ProfileHttpFailureKind::BodyStream);
    assert_eq!(error.admitted_bytes, 6);
}

#[test]
fn reqwest_adapter_preserves_redirect_non_success_repeated_raw_headers_and_exact_bytes() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        for index in 0..2 {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let read = socket.read(&mut request).unwrap();
            let line = String::from_utf8_lossy(&request[..read]);
            if index == 0 {
                assert!(line.starts_with("GET /start "));
                write!(socket, "HTTP/1.1 302 Found\r\nLocation: http://{address}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
            } else {
                assert!(line.starts_with("GET /final "));
                let mut response = b"HTTP/1.1 418 Teapot\r\nContent-Type: text/plain; charset=windows-1252\r\nX-Repeat: first\r\nX-Repeat: ".to_vec();
                response.push(0xff);
                response.extend_from_slice(
                    b"\r\nContent-Encoding: gzip\r\nContent-Length: 1\r\nConnection: close\r\n\r\n\x80",
                );
                socket.write_all(&response).unwrap();
            }
        }
    });

    let client = ReqwestProfileHttpClient::new();
    let response = block_on(client.fetch(
        request(format!("http://{address}/start"), None),
        RuntimeExecutionContext::uncancellable(),
    ))
    .unwrap_or_else(|_| panic!("production adapter should preserve a non-success response"));
    server.join().unwrap();

    assert_eq!(response.status(), 418);
    assert_eq!(response.final_url(), format!("http://{address}/final"));
    assert_eq!(response.raw_body(), &[0x80]);
    assert_eq!(response.body, "€");
    let repeated = response
        .headers()
        .iter()
        .filter(|h| h.name() == "x-repeat")
        .map(|h| h.value())
        .collect::<Vec<_>>();
    assert_eq!(repeated, vec![b"first".as_slice(), &[0xff]]);
}

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}
