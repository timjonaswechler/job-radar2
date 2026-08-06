import { useCallback, useEffect, useRef, useState } from "react";

import {
  agentChatClient,
  type AgentChatApplicationEvent,
  type AgentChatClient,
  type AgentChatOperationId,
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

type AgentChatOperationToken = {
  readonly value: number;
};

type CurrentOperation = {
  generation: number;
  chatId: string;
  token: AgentChatOperationToken;
  operationId: AgentChatOperationId | null;
};

type SendOperation = {
  type: "send";
  phase: "submitted" | "streaming";
  token: AgentChatOperationToken;
  operationId: AgentChatOperationId | null;
  user: string;
  blocks: StreamBlock[];
};

type CompactOperation = {
  type: "compact";
  phase: "submitted" | "streaming";
  token: AgentChatOperationToken;
  operationId: AgentChatOperationId | null;
  reason: string | null;
};

type ActiveOperation = {
  type: "active";
  phase: "streaming";
  token: AgentChatOperationToken;
  operationId: AgentChatOperationId | null;
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
  const operationRef = useRef<CurrentOperation | null>(null);
  const pendingEventsRef = useRef<AgentChatApplicationEvent[]>([]);

  const acceptEvent = useCallback((event: AgentChatApplicationEvent) => {
    const currentOperation = operationRef.current;
    if (!currentOperation) {
      if (chatIdRef.current === null && pendingEventsRef.current.length < 10_000) {
        pendingEventsRef.current.push(event);
      }
      return;
    }
    if (
      currentOperation.generation !== generationRef.current ||
      currentOperation.chatId !== chatIdRef.current ||
      event.chatId !== currentOperation.chatId
    ) {
      return;
    }
    if (currentOperation.operationId === null) {
      if (pendingEventsRef.current.length < 10_000) {
        pendingEventsRef.current.push(event);
      }
      return;
    }
    if (
      event.operationId !== currentOperation.operationId ||
      event.sequence <= sequenceRef.current ||
      hasMismatchedProjection(event)
    ) {
      return;
    }
    sequenceRef.current = event.sequence;
    if (isTerminalEvent(event)) {
      operationRef.current = null;
      pendingEventsRef.current = [];
    }
    setState((current) => reduceEvent(current, event));
  }, []);

  const bindOperation = useCallback(
    (
      generation: number,
      chatId: string,
      token: AgentChatOperationToken,
      operationId: AgentChatOperationId,
    ) => {
      const currentOperation = operationRef.current;
      if (
        !currentOperation ||
        currentOperation.generation !== generation ||
        currentOperation.chatId !== chatId ||
        currentOperation.token !== token
      ) {
        return;
      }
      currentOperation.operationId = operationId;
      const pending = pendingEventsRef.current;
      pendingEventsRef.current = [];
      for (const event of pending) {
        acceptEvent(event);
      }
    },
    [acceptEvent],
  );

  useEffect(() => {
    const generation = generationRef.current + 1;
    generationRef.current = generation;
    chatIdRef.current = null;
    sequenceRef.current = 0;
    operationRef.current = null;
    pendingEventsRef.current = [];
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
            if (!active || generationRef.current !== generation) return;
            acceptEvent(event);
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
        const initialOperation =
          chat.status === "running"
            ? {
                generation,
                chatId: chat.id,
                token: { value: ++operationTokenRef.current },
                operationId: chat.activeOperationId,
              }
            : null;
        operationRef.current = initialOperation;
        const pendingEvents = pendingEventsRef.current;
        pendingEventsRef.current = [];
        setState({
          type: "ready",
          chat,
          configuration,
          operation: initialOperation
            ? {
                type: "active",
                phase: "streaming",
                token: initialOperation.token,
                operationId: initialOperation.operationId,
              }
            : null,
          unsavedTurn: null,
          errorCode: null,
          notice: null,
        });
        for (const event of pendingEvents) {
          acceptEvent(event);
        }
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
      operationRef.current = null;
      pendingEventsRef.current = [];
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
    acceptEvent,
  ]);

  const submit = (text: string) => {
    const trimmed = text.trim();
    if (!trimmed || state.type !== "ready" || state.chat.status !== "ready" || state.operation) {
      return;
    }
    const generation = generationRef.current;
    const chatId = state.chat.id;
    const token: AgentChatOperationToken = {
      value: ++operationTokenRef.current,
    };
    operationRef.current = {
      generation,
      chatId,
      token,
      operationId: null,
    };
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
        if (!isCurrentOperation(generation, chatId, token)) return;
        bindOperation(generation, chatId, token, operationId);
        if (!isCurrentOperation(generation, chatId, token)) return;
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
        if (!isCurrentOperation(generation, chatId, token)) return;
        operationRef.current = null;
        pendingEventsRef.current = [];
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
          if (isCurrentOperation(generation, chatId, token)) {
            operationRef.current = null;
            pendingEventsRef.current = [];
          }
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
        operationRef.current = null;
        pendingEventsRef.current = [];
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
    const token: AgentChatOperationToken = {
      value: ++operationTokenRef.current,
    };
    operationRef.current = {
      generation,
      chatId,
      token,
      operationId: null,
    };
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
        if (!isCurrentOperation(generation, chatId, token)) return;
        bindOperation(generation, chatId, token, operationId);
        if (!isCurrentOperation(generation, chatId, token)) return;
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
        if (!isCurrentOperation(generation, chatId, token)) return;
        operationRef.current = null;
        pendingEventsRef.current = [];
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
  const isCurrentOperation = (
    generation: number,
    chatId: string,
    token: AgentChatOperationToken,
  ) =>
    isCurrent(generation, chatId) && operationRef.current?.token === token;

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
        chat: releaseChat(current.chat),
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
        chat: releaseChat(current.chat),
        operation: null,
        errorCode: null,
        notice:
          current.operation?.type === "compact"
            ? "compaction_cancelled"
            : "aborted",
      };
    case "compaction_started":
      if (current.operation?.type === "compact") {
        return {
          ...current,
          operation: {
            ...current.operation,
            phase: "streaming",
            reason: event.reason,
          },
          errorCode: null,
          notice: null,
        };
      }
      if (isInProgressOperation(current.operation)) {
        return {
          ...current,
          operation: { ...current.operation, phase: "streaming" },
          errorCode: null,
          notice: null,
        };
      }
      return current;
    case "compaction_completed":
      if (!event.terminal && current.operation) {
        return {
          ...current,
          chat: event.chat,
          operation: { ...current.operation, phase: "streaming" },
          errorCode: null,
          notice: null,
        };
      }
      return {
        ...current,
        chat: event.chat,
        operation: null,
        errorCode: null,
        notice: "compacted",
      };
    case "compaction_cancelled":
      if (!event.terminal && current.operation) {
        return {
          ...current,
          operation: { ...current.operation, phase: "streaming" },
          errorCode: null,
          notice: null,
        };
      }
      return {
        ...current,
        chat: releaseChat(current.chat),
        operation: null,
        errorCode: null,
        notice: "compaction_cancelled",
      };
    case "compaction_failed":
      if (!event.terminal && current.operation) {
        return {
          ...current,
          operation: { ...current.operation, phase: "streaming" },
          errorCode: null,
          notice: null,
        };
      }
      return {
        ...current,
        chat: releaseChat(current.chat),
        operation: null,
        errorCode: event.error.code ?? "provider_failed",
      };
    case "compaction_not_saved":
      if (!event.terminal && current.operation) {
        return {
          ...current,
          chat: event.chat,
          operation: { ...current.operation, phase: "streaming" },
          errorCode: null,
          notice: null,
        };
      }
      return {
        ...current,
        chat: event.chat,
        operation: null,
        errorCode: event.error.code ?? "not_saved",
      };
  }
  return current;
}

function isInProgressOperation(
  operation: AgentChatOperation | null,
): operation is SendOperation | ActiveOperation {
  return operation?.type === "send" || operation?.type === "active";
}

function releaseChat(chat: AgentChatProjection): AgentChatProjection {
  return chat.status === "running" || chat.activeOperationId !== null
    ? { ...chat, status: "ready", activeOperationId: null }
    : chat;
}

function isTerminalEvent(event: AgentChatApplicationEvent): boolean {
  return event.terminal;
}

function hasMismatchedProjection(event: AgentChatApplicationEvent): boolean {
  return (
    "chat" in event &&
    (event.chat.id !== event.chatId ||
      (event.chat.activeOperationId !== null &&
        event.chat.activeOperationId !== event.operationId))
  );
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
