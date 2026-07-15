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
        let result = (|| {
            writeln!(self.writer, "{}", authorization.instructions())?;
            writeln!(self.writer, "Open: {}", authorization.url())?;
            read_secret_authorization_input(self.reader, self.writer)
        })()
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
