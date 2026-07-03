# Agent-assisted application artifacts

Status: Draft issue / design note
Date: 2026-07-02

## Problem

Job Radar can already help users discover and manage Job Postings. A natural next step is to help users work on application-process artifacts for a selected Job Posting, for example fit analyses, cover-letter drafts, interview preparation notes, follow-up messages, or CV tailoring notes.

The desired interaction is iterative:

1. The user opens a UI page.
2. Job Radar builds a page/task-specific initial prompt from trusted app context.
3. An agent responds.
4. The user can correct the response, add missing facts, ask for a better version, or accept parts of the output.
5. Accepted output becomes an application artifact or a new artifact version.

Different users have different AI workflows. Some will want bring-your-own-key API access. Others may want local models, gateways, or external CLI/harness agents. The design should leave room for both without making the first implementation too large.

## Goal

Introduce a small, app-owned agent runtime for application artifacts:

- page-specific Agent Tasks,
- automatic prompt construction from domain-owned context,
- persisted conversations,
- artifact proposals instead of direct uncontrolled writes,
- provider adapters that can start with OpenAI-compatible API access and later support more providers or external agents.

## Non-goals for the first slice

- No autonomous workflow engine.
- No external CLI/harness execution in the MVP.
- No ACP integration in the MVP.
- No direct agent mutation of application artifacts without user confirmation.
- No agent-specific Rust special cases per UI page.
- No API keys in frontend state, SQLite plain text, or settings JSON.

## Candidate domain language

These terms are candidate additions and should be validated before becoming canonical vocabulary in `CONTEXT.md`.

- **Agent Task**: A named, page- or workflow-specific task Job Radar can ask an agent to perform, such as `analyze_job_fit` or `improve_cover_letter`.
- **Application Artifact**: A persisted user-facing artifact created during the application process for a Job Posting, such as a cover-letter draft, fit analysis, interview notes, or follow-up email.
- **Artifact Version**: An immutable saved version of an Application Artifact. Agent output creates a proposal; accepting it creates a version.
- **Agent Conversation**: The persisted message thread for one Agent Task, optionally associated with a Job Posting and/or Application Artifact.
- **Agent Provider**: A configured way to obtain model output, e.g. OpenAI-compatible API access, Anthropic, Ollama, or later an external agent.

## Proposed architecture

```txt
UI page
  -> Agent Task invocation
      -> Prompt Builder
          -> Agent Runtime
              -> Conversation Store
              -> Artifact Store
              -> Agent Provider Adapter
                  -> OpenAI-compatible API adapter        [MVP]
                  -> Anthropic / Google / Ollama adapters [later]
                  -> ACP / CLI external agent adapter     [later]
```

The UI should not know provider-specific request shapes. It should invoke an Agent Task with domain context and render streamed or completed messages.

## MVP slice

### 1. Agent Task Registry

Define a small registry of tasks, each with:

- stable task key,
- page/surface where it appears,
- required context inputs,
- prompt template,
- expected output mode: markdown text first, structured proposals later.

Example tasks:

- `analyze_job_fit` for a selected Job Posting,
- `draft_cover_letter` for a selected Job Posting and candidate profile,
- `improve_application_artifact` for an existing Application Artifact.

### 2. Prompt Builder

Build prompts from domain-owned context:

- Job Posting data,
- user-provided candidate/profile facts if available,
- existing Application Artifact content if present,
- prior Agent Conversation messages,
- current user instruction.

External provider DTOs should be translated at the provider adapter edge. The Prompt Builder should produce app-owned request/message types.

### 3. Conversation persistence

Persist Agent Conversations and Messages:

```txt
AgentConversation
  id
  task_key
  job_posting_id?
  artifact_id?
  provider_id?
  created_at
  updated_at

AgentMessage
  id
  conversation_id
  role: system | user | assistant
  content
  created_at
```

### 4. Application Artifact persistence

Persist artifacts separately from conversations:

```txt
ApplicationArtifact
  id
  job_posting_id
  kind: fit_analysis | cover_letter | interview_notes | follow_up | other
  title
  current_version_id?
  created_at
  updated_at

ApplicationArtifactVersion
  id
  artifact_id
  content
  created_by: user | agent
  conversation_id?
  created_at
```

Agent output should be shown as a proposal. The user explicitly accepts it to create a new Artifact Version.

### 5. OpenAI-compatible Agent Provider

Start with one provider adapter:

- provider name,
- base URL,
- model ID,
- API key reference,
- context window / output token limits,
- optional custom headers later.

This gives broad coverage for OpenAI, OpenRouter, LM Studio, local OpenAI-compatible servers, and some Ollama setups.

API keys should be managed by the Rust/Tauri backend and stored in the OS keychain where possible. Environment variables can be a later option.

## Later slices

### More direct LLM providers

Add adapters only when needed:

- Anthropic,
- Google Gemini,
- Ollama-native,
- OpenRouter first-class,
- local model servers.

### Structured artifact proposals

Move from plain markdown to structured outputs such as:

```json
{
  "proposalType": "replace_artifact_content",
  "artifactKind": "cover_letter",
  "contentMarkdown": "...",
  "rationale": "..."
}
```

### External agents / ACP

After the app-owned runtime works, consider an external-agent adapter. This is a separate category from normal model providers.

External agents usually own their own runtime, auth, tools, subscriptions, instructions, and model selection. Job Radar should treat them as a separate integration path rather than forcing them through the normal LLM provider adapter.

## Lessons from Zed

Zed is a useful architecture reference, but its code is primarily GPL-3.0-or-later. We should learn from its public architecture and documentation, not copy GPL implementation into this project without an explicit license decision.

### Zed separates model providers from external agents

Zed distinguishes:

- model access paths for Zed-owned AI features,
- External Agents, which run separately and usually own their own auth/model configuration.

References:

- LLM Providers overview: <https://zed.dev/docs/ai/llm-providers>
- Use API Access / BYOK: <https://zed.dev/docs/ai/use-api-access>
- Agent configuration overview: <https://zed.dev/docs/assistant/configuration>
- External Agents: <https://zed.dev/docs/ai/external-agents>

### Zed provider architecture references

Relevant Zed source permalinks from commit `17090674b34288db75128f96dfb336116e058ff2`:

- `LanguageModel` trait with `stream_completion`:
  <https://github.com/zed-industries/zed/blob/17090674b34288db75128f96dfb336116e058ff2/crates/language_model/src/language_model.rs#L53-L171>
- `LanguageModelProvider` trait:
  <https://github.com/zed-industries/zed/blob/17090674b34288db75128f96dfb336116e058ff2/crates/language_model/src/language_model.rs#L309-L396>
- provider registry shape:
  <https://github.com/zed-industries/zed/blob/17090674b34288db75128f96dfb336116e058ff2/crates/language_model/src/registry.rs#L45-L60>
- provider registration:
  <https://github.com/zed-industries/zed/blob/17090674b34288db75128f96dfb336116e058ff2/crates/language_models/src/language_models.rs#L262-L330>
- dynamic OpenAI-/Anthropic-compatible provider registration from settings:
  <https://github.com/zed-industries/zed/blob/17090674b34288db75128f96dfb336116e058ff2/crates/language_models/src/language_models.rs#L187-L260>
- OpenAI-compatible provider implementation shape:
  <https://github.com/zed-industries/zed/blob/17090674b34288db75128f96dfb336116e058ff2/crates/language_models/src/provider/open_ai_compatible.rs#L49-L174>
- OpenAI-compatible request streaming and mapping:
  <https://github.com/zed-industries/zed/blob/17090674b34288db75128f96dfb336116e058ff2/crates/language_models/src/provider/open_ai_compatible.rs#L176-L467>

### Zed credential handling references

- API keys are represented as a state object that can load from environment variables or system keychain:
  <https://github.com/zed-industries/zed/blob/17090674b34288db75128f96dfb336116e058ff2/crates/language_model/src/api_key.rs#L14-L24>
- Key lookup prefers non-empty environment variables before keychain loading:
  <https://github.com/zed-industries/zed/blob/17090674b34288db75128f96dfb336116e058ff2/crates/language_model/src/api_key.rs#L155-L203>
- API-compatible provider state stores/removes API keys via a credentials provider:
  <https://github.com/zed-industries/zed/blob/17090674b34288db75128f96dfb336116e058ff2/crates/language_models/src/provider/api_compatible.rs#L17-L97>

### Zed conversation/thread references

- Thread messages are converted to provider request messages through app-owned message types:
  <https://github.com/zed-industries/zed/blob/17090674b34288db75128f96dfb336116e058ff2/crates/agent/src/thread.rs#L181-L258>
- User message content can include text, images, and mentions/context attachments:
  <https://github.com/zed-industries/zed/blob/17090674b34288db75128f96dfb336116e058ff2/crates/agent/src/thread.rs#L260-L360>
- Completion requests are assembled from thread state, available tools, model settings, and messages:
  <https://github.com/zed-industries/zed/blob/17090674b34288db75128f96dfb336116e058ff2/crates/agent/src/thread.rs#L3886-L3950>
- System prompts are built centrally before request history is appended:
  <https://github.com/zed-industries/zed/blob/17090674b34288db75128f96dfb336116e058ff2/crates/agent/src/thread.rs#L4130-L4168>

### Zed external-agent / ACP references

- Zed's External Agents are ACP-based and are configured separately from LLM providers:
  <https://zed.dev/docs/ai/external-agents>
- ACP repository:
  <https://github.com/agentclientprotocol/agent-client-protocol>
- Zed `AgentServer` abstraction:
  <https://github.com/zed-industries/zed/blob/17090674b34288db75128f96dfb336116e058ff2/crates/agent_servers/src/agent_servers.rs#L50-L104>
- Custom agent server config and environment handling:
  <https://github.com/zed-industries/zed/blob/17090674b34288db75128f96dfb336116e058ff2/crates/agent_servers/src/custom.rs#L193-L281>
- ACP stdio process spawning and JSON-lines transport setup:
  <https://github.com/zed-industries/zed/blob/17090674b34288db75128f96dfb336116e058ff2/crates/agent_servers/src/acp.rs#L805-L940>
- ACP client initialization/handshake:
  <https://github.com/zed-industries/zed/blob/17090674b34288db75128f96dfb336116e058ff2/crates/agent_servers/src/acp.rs#L981-L1045>

## Lessons from T3 Code

T3 Code is another useful architecture reference. Its code is MIT-licensed, but it solves a different problem: it is a web/desktop control plane for coding agents and external provider runtimes, not a small app-owned text generation runtime for domain artifacts. Use it as inspiration for provider/runtime seams and later external-agent integrations, not as MVP scope.

T3 Code's README describes it as a minimal web GUI for coding agents such as Codex, Claude, Cursor, and OpenCode:
<https://github.com/pingdotgg/t3code/blob/32d17d3db55187b48389c005a319135b0badfea2/README.md#L3-L14>

### T3 Code architecture references

Relevant T3 Code source permalinks from commit `32d17d3db55187b48389c005a319135b0badfea2`:

- local Node.js WebSocket server between a React UI and provider runtimes:
  <https://github.com/pingdotgg/t3code/blob/32d17d3db55187b48389c005a319135b0badfea2/docs/architecture/overview.md#L3-L38>
- provider adapter contract with session lifecycle, turns, approvals, rollback, and canonical event stream:
  <https://github.com/pingdotgg/t3code/blob/32d17d3db55187b48389c005a319135b0badfea2/apps/server/src/provider/Services/ProviderAdapter.ts#L45-L125>
- provider service as cross-provider router/validator that resolves provider instances before calling adapters:
  <https://github.com/pingdotgg/t3code/blob/32d17d3db55187b48389c005a319135b0badfea2/apps/server/src/provider/Layers/ProviderService.ts#L522-L705>
- Codex adapter translating app-owned turn inputs into provider runtime calls:
  <https://github.com/pingdotgg/t3code/blob/32d17d3db55187b48389c005a319135b0badfea2/apps/server/src/provider/Layers/CodexAdapter.ts#L1369-L1570>
- canonical runtime event vocabulary for provider-native events:
  <https://github.com/pingdotgg/t3code/blob/32d17d3db55187b48389c005a319135b0badfea2/packages/contracts/src/providerRuntime.ts#L148-L178>
- provider instance model separating driver kind from user-configured instance id:
  <https://github.com/pingdotgg/t3code/blob/32d17d3db55187b48389c005a319135b0badfea2/packages/contracts/src/providerInstance.ts#L1-L33>
- provider instance config envelope with opaque driver-specific config:
  <https://github.com/pingdotgg/t3code/blob/32d17d3db55187b48389c005a319135b0badfea2/packages/contracts/src/providerInstance.ts#L115-L131>
- sensitive provider environment values stored separately and redacted from settings:
  <https://github.com/pingdotgg/t3code/blob/32d17d3db55187b48389c005a319135b0badfea2/apps/server/src/serverSettings.ts#L325-L463>
- file-based server secret store with restrictive permissions:
  <https://github.com/pingdotgg/t3code/blob/32d17d3db55187b48389c005a319135b0badfea2/apps/server/src/auth/ServerSecretStore.ts#L158-L203>
- ACP session runtime as later external-agent prior art, not MVP scope:
  <https://github.com/pingdotgg/t3code/blob/32d17d3db55187b48389c005a319135b0badfea2/apps/server/src/provider/acp/AcpSessionRuntime.ts#L86-L165>

### T3 Code takeaways for Job Radar

- Keep the UI behind app-owned commands/events; do not expose provider-native DTOs to React pages.
- Use a provider adapter seam, but make Job Radar's MVP adapter smaller than T3 Code's coding-agent contract.
- Prefer `AgentProviderConfig.id` / provider instance ids over assuming one configuration per provider kind.
- Normalize provider output into app-owned Agent Messages or Agent Events before persistence.
- Keep sensitive provider values outside ordinary settings rows/documents and redact them when settings are read back.
- Treat external CLI agents and ACP as a later integration category, separate from normal LLM provider adapters.
- Do not import coding-agent-specific concepts such as worktrees, terminal tools, checkpoint rollback, or file mutation approvals into the application-artifact MVP unless a concrete Job Radar use case appears.

## Other open-source references

- Vercel AI SDK: provider-agnostic TypeScript AI toolkit with streaming, structured output, tools, and UI support.
  <https://github.com/vercel/ai>
- assistant-ui: React chat/thread UI primitives and adapters for production chat UX.
  <https://github.com/assistant-ui/assistant-ui>
- Continue: open-source coding-agent reference, now read-only, useful as prior art for configuration and agent UX patterns.
  <https://github.com/continuedev/continue>

## Open questions

1. What is the first Application Artifact kind Job Radar should support?
   - fit analysis,
   - cover letter,
   - interview notes,
   - follow-up email?
2. Where should candidate/user profile facts live?
3. Should Agent Conversations be tied to one Job Posting, one Artifact, or both?
4. Should the MVP use streaming responses or simple request/response first?
5. Which keychain crate/API should the Rust backend use?
6. Should OpenAI-compatible provider configuration live in SQLite, settings JSON, or both?
7. How should accepted agent output be shown in the Job Posting Queue?
8. Do we need a per-task safety policy before any generated content can be saved?

## Acceptance criteria for the design phase

- A reviewed PRD exists for the MVP scope.
- Candidate domain terms are either accepted into `CONTEXT.md` or kept local to this PRD.
- Provider adapter seam is documented with app-owned request/response types.
- Credential storage approach is documented before implementation.
- The first Agent Task is chosen.
- MVP excludes ACP/CLI execution explicitly.
- Follow-up issues can be cut into small implementation slices.

## Possible implementation issues

1. Add app-owned Agent Task and message types.
2. Add SQLite tables for Agent Conversations and Messages.
3. Add SQLite tables for Application Artifacts and Artifact Versions.
4. Add Rust command/API for provider configuration and keychain-backed API key storage.
5. Add OpenAI-compatible provider adapter.
6. Add first page-level Agent Task, probably Job Posting fit analysis.
7. Add UI conversation panel for one task.
8. Add accept-as-artifact-version flow.
9. Add tests around prompt construction, provider DTO translation, and artifact versioning.
