import { useEffect, useRef, useState } from "react";

import {
  agentChatClient,
  type AgentChatApplicationEvent,
  type AgentChatClient,
  type AgentChatContent,
  type AgentChatCreateInput,
  type AgentChatOpenInput,
  type AgentChatProjection,
  type AgentChatReasoningLevel,
} from "@/lib/api/agent-chat";
import {
  agentConfigurationClient,
  type AgentConfigurationClient,
  type AgentConfigurationStatus,
} from "@/lib/api/agent-configuration";

export type AgentChatRequest =
  | ({ type: "create" } & AgentChatCreateInput)
  | ({ type: "open" } & AgentChatOpenInput);

type SendOperation = {
  type: "send";
  phase: "submitted" | "streaming";
  token: number;
  operationId: number | null;
  user: string;
  blocks: StreamBlock[];
};

type CompactOperation = {
  type: "compact";
  phase: "submitted" | "streaming";
  token: number;
  operationId: number | null;
  reason: string | null;
};

type ActiveOperation = {
  type: "active";
  phase: "streaming";
  token: number;
  operationId: number | null;
};

type StreamBlock = {
  index: number;
  type: "text" | "reasoning";
  text: string;
};

export type AgentChatOperation =
  | SendOperation
  | CompactOperation
  | ActiveOperation;

export type AgentChatReadyState = {
  type: "ready";
  chat: AgentChatProjection;
  configuration: AgentConfigurationStatus;
  operation: AgentChatOperation | null;
  unsavedTurn: { user: string; response: AgentChatContent[] } | null;
  errorCode: string | null;
  notice: "aborted" | "compacted" | "compaction_cancelled" | null;
};

export type AgentChatLifecycleState =
  | { type: "loading" }
  | { type: "failed"; code: string }
  | AgentChatReadyState;

export type AgentChatLifecycle = {
  state: AgentChatLifecycleState;
  submit(text: string): void;
  stop(): void;
  reload(): void;
  compact(): void;
  selectModel(providerId: string, modelId: string): void;
  selectReasoning(reasoningLevel: AgentChatReasoningLevel): void;
};

type UseAgentChatOptions = {
  request: AgentChatRequest;
  chatClient?: AgentChatClient;
  configurationClient?: AgentConfigurationClient;
};

export function useAgentChat({
  request,
  chatClient: chatApi = agentChatClient,
  configurationClient: configurationApi = agentConfigurationClient,
}: UseAgentChatOptions): AgentChatLifecycle {
  const [state, setState] = useState<AgentChatLifecycleState>({ type: "loading" });
  const generationRef = useRef(0);
  const chatIdRef = useRef<string | null>(null);
  const sequenceRef = useRef(0);
  const operationTokenRef = useRef(0);
  const selectionTokenRef = useRef(0);
  const reloadTokenRef = useRef(0);

  useEffect(() => {
    const generation = generationRef.current + 1;
    generationRef.current = generation;
    chatIdRef.current = null;
    sequenceRef.current = 0;
    let active = true;
    let unlisten: (() => void) | undefined;

    setState({ type: "loading" });

    const handleMalformedEvent = () => {
      if (!active) return;
      setState((current) =>
        current.type === "ready"
          ? { ...current, errorCode: "transport_unavailable" }
          : current,
      );
    };

    const initialize = async () => {
      try {
        const stopListening = await chatApi.listen(
          (event) => {
            if (
              !active ||
              generationRef.current !== generation ||
              event.chatId !== chatIdRef.current ||
              event.sequence <= sequenceRef.current
            ) {
              return;
            }
            if (hasMismatchedProjection(event)) return;
            sequenceRef.current = event.sequence;
            setState((current) => reduceEvent(current, event));
          },
          handleMalformedEvent,
        );
        if (!active || generationRef.current !== generation) {
          stopListening();
          return;
        }
        unlisten = stopListening;

        const [chat, configuration] = await Promise.all([
          request.type === "create"
            ? chatApi.create({
                systemPrompt: request.systemPrompt,
                providerId: request.providerId,
                modelId: request.modelId,
                reasoningLevel: request.reasoningLevel,
              })
            : chatApi.open({
                id: request.id,
                systemPrompt: request.systemPrompt,
              }),
          configurationApi.getStatus(),
        ]);
        if (!active || generationRef.current !== generation) return;

        chatIdRef.current = chat.id;
        sequenceRef.current = 0;
        setState({
          type: "ready",
          chat,
          configuration,
          operation:
            chat.status === "running"
              ? {
                  type: "active",
                  phase: "streaming",
                  token: 0,
                  operationId: null,
                }
              : null,
          unsavedTurn: null,
          errorCode: null,
          notice: null,
        });
      } catch (error) {
        unlisten?.();
        unlisten = undefined;
        if (active && generationRef.current === generation) {
          setState({ type: "failed", code: errorCode(error) });
        }
      }
    };

    void initialize();
    return () => {
      active = false;
      chatIdRef.current = null;
      sequenceRef.current = 0;
      unlisten?.();
    };
  }, [
    chatApi,
    configurationApi,
    request.type,
    request.systemPrompt,
    request.type === "open" ? request.id : request.providerId,
    request.type === "create" ? request.modelId : null,
    request.type === "create" ? request.reasoningLevel : null,
  ]);

  const submit = (text: string) => {
    const trimmed = text.trim();
    if (!trimmed || state.type !== "ready" || state.chat.status !== "ready" || state.operation) {
      return;
    }
    const generation = generationRef.current;
    const chatId = state.chat.id;
    const token = ++operationTokenRef.current;
    setState((current) =>
      current.type === "ready"
        ? {
            ...current,
            operation: {
              type: "send",
              phase: "submitted",
              token,
              operationId: null,
              user: trimmed,
              blocks: [],
            },
            unsavedTurn: null,
            errorCode: null,
            notice: null,
          }
        : current,
    );

    void (async () => {
      try {
        const operationId = await chatApi.send(chatId, trimmed);
        if (!isCurrent(generation, chatId)) return;
        setState((current) =>
          current.type === "ready" &&
          current.operation?.type === "send" &&
          current.operation.token === token
            ? {
                ...current,
                operation: { ...current.operation, operationId },
              }
            : current,
        );
      } catch (error) {
        const chat = await chatApi.snapshot(chatId).catch(() => null);
        if (!isCurrent(generation, chatId)) return;
        setState((current) =>
          current.type === "ready" &&
          current.operation?.type === "send" &&
          current.operation.token === token
            ? {
                ...current,
                chat: chat ?? current.chat,
                operation: null,
                errorCode: errorCode(error),
              }
            : current,
        );
      }
    })();
  };

  const stop = () => {
    if (state.type !== "ready" || !state.operation) return;
    const generation = generationRef.current;
    const chatId = state.chat.id;
    const token = state.operation.token;
    const operationId = state.operation.operationId;

    void (async () => {
      try {
        const stopped = await chatApi.stop(chatId, operationId);
        if (!isCurrent(generation, chatId)) return;
        if (!stopped) {
          setState((current) =>
            current.type === "ready" && current.operation?.token === token
              ? { ...current, operation: null, errorCode: "not_running" }
              : current,
          );
        }
      } catch (error) {
        if (!isCurrent(generation, chatId)) return;
        setState((current) =>
          current.type === "ready" && current.operation?.token === token
            ? { ...current, errorCode: errorCode(error) }
            : current,
        );
      }
    })();
  };

  const reload = () => {
    if (state.type !== "ready") return;
    const generation = generationRef.current;
    const chatId = state.chat.id;
    const token = ++reloadTokenRef.current;
    void (async () => {
      try {
        const chat = await chatApi.reload(chatId);
        if (!isCurrent(generation, chatId) || reloadTokenRef.current !== token) return;
        setState((current) =>
          current.type === "ready"
            ? {
                ...current,
                chat,
                operation: null,
                unsavedTurn: null,
                errorCode: null,
                notice: null,
              }
            : current,
        );
      } catch (error) {
        if (!isCurrent(generation, chatId) || reloadTokenRef.current !== token) return;
        setState((current) =>
          current.type === "ready"
            ? { ...current, errorCode: errorCode(error) }
            : current,
        );
      }
    })();
  };

  const compact = () => {
    if (state.type !== "ready" || state.chat.status !== "ready" || state.operation) {
      return;
    }
    const generation = generationRef.current;
    const chatId = state.chat.id;
    const token = ++operationTokenRef.current;
    setState((current) =>
      current.type === "ready"
        ? {
            ...current,
            operation: {
              type: "compact",
              phase: "submitted",
              token,
              operationId: null,
              reason: null,
            },
            errorCode: null,
            notice: null,
          }
        : current,
    );

    void (async () => {
      try {
        const operationId = await chatApi.compact(chatId, null);
        if (!isCurrent(generation, chatId)) return;
        setState((current) =>
          current.type === "ready" &&
          current.operation?.type === "compact" &&
          current.operation.token === token
            ? {
                ...current,
                operation: { ...current.operation, operationId },
              }
            : current,
        );
      } catch (error) {
        const chat = await chatApi.snapshot(chatId).catch(() => null);
        if (!isCurrent(generation, chatId)) return;
        setState((current) =>
          current.type === "ready" &&
          current.operation?.type === "compact" &&
          current.operation.token === token
            ? {
                ...current,
                chat: chat ?? current.chat,
                operation: null,
                errorCode: errorCode(error),
              }
            : current,
        );
      }
    })();
  };

  const selectModel = (providerId: string, modelId: string) => {
    if (
      state.type !== "ready" ||
      state.operation ||
      (state.chat.status !== "ready" && state.chat.status !== "model_unavailable")
    ) {
      return;
    }
    const generation = generationRef.current;
    const chatId = state.chat.id;
    const token = ++selectionTokenRef.current;
    void (async () => {
      try {
        const chat = await chatApi.setModel(chatId, providerId, modelId);
        if (!isCurrent(generation, chatId) || selectionTokenRef.current !== token) return;
        setState((current) =>
          current.type === "ready"
            ? { ...current, chat, errorCode: null, notice: null }
            : current,
        );
      } catch (error) {
        const chat = await chatApi.snapshot(chatId).catch(() => null);
        if (!isCurrent(generation, chatId) || selectionTokenRef.current !== token) return;
        setState((current) =>
          current.type === "ready"
            ? { ...current, chat: chat ?? current.chat, errorCode: errorCode(error) }
            : current,
        );
      }
    })();
  };

  const selectReasoning = (reasoningLevel: AgentChatReasoningLevel) => {
    if (
      state.type !== "ready" ||
      state.operation ||
      state.chat.status !== "ready"
    ) {
      return;
    }
    const generation = generationRef.current;
    const chatId = state.chat.id;
    const token = ++selectionTokenRef.current;
    void (async () => {
      try {
        const chat = await chatApi.setReasoningLevel(chatId, reasoningLevel);
        if (!isCurrent(generation, chatId) || selectionTokenRef.current !== token) return;
        setState((current) =>
          current.type === "ready"
            ? { ...current, chat, errorCode: null, notice: null }
            : current,
        );
      } catch (error) {
        const chat = await chatApi.snapshot(chatId).catch(() => null);
        if (!isCurrent(generation, chatId) || selectionTokenRef.current !== token) return;
        setState((current) =>
          current.type === "ready"
            ? { ...current, chat: chat ?? current.chat, errorCode: errorCode(error) }
            : current,
        );
      }
    })();
  };

  const isCurrent = (generation: number, chatId: string) =>
    generationRef.current === generation && chatIdRef.current === chatId;

  return { state, submit, stop, reload, compact, selectModel, selectReasoning };
}

function reduceEvent(
  current: AgentChatLifecycleState,
  event: AgentChatApplicationEvent,
): AgentChatLifecycleState {
  if (current.type !== "ready") return current;

  switch (event.type) {
    case "started":
      return current.operation?.type === "send"
        ? {
            ...current,
            operation: { ...current.operation, phase: "streaming" },
          }
        : current;
    case "content_started":
      if (current.operation?.type !== "send") return current;
      return {
        ...current,
        operation: {
          ...current.operation,
          phase: "streaming",
          blocks: current.operation.blocks.some((block) => block.index === event.index)
            ? current.operation.blocks
            : [
                ...current.operation.blocks,
                { index: event.index, type: event.kind, text: "" },
              ],
        },
      };
    case "content_delta":
      if (current.operation?.type !== "send") return current;
      return {
        ...current,
        operation: {
          ...current.operation,
          phase: "streaming",
          blocks: current.operation.blocks.map((block) =>
            block.index === event.index
              ? { ...block, text: block.text + event.delta }
              : block,
          ),
        },
      };
    case "completed":
      return {
        ...current,
        chat: event.chat,
        operation: null,
        unsavedTurn: null,
        errorCode: null,
        notice: null,
      };
    case "failed":
      return {
        ...current,
        operation: null,
        errorCode: event.error.code ?? "unavailable",
      };
    case "not_saved":
      return {
        ...current,
        chat: event.chat,
        operation: null,
        unsavedTurn: {
          user: current.operation?.type === "send" ? current.operation.user : "",
          response: event.response,
        },
        errorCode: event.error.code ?? "not_saved",
        notice: null,
      };
    case "aborted":
      return {
        ...current,
        operation: null,
        errorCode: null,
        notice:
          current.operation?.type === "compact"
            ? "compaction_cancelled"
            : "aborted",
      };
    case "compaction_started":
      return {
        ...current,
        operation: {
          type: "compact",
          phase: "streaming",
          token:
            current.operation?.type === "compact"
              ? current.operation.token
              : 0,
          operationId:
            current.operation?.type === "compact"
              ? current.operation.operationId
              : null,
          reason: event.reason,
        },
        errorCode: null,
        notice: null,
      };
    case "compaction_completed":
      return {
        ...current,
        chat: event.chat,
        operation: null,
        errorCode: null,
        notice: "compacted",
      };
    case "compaction_cancelled":
      return {
        ...current,
        operation: null,
        errorCode: null,
        notice: "compaction_cancelled",
      };
    case "compaction_failed":
      return {
        ...current,
        operation: null,
        errorCode: event.error.code ?? "provider_failed",
      };
    case "compaction_not_saved":
      return {
        ...current,
        chat: event.chat,
        operation: null,
        errorCode: event.error.code ?? "not_saved",
      };
  }
  return current;
}

function hasMismatchedProjection(event: AgentChatApplicationEvent): boolean {
  return "chat" in event && event.chat.id !== event.chatId;
}

function errorCode(error: unknown): string {
  return isRecordWithStringCode(error) ? error.code : "unavailable";
}

function isRecordWithStringCode(
  value: unknown,
): value is { code: string } {
  return (
    typeof value === "object" &&
    value !== null &&
    "code" in value &&
    typeof value.code === "string"
  );
}
