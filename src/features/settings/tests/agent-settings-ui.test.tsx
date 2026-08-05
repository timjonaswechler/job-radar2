// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";

import { AgentProviderSettings } from "@/features/settings/agent-provider-settings";
import {
  AGENT_SUBSCRIPTION_LOGIN_PROGRESS_EVENT,
  createAgentConfigurationClient,
  type AgentConfigurationClient,
  type AgentConfigurationStatus,
  type SubscriptionLoginProgress,
} from "@/lib/api/agent-configuration";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

afterEach(cleanup);

const configuredOnlyStatus: AgentConfigurationStatus = {
  providers: [
    {
      id: "synthetic-provider",
      displayName: "Synthetic Provider",
      authenticationMethods: ["api_key"],
      activeAuthentication: "api_key",
      configuredByModelsFile: false,
      capability: "configured_only",
      executable: false,
      models: [
        {
          id: "synthetic-model",
          displayName: "Synthetic Model",
          reasoningLevels: ["off", "x_high"],
          executable: false,
        },
      ],
    },
  ],
  authenticationConfiguration: "ready",
  modelConfiguration: "ready",
  diagnostics: [],
};

function injectedClient(status = configuredOnlyStatus): AgentConfigurationClient {
  return {
    getStatus: vi.fn(async () => status),
    submitApiKey: vi.fn(async () => status),
    loginSubscription: vi.fn(async () => status),
    cancelSubscriptionLogin: vi.fn(async () => undefined),
    removeAuthentication: vi.fn(async () => status),
    reload: vi.fn(async () => status),
    openDataFolder: vi.fn(async () => undefined),
    listenToSubscriptionLoginProgress: vi.fn(async () => () => undefined),
  };
}

describe("Agent configuration transport", () => {
  test("carries login attempt identity through start, cancel, and progress", async () => {
    const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
    let progressHandler:
      | ((event: { payload: SubscriptionLoginProgress }) => void)
      | undefined;
    const client = createAgentConfigurationClient(
      async <T,>(command: string, args?: Record<string, unknown>) => {
        calls.push({ command, args });
        return configuredOnlyStatus as T;
      },
      async <T,>(_event: string, handler: (event: { payload: T }) => void) => {
        progressHandler = handler as typeof progressHandler;
        return () => undefined;
      },
    );

    await client.loginSubscription("synthetic-provider", "attempt-one");
    await client.cancelSubscriptionLogin("attempt-one");

    expect(calls).toEqual([
      {
        command: "login_agent_subscription",
        args: { providerId: "synthetic-provider", attemptId: "attempt-one" },
      },
      {
        command: "cancel_agent_subscription_login",
        args: { attemptId: "attempt-one" },
      },
    ]);

    let received: SubscriptionLoginProgress | null = null;
    await client.listenToSubscriptionLoginProgress((progress) => {
      received = progress;
    });
    progressHandler?.({
      payload: {
        attemptId: "attempt-one",
        providerId: "synthetic-provider",
        stage: "waiting_for_browser",
      },
    });
    expect(received).toEqual({
      attemptId: "attempt-one",
      providerId: "synthetic-provider",
      stage: "waiting_for_browser",
    });
    expect(AGENT_SUBSCRIPTION_LOGIN_PROGRESS_EVENT).toBe(
      "agent-subscription-login-progress",
    );
  });
});

describe("Agent provider settings", () => {
  test("keeps configured non-executable providers visible without calling them executable", async () => {
    render(<AgentProviderSettings client={injectedClient()} />);

    expect(await screen.findByText("Synthetic Provider")).toBeVisible();
    expect(
      screen.getByText("settings.agents.status.configuredOnly"),
    ).toBeVisible();
    expect(screen.queryByText("settings.agents.status.executable")).not.toBeInTheDocument();
    expect(screen.getByText("settings.agents.modelCount")).toBeVisible();
  });
});
