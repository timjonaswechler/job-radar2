// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

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
  test("rejects malformed status responses at the transport seam", async () => {
    const client = createAgentConfigurationClient(
      async <T,>() => ({ providers: [] }) as T,
      async <T,>(_event: string, _handler: (event: { payload: T }) => void) =>
        () => undefined,
    );

    await expect(client.getStatus()).rejects.toThrow(
      /invalid Agent configuration status/i,
    );
  });

  test("ignores malformed login progress before it reaches settings state", async () => {
    let emit: ((event: { payload: unknown }) => void) | undefined;
    let malformedCount = 0;
    const client = createAgentConfigurationClient(
      async <T,>() => configuredOnlyStatus as T,
      async <T,>(_event: string, handler: (event: { payload: T }) => void) => {
        emit = handler as (event: { payload: unknown }) => void;
        return () => undefined;
      },
    );

    await client.listenToSubscriptionLoginProgress(
      () => undefined,
      () => {
        malformedCount += 1;
      },
    );
    emit?.({
      payload: {
        attemptId: "attempt-one",
        providerId: "synthetic-provider",
        stage: "unknown-stage",
      },
    });

    expect(malformedCount).toBe(1);
  });

  test("carries login attempt identity through start, cancel, and progress", async () => {
    const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
    let progressHandler:
      | ((event: { payload: SubscriptionLoginProgress }) => void)
      | undefined;
    const client = createAgentConfigurationClient(
      async <T,>(command: string, args?: Record<string, unknown>) => {
        calls.push({ command, args });
        return (command === "cancel_agent_subscription_login"
          ? undefined
          : configuredOnlyStatus) as T;
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

  test("does not show progress from a previous login attempt", async () => {
    const user = userEvent.setup();
    const subscriptionStatus: AgentConfigurationStatus = {
      ...configuredOnlyStatus,
      providers: [
        {
          ...configuredOnlyStatus.providers[0],
          authenticationMethods: ["subscription"],
          activeAuthentication: null,
          capability: "catalog_only",
          executable: false,
        },
      ],
    };
    const attempts: string[] = [];
    let resolveSecondLogin: ((status: AgentConfigurationStatus) => void) | undefined;
    const secondLogin = new Promise<AgentConfigurationStatus>((resolve) => {
      resolveSecondLogin = resolve;
    });
    let progressHandler: ((progress: SubscriptionLoginProgress) => void) | undefined;
    const client: AgentConfigurationClient = {
      ...injectedClient(subscriptionStatus),
      loginSubscription: vi.fn(async (_providerId, attemptId) => {
        attempts.push(attemptId);
        return attempts.length === 2 ? secondLogin : subscriptionStatus;
      }),
      listenToSubscriptionLoginProgress: vi.fn(async (handler) => {
        progressHandler = handler;
        return () => undefined;
      }),
    };

    render(<AgentProviderSettings client={client} />);
    await user.click(await screen.findByRole("button", { name: /Synthetic Provider/ }));
    await user.click(screen.getByRole("button", { name: "settings.agents.actions.login" }));
    await waitFor(() => expect(attempts).toHaveLength(1));
    await user.click(
      await screen.findByRole("button", { name: "settings.agents.actions.login" }),
    );
    await waitFor(() => expect(attempts).toHaveLength(2));

    progressHandler?.({
      attemptId: attempts[0],
      providerId: "synthetic-provider",
      stage: "opening_browser",
    });
    expect(
      screen.queryByText("settings.agents.progress.openingBrowser"),
    ).not.toBeInTheDocument();

    progressHandler?.({
      attemptId: attempts[1],
      providerId: "synthetic-provider",
      stage: "opening_browser",
    });
    await waitFor(() =>
      expect(
        screen.getByText("settings.agents.progress.openingBrowser"),
      ).toBeVisible(),
    );
    await user.click(
      screen.getByRole("button", {
        name: "settings.agents.actions.cancelLogin",
      }),
    );
    progressHandler?.({
      attemptId: attempts[1],
      providerId: "synthetic-provider",
      stage: "waiting_for_browser",
    });
    expect(
      screen.queryByText("settings.agents.progress.waitingForBrowser"),
    ).not.toBeInTheDocument();
    resolveSecondLogin?.(subscriptionStatus);
  });
});
