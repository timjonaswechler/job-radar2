import { useMemo, useState } from "react";

import { AgentChatShell } from "@/features/agent-chat/agent-chat-shell";
import {
  type AgentChatClient,
  type AgentChatProjection,
} from "@/lib/api/agent-chat";
import type {
  AgentConfigurationClient,
  AgentConfigurationStatus,
} from "@/lib/api/agent-configuration";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

import { AgentCanvasEditor } from "./agent-canvas-editor";

const REVIEW_SYSTEM_PROMPT =
  "You are reviewing the reusable Job Radar Agent Chat shell. Reply concisely and do not claim to change the caller-owned Canvas.";

const configuration: AgentConfigurationStatus = {
  authenticationConfiguration: "ready",
  modelConfiguration: "ready",
  diagnostics: [],
  providers: [
    {
      id: "openai-codex",
      displayName: "OpenAI Codex",
      authenticationMethods: ["subscription"],
      activeAuthentication: "subscription",
      configuredByModelsFile: false,
      available: true,
      models: [
        {
          id: "gpt-5.4",
          displayName: "GPT-5.4",
          reasoningLevels: ["off", "minimal", "low", "medium", "high", "x_high"],
        },
        {
          id: "gpt-5.6-sol",
          displayName: "GPT-5.6 Sol",
          reasoningLevels: [
            "off",
            "minimal",
            "low",
            "medium",
            "high",
            "x_high",
            "max",
          ],
        },
      ],
    },
  ],
};

const baseProjection: AgentChatProjection = {
  id: "00000000-0000-7000-8000-000000000230",
  status: "ready",
  history: [
    {
      type: "turn",
      user: "Prepare a concise application draft.",
      assistant: [
        {
          type: "reasoning",
          text: "I will use only the supplied evidence and keep the draft concise.",
        },
        { type: "text", text: "Here is the saved review response." },
      ],
    },
    { type: "compaction", reason: "manual", tokens_before: 18420 },
  ],
  selectedProviderId: "openai-codex",
  selectedModelId: "gpt-5.4",
  reasoningLevel: "medium",
  contextTokens: 4380,
  contextWindow: 272000,
  recoveryNotices: [],
};

type ReviewMode =
  | "live"
  | "model_unavailable"
  | "read_only_locked"
  | "read_only_unsupported"
  | "damaged"
  | "not_saved"
  | "recovered";

function projectionFor(mode: Exclude<ReviewMode, "live">): AgentChatProjection {
  if (mode === "recovered") {
    return {
      ...baseProjection,
      recoveryNotices: ["incomplete_final_turn_discarded"],
    };
  }
  return { ...baseProjection, status: mode };
}

function reviewClients(projection: AgentChatProjection): {
  chatClient: AgentChatClient;
  configurationClient: AgentConfigurationClient;
} {
  const chatClient: AgentChatClient = {
    create: async () => projection,
    open: async () => projection,
    send: async () => undefined,
    stop: async () => true,
    setModel: async (_chatId, providerId, modelId) => ({
      ...projection,
      status: "ready",
      selectedProviderId: providerId,
      selectedModelId: modelId,
    }),
    setReasoningLevel: async (_chatId, reasoningLevel) => ({
      ...projection,
      reasoningLevel,
    }),
    compact: async () => undefined,
    listen: async () => () => undefined,
  };
  const configurationClient: AgentConfigurationClient = {
    getStatus: async () => configuration,
    submitApiKey: async () => configuration,
    loginSubscription: async () => configuration,
    cancelSubscriptionLogin: async () => true,
    removeAuthentication: async () => configuration,
    reload: async () => configuration,
    openDataFolder: async () => undefined,
    listenToSubscriptionLoginProgress: async () => () => undefined,
  };
  return { chatClient, configurationClient };
}

/** Hidden production-API and exceptional-state review harness for issue #230. */
export function AgentChatPrototype() {
  const [mode, setMode] = useState<ReviewMode>("live");
  const clients = useMemo(
    () => (mode === "live" ? null : reviewClients(projectionFor(mode))),
    [mode],
  );

  return (
    <div className="flex size-full min-h-0 flex-col">
      <div className="flex h-10 shrink-0 items-center justify-between gap-3 border-b px-4">
        <p className="text-xs text-muted-foreground">
          Issue #230 human review harness
        </p>
        <Select
          onValueChange={(value) => setMode(value as ReviewMode)}
          value={mode}
        >
          <SelectTrigger aria-label="Review state" size="sm">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectGroup>
              <SelectItem value="live">Live persistent API</SelectItem>
              <SelectItem value="model_unavailable">Model unavailable</SelectItem>
              <SelectItem value="read_only_locked">Read-only locked</SelectItem>
              <SelectItem value="read_only_unsupported">
                Read-only unsupported
              </SelectItem>
              <SelectItem value="damaged">Damaged</SelectItem>
              <SelectItem value="not_saved">Not saved</SelectItem>
              <SelectItem value="recovered">Recovered final turn</SelectItem>
            </SelectGroup>
          </SelectContent>
        </Select>
      </div>
      <div className="min-h-0 flex-1">
        <AgentChatShell
          key={mode}
          request={{
            type: "create",
            systemPrompt: REVIEW_SYSTEM_PROMPT,
            providerId: "openai-codex",
            modelId: "gpt-5.4",
            reasoningLevel: "medium",
          }}
          title="Agent Chat UI review"
          contextLabel="Lab context · Not linked to a domain object"
          canvas={<AgentCanvasEditor />}
          chatClient={clients?.chatClient}
          configurationClient={clients?.configurationClient}
        />
      </div>
    </div>
  );
}
