import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { AgentChatReasoningLevel } from "@/lib/api/agent-chat";
import { isArrayOf, isRecord } from "@/lib/api/wire";

const MAX_IDENTIFIER_LENGTH = 128;
const MAX_TEXT_LENGTH = 100_000;
const MAX_PROVIDERS = 1_000;
const MAX_MODELS = 10_000;
const MAX_REASONING_LEVELS = 7;
const MAX_DIAGNOSTICS = 1_000;

const authenticationKinds = ["api_key", "subscription"] as const;
const configurationStates = ["ready", "invalid"] as const;
const capabilities = ["catalog_only", "configured_only", "executable"] as const;
const loginStages = [
  "starting",
  "opening_browser",
  "waiting_for_browser",
  "displaying_device_code",
  "finalizing",
  "completed",
  "cancelled",
  "failed",
] as const;
const reasoningLevels: readonly AgentChatReasoningLevel[] = [
  "off",
  "minimal",
  "low",
  "medium",
  "high",
  "x_high",
  "max",
];

export type AgentAuthenticationKind = (typeof authenticationKinds)[number];
export type AgentConfigurationState = (typeof configurationStates)[number];
export type AgentProviderCapability = (typeof capabilities)[number];

export type AgentConfigurationDiagnostic = {
  code: string;
  message: string;
};

export type AgentModelStatus = {
  id: string;
  displayName: string;
  reasoningLevels: AgentChatReasoningLevel[];
  executable: boolean;
};

export type ProviderConfigurationStatus = {
  id: string;
  displayName: string;
  authenticationMethods: AgentAuthenticationKind[];
  activeAuthentication: AgentAuthenticationKind | null;
  configuredByModelsFile: boolean;
  capability: AgentProviderCapability;
  executable: boolean;
  models: AgentModelStatus[];
};

export type AgentConfigurationStatus = {
  providers: ProviderConfigurationStatus[];
  authenticationConfiguration: AgentConfigurationState;
  modelConfiguration: AgentConfigurationState;
  diagnostics: AgentConfigurationDiagnostic[];
};

export type SubscriptionLoginStage = (typeof loginStages)[number];

export type SubscriptionLoginProgress = {
  attemptId: string;
  providerId: string;
  stage: SubscriptionLoginStage;
};

export type AgentConfigurationError = {
  code?: string;
  message?: string;
};

export const AGENT_SUBSCRIPTION_LOGIN_PROGRESS_EVENT =
  "agent-subscription-login-progress";

export class AgentConfigurationTransportError extends Error {
  readonly code: string;

  constructor(code = "transport_unavailable") {
    super("Agent configuration transport is unavailable.");
    this.name = "AgentConfigurationTransportError";
    this.code = code;
  }
}

export type AgentConfigurationClient = {
  getStatus(): Promise<AgentConfigurationStatus>;
  submitApiKey(providerId: string, apiKey: string): Promise<AgentConfigurationStatus>;
  loginSubscription(providerId: string, attemptId: string): Promise<AgentConfigurationStatus>;
  cancelSubscriptionLogin(attemptId: string): Promise<void>;
  removeAuthentication(providerId: string): Promise<AgentConfigurationStatus>;
  reload(): Promise<AgentConfigurationStatus>;
  openDataFolder(): Promise<void>;
  listenToSubscriptionLoginProgress(
    handler: (progress: SubscriptionLoginProgress) => void,
    onMalformed?: () => void,
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
    getStatus: () =>
      invokeDecoded(
        invokeCommand,
        "get_agent_configuration_status",
        undefined,
        decodeAgentConfigurationStatus,
      ),
    submitApiKey: (providerId, apiKey) =>
      invokeDecoded(
        invokeCommand,
        "submit_agent_api_key",
        { providerId, apiKey },
        decodeAgentConfigurationStatus,
      ),
    loginSubscription: (providerId, attemptId) =>
      invokeDecoded(
        invokeCommand,
        "login_agent_subscription",
        { providerId, attemptId },
        decodeAgentConfigurationStatus,
      ),
    cancelSubscriptionLogin: (attemptId) =>
      invokeDecoded(
        invokeCommand,
        "cancel_agent_subscription_login",
        { attemptId },
        decodeVoid,
      ),
    removeAuthentication: (providerId) =>
      invokeDecoded(
        invokeCommand,
        "remove_agent_authentication",
        { providerId },
        decodeAgentConfigurationStatus,
      ),
    reload: () =>
      invokeDecoded(
        invokeCommand,
        "reload_agent_configuration",
        undefined,
        decodeAgentConfigurationStatus,
      ),
    openDataFolder: () =>
      invokeDecoded(
        invokeCommand,
        "open_agent_data_folder",
        undefined,
        decodeVoid,
      ),
    listenToSubscriptionLoginProgress: (handler, onMalformed) =>
      listenToEvent<unknown>(
        AGENT_SUBSCRIPTION_LOGIN_PROGRESS_EVENT,
        (event) => {
          try {
            handler(decodeSubscriptionLoginProgress(event.payload));
          } catch {
            onMalformed?.();
          }
        },
      ),
  };
}

export function decodeAgentConfigurationStatus(
  value: unknown,
): AgentConfigurationStatus {
  if (
    !isRecord(value) ||
    !isArrayOf(value.providers, isProviderConfigurationStatus) ||
    value.providers.length > MAX_PROVIDERS ||
    !includes(configurationStates, value.authenticationConfiguration) ||
    !includes(configurationStates, value.modelConfiguration) ||
    !isArrayOf(value.diagnostics, isAgentConfigurationDiagnostic) ||
    value.diagnostics.length > MAX_DIAGNOSTICS
  ) {
    throw invalidResponse("Agent configuration status");
  }

  return {
    providers: value.providers,
    authenticationConfiguration: value.authenticationConfiguration,
    modelConfiguration: value.modelConfiguration,
    diagnostics: value.diagnostics,
  };
}

export function decodeSubscriptionLoginProgress(
  value: unknown,
): SubscriptionLoginProgress {
  if (
    !isRecord(value) ||
    !isIdentifier(value.attemptId) ||
    !isIdentifier(value.providerId) ||
    !includes(loginStages, value.stage)
  ) {
    throw invalidResponse("subscription login progress");
  }
  return {
    attemptId: value.attemptId,
    providerId: value.providerId,
    stage: value.stage,
  };
}

export function decodeAgentConfigurationError(
  value: unknown,
): AgentConfigurationTransportError {
  if (value instanceof AgentConfigurationTransportError) return value;
  if (isRecord(value) && isErrorCode(value.code)) {
    return new AgentConfigurationTransportError(value.code);
  }
  return new AgentConfigurationTransportError();
}

function isProviderConfigurationStatus(
  value: unknown,
): value is ProviderConfigurationStatus {
  if (
    !isRecord(value) ||
    !isIdentifier(value.id) ||
    !isDisplayText(value.displayName) ||
    !isArrayOf(value.authenticationMethods, isAuthenticationKind) ||
    value.authenticationMethods.length > authenticationKinds.length ||
    !isNullableAuthenticationKind(value.activeAuthentication) ||
    !isBoolean(value.configuredByModelsFile) ||
    !includes(capabilities, value.capability) ||
    !isBoolean(value.executable) ||
    !isArrayOf(value.models, isAgentModelStatus) ||
    value.models.length > MAX_MODELS
  ) {
    return false;
  }

  if (value.executable !== (value.capability === "executable")) return false;
  if (
    value.activeAuthentication !== null &&
    !value.authenticationMethods.includes(value.activeAuthentication)
  ) {
    return false;
  }
  return value.models.every(
    (model) => !model.executable || value.capability === "executable",
  );
}

function isAgentModelStatus(value: unknown): value is AgentModelStatus {
  return (
    isRecord(value) &&
    isIdentifier(value.id) &&
    isDisplayText(value.displayName) &&
    isArrayOf(value.reasoningLevels, isReasoningLevel) &&
    value.reasoningLevels.length > 0 &&
    value.reasoningLevels.length <= MAX_REASONING_LEVELS &&
    isBoolean(value.executable)
  );
}

function isAgentConfigurationDiagnostic(
  value: unknown,
): value is AgentConfigurationDiagnostic {
  return (
    isRecord(value) &&
    isErrorCode(value.code) &&
    isBoundedText(value.message)
  );
}

function isAuthenticationKind(value: unknown): value is AgentAuthenticationKind {
  return includes(authenticationKinds, value);
}

function isNullableAuthenticationKind(
  value: unknown,
): value is AgentAuthenticationKind | null {
  return value === null || isAuthenticationKind(value);
}

function isReasoningLevel(value: unknown): value is AgentChatReasoningLevel {
  return includes(reasoningLevels, value);
}

function isIdentifier(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    value.length <= MAX_IDENTIFIER_LENGTH &&
    /^[A-Za-z0-9._-]+$/.test(value)
  );
}

function isDisplayText(value: unknown): value is string {
  return isBoundedText(value) && value.trim().length > 0;
}

function isBoundedText(value: unknown): value is string {
  return typeof value === "string" && value.length <= MAX_TEXT_LENGTH;
}

function isBoolean(value: unknown): value is boolean {
  return typeof value === "boolean";
}

function isErrorCode(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    value.length <= MAX_IDENTIFIER_LENGTH &&
    /^[a-z0-9_]+$/.test(value)
  );
}

async function invokeDecoded<T>(
  invokeCommand: Invoke,
  command: string,
  args: Record<string, unknown> | undefined,
  decode: (value: unknown) => T,
): Promise<T> {
  let value: unknown;
  try {
    value = await invokeCommand<unknown>(command, args);
  } catch (error) {
    throw decodeAgentConfigurationError(error);
  }
  return decode(value);
}

function decodeVoid(value: unknown): void {
  if (value !== null && value !== undefined) {
    throw invalidResponse("Agent configuration command");
  }
}

function invalidResponse(label: string): Error {
  return new Error(`Invalid ${label} response.`);
}

function includes<const T extends string>(
  values: readonly T[],
  value: unknown,
): value is T {
  return values.includes(value as T);
}

export const agentConfigurationClient = createAgentConfigurationClient(
  invoke,
  listen,
);
