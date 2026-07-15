# Define the minimal Agent Conversation contract

Status: accepted

Job Radar will expose a provider-neutral `AgentConversation` module in `job_radar_lib` for ephemeral multi-turn exchanges. It is deliberately smaller than Pi's full `Agent` and `AgentSession`: application workflows may use an Agent Conversation, but persistence, tools, compaction, Tauri contracts, and Source Profile authoring behavior remain outside it.

## Caller-facing contract

The caller supplies an immutable system prompt, a `ConversationProvider`, an initial Agent Model, and a Reasoning Level. The Agent Conversation exposes its completed transcript and available models, permits model and Reasoning Level changes between turns, and starts one streamed turn at a time.

The intended surface is:

```rust
impl AgentConversation {
    pub fn new(
        system_prompt: String,
        provider: impl ConversationProvider,
        model: ModelId,
        reasoning: ReasoningLevel,
    ) -> Result<Self, AgentError>;

    pub fn messages(&self) -> &[Message];
    pub fn available_models(&self) -> &[Model];
    pub fn model(&self) -> &Model;
    pub fn reasoning_level(&self) -> ReasoningLevel;

    pub fn select_model(&mut self, model: ModelId) -> Result<(), AgentError>;
    pub fn set_reasoning_level(&mut self, level: ReasoningLevel) -> ReasoningLevel;
    pub fn send(&mut self, text: String) -> Result<ConversationEventStream<'_>, AgentError>;
}
```

The stream's mutable borrow prevents concurrent turns and prevents model or Reasoning Level changes during a turn. The initial slice has no reset, transcript replacement, abort method, or wait/queue interface. The debug harness exits completely on `Ctrl+C`.

## Messages and turn state

Messages are app-owned reduced Pi shapes:

- `UserMessage` contains text.
- `AssistantMessage` contains ordered `Text` and `Reasoning` content blocks, the effective Agent Model, token usage, and a finish reason.
- `Message` is the closed `User | Assistant` union for this slice.

Visible reasoning text follows Pi's generic thinking-content behavior: it may contain a provider-supplied reasoning summary or other provider-approved reasoning text. Opaque reasoning payloads, signatures, response IDs, and other data required for stateless provider replay remain inaccessible to callers but may accompany the internal message representation.

A turn is transactional. The active User Message and partial Assistant Message remain pending while streaming. `Completed` commits both messages together. `Failed` and `Aborted` commit neither, so provider history contains only complete User-/Assistant-message pairs. Errors are never represented as Assistant Messages.

Token usage preserves provider-neutral counts:

```rust
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub reasoning: Option<u64>,
    pub total: u64,
}

pub enum FinishReason {
    Completed,
    LengthLimit,
}
```

Unknown usage values become `0` or `None`. Tool use, monetary cost, errors, and aborts are not completion reasons. Abort is a separate terminal stream outcome.

## Models and reasoning

`ProviderId` and `ModelId` are open, validated identifiers. The open shape preserves Pi-like future extensibility without introducing dynamic provider registration in this slice.

A caller-facing Agent Model contains only its ID, display name, Provider ID, and supported Reasoning Levels. Base URLs, API kinds, headers, pricing, request compatibility, and provider transport metadata stay behind the provider seam.

The initial OpenAI Codex adapter uses the model catalog pinned in `docs/research/pi-rust-agent-baseline.md`; it performs no live discovery and infers no capabilities from model-name strings. Unknown model IDs produce `ModelUnavailable`.

Reasoning Levels match Pi:

```rust
pub enum ReasoningLevel {
    Off,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}
```

The selected Agent Model declares supported levels. Unsupported selections clamp as Pi does to the nearest supported level, preferring the higher level at equal distance. Agent Conversation stores the effective level; model changes normalize it again. Provider-specific reasoning values remain adapter details.

## Provider seam and streaming

`ConversationProvider` is the sole provider seam. It accepts an app-owned request containing the system prompt, completed transcript, selected Agent Model, effective Reasoning Level, and internal conversation identifier, then returns provider-neutral events. OpenAI HTTP, SSE, payload, and error representations are translated inside the production adapter.

```rust
trait ConversationProvider {
    fn models(&self) -> &[Model];
    fn stream(&self, request: ConversationRequest) -> ProviderEventStream;
}
```

The public stream protocol is a reduced form of Pi's block-oriented protocol:

```rust
pub enum ConversationEvent {
    Started,
    ContentStarted { index: usize, kind: ContentKind },
    ContentDelta { index: usize, delta: String },
    ContentFinished { index: usize },
    Completed { message: AssistantMessage },
    Failed { error: AgentError },
    Aborted,
}

pub enum ContentKind {
    Text,
    Reasoning,
}
```

Each stream emits exactly one `Started`, followed by ordered and indexed content events, followed by exactly one of `Completed`, `Failed`, or `Aborted`. No event follows the terminal event. Provider request, model, transport, rate-limit, and runtime failures are encoded in this protocol rather than escaping as provider-specific stream errors.

A regularly compiled `agent::testing::ScriptedProvider` is the accepted deterministic second adapter. It consumes ordered scripted turns, verifies expected app-owned requests, emits scripted events without network or timing dependencies, records requests for assertions, supports failures and aborts, and fails deterministically on missing or unexpected calls. Its fixtures must contain synthetic data only. External integration tests use the same provider seam as production callers.

## Errors

Caller-visible failures use a redacted app-owned shape:

```rust
pub struct AgentError {
    pub category: AgentErrorCategory,
    pub message: String,
    pub retry_after: Option<Duration>,
}

pub enum AgentErrorCategory {
    Authentication,
    ModelUnavailable,
    Transport,
    RateLimited,
    Provider,
    InvalidConfiguration,
}
```

Adapters translate external error types into these categories. Public messages never include credentials, authorization codes, API keys, account identifiers, email addresses, authorization headers, raw provider response bodies, or credential-storage paths. Internal causes may be retained only where they cannot escape redaction. `retry_after` is present only for a safely parsed provider delay.

## Authentication

Authentication is separate from Agent Conversation. `AgentAuthentication` owns status, login, logout, credential resolution, refresh, and storage coordination. `AuthInteraction` lets the debug harness—and a future UI adapter—display authorization instructions and progress and provide secret manual input without introducing a Tauri contract into the agent module.

```rust
pub enum AuthStatus {
    NotConfigured,
    Configured,
}
```

`Configured` means only that stored OAuth credentials exist; status inspection performs no refresh and makes no validity claim. The production provider shares the underlying auth storage with `AgentAuthentication` and resolves credential validity and locked refresh internally immediately before each provider request; credentials never cross the `ConversationProvider` interface. Status never includes account data, expiry, credential source, or file path. Secret input types do not implement display, debug, or serialization.

## Hidden behavior and exclusions

Agent Conversation hides transcript assembly, active-turn state, conversation identifiers, model-capability validation, reasoning normalization, replay metadata, and stream reduction. The provider and authentication modules hide credential resolution per turn, provider request conversion, and provider error translation. Deleting these modules would force every caller to coordinate these concerns itself; their interfaces therefore earn distinct deep modules.

The accepted contract does not include tools, tool loops, queues, persisted conversations, resume or branching, compaction, token budgeting, context trimming, transcript mutation, application-domain prompt assembly, Source Profile authoring workflows, Tauri commands or events, React UI, provider registries, live model discovery, API keys, environment authentication, or providers other than OpenAI Codex. Those capabilities require separate decisions when they enter scope.
