use super::SecretAuthorizationInput;
use crate::{AgentError, AgentErrorCategory};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

const CALLBACK_ADDRESS: &str = "127.0.0.1:1455";
const CALLBACK_PATH: &str = "/auth/callback";
const MAX_REQUEST_BYTES: usize = 8 * 1024;
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const READ_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) trait Cancellation: Send + Sync {
    fn is_cancelled(&self) -> bool;
    fn begin_finalizing(&self) -> bool;
}

pub(crate) fn bind() -> Result<TcpListener, AgentError> {
    let listener = TcpListener::bind(CALLBACK_ADDRESS).map_err(|_| unavailable())?;
    listener.set_nonblocking(true).map_err(|_| unavailable())?;
    Ok(listener)
}

pub(crate) async fn capture(
    listener: &TcpListener,
    expected_state: &str,
    cancellation: &dyn Cancellation,
) -> Result<SecretAuthorizationInput, AgentError> {
    let deadline = Instant::now() + CALLBACK_TIMEOUT;
    loop {
        if cancellation.is_cancelled() {
            return Err(cancelled());
        }
        if Instant::now() >= deadline {
            return Err(unavailable());
        }
        match listener.accept() {
            Ok((mut stream, peer)) if peer.ip().is_loopback() => {
                let Some(request) = read_request(&mut stream, deadline, cancellation) else {
                    let _ = stream.write_all(response(false));
                    continue;
                };
                let Some(callback) = parse(&request, expected_state) else {
                    let _ = stream.write_all(response(false));
                    continue;
                };
                if !cancellation.begin_finalizing() {
                    return Err(cancelled());
                }
                stream
                    .write_all(response(true))
                    .map_err(|_| unavailable())?;
                return Ok(SecretAuthorizationInput::new(callback));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Err(_) => return Err(unavailable()),
        }
    }
}

fn read_request(
    stream: &mut TcpStream,
    deadline: Instant,
    cancellation: &dyn Cancellation,
) -> Option<Vec<u8>> {
    stream.set_nonblocking(true).ok()?;
    let mut request = Vec::new();
    let mut buffer = [0_u8; 512];
    let mut last_progress = Instant::now();
    loop {
        let now = Instant::now();
        if cancellation.is_cancelled()
            || now >= deadline
            || now.saturating_duration_since(last_progress) >= READ_TIMEOUT
        {
            return None;
        }
        match stream.read(&mut buffer) {
            Ok(0) => return None,
            Ok(read) => {
                last_progress = Instant::now();
                request.extend_from_slice(&buffer[..read]);
                if request.len() > MAX_REQUEST_BYTES {
                    return None;
                }
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    return Some(request);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }
}

fn parse(request: &[u8], expected_state: &str) -> Option<String> {
    let request = std::str::from_utf8(request).ok()?;
    let request_line = request.split("\r\n").next()?;
    let mut parts = request_line.split(' ');
    let method = parts.next()?;
    let target = parts.next()?;
    let version = parts.next()?;
    if parts.next().is_some()
        || method != "GET"
        || !matches!(version, "HTTP/1.0" | "HTTP/1.1")
        || !target.starts_with(CALLBACK_PATH)
    {
        return None;
    }
    let (path, query) = target.split_once('?')?;
    if path != CALLBACK_PATH || query.is_empty() || target.contains('#') {
        return None;
    }
    let parameters: HashMap<_, _> = url::form_urlencoded::parse(query.as_bytes()).collect();
    let code = parameters.get("code").filter(|value| !value.is_empty())?;
    let state = parameters.get("state").filter(|value| !value.is_empty())?;
    if state.as_ref() != expected_state || code.is_empty() {
        return None;
    }
    Some(format!("http://localhost:1455{target}"))
}

fn response(success: bool) -> &'static [u8] {
    if success {
        b"HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: 48\r\nConnection: close\r\nCache-Control: no-store\r\n\r\nAuthorization received. Return to Job Radar now."
    } else {
        b"HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: 47\r\nConnection: close\r\nCache-Control: no-store\r\n\r\nCallback was not accepted. Return to Job Radar."
    }
}

fn unavailable() -> AgentError {
    AgentError::fixed(
        AgentErrorCategory::InvalidConfiguration,
        "subscription login is unavailable",
    )
}

fn cancelled() -> AgentError {
    AgentError::fixed(
        AgentErrorCategory::Authentication,
        "subscription login was cancelled",
    )
}

#[cfg(test)]
mod tests {
    use super::{parse, read_request, Cancellation, MAX_REQUEST_BYTES};
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::time::{Duration, Instant};

    struct Active;

    impl Cancellation for Active {
        fn is_cancelled(&self) -> bool {
            false
        }

        fn begin_finalizing(&self) -> bool {
            true
        }
    }

    #[test]
    fn callback_requires_exact_path_method_and_expected_state() {
        let accepted = b"GET /auth/callback?code=synthetic-code&state=expected HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert!(parse(accepted, "expected").is_some());
        assert!(parse(accepted, "other").is_none());
        assert!(parse(
            b"POST /auth/callback?code=x&state=expected HTTP/1.1\r\n\r\n",
            "expected"
        )
        .is_none());
        assert!(parse(
            b"GET /auth/callback/extra?code=x&state=expected HTTP/1.1\r\n\r\n",
            "expected"
        )
        .is_none());
    }

    #[test]
    fn callback_reader_rejects_requests_over_the_byte_bound() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let writer = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            stream
                .write_all(&vec![b'a'; MAX_REQUEST_BYTES + 1])
                .unwrap();
        });
        let (mut stream, _) = listener.accept().unwrap();

        assert!(read_request(
            &mut stream,
            Instant::now() + Duration::from_secs(1),
            &Active,
        )
        .is_none());
        writer.join().unwrap();
    }

    #[test]
    fn callback_rejects_missing_or_fragment_borne_secrets() {
        assert!(parse(
            b"GET /auth/callback?state=expected HTTP/1.1\r\n\r\n",
            "expected"
        )
        .is_none());
        assert!(parse(
            b"GET /auth/callback?code=x&state=expected#fragment HTTP/1.1\r\n\r\n",
            "expected"
        )
        .is_none());
    }
}
