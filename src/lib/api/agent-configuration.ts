import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type AgentAuthenticationKind = "api_key" | "subscription";
export type AgentConfigurationState = "ready" | "invalid";

export type AgentConfigurationDiagnostic = {
  code: string;
  message: string;
};

export type AgentModelStatus = {
  id: string;
  displayName: string;
  reasoningLevels: string[];
};

export type ProviderConfigurationStatus = {
  id: string;
  displayName: string;
  authenticationMethods: AgentAuthenticationKind[];
  activeAuthentication: AgentAuthenticationKind | null;
  configuredByModelsFile: boolean;
  available: boolean;
  models: AgentModelStatus[];
};

export type AgentConfigurationStatus = {
  providers: ProviderConfigurationStatus[];
  authenticationConfiguration: AgentConfigurationState;
  modelConfiguration: AgentConfigurationState;
  diagnostics: AgentConfigurationDiagnostic[];
};

export type SubscriptionLoginStage =
  | "starting"
  | "opening_browser"
  | "waiting_for_browser"
  | "finalizing"
  | "completed"
  | "cancelled"
  | "failed";

export type SubscriptionLoginProgress = {
  providerId: string;
  stage: SubscriptionLoginStage;
};

export type AgentConfigurationError = {
  code?: string;
  message?: string;
};

export const AGENT_SUBSCRIPTION_LOGIN_PROGRESS_EVENT =
  "agent-subscription-login-progress";

export type AgentConfigurationClient = {
  getStatus(): Promise<AgentConfigurationStatus>;
  submitApiKey(providerId: string, apiKey: string): Promise<AgentConfigurationStatus>;
  loginSubscription(providerId: string): Promise<AgentConfigurationStatus>;
  cancelSubscriptionLogin(providerId: string): Promise<boolean>;
  removeAuthentication(providerId: string): Promise<AgentConfigurationStatus>;
  reload(): Promise<AgentConfigurationStatus>;
  openDataFolder(): Promise<void>;
  listenToSubscriptionLoginProgress(
    handler: (progress: SubscriptionLoginProgress) => void,
  ): Promise<UnlistenFn>;
};

type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
type Listen = <T>(
  event: string,
  handler: (event: { payload: T }) => void,
) => Promise<UnlistenFn>;

export function createAgentConfigurationClient(
  invokeCommand: Invoke,
  listenToEvent: Listen,
): AgentConfigurationClient {
  return {
    getStatus: () => invokeCommand("get_agent_configuration_status"),
    submitApiKey: (providerId, apiKey) =>
      invokeCommand("submit_agent_api_key", { providerId, apiKey }),
    loginSubscription: (providerId) =>
      invokeCommand("login_agent_subscription", { providerId }),
    cancelSubscriptionLogin: (providerId) =>
      invokeCommand("cancel_agent_subscription_login", { providerId }),
    removeAuthentication: (providerId) =>
      invokeCommand("remove_agent_authentication", { providerId }),
    reload: () => invokeCommand("reload_agent_configuration"),
    openDataFolder: () => invokeCommand("open_agent_data_folder"),
    listenToSubscriptionLoginProgress: (handler) =>
      listenToEvent<SubscriptionLoginProgress>(
        AGENT_SUBSCRIPTION_LOGIN_PROGRESS_EVENT,
        (event) => handler(event.payload),
      ),
  };
}

export const agentConfigurationClient = createAgentConfigurationClient(
  invoke,
  listen,
);
