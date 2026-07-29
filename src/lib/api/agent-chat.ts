import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type AgentChatReasoningLevel =
  | "off"
  | "minimal"
  | "low"
  | "medium"
  | "high"
  | "x_high"
  | "max";

export type AgentChatStatus =
  | "ready"
  | "running"
  | "model_unavailable"
  | "read_only_locked"
  | "read_only_unsupported"
  | "damaged"
  | "not_saved";

export type AgentChatContent =
  | { type: "text"; text: string }
  | { type: "reasoning"; text: string }
  | { type: "redacted_reasoning" };

export type AgentChatHistoryEntry =
  | {
      type: "turn";
      user: string;
      assistant: AgentChatContent[];
    }
  | {
      type: "compaction";
      reason: string | null;
      tokens_before: number;
    };

export type AgentChatRecoveryNotice = "incomplete_final_turn_discarded";

export type AgentChatProjection = {
  id: string;
  status: AgentChatStatus;
  history: AgentChatHistoryEntry[];
  selectedProviderId: string | null;
  selectedModelId: string | null;
  reasoningLevel: AgentChatReasoningLevel;
  contextTokens: number;
  contextWindow: number | null;
  recoveryNotices: AgentChatRecoveryNotice[];
};

export type AgentChatApplicationError = {
  code?: string;
  message?: string;
};

export type AgentChatCreateInput = {
  systemPrompt: string;
  providerId: string;
  modelId: string;
  reasoningLevel: AgentChatReasoningLevel;
};

export type AgentChatOpenInput = {
  id: string;
  systemPrompt: string;
};

export type AgentChatApplicationEvent = {
  chatId: string;
  sequence: number;
} & (
  | { type: "started" }
  | { type: "content_started"; index: number; kind: "text" | "reasoning" }
  | { type: "content_delta"; index: number; delta: string }
  | { type: "content_finished"; index: number }
  | { type: "completed"; chat: AgentChatProjection }
  | { type: "failed"; error: AgentChatApplicationError }
  | { type: "aborted" }
  | {
      type: "not_saved";
      response: AgentChatContent[];
      error: AgentChatApplicationError;
      chat: AgentChatProjection;
    }
  | { type: "compaction_started"; reason: string }
  | {
      type: "compaction_completed";
      reason: string;
      chat: AgentChatProjection;
    }
  | { type: "compaction_cancelled"; reason: string }
  | { type: "compaction_failed"; error: AgentChatApplicationError }
  | {
      type: "compaction_not_saved";
      error: AgentChatApplicationError;
      chat: AgentChatProjection;
    }
);

export const AGENT_CHAT_EVENT = "agent-chat-event";

export type AgentChatClient = {
  create(input: AgentChatCreateInput): Promise<AgentChatProjection>;
  open(input: AgentChatOpenInput): Promise<AgentChatProjection>;
  send(chatId: string, text: string): Promise<void>;
  stop(chatId: string): Promise<boolean>;
  setModel(
    chatId: string,
    providerId: string,
    modelId: string,
  ): Promise<AgentChatProjection>;
  setReasoningLevel(
    chatId: string,
    reasoningLevel: AgentChatReasoningLevel,
  ): Promise<AgentChatProjection>;
  compact(chatId: string, focus: string | null): Promise<void>;
  listen(handler: (event: AgentChatApplicationEvent) => void): Promise<UnlistenFn>;
};

type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
type Listen = <T>(
  event: string,
  handler: (event: { payload: T }) => void,
) => Promise<UnlistenFn>;

export function createAgentChatClient(
  invokeCommand: Invoke,
  listenToEvent: Listen,
): AgentChatClient {
  return {
    create: (input) => invokeCommand("create_agent_chat", { input }),
    open: (input) => invokeCommand("open_agent_chat", { input }),
    send: (chatId, text) =>
      invokeCommand("send_agent_chat_message", { chatId, text }),
    stop: (chatId) => invokeCommand("stop_agent_chat", { chatId }),
    setModel: (chatId, providerId, modelId) =>
      invokeCommand("set_agent_chat_model", { chatId, providerId, modelId }),
    setReasoningLevel: (chatId, reasoningLevel) =>
      invokeCommand("set_agent_chat_reasoning_level", {
        chatId,
        reasoningLevel,
      }),
    compact: (chatId, focus) =>
      invokeCommand("compact_agent_chat", { chatId, focus }),
    listen: (handler) =>
      listenToEvent<AgentChatApplicationEvent>(AGENT_CHAT_EVENT, (event) =>
        handler(event.payload),
      ),
  };
}

export const agentChatClient = createAgentChatClient(invoke, listen);
