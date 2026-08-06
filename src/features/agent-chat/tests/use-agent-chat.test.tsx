// @vitest-environment jsdom

import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";

import { useAgentChat } from "@/features/agent-chat/use-agent-chat";
import type {
  AgentChatApplicationEvent,
  AgentChatClient,
  AgentChatProjection,
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

function clients() {
  const handlers: Array<(event: AgentChatApplicationEvent) => void> = [];
  const unlisten = vi.fn();
  const chatClient: AgentChatClient = {
    create: vi.fn(async () => projection("chat-create")),
    open: vi.fn(async ({ id }) => projection(id)),
    snapshot: vi.fn(async (id) => projection(id)),
    reload: vi.fn(async (id) => projection(id)),
    send: vi.fn(async () => 1),
    stop: vi.fn(async () => true),
    setModel: vi.fn(async (_id, _providerId, modelId) =>
      projection("chat-one", modelId),
    ),
    setReasoningLevel: vi.fn(async (id) => projection(id)),
    compact: vi.fn(async () => 2),
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

    act(() => {
      oldHandler({
        chatId: "chat-two",
        sequence: 99,
        type: "completed",
        chat: projection("chat-two", "stale-model"),
      });
      currentHandler({
        chatId: "chat-two",
        sequence: 1,
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
