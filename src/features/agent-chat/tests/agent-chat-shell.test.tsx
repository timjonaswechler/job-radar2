// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test, vi } from "vitest";

import { AgentChatShell } from "@/features/agent-chat/agent-chat-shell";
import type {
  AgentChatApplicationEvent,
  AgentChatClient,
  AgentChatProjection,
} from "@/lib/api/agent-chat";
import type {
  AgentConfigurationClient,
  AgentConfigurationStatus,
} from "@/lib/api/agent-configuration";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
vi.stubGlobal("ResizeObserver", ResizeObserverStub);
Element.prototype.getAnimations = () => [];

const projection: AgentChatProjection = {
  id: "synthetic-chat-id",
  status: "ready",
  history: [
    {
      type: "turn",
      user: "Prepare a concise draft",
      assistant: [
        { type: "reasoning", text: "I will use only the supplied evidence." },
        { type: "text", text: "Here is the durable draft." },
      ],
    },
  ],
  selectedProviderId: "provider-one",
  selectedModelId: "model-one",
  reasoningLevel: "medium",
  contextTokens: 1200,
  contextWindow: 128000,
  recoveryNotices: [],
};

const configuration: AgentConfigurationStatus = {
  authenticationConfiguration: "ready",
  modelConfiguration: "ready",
  diagnostics: [],
  providers: [
    {
      id: "provider-one",
      displayName: "Provider One",
      authenticationMethods: ["api_key"],
      activeAuthentication: "api_key",
      configuredByModelsFile: false,
      capability: "executable",
      executable: true,
      models: [
        {
          id: "model-one",
          displayName: "Model One",
          reasoningLevels: ["off", "medium", "high"],
          executable: true,
        },
        {
          id: "model-two",
          displayName: "Model Two",
          reasoningLevels: ["off", "high"],
          executable: true,
        },
      ],
    },
    {
      id: "configured-provider",
      displayName: "Configured Provider",
      authenticationMethods: ["api_key"],
      activeAuthentication: "api_key",
      configuredByModelsFile: false,
      capability: "configured_only",
      executable: false,
      models: [
        {
          id: "configured-model",
          displayName: "Configured Model",
          reasoningLevels: ["off", "x_high"],
          executable: false,
        },
      ],
    },
  ],
};

function clients(chat: AgentChatProjection = projection) {
  let eventHandler: ((event: AgentChatApplicationEvent) => void) | undefined;
  const chatClient: AgentChatClient = {
    create: vi.fn(async () => chat),
    open: vi.fn(async () => chat),
    send: vi.fn(async () => undefined),
    stop: vi.fn(async () => true),
    setModel: vi.fn(async () => chat),
    setReasoningLevel: vi.fn(async () => chat),
    compact: vi.fn(async () => undefined),
    listen: vi.fn(async (handler) => {
      eventHandler = handler;
      return () => undefined;
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
  return { chatClient, configurationClient, event: () => eventHandler };
}

afterEach(cleanup);

test("opens a persistent Agent Chat and renders durable messages in the reusable split shell", async () => {
  const { chatClient, configurationClient } = clients();

  render(
    <AgentChatShell
      request={{
        type: "open",
        id: "synthetic-chat-id",
        systemPrompt: "synthetic private prompt",
      }}
      title="Application draft"
      contextLabel="Job Posting · Product Designer"
      canvas={<div>Caller-owned canvas</div>}
      chatClient={chatClient}
      configurationClient={configurationClient}
    />,
  );

  expect(screen.getByRole("status")).toHaveTextContent(
    "agentChat.loading",
  );
  expect(await screen.findByRole("region", { name: "Agent Chat" })).toBeVisible();
  expect(screen.getByText("Job Posting · Product Designer")).toBeVisible();
  expect(screen.getByText("Prepare a concise draft")).toBeVisible();
  expect(screen.getByText("I will use only the supplied evidence.")).toBeVisible();
  expect(screen.getByText("Here is the durable draft.")).toBeVisible();
  expect(screen.getByText("Caller-owned canvas")).toBeVisible();

  await waitFor(() =>
    expect(chatClient.open).toHaveBeenCalledWith({
      id: "synthetic-chat-id",
      systemPrompt: "synthetic private prompt",
    }),
  );
  expect(chatClient.listen).toHaveBeenCalledOnce();
});

test("sends text, renders indexed streaming content, and stops through the typed client", async () => {
  const user = userEvent.setup();
  const { chatClient, configurationClient, event } = clients();

  render(
    <AgentChatShell
      request={{
        type: "open",
        id: "synthetic-chat-id",
        systemPrompt: "synthetic private prompt",
      }}
      title="Application draft"
      contextLabel="Job Posting · Product Designer"
      canvas={<div>Caller-owned canvas</div>}
      chatClient={chatClient}
      configurationClient={configurationClient}
    />,
  );

  const composer = await screen.findByRole("textbox", {
    name: "agentChat.composer.label",
  });
  expect(composer).toBeEnabled();
  fireEvent.change(composer, {
    target: { value: "Revise the opening paragraph" },
  });
  expect(chatClient.open).toHaveBeenCalledOnce();
  expect(composer).toHaveValue("Revise the opening paragraph");
  const sendButton = screen.getByRole("button", {
    name: "agentChat.actions.send",
  });
  expect(sendButton).toBeEnabled();
  await user.click(sendButton);

  await waitFor(() =>
    expect(chatClient.send).toHaveBeenCalledWith(
      "synthetic-chat-id",
      "Revise the opening paragraph",
    ),
  );
  expect(
    screen.getByRole("textbox", { name: "agentChat.composer.label" }),
  ).toHaveValue("");
  expect(screen.getByText("Revise the opening paragraph")).toBeVisible();

  event()?.({
    chatId: "synthetic-chat-id",
    sequence: 10,
    type: "started",
  });
  event()?.({
    chatId: "synthetic-chat-id",
    sequence: 11,
    type: "content_started",
    index: 0,
    kind: "reasoning",
  });
  event()?.({
    chatId: "synthetic-chat-id",
    sequence: 12,
    type: "content_delta",
    index: 0,
    delta: "Checking the evidence.",
  });
  event()?.({
    chatId: "synthetic-chat-id",
    sequence: 13,
    type: "content_started",
    index: 1,
    kind: "text",
  });
  event()?.({
    chatId: "synthetic-chat-id",
    sequence: 14,
    type: "content_delta",
    index: 1,
    delta: "Updated opening",
  });

  expect(await screen.findByText("Checking the evidence.")).toBeVisible();
  expect(screen.getByText("Updated opening")).toBeVisible();

  event()?.({
    chatId: "different-chat",
    sequence: 100,
    type: "content_delta",
    index: 1,
    delta: "WRONG-CHAT-CANARY",
  });
  event()?.({
    chatId: "synthetic-chat-id",
    sequence: 13,
    type: "content_delta",
    index: 1,
    delta: "STALE-EVENT-CANARY",
  });
  expect(screen.queryByText(/WRONG-CHAT-CANARY/)).not.toBeInTheDocument();
  expect(screen.queryByText(/STALE-EVENT-CANARY/)).not.toBeInTheDocument();

  await user.click(
    screen.getByRole("button", { name: "agentChat.actions.stop" }),
  );
  await waitFor(() =>
    expect(chatClient.stop).toHaveBeenCalledWith("synthetic-chat-id"),
  );

  event()?.({
    chatId: "synthetic-chat-id",
    sequence: 15,
    type: "aborted",
  });
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "agentChat.actions.send" }),
    ).toBeDisabled(),
  );
  expect(screen.queryByText("Revise the opening paragraph")).not.toBeInTheDocument();
});

test("shows the stable authentication failure instead of a generic unavailable message", async () => {
  const { chatClient, configurationClient, event } = clients();

  render(
    <AgentChatShell
      request={{
        type: "open",
        id: "synthetic-chat-id",
        systemPrompt: "synthetic private prompt",
      }}
      title="Application draft"
      contextLabel="Job Posting · Product Designer"
      canvas={<div>Caller-owned canvas</div>}
      chatClient={chatClient}
      configurationClient={configurationClient}
    />,
  );

  const composer = await screen.findByRole("textbox", {
    name: "agentChat.composer.label",
  });
  fireEvent.change(composer, { target: { value: "Synthetic request" } });
  await userEvent.setup().click(
    screen.getByRole("button", { name: "agentChat.actions.send" }),
  );
  event()?.({
    chatId: "synthetic-chat-id",
    sequence: 20,
    type: "failed",
    error: {
      code: "authentication_unavailable",
      message: "PRIVATE-PROVIDER-MESSAGE-CANARY",
    },
  });

  expect(await screen.findByRole("alert")).toHaveTextContent(
    "agentChat.errors.authenticationUnavailable",
  );
  expect(screen.getByRole("alert")).not.toHaveTextContent(
    "PRIVATE-PROVIDER-MESSAGE-CANARY",
  );
});

test("keeps a provider response visibly separate when the completed turn was not saved", async () => {
  const { chatClient, configurationClient, event } = clients();

  render(
    <AgentChatShell
      request={{
        type: "open",
        id: "synthetic-chat-id",
        systemPrompt: "synthetic private prompt",
      }}
      title="Application draft"
      contextLabel="Job Posting · Product Designer"
      canvas={<div>Caller-owned canvas</div>}
      chatClient={chatClient}
      configurationClient={configurationClient}
    />,
  );

  const composer = await screen.findByRole("textbox", {
    name: "agentChat.composer.label",
  });
  fireEvent.change(composer, { target: { value: "Unsaved user turn" } });
  await userEvent.setup().click(
    screen.getByRole("button", { name: "agentChat.actions.send" }),
  );

  event()?.({
    chatId: "synthetic-chat-id",
    sequence: 30,
    type: "not_saved",
    response: [{ type: "text", text: "Visible but non-durable response" }],
    error: { code: "not_saved", message: "PRIVATE-STORAGE-CANARY" },
    chat: { ...projection, status: "not_saved" },
  });

  expect(await screen.findByText("Visible but non-durable response")).toBeVisible();
  expect(screen.getByText("Unsaved user turn")).toBeVisible();
  expect(screen.getByRole("alert")).toHaveTextContent(
    "agentChat.errors.notSaved",
  );
  expect(screen.getByRole("alert")).not.toHaveTextContent(
    "PRIVATE-STORAGE-CANARY",
  );
  expect(composer).toBeDisabled();
  expect(screen.getAllByText("agentChat.status.notSaved")).toHaveLength(2);
});

test("discloses a recovered incomplete final turn without making the Chat read-only", async () => {
  const { chatClient, configurationClient } = clients({
    ...projection,
    recoveryNotices: ["incomplete_final_turn_discarded"],
  });

  render(
    <AgentChatShell
      request={{
        type: "open",
        id: "synthetic-chat-id",
        systemPrompt: "synthetic private prompt",
      }}
      title="Application draft"
      contextLabel="Job Posting · Product Designer"
      canvas={<div>Caller-owned canvas</div>}
      chatClient={chatClient}
      configurationClient={configurationClient}
    />,
  );

  expect(await screen.findByRole("alert")).toHaveTextContent(
    "agentChat.recovery.incompleteFinalTurnDiscarded",
  );
  expect(
    screen.getByRole("textbox", { name: "agentChat.composer.label" }),
  ).toBeEnabled();
});

test("can stop an operation that was already running when the Chat opened", async () => {
  const user = userEvent.setup();
  const { chatClient, configurationClient } = clients({
    ...projection,
    status: "running",
  });

  render(
    <AgentChatShell
      request={{
        type: "open",
        id: "synthetic-chat-id",
        systemPrompt: "synthetic private prompt",
      }}
      title="Application draft"
      contextLabel="Job Posting · Product Designer"
      canvas={<div>Caller-owned canvas</div>}
      chatClient={chatClient}
      configurationClient={configurationClient}
    />,
  );

  const stopButton = await screen.findByRole("button", {
    name: "agentChat.actions.stop",
  });
  await user.click(stopButton);
  expect(chatClient.stop).toHaveBeenCalledWith("synthetic-chat-id");
  expect(
    screen.getByRole("textbox", { name: "agentChat.composer.label" }),
  ).toBeDisabled();
});

test.each([
  ["model_unavailable", "agentChat.states.modelUnavailable"],
  ["read_only_locked", "agentChat.states.readOnlyLocked"],
  ["read_only_unsupported", "agentChat.states.readOnlyUnsupported"],
  ["damaged", "agentChat.states.damaged"],
] as const)("renders the %s projection as an explicit non-writable state", async (status, label) => {
  const { chatClient, configurationClient } = clients({ ...projection, status });

  render(
    <AgentChatShell
      request={{
        type: "open",
        id: "synthetic-chat-id",
        systemPrompt: "synthetic private prompt",
      }}
      title="Application draft"
      contextLabel="Job Posting · Product Designer"
      canvas={<div>Caller-owned canvas</div>}
      chatClient={chatClient}
      configurationClient={configurationClient}
    />,
  );

  expect(await screen.findByRole("alert")).toHaveTextContent(label);
  expect(
    screen.getByRole("textbox", { name: "agentChat.composer.label" }),
  ).toBeDisabled();
  const modelSelector = screen.getByRole("combobox", {
    name: "agentChat.actions.selectModel",
  });
  if (status === "model_unavailable") {
    expect(modelSelector).toBeEnabled();
  } else {
    expect(modelSelector).toBeDisabled();
  }
});

test("runs manual compaction and renders its durable history marker without a summary", async () => {
  const user = userEvent.setup();
  const { chatClient, configurationClient, event } = clients();

  render(
    <AgentChatShell
      request={{
        type: "open",
        id: "synthetic-chat-id",
        systemPrompt: "synthetic private prompt",
      }}
      title="Application draft"
      contextLabel="Job Posting · Product Designer"
      canvas={<div>Caller-owned canvas</div>}
      chatClient={chatClient}
      configurationClient={configurationClient}
    />,
  );

  await user.click(
    await screen.findByRole("button", { name: "agentChat.actions.compact" }),
  );
  await waitFor(() =>
    expect(chatClient.compact).toHaveBeenCalledWith("synthetic-chat-id", null),
  );

  event()?.({
    chatId: "synthetic-chat-id",
    sequence: 40,
    type: "compaction_started",
    reason: "manual",
  });
  expect(await screen.findByRole("status")).toHaveTextContent(
    "agentChat.compaction.running",
  );

  event()?.({
    chatId: "synthetic-chat-id",
    sequence: 41,
    type: "compaction_completed",
    reason: "manual",
    chat: {
      ...projection,
      history: [
        ...projection.history,
        { type: "compaction", reason: "manual", tokens_before: 12345 },
      ],
    },
  });

  expect(await screen.findByText("agentChat.compaction.marker")).toBeVisible();
  expect(screen.queryByText("PRIVATE-COMPACTION-SUMMARY")).not.toBeInTheDocument();
});

test("keeps the durable Chat readable when compaction could not be saved", async () => {
  const user = userEvent.setup();
  const { chatClient, configurationClient, event } = clients();

  render(
    <AgentChatShell
      request={{
        type: "open",
        id: "synthetic-chat-id",
        systemPrompt: "synthetic private prompt",
      }}
      title="Application draft"
      contextLabel="Job Posting · Product Designer"
      canvas={<div>Caller-owned canvas</div>}
      chatClient={chatClient}
      configurationClient={configurationClient}
    />,
  );

  await user.click(
    await screen.findByRole("button", { name: "agentChat.actions.compact" }),
  );
  event()?.({
    chatId: "synthetic-chat-id",
    sequence: 50,
    type: "compaction_not_saved",
    error: { code: "not_saved", message: "PRIVATE-STORAGE-CANARY" },
    chat: { ...projection, status: "not_saved" },
  });

  expect(await screen.findByRole("alert")).toHaveTextContent(
    "agentChat.errors.notSaved",
  );
  expect(screen.getByRole("alert")).not.toHaveTextContent(
    "PRIVATE-STORAGE-CANARY",
  );
  expect(screen.getByText("Prepare a concise draft")).toBeVisible();
  expect(
    screen.getByRole("textbox", { name: "agentChat.composer.label" }),
  ).toBeDisabled();
});

test("changes the Agent Model and Reasoning Level explicitly", async () => {
  const user = userEvent.setup();
  const { chatClient, configurationClient } = clients();

  render(
    <AgentChatShell
      request={{
        type: "open",
        id: "synthetic-chat-id",
        systemPrompt: "synthetic private prompt",
      }}
      title="Application draft"
      contextLabel="Job Posting · Product Designer"
      canvas={<div>Caller-owned canvas</div>}
      chatClient={chatClient}
      configurationClient={configurationClient}
    />,
  );

  const modelSelector = await screen.findByRole("combobox", {
    name: "agentChat.actions.selectModel",
  });
  modelSelector.focus();
  fireEvent.keyDown(modelSelector, { key: "ArrowDown" });
  await waitFor(() => expect(modelSelector).toHaveAttribute("aria-expanded", "true"));
  expect(
    screen.queryByRole("option", { name: "Configured Provider · Configured Model" }),
  ).not.toBeInTheDocument();
  await user.click(
    await screen.findByRole("option", { name: "Provider One · Model Two" }),
  );
  await waitFor(() =>
    expect(chatClient.setModel).toHaveBeenCalledWith(
      "synthetic-chat-id",
      "provider-one",
      "model-two",
    ),
  );

  const reasoningSelector = screen.getByRole("combobox", {
    name: "agentChat.actions.selectReasoning",
  });
  reasoningSelector.focus();
  fireEvent.keyDown(reasoningSelector, { key: "ArrowDown" });
  await waitFor(() =>
    expect(reasoningSelector).toHaveAttribute("aria-expanded", "true"),
  );
  await user.click(
    await screen.findByRole("option", { name: "agentChat.reasoning.high" }),
  );
  await waitFor(() =>
    expect(chatClient.setReasoningLevel).toHaveBeenCalledWith(
      "synthetic-chat-id",
      "high",
    ),
  );
});
