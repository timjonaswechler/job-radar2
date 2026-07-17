import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  AGENT_SUBSCRIPTION_LOGIN_PROGRESS_EVENT,
  createAgentConfigurationClient,
  type AgentConfigurationStatus,
  type SubscriptionLoginProgress,
} from "@/lib/api/agent-configuration";

const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
const status: AgentConfigurationStatus = {
  providers: [],
  authenticationConfiguration: "ready",
  modelConfiguration: "ready",
  diagnostics: [],
};
let progressHandler:
  | ((event: { payload: SubscriptionLoginProgress }) => void)
  | undefined;

const client = createAgentConfigurationClient(
  async <T>(command: string, args?: Record<string, unknown>) => {
    calls.push({ command, args });
    if (command === "cancel_agent_subscription_login") return true as T;
    if (command === "open_agent_data_folder") return undefined as T;
    return status as T;
  },
  async <T>(_event: string, handler: (event: { payload: T }) => void) => {
    progressHandler = handler as typeof progressHandler;
    return () => undefined;
  },
);

await client.getStatus();
await client.submitApiKey("provider-one", "synthetic-test-value");
await client.loginSubscription("provider-one");
await client.cancelSubscriptionLogin("provider-one");
await client.removeAuthentication("provider-one");
await client.reload();
await client.openDataFolder();

assert.deepEqual(calls, [
  { command: "get_agent_configuration_status", args: undefined },
  {
    command: "submit_agent_api_key",
    args: { providerId: "provider-one", apiKey: "synthetic-test-value" },
  },
  { command: "login_agent_subscription", args: { providerId: "provider-one" } },
  {
    command: "cancel_agent_subscription_login",
    args: { providerId: "provider-one" },
  },
  {
    command: "remove_agent_authentication",
    args: { providerId: "provider-one" },
  },
  { command: "reload_agent_configuration", args: undefined },
  { command: "open_agent_data_folder", args: undefined },
]);

let receivedProgress: SubscriptionLoginProgress | null = null;
await client.listenToSubscriptionLoginProgress((progress) => {
  receivedProgress = progress;
});
assert.ok(progressHandler);
progressHandler?.({
  payload: { providerId: "provider-one", stage: "waiting_for_browser" },
});
assert.deepEqual(receivedProgress, {
  providerId: "provider-one",
  stage: "waiting_for_browser",
});
assert.equal(
  AGENT_SUBSCRIPTION_LOGIN_PROGRESS_EVENT,
  "agent-subscription-login-progress",
);

const componentSource = readFileSync(
  "src/features/settings/agent-provider-settings.tsx",
  "utf8",
);
assert.match(componentSource, /type="password"/);
assert.match(componentSource, /input\.value = "";[\s\S]*onSubmitApiKey\(apiKey\)/);
assert.match(componentSource, /<AlertDialog[\s\S]*handleRemove/);
assert.match(componentSource, /role="status" aria-live="polite"/);
assert.match(componentSource, /<Accordion[\s\S]*<AccordionTrigger/);
assert.doesNotMatch(componentSource, /diagnostic\.message/);
assert.doesNotMatch(componentSource, /\b(?:error|configurationError)\.message\b/);
