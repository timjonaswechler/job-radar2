#[cfg(not(debug_assertions))]
compile_error!("the agent debug harness is unavailable in release builds");

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("agent debug runtime must initialize")
        .block_on(future)
}

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
    match line.trim() {
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
    active: std::collections::BTreeMap<usize, agent::ContentKind>,
}

impl EventRenderer {
    fn render(
        &mut self,
        event: &agent::ConversationEvent,
        writer: &mut impl std::io::Write,
    ) -> std::io::Result<()> {
        use agent::{ContentKind, ConversationEvent};
        match event {
            ConversationEvent::Started | ConversationEvent::Completed { .. } => {}
            ConversationEvent::ContentStarted { index, kind } => {
                self.active.insert(*index, *kind);
                write!(
                    writer,
                    "{}",
                    match kind {
                        ContentKind::Text => "assistant> ",
                        ContentKind::Reasoning => "[reasoning] ",
                    }
                )?;
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

fn safe_error_message(category: agent::AgentErrorCategory) -> &'static str {
    use agent::AgentErrorCategory;
    match category {
        AgentErrorCategory::Authentication => "authentication failed",
        AgentErrorCategory::ModelUnavailable => "model unavailable",
        AgentErrorCategory::Transport => "transport unavailable",
        AgentErrorCategory::RateLimited => "rate limited",
        AgentErrorCategory::ContextOverflow => "context window exceeded",
        AgentErrorCategory::Provider => "provider failed",
        AgentErrorCategory::InvalidConfiguration => "invalid configuration",
    }
}

#[derive(Debug)]
enum HarnessFailure {
    Io,
    Agent(agent::AgentErrorCategory),
    Configuration,
}

impl From<std::io::Error> for HarnessFailure {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}

impl From<agent::AgentError> for HarnessFailure {
    fn from(error: agent::AgentError) -> Self {
        Self::Agent(error.category)
    }
}

impl From<agent::ConfigurationError> for HarnessFailure {
    fn from(_: agent::ConfigurationError) -> Self {
        Self::Configuration
    }
}

type HarnessResult<T> = Result<T, HarnessFailure>;

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

struct DebugLoginInteraction<'a, R, W> {
    reader: &'a mut R,
    writer: &'a mut W,
}

impl<R, W> agent::LoginInteraction for DebugLoginInteraction<'_, R, W>
where
    R: std::io::BufRead + Send,
    W: std::io::Write + Send,
{
    fn select_method(
        &mut self,
        _attempt: &agent::LoginAttemptId,
        _provider: &agent::ProviderId,
    ) -> agent::InteractionFuture<'_, agent::LoginMethod> {
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
                agent::LoginMethod::Browser
            } else {
                agent::LoginMethod::DeviceCode
            }
        })
        .ok_or(agent::InteractionError);
        Box::pin(async move { result })
    }

    fn open_url(
        &mut self,
        _attempt: &agent::LoginAttemptId,
        url: &str,
    ) -> Result<(), agent::InteractionError> {
        writeln!(self.writer, "Waiting for the browser callback...")
            .map_err(|_| agent::InteractionError)?;
        self.writer.flush().map_err(|_| agent::InteractionError)?;
        open_system_browser(url)
            .then_some(())
            .ok_or(agent::InteractionError)
    }

    fn display_device_code(
        &mut self,
        _attempt: &agent::LoginAttemptId,
        verification_uri: &str,
        user_code: &str,
    ) -> Result<(), agent::InteractionError> {
        writeln!(self.writer, "Open: {verification_uri}")
            .and_then(|_| writeln!(self.writer, "Device code: {user_code}"))
            .and_then(|_| self.writer.flush())
            .map_err(|_| agent::InteractionError)
    }

    fn report(&mut self, progress: agent::LoginProgress) {
        let _ = writeln!(self.writer, "login: {:?}", progress.stage);
    }
}

fn new_conversation(
    configuration: &agent::Configuration,
) -> Result<agent::Conversation, agent::AgentError> {
    let provider = configuration.provider();
    let models = provider.model_snapshot();
    let model = models
        .iter()
        .find(|model| model.id().as_str() == "gpt-5.4")
        .or_else(|| models.first())
        .ok_or_else(|| agent::AgentError {
            category: agent::AgentErrorCategory::ModelUnavailable,
            message: "model unavailable".to_owned(),
            retry_after: None,
        })?;
    agent::Conversation::new(
        "You are a concise, helpful assistant.".to_owned(),
        provider,
        model.id().clone(),
        agent::ReasoningLevel::Medium,
    )
}

fn stream_prompt(
    conversation: &mut agent::Conversation,
    text: String,
    writer: &mut impl std::io::Write,
) -> HarnessResult<()> {
    use futures_util::StreamExt;
    block_on(async {
        let mut stream = conversation.send(text)?;
        let mut renderer = EventRenderer::default();
        while let Some(event) = stream.next().await {
            renderer.render(&event, writer)?;
        }
        Ok(())
    })
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

fn choose_model(
    conversation: &mut agent::Conversation,
    reader: &mut impl std::io::BufRead,
    writer: &mut impl std::io::Write,
) -> HarnessResult<()> {
    let options: Vec<_> = conversation
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
    }
    Ok(())
}

fn choose_reasoning(
    conversation: &mut agent::Conversation,
    reader: &mut impl std::io::BufRead,
    writer: &mut impl std::io::Write,
) -> HarnessResult<()> {
    let levels = conversation.model().supported_reasoning_levels().to_vec();
    let options: Vec<_> = levels.iter().map(|level| format!("{level:?}")).collect();
    if let Some(selected) = numbered_selection(reader, writer, "Select reasoning level", &options)?
    {
        conversation.set_reasoning_level(levels[selected]);
    }
    Ok(())
}

fn run_with_io<R, W>(reader: &mut R, writer: &mut W) -> HarnessResult<()>
where
    R: std::io::BufRead + Send,
    W: std::io::Write + Send,
{
    let root = job_radar_lib::current_user_agents_data_root()?;
    let configuration = agent::Configuration::new(root)?;
    let mut conversation = new_conversation(&configuration).ok();

    writeln!(writer, "Job Radar agent debug harness")?;
    writeln!(writer, "Commands: /login /logout /model /settings /quit")?;
    writeln!(
        writer,
        "authentication: {}",
        if conversation.is_some() {
            "configured"
        } else {
            "not configured"
        }
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
            Input::UnknownCommand => writeln!(writer, "unknown command")?,
            Input::Model => match conversation.as_mut() {
                Some(active) => choose_model(active, reader, writer)?,
                None => writeln!(writer, "error: authentication failed")?,
            },
            Input::Settings => match conversation.as_mut() {
                Some(active) => choose_reasoning(active, reader, writer)?,
                None => writeln!(writer, "error: authentication failed")?,
            },
            Input::Login => {
                let attempt = agent::LoginAttemptId::generate();
                let provider = agent::ProviderId::new("openai-codex")?;
                let result = {
                    let mut interaction = DebugLoginInteraction { reader, writer };
                    block_on(configuration.login(attempt, provider, &mut interaction))
                };
                match result {
                    Ok(_) => {
                        conversation = new_conversation(&configuration).ok();
                        writeln!(writer, "authentication: configured")?;
                    }
                    Err(error) => write_failure(writer, error.into())?,
                }
            }
            Input::Logout => {
                configuration.remove_authentication(agent::ProviderId::new("openai-codex")?)?;
                conversation = None;
                writeln!(writer, "authentication: not configured")?;
            }
            Input::Prompt(text) => match conversation.as_mut() {
                Some(active) => {
                    if let Err(error) = stream_prompt(active, text, writer) {
                        write_failure(writer, error)?;
                    }
                }
                None => writeln!(writer, "error: authentication failed")?,
            },
        }
    }
}

fn write_failure(writer: &mut impl std::io::Write, error: HarnessFailure) -> std::io::Result<()> {
    let message = match error {
        HarnessFailure::Io => "debug harness I/O failed",
        HarnessFailure::Agent(category) => safe_error_message(category),
        HarnessFailure::Configuration => "agent configuration failed",
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

    #[test]
    fn command_parser_separates_prompts_from_commands() {
        assert_eq!(parse_line("hello\n"), Input::Prompt("hello".to_owned()));
        assert_eq!(parse_line("/login"), Input::Login);
        assert_eq!(parse_line("/quit"), Input::Quit);
        assert_eq!(parse_line("/unknown"), Input::UnknownCommand);
    }

    #[test]
    fn numbered_selection_retries_without_exposing_hidden_state() {
        let mut input = std::io::Cursor::new(b"wrong\n2\n".to_vec());
        let mut output = Vec::new();
        let selected = numbered_selection(
            &mut input,
            &mut output,
            "Select",
            &["First".to_owned(), "Second".to_owned()],
        )
        .unwrap();
        assert_eq!(selected, Some(1));
        assert!(String::from_utf8(output)
            .unwrap()
            .contains("Enter a number"));
    }
}
