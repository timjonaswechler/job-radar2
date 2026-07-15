#[cfg(not(debug_assertions))]
compile_error!("the agent debug harness is unavailable in release builds");

#[derive(Debug, Eq, PartialEq)]
enum Input {
    Login,
    Logout,
    Model,
    Settings,
    Quit,
    Prompt(String),
    Empty,
    UnknownCommand,
}

fn parse_line(line: &str) -> Input {
    let line = line.trim();
    match line {
        "" => Input::Empty,
        "/login" => Input::Login,
        "/logout" => Input::Logout,
        "/model" => Input::Model,
        "/settings" => Input::Settings,
        "/quit" => Input::Quit,
        command if command.starts_with('/') => Input::UnknownCommand,
        prompt => Input::Prompt(prompt.to_owned()),
    }
}

#[derive(Default)]
struct EventRenderer {
    active: std::collections::BTreeMap<usize, job_radar_lib::agent::ContentKind>,
}

impl EventRenderer {
    fn render(
        &mut self,
        event: &job_radar_lib::agent::ConversationEvent,
        writer: &mut impl std::io::Write,
    ) -> std::io::Result<()> {
        use job_radar_lib::agent::{ContentKind, ConversationEvent};
        match event {
            ConversationEvent::Started | ConversationEvent::Completed { .. } => {}
            ConversationEvent::ContentStarted { index, kind } => {
                self.active.insert(*index, *kind);
                match kind {
                    ContentKind::Text => write!(writer, "assistant> ")?,
                    ContentKind::Reasoning => write!(writer, "[reasoning] ")?,
                }
                writer.flush()?;
            }
            ConversationEvent::ContentDelta { index, delta } => {
                if self.active.contains_key(index) {
                    write!(writer, "{delta}")?;
                    writer.flush()?;
                }
            }
            ConversationEvent::ContentFinished { index } => {
                self.active.remove(index);
                writeln!(writer)?;
            }
            ConversationEvent::Failed { error } => {
                if !self.active.is_empty() {
                    writeln!(writer)?;
                }
                self.active.clear();
                writeln!(writer, "error: {}", safe_error_message(error.category))?;
            }
            ConversationEvent::Aborted => {
                if !self.active.is_empty() {
                    writeln!(writer)?;
                }
                self.active.clear();
                writeln!(writer, "turn aborted")?;
            }
        }
        Ok(())
    }
}

fn safe_error_message(category: job_radar_lib::agent::AgentErrorCategory) -> &'static str {
    use job_radar_lib::agent::AgentErrorCategory;
    match category {
        AgentErrorCategory::Authentication => "authentication failed",
        AgentErrorCategory::ModelUnavailable => "model unavailable",
        AgentErrorCategory::Transport => "transport unavailable",
        AgentErrorCategory::RateLimited => "rate limited",
        AgentErrorCategory::Provider => "provider failed",
        AgentErrorCategory::InvalidConfiguration => "invalid configuration",
    }
}

#[derive(Debug)]
enum HarnessFailure {
    Io,
    Agent(job_radar_lib::agent::AgentErrorCategory),
}

impl From<std::io::Error> for HarnessFailure {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}

impl From<job_radar_lib::agent::AgentError> for HarnessFailure {
    fn from(error: job_radar_lib::agent::AgentError) -> Self {
        Self::Agent(error.category)
    }
}

type HarnessResult<T> = Result<T, HarnessFailure>;

const OAUTH_CALLBACK_ADDRESS: &str = "127.0.0.1:1455";
const OAUTH_CALLBACK_PATH: &str = "/auth/callback";
const OAUTH_CALLBACK_MAX_REQUEST_BYTES: usize = 8 * 1024;
const OAUTH_CALLBACK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5 * 60);
const OAUTH_CALLBACK_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

fn capture_loopback_callback(
    address: std::net::SocketAddr,
    overall_timeout: std::time::Duration,
    read_timeout: std::time::Duration,
    launch_browser: impl FnOnce(std::net::SocketAddr) -> bool,
) -> Option<job_radar_lib::agent::openai_codex::SecretAuthorizationInput> {
    use std::io::Write;

    if !address.ip().is_loopback() {
        return None;
    }
    let listener = std::net::TcpListener::bind(address).ok()?;
    let bound_address = listener.local_addr().ok()?;
    if !bound_address.ip().is_loopback() || listener.set_nonblocking(true).is_err() {
        return None;
    }
    if !launch_browser(bound_address) {
        return None;
    }

    let deadline = std::time::Instant::now() + overall_timeout;
    loop {
        if std::time::Instant::now() >= deadline {
            return None;
        }
        match listener.accept() {
            Ok((mut stream, peer)) => {
                if !peer.ip().is_loopback() {
                    continue;
                }
                let Some(request) =
                    read_bounded_callback_request(&mut stream, deadline, read_timeout)
                else {
                    let _ = stream.write_all(neutral_browser_response(false));
                    continue;
                };
                let Some(callback) = parse_callback_request(&request) else {
                    let _ = stream.write_all(neutral_browser_response(false));
                    continue;
                };
                if stream.write_all(neutral_browser_response(true)).is_err() {
                    return None;
                }
                return Some(
                    job_radar_lib::agent::openai_codex::SecretAuthorizationInput::new(callback),
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(_) => return None,
        }
    }
}

fn read_bounded_callback_request(
    stream: &mut std::net::TcpStream,
    deadline: std::time::Instant,
    read_timeout: std::time::Duration,
) -> Option<Vec<u8>> {
    use std::io::Read;

    stream.set_nonblocking(true).ok()?;
    let mut request = Vec::new();
    let mut buffer = [0_u8; 512];
    let mut last_progress = std::time::Instant::now();
    loop {
        let now = std::time::Instant::now();
        if now >= deadline || now.saturating_duration_since(last_progress) >= read_timeout {
            return None;
        }
        match stream.read(&mut buffer) {
            Ok(0) => return None,
            Ok(read) => {
                last_progress = std::time::Instant::now();
                request.extend_from_slice(&buffer[..read]);
                if request.len() > OAUTH_CALLBACK_MAX_REQUEST_BYTES {
                    return None;
                }
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    return Some(request);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }
}

fn parse_callback_request(request: &[u8]) -> Option<String> {
    let request = std::str::from_utf8(request).ok()?;
    let request_line = request.split("\r\n").next()?;
    let mut parts = request_line.split(' ');
    let method = parts.next()?;
    let target = parts.next()?;
    let version = parts.next()?;
    if parts.next().is_some()
        || method != "GET"
        || !matches!(version, "HTTP/1.0" | "HTTP/1.1")
        || !target.starts_with(OAUTH_CALLBACK_PATH)
    {
        return None;
    }
    let (path, query) = target.split_once('?')?;
    if path != OAUTH_CALLBACK_PATH || query.is_empty() || target.contains('#') {
        return None;
    }
    let parameters: std::collections::HashMap<_, _> =
        url::form_urlencoded::parse(query.as_bytes()).collect();
    if parameters.get("code").is_none_or(|value| value.is_empty())
        || parameters.get("state").is_none_or(|value| value.is_empty())
    {
        return None;
    }
    Some(format!("http://localhost:1455{target}"))
}

fn neutral_browser_response(success: bool) -> &'static [u8] {
    if success {
        b"HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: 48\r\nConnection: close\r\nCache-Control: no-store\r\n\r\nAuthorization received. Return to Job Radar now."
    } else {
        b"HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: 47\r\nConnection: close\r\nCache-Control: no-store\r\n\r\nCallback was not accepted. Return to Job Radar."
    }
}

fn open_system_browser(url: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = url;
        false
    }
}

fn authorize_browser_with_capture(
    reader: &mut impl std::io::BufRead,
    writer: &mut impl std::io::Write,
    instructions: &str,
    authorization_url: &str,
    capture: impl FnOnce(&str) -> Option<job_radar_lib::agent::openai_codex::SecretAuthorizationInput>,
) -> std::io::Result<job_radar_lib::agent::openai_codex::SecretAuthorizationInput> {
    writeln!(writer, "{instructions}")?;
    writeln!(writer, "Waiting for the browser callback...")?;
    writer.flush()?;
    if let Some(input) = capture(authorization_url) {
        return Ok(input);
    }
    writeln!(
        writer,
        "Automatic callback unavailable; use the manual fallback."
    )?;
    writeln!(writer, "Open: {authorization_url}")?;
    read_secret_authorization_input(reader, writer)
}

fn read_secret_authorization_input(
    reader: &mut impl std::io::BufRead,
    writer: &mut impl std::io::Write,
) -> std::io::Result<job_radar_lib::agent::openai_codex::SecretAuthorizationInput> {
    write!(writer, "Paste the authorization result: ")?;
    writer.flush()?;
    let mut input = String::new();
    reader.read_line(&mut input)?;
    Ok(job_radar_lib::agent::openai_codex::SecretAuthorizationInput::new(input.trim().to_owned()))
}

struct DebugAuthInteraction<'a, R, W> {
    reader: &'a mut R,
    writer: &'a mut W,
}

impl<R, W> job_radar_lib::agent::openai_codex::AuthInteraction for DebugAuthInteraction<'_, R, W>
where
    R: std::io::BufRead + Send,
    W: std::io::Write + Send,
{
    fn select_login_method(
        &mut self,
    ) -> job_radar_lib::agent::openai_codex::AuthFuture<
        '_,
        job_radar_lib::agent::openai_codex::LoginMethod,
    > {
        use job_radar_lib::agent::openai_codex::LoginMethod;
        let result = numbered_selection(
            self.reader,
            self.writer,
            "Select login method",
            &["Browser PKCE".to_owned(), "Device code".to_owned()],
        )
        .ok()
        .flatten()
        .map(|selected| {
            if selected == 0 {
                LoginMethod::Browser
            } else {
                LoginMethod::DeviceCode
            }
        })
        .ok_or_else(harness_auth_error);
        Box::pin(async move { result })
    }

    fn authorize_browser(
        &mut self,
        authorization: job_radar_lib::agent::openai_codex::BrowserAuthorization,
    ) -> job_radar_lib::agent::openai_codex::AuthFuture<
        '_,
        job_radar_lib::agent::openai_codex::SecretAuthorizationInput,
    > {
        let result = authorize_browser_with_capture(
            self.reader,
            self.writer,
            authorization.instructions(),
            authorization.url(),
            |authorization_url| {
                let address = OAUTH_CALLBACK_ADDRESS.parse().ok()?;
                capture_loopback_callback(
                    address,
                    OAUTH_CALLBACK_TIMEOUT,
                    OAUTH_CALLBACK_READ_TIMEOUT,
                    |_| open_system_browser(authorization_url),
                )
            },
        )
        .map_err(|_| harness_auth_error());
        Box::pin(async move { result })
    }

    fn display_device_code(
        &mut self,
        authorization: job_radar_lib::agent::openai_codex::DeviceAuthorization,
    ) -> job_radar_lib::agent::openai_codex::AuthFuture<'_, ()> {
        let result = (|| {
            writeln!(self.writer, "Open: {}", authorization.verification_uri())?;
            writeln!(self.writer, "Device code: {}", authorization.user_code())?;
            writeln!(
                self.writer,
                "This code expires in {} minutes.",
                authorization.expires_in().as_secs() / 60
            )?;
            self.writer.flush()
        })()
        .map_err(|_| harness_auth_error());
        Box::pin(async move { result })
    }
}

fn harness_auth_error() -> job_radar_lib::agent::AgentError {
    job_radar_lib::agent::AgentError {
        category: job_radar_lib::agent::AgentErrorCategory::InvalidConfiguration,
        message: "debug authentication interaction failed".to_owned(),
        retry_after: None,
    }
}

fn stream_prompt(
    conversation: &mut job_radar_lib::agent::AgentConversation,
    text: String,
    writer: &mut impl std::io::Write,
) -> HarnessResult<()> {
    use futures_util::StreamExt;
    tauri::async_runtime::block_on(async {
        let mut stream = conversation.send(text)?;
        let mut renderer = EventRenderer::default();
        while let Some(event) = stream.next().await {
            renderer.render(&event, writer)?;
        }
        Ok(())
    })
}

fn choose_model(
    conversation: &mut job_radar_lib::agent::AgentConversation,
    reader: &mut impl std::io::BufRead,
    writer: &mut impl std::io::Write,
) -> HarnessResult<()> {
    let options: Vec<String> = conversation
        .available_models()
        .iter()
        .map(|model| format!("{} ({})", model.display_name(), model.id().as_str()))
        .collect();
    let ids: Vec<_> = conversation
        .available_models()
        .iter()
        .map(|model| model.id().clone())
        .collect();
    if let Some(selected) = numbered_selection(reader, writer, "Select model", &options)? {
        conversation.select_model(ids[selected].clone())?;
        writeln!(
            writer,
            "model: {} ({})",
            conversation.model().display_name(),
            conversation.model().id().as_str()
        )?;
        writeln!(writer, "reasoning: {:?}", conversation.reasoning_level())?;
    }
    Ok(())
}

fn choose_reasoning(
    conversation: &mut job_radar_lib::agent::AgentConversation,
    reader: &mut impl std::io::BufRead,
    writer: &mut impl std::io::Write,
) -> HarnessResult<()> {
    let levels = conversation.model().supported_reasoning_levels().to_vec();
    let options: Vec<String> = levels.iter().map(|level| format!("{level:?}")).collect();
    if let Some(selected) = numbered_selection(reader, writer, "Select reasoning level", &options)?
    {
        let effective = conversation.set_reasoning_level(levels[selected]);
        writeln!(writer, "reasoning: {effective:?}")?;
    }
    Ok(())
}

fn numbered_selection(
    reader: &mut impl std::io::BufRead,
    writer: &mut impl std::io::Write,
    title: &str,
    options: &[String],
) -> std::io::Result<Option<usize>> {
    writeln!(writer, "{title}")?;
    for (index, option) in options.iter().enumerate() {
        writeln!(writer, "{}) {option}", index + 1)?;
    }
    loop {
        write!(writer, "> ")?;
        writer.flush()?;
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        if let Ok(selected) = line.trim().parse::<usize>() {
            if (1..=options.len()).contains(&selected) {
                return Ok(Some(selected - 1));
            }
        }
        writeln!(writer, "Enter a number from 1 to {}.", options.len())?;
    }
}

fn run_with_io<R, W>(reader: &mut R, writer: &mut W) -> HarnessResult<()>
where
    R: std::io::BufRead + Send,
    W: std::io::Write + Send,
{
    use job_radar_lib::agent::models::{ModelId, ReasoningLevel};
    use job_radar_lib::agent::openai_codex::{
        AgentAuthentication, AuthStatus, OpenAiCodexProvider,
    };
    use job_radar_lib::agent::AgentConversation;

    let authentication = AgentAuthentication::for_current_user()?;
    let provider = OpenAiCodexProvider::for_current_user()?;
    let mut conversation = AgentConversation::new(
        "You are a concise, helpful assistant.".to_owned(),
        provider,
        ModelId::new("gpt-5.4")?,
        ReasoningLevel::Medium,
    )?;

    writeln!(writer, "Job Radar agent debug harness")?;
    writeln!(writer, "Commands: /login /logout /model /settings /quit")?;
    match authentication.status()? {
        AuthStatus::Configured => writeln!(writer, "authentication: configured")?,
        AuthStatus::NotConfigured => writeln!(writer, "authentication: not configured")?,
    }
    writeln!(
        writer,
        "model: {} ({}) · reasoning: {:?}",
        conversation.model().display_name(),
        conversation.model().id().as_str(),
        conversation.reasoning_level()
    )?;

    loop {
        write!(writer, "you> ")?;
        writer.flush()?;
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(());
        }
        match parse_line(&line) {
            Input::Empty => {}
            Input::Quit => return Ok(()),
            Input::UnknownCommand => {
                writeln!(writer, "unknown command")?;
            }
            Input::Model => {
                if let Err(error) = choose_model(&mut conversation, reader, writer) {
                    write_failure(writer, error)?;
                }
            }
            Input::Settings => {
                if let Err(error) = choose_reasoning(&mut conversation, reader, writer) {
                    write_failure(writer, error)?;
                }
            }
            Input::Login => {
                let result = {
                    let mut interaction = DebugAuthInteraction { reader, writer };
                    tauri::async_runtime::block_on(authentication.login(&mut interaction))
                };
                match result {
                    Ok(()) => writeln!(writer, "authentication: configured")?,
                    Err(error) => write_failure(writer, HarnessFailure::from(error))?,
                }
            }
            Input::Logout => match authentication.logout() {
                Ok(()) => writeln!(writer, "authentication: not configured")?,
                Err(error) => write_failure(writer, HarnessFailure::from(error))?,
            },
            Input::Prompt(text) => {
                if let Err(error) = stream_prompt(&mut conversation, text, writer) {
                    write_failure(writer, error)?;
                }
            }
        }
    }
}

fn write_failure(writer: &mut impl std::io::Write, error: HarnessFailure) -> std::io::Result<()> {
    let message = match error {
        HarnessFailure::Io => "debug harness I/O failed",
        HarnessFailure::Agent(category) => safe_error_message(category),
    };
    writeln!(writer, "error: {message}")
}

fn main() {
    let mut reader = std::io::BufReader::new(std::io::stdin());
    let mut writer = std::io::stdout();
    if let Err(error) = run_with_io(&mut reader, &mut writer) {
        let _ = write_failure(&mut writer, error);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static LOOPBACK_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn conversation_with_two_models() -> job_radar_lib::agent::AgentConversation {
        use job_radar_lib::agent::models::{Model, ModelId, ProviderId, ReasoningLevel};
        use job_radar_lib::agent::testing::ScriptedProvider;
        use job_radar_lib::agent::AgentConversation;

        let provider_id = ProviderId::new("synthetic-provider").unwrap();
        let first = Model::new(
            ModelId::new("first").unwrap(),
            "First model",
            provider_id.clone(),
            vec![ReasoningLevel::Off, ReasoningLevel::Medium],
        )
        .unwrap();
        let second = Model::new(
            ModelId::new("second").unwrap(),
            "Second model",
            provider_id,
            vec![ReasoningLevel::Low, ReasoningLevel::High],
        )
        .unwrap();
        AgentConversation::new(
            "Synthetic system".to_owned(),
            ScriptedProvider::new(vec![first.clone(), second], Vec::new()),
            first.id().clone(),
            ReasoningLevel::Medium,
        )
        .unwrap()
    }

    #[test]
    fn ordinary_prompt_streams_through_the_public_conversation_contract() {
        use job_radar_lib::agent::models::{Model, ModelId, ProviderId, ReasoningLevel};
        use job_radar_lib::agent::testing::{
            ExpectedConversationRequest, ScriptedProvider, ScriptedTurn,
        };
        use job_radar_lib::agent::{
            AgentConversation, ContentKind, FinishReason, Message, ProviderEvent,
            ProviderTurnCompletion, TokenUsage, UserMessage,
        };

        let model = Model::new(
            ModelId::new("synthetic-model").unwrap(),
            "Synthetic model",
            ProviderId::new("synthetic-provider").unwrap(),
            vec![ReasoningLevel::Medium],
        )
        .unwrap();
        let provider = ScriptedProvider::new(
            vec![model.clone()],
            vec![ScriptedTurn::new(
                ExpectedConversationRequest::new(
                    "Synthetic system",
                    vec![Message::User(UserMessage::new("hello"))],
                    model.id().clone(),
                    ReasoningLevel::Medium,
                ),
                vec![
                    ProviderEvent::Started,
                    ProviderEvent::ContentStarted {
                        index: 0,
                        kind: ContentKind::Text,
                    },
                    ProviderEvent::ContentDelta {
                        index: 0,
                        delta: "hi there".to_owned(),
                    },
                    ProviderEvent::ContentFinished { index: 0 },
                    ProviderEvent::Completed(ProviderTurnCompletion::new(
                        TokenUsage::default(),
                        FinishReason::Completed,
                    )),
                ],
            )],
        );
        let mut conversation = AgentConversation::new(
            "Synthetic system".to_owned(),
            provider,
            model.id().clone(),
            ReasoningLevel::Medium,
        )
        .unwrap();
        let mut output = Vec::new();

        stream_prompt(&mut conversation, "hello".to_owned(), &mut output).unwrap();

        assert!(String::from_utf8(output)
            .unwrap()
            .contains("assistant> hi there"));
        assert_eq!(conversation.messages().len(), 2);
    }

    #[test]
    fn secret_authorization_input_is_not_written_back_or_logged() {
        let mut input = std::io::Cursor::new(b"synthetic-secret-manual-input\n");
        let mut output = Vec::new();

        let _secret = read_secret_authorization_input(&mut input, &mut output).unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Paste the authorization result"));
        assert!(!output.contains("synthetic-secret-manual-input"));
    }

    #[test]
    fn model_and_settings_menus_change_only_public_conversation_selection() {
        use job_radar_lib::agent::models::ReasoningLevel;

        let mut conversation = conversation_with_two_models();
        let mut model_input = std::io::Cursor::new(b"2\n");
        let mut settings_input = std::io::Cursor::new(b"1\n");
        let mut output = Vec::new();

        choose_model(&mut conversation, &mut model_input, &mut output).unwrap();
        assert_eq!(conversation.model().id().as_str(), "second");
        assert_eq!(conversation.reasoning_level(), ReasoningLevel::High);
        choose_reasoning(&mut conversation, &mut settings_input, &mut output).unwrap();
        assert_eq!(conversation.reasoning_level(), ReasoningLevel::Low);
        assert!(conversation.messages().is_empty());

        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Second model (second)"));
        assert!(output.contains("Low"));
    }

    #[test]
    fn event_renderer_distinguishes_reasoning_and_never_prints_error_payloads() {
        use job_radar_lib::agent::{
            AgentError, AgentErrorCategory, ContentKind, ConversationEvent,
        };

        let mut renderer = EventRenderer::default();
        let mut output = Vec::new();
        renderer
            .render(
                &ConversationEvent::ContentStarted {
                    index: 0,
                    kind: ContentKind::Reasoning,
                },
                &mut output,
            )
            .unwrap();
        renderer
            .render(
                &ConversationEvent::ContentDelta {
                    index: 0,
                    delta: "short summary".to_owned(),
                },
                &mut output,
            )
            .unwrap();
        renderer
            .render(
                &ConversationEvent::ContentFinished { index: 0 },
                &mut output,
            )
            .unwrap();
        renderer
            .render(
                &ConversationEvent::Failed {
                    error: AgentError {
                        category: AgentErrorCategory::Authentication,
                        message: "unsafe-provider-payload".to_owned(),
                        retry_after: None,
                    },
                },
                &mut output,
            )
            .unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("[reasoning] short summary"));
        assert!(output.contains("authentication failed"));
        assert!(!output.contains("unsafe-provider-payload"));
    }

    #[test]
    fn numbered_menu_reprompts_until_a_valid_selection() {
        let mut input = std::io::Cursor::new(b"nope\n3\n2\n");
        let mut output = Vec::new();

        let selected = numbered_selection(
            &mut input,
            &mut output,
            "Choose",
            &["First".to_owned(), "Second".to_owned()],
        )
        .unwrap();

        assert_eq!(selected, Some(1));
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("1) First"));
        assert!(output.contains("2) Second"));
        assert_eq!(output.matches("Enter a number from 1 to 2.").count(), 2);
    }

    #[test]
    fn numbered_menu_treats_eof_as_cancelled() {
        let mut input = std::io::Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();

        assert_eq!(
            numbered_selection(&mut input, &mut output, "Choose", &["Only".to_owned()],).unwrap(),
            None
        );
    }

    #[test]
    fn browser_authorization_uses_automatic_capture_and_redacted_manual_fallback() {
        use job_radar_lib::agent::openai_codex::SecretAuthorizationInput;

        let mut automatic_input = std::io::Cursor::new(Vec::<u8>::new());
        let mut automatic_output = Vec::new();
        let automatic = authorize_browser_with_capture(
            &mut automatic_input,
            &mut automatic_output,
            "Complete authentication in the browser.",
            "https://example.invalid/authorize?state=synthetic-state",
            |_| Some(SecretAuthorizationInput::new("synthetic-callback")),
        );
        assert!(automatic.is_ok());
        let automatic_output = String::from_utf8(automatic_output).unwrap();
        assert!(automatic_output.contains("Waiting for the browser callback"));
        assert!(!automatic_output.contains("synthetic-state"));
        assert!(!automatic_output.contains("synthetic-callback"));

        let mut fallback_input = std::io::Cursor::new(b"synthetic-manual-callback\n");
        let mut fallback_output = Vec::new();
        let fallback = authorize_browser_with_capture(
            &mut fallback_input,
            &mut fallback_output,
            "Complete authentication in the browser.",
            "https://example.invalid/authorize?state=synthetic-state",
            |_| None,
        );
        assert!(fallback.is_ok());
        let fallback_output = String::from_utf8(fallback_output).unwrap();
        assert!(fallback_output.contains("Automatic callback unavailable"));
        assert!(fallback_output.contains("Open: https://example.invalid/authorize"));
        assert!(fallback_output.contains("Paste the authorization result"));
        assert!(!fallback_output.contains("synthetic-manual-callback"));
    }

    #[test]
    fn loopback_capture_binds_loopback_before_launch_and_returns_neutral_response() {
        let _guard = LOOPBACK_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        use std::io::{Read, Write};
        use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
        use std::sync::mpsc;
        use std::time::Duration;

        let (response_sender, response_receiver) = mpsc::channel();
        let captured = capture_loopback_callback(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            Duration::from_secs(1),
            Duration::from_millis(200),
            move |bound_address: SocketAddr| {
                assert_eq!(bound_address.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
                std::thread::spawn(move || {
                    let mut browser = TcpStream::connect(bound_address).unwrap();
                    browser
                        .write_all(b"GET /auth/callback?code=synthetic-code&state=synthetic-state HTTP/1.1\r\nHost: localhost\r\n\r\n")
                        .unwrap();
                    let mut response = Vec::new();
                    browser.read_to_end(&mut response).unwrap();
                    response_sender.send(response).unwrap();
                });
                true
            },
        );

        assert!(captured.is_some());
        let browser_response = String::from_utf8(
            response_receiver
                .recv_timeout(Duration::from_secs(1))
                .unwrap(),
        )
        .unwrap();
        assert!(browser_response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(browser_response.contains("Content-Length: 48\r\n"));
        assert!(browser_response.contains("Return to Job Radar"));
        assert!(!browser_response.contains("synthetic-code"));
        assert!(!browser_response.contains("synthetic-state"));
    }

    #[test]
    fn loopback_capture_rejects_wrong_method_path_malformed_and_oversized_requests() {
        let _guard = LOOPBACK_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        use std::io::{Read, Write};
        use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
        use std::sync::mpsc;
        use std::time::Duration;

        fn request(address: SocketAddr, bytes: &[u8]) -> String {
            let mut browser = TcpStream::connect(address).unwrap();
            browser.write_all(bytes).unwrap();
            let mut response = String::new();
            browser.read_to_string(&mut response).unwrap();
            response
        }

        let (responses_sender, responses_receiver) = mpsc::channel();
        let captured = capture_loopback_callback(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            Duration::from_secs(2),
            Duration::from_millis(200),
            move |bound_address: SocketAddr| {
                std::thread::spawn(move || {
                    let mut responses = Vec::new();
                    responses.push(request(
                        bound_address,
                        b"POST /auth/callback?code=synthetic-code&state=synthetic-state HTTP/1.1\r\n\r\n",
                    ));
                    responses.push(request(
                        bound_address,
                        b"GET /wrong?code=synthetic-code&state=synthetic-state HTTP/1.1\r\n\r\n",
                    ));
                    responses.push(request(bound_address, b"malformed\r\n\r\n"));
                    let mut oversized = vec![b'x'; OAUTH_CALLBACK_MAX_REQUEST_BYTES + 1];
                    oversized.extend_from_slice(b"\r\n\r\n");
                    responses.push(request(bound_address, &oversized));
                    responses.push(request(
                        bound_address,
                        b"GET /auth/callback?code=synthetic-code&state=synthetic-state HTTP/1.1\r\n\r\n",
                    ));
                    responses_sender.send(responses).unwrap();
                });
                true
            },
        );

        assert!(captured.is_some());
        let responses = responses_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        assert_eq!(responses.len(), 5);
        assert!(responses[..4]
            .iter()
            .all(|response| response.starts_with("HTTP/1.1 400 Bad Request\r\n")));
        assert!(responses[4].starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(responses
            .iter()
            .all(|response| !response.contains("synthetic-code")
                && !response.contains("synthetic-state")));
    }

    #[test]
    fn loopback_capture_rejects_non_loopback_and_fails_before_launch_when_port_is_busy() {
        use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use std::time::Duration;

        let launched = Arc::new(AtomicBool::new(false));
        let launched_for_non_loopback = Arc::clone(&launched);
        assert!(capture_loopback_callback(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            Duration::from_millis(20),
            Duration::from_millis(10),
            move |_| {
                launched_for_non_loopback.store(true, Ordering::SeqCst);
                true
            },
        )
        .is_none());
        assert!(!launched.load(Ordering::SeqCst));

        let held_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let occupied_address = held_listener.local_addr().unwrap();
        let launched_for_busy_port = Arc::clone(&launched);
        assert!(capture_loopback_callback(
            occupied_address,
            Duration::from_millis(20),
            Duration::from_millis(10),
            move |_| {
                launched_for_busy_port.store(true, Ordering::SeqCst);
                true
            },
        )
        .is_none());
        assert!(!launched.load(Ordering::SeqCst));
    }

    #[test]
    fn loopback_capture_enforces_overall_timeout_against_slow_requests() {
        let _guard = LOOPBACK_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        use std::io::Write;
        use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
        use std::time::{Duration, Instant};

        let started = Instant::now();
        let captured = capture_loopback_callback(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            Duration::from_millis(50),
            Duration::from_millis(200),
            move |bound_address: SocketAddr| {
                std::thread::spawn(move || {
                    let mut browser = TcpStream::connect(bound_address).unwrap();
                    for byte in b"GET " {
                        if browser.write_all(&[*byte]).is_err() {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                });
                true
            },
        );

        assert!(captured.is_none());
        assert!(started.elapsed() < Duration::from_millis(300));
    }

    #[test]
    fn command_parser_recognizes_commands_prompts_and_blank_lines() {
        assert_eq!(parse_line("/login\n"), Input::Login);
        assert_eq!(parse_line(" /logout "), Input::Logout);
        assert_eq!(parse_line("/model"), Input::Model);
        assert_eq!(parse_line("/settings"), Input::Settings);
        assert_eq!(parse_line("/quit"), Input::Quit);
        assert_eq!(
            parse_line("  hello world  "),
            Input::Prompt("hello world".into())
        );
        assert_eq!(parse_line("  \n"), Input::Empty);
        assert_eq!(parse_line("/unknown"), Input::UnknownCommand);
    }
}
