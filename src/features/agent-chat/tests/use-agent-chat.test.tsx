// @vitest-environment jsdom

import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";

import { useAgentChat } from "@/features/agent-chat/use-agent-chat";
import {
  decodeAgentChatOperationId,
  type AgentChatApplicationEvent,
  type AgentChatOperationId,
  type AgentChatClient,
  type AgentChatProjection,
} from "@/lib/api/agent-chat";
import type {
  AgentConfigurationClient,
  AgentConfigurationStatus,
} from "@/lib/api/agent-configuration";

const configuration: AgentConfigurationStatus = {
  providers: [],
  authenticationConfiguration: "ready",
  modelConfiguration: "ready",
  diagnostics: [],
};

const projection = (id: string, selectedModelId = "model-one"): AgentChatProjection => ({
  id,
  status: "ready",
  activeOperationId: null,
  history: [],
  selectedProviderId: "provider-one",
  selectedModelId,
  reasoningLevel: "medium",
  contextTokens: 0,
  contextWindow: 128000,
  recoveryNotices: [],
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((nextResolve) => {
    resolve = nextResolve;
  });
  return { promise, resolve };
}

function clients(initialChat?: AgentChatProjection) {
  const handlers: Array<(event: AgentChatApplicationEvent) => void> = [];
  const unlisten = vi.fn();
  const chatClient: AgentChatClient = {
    create: vi.fn(async () => projection("chat-create")),
    open: vi.fn(async ({ id }) =>
      initialChat && initialChat.id === id ? initialChat : projection(id),
    ),
    snapshot: vi.fn(async (id) => projection(id)),
    reload: vi.fn(async (id) => projection(id)),
    send: vi.fn(async () => decodeAgentChatOperationId(1)),
    stop: vi.fn(async () => true),
    setModel: vi.fn(async (_id, _providerId, modelId) =>
      projection("chat-one", modelId),
    ),
    setReasoningLevel: vi.fn(async (id) => projection(id)),
    compact: vi.fn(async () => decodeAgentChatOperationId(2)),
    listen: vi.fn(async (handler) => {
      handlers.push(handler);
      return unlisten;
    }),
  };
  const configurationClient: AgentConfigurationClient = {
    getStatus: vi.fn(async () => configuration),
    submitApiKey: vi.fn(),
    loginSubscription: vi.fn(),
    cancelSubscriptionLogin: vi.fn(),
    removeAuthentication: vi.fn(),
    reload: vi.fn(),
    openDataFolder: vi.fn(),
    listenToSubscriptionLoginProgress: vi.fn(),
  };
  return { chatClient, configurationClient, handlers, unlisten };
}

afterEach(() => vi.restoreAllMocks());

describe("Agent Chat lifecycle ownership", () => {
  test("resets sequence and rejects events from the previous request generation", async () => {
    const clientsForTest = clients();
    const { result, rerender, unmount } = renderHook(
      ({ chatId }: { chatId: string }) =>
        useAgentChat({
          request: { type: "open", id: chatId, systemPrompt: "private" },
          chatClient: clientsForTest.chatClient,
          configurationClient: clientsForTest.configurationClient,
        }),
      { initialProps: { chatId: "chat-one" } },
    );

    await waitFor(() => expect(result.current.state).toMatchObject({ type: "ready" }));
    const oldHandler = clientsForTest.handlers[0];

    rerender({ chatId: "chat-two" });
    await waitFor(() => expect(result.current.state).toMatchObject({
      type: "ready",
      chat: { id: "chat-two" },
    }));
    const currentHandler = clientsForTest.handlers[1];
    act(() => result.current.submit("current request"));
    await waitFor(() =>
      expect(clientsForTest.chatClient.send).toHaveBeenCalledWith(
        "chat-two",
        "current request",
      ),
    );

    act(() => {
      oldHandler({
        chatId: "chat-two",
        operationId: decodeAgentChatOperationId(1),
        sequence: 99,
        terminal: true,
        type: "completed",
        chat: projection("chat-two", "stale-model"),
      });
      currentHandler({
        chatId: "chat-two",
        operationId: decodeAgentChatOperationId(1),
        sequence: 1,
        terminal: true,
        type: "completed",
        chat: projection("chat-two", "current-model"),
      });
    });

    expect(result.current.state).toMatchObject({
      type: "ready",
      chat: { id: "chat-two", selectedModelId: "current-model" },
    });
    unmount();
    expect(clientsForTest.unlisten).toHaveBeenCalledTimes(2);
  });

  test("buffers a reopened Chat event emitted during initialization", async () => {
    const initialChat: AgentChatProjection = {
      ...projection("chat-one"),
      status: "running",
      activeOperationId: decodeAgentChatOperationId(7),
    };
    const clientsForTest = clients(initialChat);
    const opened = deferred<AgentChatProjection>();
    vi.mocked(clientsForTest.chatClient.open).mockImplementationOnce(
      () => opened.promise,
    );
    const { result } = renderHook(() =>
      useAgentChat({
        request: { type: "open", id: "chat-one", systemPrompt: "private" },
        chatClient: clientsForTest.chatClient,
        configurationClient: clientsForTest.configurationClient,
      }),
    );
    await waitFor(() => expect(clientsForTest.handlers).toHaveLength(1));

    act(() => {
      clientsForTest.handlers[0]({
        chatId: "chat-one",
        operationId: decodeAgentChatOperationId(7),
        sequence: 1,
        terminal: true,
        type: "failed",
        error: { code: "provider_failed" },
      });
    });
    expect(result.current.state).toMatchObject({ type: "loading" });

    await act(async () => {
      opened.resolve(initialChat);
      await opened.promise;
    });
    await waitFor(() =>
      expect(result.current.state).toMatchObject({
        type: "ready",
        operation: null,
        chat: { status: "ready", activeOperationId: null },
      }),
    );
  });

  test("buffers events until the backend operation identity is returned", async () => {
    const clientsForTest = clients();
    const operation = deferred<AgentChatOperationId>();
    vi.mocked(clientsForTest.chatClient.send).mockImplementationOnce(
      () => operation.promise,
    );
    const { result } = renderHook(() =>
      useAgentChat({
        request: { type: "open", id: "chat-one", systemPrompt: "private" },
        chatClient: clientsForTest.chatClient,
        configurationClient: clientsForTest.configurationClient,
      }),
    );
    await waitFor(() => expect(result.current.state).toMatchObject({ type: "ready" }));

    act(() => result.current.submit("buffered request"));
    clientsForTest.handlers[0]({
      chatId: "chat-one",
      operationId: decodeAgentChatOperationId(7),
      sequence: 1,
      terminal: false,
      type: "started",
    });
    expect(result.current.state).toMatchObject({
      type: "ready",
      operation: { phase: "submitted", operationId: null },
    });

    await act(async () => {
      operation.resolve(decodeAgentChatOperationId(7));
      await operation.promise;
    });
    expect(result.current.state).toMatchObject({
      type: "ready",
      operation: { phase: "streaming", operationId: decodeAgentChatOperationId(7) },
    });
  });

  test("rejects stale operation and out-of-order events after terminal authority changes", async () => {
    const clientsForTest = clients();
    vi.mocked(clientsForTest.chatClient.send)
      .mockResolvedValueOnce(decodeAgentChatOperationId(1))
      .mockResolvedValueOnce(decodeAgentChatOperationId(2));
    const { result } = renderHook(() =>
      useAgentChat({
        request: { type: "open", id: "chat-one", systemPrompt: "private" },
        chatClient: clientsForTest.chatClient,
        configurationClient: clientsForTest.configurationClient,
      }),
    );
    await waitFor(() => expect(result.current.state).toMatchObject({ type: "ready" }));
    act(() => result.current.submit("first request"));
    await waitFor(() => expect(clientsForTest.chatClient.send).toHaveBeenCalledOnce());

    const emit = clientsForTest.handlers[0];
    act(() => {
      emit({ chatId: "chat-one", operationId: decodeAgentChatOperationId(1), sequence: 10, terminal: false, type: "started" });
      emit({ chatId: "chat-one", operationId: decodeAgentChatOperationId(2), sequence: 11, terminal: false, type: "started" });
      emit({ chatId: "chat-one", operationId: decodeAgentChatOperationId(1), sequence: 9, terminal: false, type: "content_delta", index: 0, delta: "STALE" });
      emit({ chatId: "chat-one", operationId: decodeAgentChatOperationId(1), sequence: 12, terminal: true, type: "completed", chat: projection("chat-one", "completed-model") });
      emit({ chatId: "chat-one", operationId: decodeAgentChatOperationId(1), sequence: 13, terminal: true, type: "failed", error: { code: "stale" } });
    });
    expect(result.current.state).toMatchObject({
      type: "ready",
      operation: null,
      chat: { selectedModelId: "completed-model" },
    });

    act(() => result.current.submit("second request"));
    await waitFor(() => expect(clientsForTest.chatClient.send).toHaveBeenCalledTimes(2));
    act(() => {
      emit({ chatId: "chat-one", operationId: decodeAgentChatOperationId(1), sequence: 14, terminal: true, type: "completed", chat: projection("chat-one", "old-model") });
      emit({ chatId: "chat-one", operationId: decodeAgentChatOperationId(2), sequence: 15, terminal: false, type: "started" });
    });
    expect(result.current.state).toMatchObject({
      type: "ready",
      operation: { operationId: decodeAgentChatOperationId(2), phase: "streaming" },
      chat: { selectedModelId: "completed-model" },
    });
  });

  test("releases a reopened running Chat without losing its operation identity", async () => {
    const clientsForTest = clients({
      ...projection("chat-one"),
      status: "running",
      activeOperationId: decodeAgentChatOperationId(7),
    });
    const { result } = renderHook(() =>
      useAgentChat({
        request: { type: "open", id: "chat-one", systemPrompt: "private" },
        chatClient: clientsForTest.chatClient,
        configurationClient: clientsForTest.configurationClient,
      }),
    );
    await waitFor(() =>
      expect(result.current.state).toMatchObject({
        type: "ready",
        operation: { type: "active", operationId: decodeAgentChatOperationId(7) },
      }),
    );

    const emit = clientsForTest.handlers[0];
    act(() => {
      emit({ chatId: "chat-one", operationId: decodeAgentChatOperationId(7), sequence: 1, terminal: false, type: "compaction_started", reason: "threshold" });
      emit({ chatId: "chat-one", operationId: decodeAgentChatOperationId(7), sequence: 2, terminal: true, type: "failed", error: { code: "provider_failed" } });
    });
    expect(result.current.state).toMatchObject({
      type: "ready",
      operation: null,
      chat: { status: "ready", activeOperationId: null },
    });
  });

  test("keeps operation authority across nonterminal automatic compaction events", async () => {
    const clientsForTest = clients();
    const { result } = renderHook(() =>
      useAgentChat({
        request: { type: "open", id: "chat-one", systemPrompt: "private" },
        chatClient: clientsForTest.chatClient,
        configurationClient: clientsForTest.configurationClient,
      }),
    );
    await waitFor(() => expect(result.current.state).toMatchObject({ type: "ready" }));
    act(() => result.current.submit("compaction request"));
    await waitFor(() => expect(clientsForTest.chatClient.send).toHaveBeenCalledOnce());

    const emit = clientsForTest.handlers[0];
    act(() => {
      emit({ chatId: "chat-one", operationId: decodeAgentChatOperationId(1), sequence: 1, terminal: false, type: "compaction_started", reason: "threshold" });
      emit({
        chatId: "chat-one",
        operationId: decodeAgentChatOperationId(1),
        sequence: 2,
        terminal: false,
        type: "compaction_completed",
        reason: "threshold",
        chat: { ...projection("chat-one"), status: "running", activeOperationId: decodeAgentChatOperationId(1) },
      });
      emit({ chatId: "chat-one", operationId: decodeAgentChatOperationId(1), sequence: 3, terminal: false, type: "compaction_failed", error: { code: "retrying" } });
    });
    expect(result.current.state).toMatchObject({
      type: "ready",
      operation: { type: "send", operationId: decodeAgentChatOperationId(1) },
      chat: { status: "running", activeOperationId: decodeAgentChatOperationId(1) },
    });

    act(() => {
      emit({
        chatId: "chat-one",
        operationId: decodeAgentChatOperationId(1),
        sequence: 4,
        terminal: true,
        type: "completed",
        chat: projection("chat-one", "completed-model"),
      });
    });
    expect(result.current.state).toMatchObject({
      type: "ready",
      operation: null,
      chat: { status: "ready", activeOperationId: null },
    });
  });

  test("does not let an older model response regress a newer selection", async () => {
    const clientsForTest = clients();
    const first = deferred<AgentChatProjection>();
    const second = deferred<AgentChatProjection>();
    vi.mocked(clientsForTest.chatClient.setModel)
      .mockImplementationOnce(() => first.promise)
      .mockImplementationOnce(() => second.promise);

    const { result } = renderHook(() =>
      useAgentChat({
        request: { type: "open", id: "chat-one", systemPrompt: "private" },
        chatClient: clientsForTest.chatClient,
        configurationClient: clientsForTest.configurationClient,
      }),
    );
    await waitFor(() => expect(result.current.state).toMatchObject({ type: "ready" }));

    act(() => {
      result.current.selectModel("provider-one", "model-one");
      result.current.selectModel("provider-one", "model-two");
    });
    await act(async () => {
      second.resolve(projection("chat-one", "model-two"));
      await second.promise;
    });
    await act(async () => {
      first.resolve(projection("chat-one", "model-one"));
      await first.promise;
    });

    expect(result.current.state).toMatchObject({
      type: "ready",
      chat: { selectedModelId: "model-two" },
    });
  });
});
