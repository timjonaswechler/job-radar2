import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import {
  isArrayOf,
  isNonNegativeSafeInteger,
  isRecord,
} from "@/lib/api/wire";

const MAX_IDENTIFIER_LENGTH = 128;
const MAX_TEXT_LENGTH = 1_000_000;
const MAX_WIRE_COUNT = 10_000_000;
const MAX_HISTORY_ENTRIES = 10_000;
const MAX_CONTENT_BLOCKS = 10_000;

export type AgentChatReasoningLevel =
  | "off"
  | "minimal"
  | "low"
  | "medium"
  | "high"
  | "x_high"
  | "max";

const reasoningLevels = [
  "off",
  "minimal",
  "low",
  "medium",
  "high",
  "x_high",
  "max",
] as const;

export type AgentChatStatus =
  | "ready"
  | "running"
  | "model_unavailable"
  | "read_only_locked"
  | "read_only_unsupported"
  | "damaged"
  | "not_saved";

const chatStatuses = [
  "ready",
  "running",
  "model_unavailable",
  "read_only_locked",
  "read_only_unsupported",
  "damaged",
  "not_saved",
] as const;

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

export type AgentChatOperationId = number;

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

export class AgentChatTransportError extends Error {
  readonly code: string;

  constructor(code = "transport_unavailable") {
    super("Agent Chat transport is unavailable.");
    this.name = "AgentChatTransportError";
    this.code = code;
  }
}

export type AgentChatClient = {
  create(input: AgentChatCreateInput): Promise<AgentChatProjection>;
  open(input: AgentChatOpenInput): Promise<AgentChatProjection>;
  snapshot(chatId: string): Promise<AgentChatProjection>;
  reload(chatId: string): Promise<AgentChatProjection>;
  send(chatId: string, text: string): Promise<AgentChatOperationId>;
  stop(chatId: string, operationId: AgentChatOperationId | null): Promise<boolean>;
  setModel(
    chatId: string,
    providerId: string,
    modelId: string,
  ): Promise<AgentChatProjection>;
  setReasoningLevel(
    chatId: string,
    reasoningLevel: AgentChatReasoningLevel,
  ): Promise<AgentChatProjection>;
  compact(chatId: string, focus: string | null): Promise<AgentChatOperationId>;
  listen(
    handler: (event: AgentChatApplicationEvent) => void,
    onMalformed?: () => void,
  ): Promise<UnlistenFn>;
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
    create: (input) =>
      invokeDecoded(invokeCommand, "create_agent_chat", { input }, decodeAgentChatProjection),
    open: (input) =>
      invokeDecoded(invokeCommand, "open_agent_chat", { input }, decodeAgentChatProjection),
    snapshot: (chatId) =>
      invokeDecoded(
        invokeCommand,
        "snapshot_agent_chat",
        { chatId },
        decodeAgentChatProjection,
      ),
    reload: (chatId) =>
      invokeDecoded(invokeCommand, "reload_agent_chat", { chatId }, decodeAgentChatProjection),
    send: (chatId, text) =>
      invokeDecoded(
        invokeCommand,
        "send_agent_chat_message",
        { chatId, text },
        decodeAgentChatOperationId,
      ),
    stop: (chatId, operationId) =>
      invokeDecoded(
        invokeCommand,
        "stop_agent_chat",
        { chatId, operationId },
        decodeBoolean,
      ),
    setModel: (chatId, providerId, modelId) =>
      invokeDecoded(
        invokeCommand,
        "set_agent_chat_model",
        { chatId, providerId, modelId },
        decodeAgentChatProjection,
      ),
    setReasoningLevel: (chatId, reasoningLevel) =>
      invokeDecoded(
        invokeCommand,
        "set_agent_chat_reasoning_level",
        { chatId, reasoningLevel },
        decodeAgentChatProjection,
      ),
    compact: (chatId, focus) =>
      invokeDecoded(
        invokeCommand,
        "compact_agent_chat",
        { chatId, focus },
        decodeAgentChatOperationId,
      ),
    listen: (handler, onMalformed) =>
      listenToEvent<unknown>(AGENT_CHAT_EVENT, (event) => {
        try {
          handler(decodeAgentChatEvent(event.payload));
        } catch {
          onMalformed?.();
        }
      }),
  };
}

export function decodeAgentChatProjection(value: unknown): AgentChatProjection {
  if (
    !isRecord(value) ||
    !isIdentifier(value.id) ||
    !includes(chatStatuses, value.status) ||
    !isArrayOf(value.history, isAgentChatHistoryEntry) ||
    value.history.length > MAX_HISTORY_ENTRIES ||
    !isNullableIdentifier(value.selectedProviderId) ||
    !isNullableIdentifier(value.selectedModelId) ||
    !includes(reasoningLevels, value.reasoningLevel) ||
    !isBoundedCount(value.contextTokens) ||
    !isNullableBoundedCount(value.contextWindow) ||
    !isArrayOf(value.recoveryNotices, isRecoveryNotice) ||
    value.recoveryNotices.length > MAX_WIRE_COUNT
  ) {
    throw invalidResponse("Agent Chat projection");
  }

  return {
    id: value.id,
    status: value.status,
    history: value.history,
    selectedProviderId: value.selectedProviderId,
    selectedModelId: value.selectedModelId,
    reasoningLevel: value.reasoningLevel,
    contextTokens: value.contextTokens,
    contextWindow: value.contextWindow,
    recoveryNotices: value.recoveryNotices,
  };
}

export function decodeAgentChatEvent(value: unknown): AgentChatApplicationEvent {
  if (
    !isRecord(value) ||
    !isIdentifier(value.chatId) ||
    !isPositiveBoundedCount(value.sequence) ||
    typeof value.type !== "string"
  ) {
    throw invalidResponse("Agent Chat event");
  }

  const base = { chatId: value.chatId, sequence: value.sequence };
  switch (value.type) {
    case "started":
    case "aborted":
      return { ...base, type: value.type };
    case "content_started":
      if (
        !isBoundedIndex(value.index) ||
        !includes(["text", "reasoning"] as const, value.kind)
      ) {
        throw invalidResponse("Agent Chat event");
      }
      return { ...base, type: value.type, index: value.index, kind: value.kind };
    case "content_delta":
      if (!isBoundedIndex(value.index) || !isBoundedText(value.delta)) {
        throw invalidResponse("Agent Chat event");
      }
      return { ...base, type: value.type, index: value.index, delta: value.delta };
    case "content_finished":
      if (!isBoundedIndex(value.index)) throw invalidResponse("Agent Chat event");
      return { ...base, type: value.type, index: value.index };
    case "completed":
      return {
        ...base,
        type: value.type,
        chat: decodeAgentChatProjection(value.chat),
      };
    case "failed":
      return { ...base, type: value.type, error: decodeAgentChatError(value.error) };
    case "not_saved":
      if (
        !isArrayOf(value.response, isAgentChatContent) ||
        value.response.length > MAX_CONTENT_BLOCKS
      ) {
        throw invalidResponse("Agent Chat event");
      }
      return {
        ...base,
        type: value.type,
        response: value.response,
        error: decodeAgentChatError(value.error),
        chat: decodeAgentChatProjection(value.chat),
      };
    case "compaction_started":
    case "compaction_cancelled":
      if (!isBoundedText(value.reason)) throw invalidResponse("Agent Chat event");
      return { ...base, type: value.type, reason: value.reason };
    case "compaction_completed":
      if (!isBoundedText(value.reason)) throw invalidResponse("Agent Chat event");
      return {
        ...base,
        type: value.type,
        reason: value.reason,
        chat: decodeAgentChatProjection(value.chat),
      };
    case "compaction_failed":
      return {
        ...base,
        type: value.type,
        error: decodeAgentChatError(value.error),
      };
    case "compaction_not_saved":
      return {
        ...base,
        type: value.type,
        error: decodeAgentChatError(value.error),
        chat: decodeAgentChatProjection(value.chat),
      };
    default:
      throw invalidResponse("Agent Chat event");
  }
}

export function decodeAgentChatError(value: unknown): AgentChatApplicationError {
  if (
    !isRecord(value) ||
    !isSafeErrorCode(value.code) ||
    !isBoundedText(value.message)
  ) {
    throw invalidResponse("Agent Chat error");
  }
  return { code: value.code, message: "Agent Chat operation failed." };
}

function decodeAgentChatOperationId(value: unknown): AgentChatOperationId {
  if (!isBoundedCount(value)) throw invalidResponse("Agent Chat operation ID");
  return value;
}

function decodeBoolean(value: unknown): boolean {
  if (typeof value !== "boolean") throw invalidResponse("Agent Chat result");
  return value;
}

async function invokeDecoded<T>(
  invokeCommand: Invoke,
  command: string,
  args: Record<string, unknown>,
  decode: (value: unknown) => T,
): Promise<T> {
  let value: unknown;
  try {
    value = await invokeCommand<unknown>(command, args);
  } catch (error) {
    throw decodeAgentChatCommandError(error);
  }
  return decode(value);
}

function decodeAgentChatCommandError(error: unknown): AgentChatTransportError {
  if (error instanceof AgentChatTransportError) return error;
  if (isRecord(error) && isSafeErrorCode(error.code)) {
    return new AgentChatTransportError(error.code);
  }
  return new AgentChatTransportError();
}

function isAgentChatHistoryEntry(value: unknown): value is AgentChatHistoryEntry {
  if (!isRecord(value) || typeof value.type !== "string") return false;
  if (value.type === "turn") {
    return (
      isBoundedText(value.user) &&
      isArrayOf(value.assistant, isAgentChatContent) &&
      value.assistant.length <= MAX_CONTENT_BLOCKS
    );
  }
  return (
    value.type === "compaction" &&
    (value.reason === null || isBoundedText(value.reason)) &&
    isBoundedCount(value.tokens_before)
  );
}

function isAgentChatContent(value: unknown): value is AgentChatContent {
  if (!isRecord(value) || typeof value.type !== "string") return false;
  if (value.type === "redacted_reasoning") return true;
  return (
    (value.type === "text" || value.type === "reasoning") &&
    isBoundedText(value.text)
  );
}

function isRecoveryNotice(value: unknown): value is AgentChatRecoveryNotice {
  return value === "incomplete_final_turn_discarded";
}

function isIdentifier(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    value.length <= MAX_IDENTIFIER_LENGTH &&
    /^[A-Za-z0-9._-]+$/.test(value)
  );
}

function isNullableIdentifier(value: unknown): value is string | null {
  return value === null || isIdentifier(value);
}

function isBoundedText(value: unknown): value is string {
  return typeof value === "string" && value.length <= MAX_TEXT_LENGTH;
}

function isBoundedCount(value: unknown): value is number {
  return isNonNegativeSafeInteger(value) && value <= MAX_WIRE_COUNT;
}

function isPositiveBoundedCount(value: unknown): value is number {
  return isBoundedCount(value) && value > 0;
}

function isNullableBoundedCount(value: unknown): value is number | null {
  return value === null || isBoundedCount(value);
}

function isBoundedIndex(value: unknown): value is number {
  return isBoundedCount(value);
}

function isSafeErrorCode(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    value.length <= 128 &&
    /^[a-z0-9_]+$/.test(value)
  );
}

function invalidResponse(label: string): Error {
  return new Error(`Invalid ${label} response.`);
}

function includes<const T extends string>(values: readonly T[], value: unknown): value is T {
  return values.includes(value as T);
}

export const agentChatClient = createAgentChatClient(invoke, listen);
