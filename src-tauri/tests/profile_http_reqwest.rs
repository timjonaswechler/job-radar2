use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

use job_radar_lib::{
    HttpMethod, ProfileHttpClient, ProfileHttpRequest, ReqwestProfileHttpClient,
    RuntimeExecutionContext,
};

fn request(url: String) -> ProfileHttpRequest {
    ProfileHttpRequest {
        method: HttpMethod::Get,
        url,
        headers: Vec::new(),
        body: None,
        timeout_ms: 5_000,
        authored_charset: None,
    }
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
        request(format!("http://{address}/start")),
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
