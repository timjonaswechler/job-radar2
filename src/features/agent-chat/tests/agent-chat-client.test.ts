import assert from "node:assert/strict";

import {
  AGENT_CHAT_EVENT,
  createAgentChatClient,
  type AgentChatApplicationEvent,
  type AgentChatProjection,
} from "@/lib/api/agent-chat";
import { test } from "vitest";

const projection: AgentChatProjection = {
  id: "synthetic-chat-id",
  status: "ready",
  history: [],
  selectedProviderId: "provider-one",
  selectedModelId: "model-one",
  reasoningLevel: "medium",
  contextTokens: 1200,
  contextWindow: 128000,
  recoveryNotices: [],
};

test("agent chat client uses the persistent application command and event contract", async () => {
  const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
  let eventHandler:
    | ((event: { payload: AgentChatApplicationEvent }) => void)
    | undefined;
  let listenedEvent = "";
  let unlistenCount = 0;

  const client = createAgentChatClient(
    async <T>(command: string, args?: Record<string, unknown>) => {
      calls.push({ command, args });
      if (command === "stop_agent_chat") return true as T;
      if (
        command === "send_agent_chat_message" ||
        command === "compact_agent_chat"
      ) {
        return undefined as T;
      }
      return projection as T;
    },
    async <T>(event: string, handler: (event: { payload: T }) => void) => {
      listenedEvent = event;
      eventHandler = handler as typeof eventHandler;
      return () => {
        unlistenCount += 1;
      };
    },
  );

  await client.create({
    systemPrompt: "synthetic system prompt",
    providerId: "provider-one",
    modelId: "model-one",
    reasoningLevel: "medium",
  });
  await client.open({ id: "synthetic-chat-id", systemPrompt: "synthetic prompt" });
  await client.send("synthetic-chat-id", "Hello");
  await client.stop("synthetic-chat-id");
  await client.setModel("synthetic-chat-id", "provider-two", "model-two");
  await client.setReasoningLevel("synthetic-chat-id", "high");
  await client.compact("synthetic-chat-id", null);

  assert.deepEqual(calls, [
    {
      command: "create_agent_chat",
      args: {
        input: {
          systemPrompt: "synthetic system prompt",
          providerId: "provider-one",
          modelId: "model-one",
          reasoningLevel: "medium",
        },
      },
    },
    {
      command: "open_agent_chat",
      args: {
        input: {
          id: "synthetic-chat-id",
          systemPrompt: "synthetic prompt",
        },
      },
    },
    {
      command: "send_agent_chat_message",
      args: { chatId: "synthetic-chat-id", text: "Hello" },
    },
    { command: "stop_agent_chat", args: { chatId: "synthetic-chat-id" } },
    {
      command: "set_agent_chat_model",
      args: {
        chatId: "synthetic-chat-id",
        providerId: "provider-two",
        modelId: "model-two",
      },
    },
    {
      command: "set_agent_chat_reasoning_level",
      args: { chatId: "synthetic-chat-id", reasoningLevel: "high" },
    },
    {
      command: "compact_agent_chat",
      args: { chatId: "synthetic-chat-id", focus: null },
    },
  ]);

  const received: AgentChatApplicationEvent[] = [];
  const unlisten = await client.listen((event) => received.push(event));
  assert.equal(listenedEvent, AGENT_CHAT_EVENT);
  assert.ok(eventHandler);

  eventHandler?.({
    payload: {
      chatId: "synthetic-chat-id",
      sequence: 7,
      type: "content_delta",
      index: 0,
      delta: "Synthetic response",
    },
  });
  assert.deepEqual(received, [
    {
      chatId: "synthetic-chat-id",
      sequence: 7,
      type: "content_delta",
      index: 0,
      delta: "Synthetic response",
    },
  ]);

  unlisten();
  assert.equal(unlistenCount, 1);
});
