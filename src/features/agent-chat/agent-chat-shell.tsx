import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import {
  AlertCircleIcon,
  BrainIcon,
  Minimize2Icon,
  RotateCcwIcon,
  SparklesIcon,
} from "lucide-react";

import {
  Context,
  ContextContent,
  ContextContentHeader,
  ContextTrigger,
} from "@/components/ai-elements/context";
import {
  Message,
  MessageContent,
  MessageResponse,
} from "@/components/ai/message";
import {
  PromptInput,
  PromptInputBody,
  PromptInputFooter,
  type PromptInputMessage,
  PromptInputSelect,
  PromptInputSelectContent,
  PromptInputSelectItem,
  PromptInputSelectTrigger,
  PromptInputSelectValue,
  PromptInputSubmit,
  PromptInputTextarea,
  PromptInputTools,
} from "@/components/ai-elements/prompt-input";
import {
  Reasoning,
  ReasoningContent,
  ReasoningTrigger,
} from "@/components/ai-elements/reasoning";
import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge } from "@/components/reui/badge";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { SelectGroup } from "@/components/ui/select";
import {
  agentChatClient,
  type AgentChatClient,
  type AgentChatContent,
  type AgentChatProjection,
  type AgentChatReasoningLevel,
} from "@/lib/api/agent-chat";
import {
  agentConfigurationClient,
  type AgentConfigurationClient,
} from "@/lib/api/agent-configuration";
import type { TranslationKey } from "@/lib/i18n/resources";
import {
  useAgentChat,
  type AgentChatRequest,
} from "@/features/agent-chat/use-agent-chat";

type AgentChatShellProps = {
  request: AgentChatRequest;
  title: string;
  contextLabel: string;
  canvas: ReactNode;
  chatClient?: AgentChatClient;
  configurationClient?: AgentConfigurationClient;
};


const reasoningLevels: AgentChatReasoningLevel[] = [
  "off",
  "minimal",
  "low",
  "medium",
  "high",
  "x_high",
  "max",
];

function statusKey(status: AgentChatProjection["status"]): TranslationKey {
  const keys: Record<AgentChatProjection["status"], TranslationKey> = {
    ready: "agentChat.status.saved",
    running: "agentChat.status.running",
    model_unavailable: "agentChat.status.modelUnavailable",
    read_only_locked: "agentChat.status.readOnly",
    read_only_unsupported: "agentChat.status.readOnly",
    damaged: "agentChat.status.damaged",
    not_saved: "agentChat.status.notSaved",
  };
  return keys[status];
}

function projectionStateKey(status: AgentChatProjection["status"]): TranslationKey | null {
  const keys: Partial<Record<AgentChatProjection["status"], TranslationKey>> = {
    running: "agentChat.states.running",
    model_unavailable: "agentChat.states.modelUnavailable",
    read_only_locked: "agentChat.states.readOnlyLocked",
    read_only_unsupported: "agentChat.states.readOnlyUnsupported",
    damaged: "agentChat.states.damaged",
    not_saved: "agentChat.errors.notSaved",
  };
  return keys[status] ?? null;
}

function noticeKey(
  notice: "aborted" | "compacted" | "compaction_cancelled" | null,
): TranslationKey | null {
  if (notice === "aborted") return "agentChat.notices.aborted";
  if (notice === "compacted") return "agentChat.notices.compacted";
  if (notice === "compaction_cancelled") {
    return "agentChat.notices.compactionCancelled";
  }
  return null;
}

function actionErrorKey(code: string): TranslationKey {
  const keys: Record<string, TranslationKey> = {
    authentication_unavailable: "agentChat.errors.authenticationUnavailable",
    invalid_configuration: "agentChat.errors.invalidConfiguration",
    rate_limited: "agentChat.errors.rateLimited",
    transport_unavailable: "agentChat.errors.transportUnavailable",
    model_unavailable: "agentChat.errors.modelUnavailable",
    context_full: "agentChat.errors.contextFull",
    provider_failed: "agentChat.errors.providerFailed",
    chat_busy: "agentChat.errors.chatBusy",
    chat_read_only: "agentChat.errors.readOnly",
    chat_unsupported: "agentChat.errors.unsupported",
    chat_damaged: "agentChat.errors.damaged",
    not_saved: "agentChat.errors.notSaved",
  };
  return keys[code] ?? "agentChat.errors.unavailable";
}

function AssistantContent({
  content,
  streaming = false,
}: {
  content: AgentChatContent[];
  streaming?: boolean;
}) {
  const { t } = useTranslation();
  const reasoning = content
    .filter((block): block is Extract<AgentChatContent, { type: "reasoning" }> =>
      block.type === "reasoning",
    )
    .map((block) => block.text)
    .join("\n\n");
  const hasRedactedReasoning = content.some(
    (block) => block.type === "redacted_reasoning",
  );

  return (
    <Message from="assistant">
      <MessageContent>
        {reasoning || hasRedactedReasoning ? (
          <Reasoning defaultOpen={Boolean(reasoning)} isStreaming={streaming}>
            <ReasoningTrigger>
              <BrainIcon aria-hidden="true" />
              {hasRedactedReasoning && !reasoning
                ? t("agentChat.messages.reasoningRedacted")
                : t("agentChat.messages.reasoning")}
            </ReasoningTrigger>
            {reasoning ? <ReasoningContent>{reasoning}</ReasoningContent> : null}
          </Reasoning>
        ) : null}
        {content.map((block, index) =>
          block.type === "text" ? (
            <MessageResponse key={`text-${index}`}>{block.text}</MessageResponse>
          ) : null,
        )}
      </MessageContent>
    </Message>
  );
}

function streamContent(
  blocks: Array<{ index: number; type: "text" | "reasoning"; text: string }>,
): AgentChatContent[] {
  return [...blocks]
    .sort((left, right) => left.index - right.index)
    .map((block) => ({ type: block.type, text: block.text }));
}

export function AgentChatShell({
  request,
  title,
  contextLabel,
  canvas,
  chatClient: chatApi = agentChatClient,
  configurationClient: configurationApi = agentConfigurationClient,
}: AgentChatShellProps) {
  const { t } = useTranslation();
  const {
    state,
    submit: submitMessage,
    stop: stopOperation,
    reload: reloadChat,
    compact: compactChat,
    selectModel: selectModelOperation,
    selectReasoning: selectReasoningOperation,
  } = useAgentChat({
    request,
    chatClient: chatApi,
    configurationClient: configurationApi,
  });
  const [draft, setDraft] = useState("");

  if (state.type === "loading") {
    return <p role="status">{t("agentChat.loading")}</p>;
  }

  if (state.type === "failed") {
    return (
      <Alert variant="destructive">
        <AlertCircleIcon aria-hidden="true" />
        <AlertTitle>{t("agentChat.errors.loadTitle")}</AlertTitle>
        <AlertDescription>{t("agentChat.errors.unavailable")}</AlertDescription>
      </Alert>
    );
  }

  const mutable = state.chat.status === "ready";
  const modelChangeAllowed =
    state.chat.status === "ready" || state.chat.status === "model_unavailable";
  const busy = state.operation !== null;
  const availableModels = state.configuration.providers.flatMap((provider) =>
    provider.models
      .filter((model) => provider.executable && model.executable)
      .map((model) => ({
        providerId: provider.id,
        providerName: provider.displayName,
        model,
        value: `${provider.id}\u001f${model.id}`,
      })),
  );
  const selectedModel = availableModels.find(
    (candidate) =>
      candidate.providerId === state.chat.selectedProviderId &&
      candidate.model.id === state.chat.selectedModelId,
  );
  const supportedReasoning = selectedModel?.model.reasoningLevels.filter(
    (level): level is AgentChatReasoningLevel =>
      reasoningLevels.includes(level as AgentChatReasoningLevel),
  ) ?? [state.chat.reasoningLevel];

  const submit = (message: PromptInputMessage) => {
    const text = message.text.trim();
    if (!text || !mutable || busy) return;
    setDraft("");
    submitMessage(text);
  };

  const handleModelChange = (value: unknown) => {
    const selection = availableModels.find((candidate) => candidate.value === value);
    if (!selection || busy || !modelChangeAllowed) return;
    selectModelOperation(selection.providerId, selection.model.id);
  };

  const handleReasoningChange = (value: unknown) => {
    if (
      !reasoningLevels.includes(value as AgentChatReasoningLevel) ||
      busy ||
      !mutable
    ) {
      return;
    }
    selectReasoningOperation(value as AgentChatReasoningLevel);
  };

  const promptStatus = state.operation?.phase ?? "ready";

  return (
    <div className="size-full bg-muted/30 p-4">
      <div className="size-full overflow-hidden rounded-lg border bg-background shadow-sm">
        <ResizablePanelGroup orientation="horizontal">
          <ResizablePanel defaultSize={44} minSize={32}>
            <section
              aria-label="Agent Chat"
              className="flex size-full min-h-0 flex-col bg-background"
            >
              <header className="flex min-h-11 items-center gap-2 border-b px-4">
                <SparklesIcon aria-hidden="true" className="text-muted-foreground" />
                <div className="min-w-0">
                  <h1 className="truncate text-sm font-medium">{title}</h1>
                  <p className="truncate text-xs text-muted-foreground">
                    {contextLabel}
                  </p>
                </div>
                <Button
                  aria-label={t("agentChat.actions.compact")}
                  className="ml-auto"
                  disabled={!mutable || busy || state.chat.history.length === 0}
                  onClick={() => void compactChat()}
                  size="icon-sm"
                  type="button"
                  variant="ghost"
                >
                  <Minimize2Icon aria-hidden="true" />
                </Button>
                {state.chat.status === "not_saved" ? (
                  <Button
                    aria-label={t("agentChat.actions.reload")}
                    disabled={busy}
                    onClick={() => void reloadChat()}
                    size="icon-sm"
                    type="button"
                    variant="ghost"
                  >
                    <RotateCcwIcon aria-hidden="true" />
                  </Button>
                ) : null}
                <Badge size="sm" variant="secondary">
                  {t(statusKey(state.chat.status))}
                </Badge>
              </header>

              <div aria-live="polite" className="px-3 pt-3">
                {state.errorCode ? (
                  <Alert variant="destructive">
                    <AlertCircleIcon aria-hidden="true" />
                    <AlertTitle>{t("agentChat.errors.actionTitle")}</AlertTitle>
                    <AlertDescription>
                      {t(actionErrorKey(state.errorCode))}
                    </AlertDescription>
                  </Alert>
                ) : projectionStateKey(state.chat.status) ? (
                  <Alert
                    variant={state.chat.status === "damaged" ? "destructive" : "warning"}
                  >
                    <AlertCircleIcon aria-hidden="true" />
                    <AlertTitle>{t("agentChat.states.title")}</AlertTitle>
                    <AlertDescription>
                      {t(projectionStateKey(state.chat.status) as TranslationKey)}
                    </AlertDescription>
                  </Alert>
                ) : null}
                {state.chat.recoveryNotices.includes(
                  "incomplete_final_turn_discarded",
                ) ? (
                  <Alert className="mt-2" variant="warning">
                    <AlertCircleIcon aria-hidden="true" />
                    <AlertTitle>{t("agentChat.recovery.title")}</AlertTitle>
                    <AlertDescription>
                      {t("agentChat.recovery.incompleteFinalTurnDiscarded")}
                    </AlertDescription>
                  </Alert>
                ) : null}
                {state.operation?.type === "compact" ? (
                  <p className="text-xs text-muted-foreground" role="status">
                    {t("agentChat.compaction.running")}
                  </p>
                ) : null}
                {noticeKey(state.notice) ? (
                  <p className="text-xs text-muted-foreground" role="status">
                    {t(noticeKey(state.notice) as TranslationKey)}
                  </p>
                ) : null}
              </div>

              <ScrollArea className="min-h-0 flex-1">
                <div className="mx-auto flex w-full max-w-3xl flex-col gap-7 px-5 py-8">
                  {state.chat.history.map((entry, index) =>
                    entry.type === "turn" ? (
                      <div className="flex flex-col gap-7" key={`turn-${index}`}>
                        <Message from="user">
                          <MessageContent>{entry.user}</MessageContent>
                        </Message>
                        <AssistantContent content={entry.assistant} />
                      </div>
                    ) : (
                      <div
                        className="flex items-center justify-center gap-2 text-xs text-muted-foreground"
                        key={`compaction-${index}`}
                        role="note"
                      >
                        <Minimize2Icon aria-hidden="true" />
                        <span>{t("agentChat.compaction.marker")}</span>
                        <span>
                          {t("agentChat.compaction.tokensBefore", {
                            count: entry.tokens_before,
                          })}
                        </span>
                      </div>
                    ),
                  )}
                  {state.operation?.type === "send" ? (
                    <div className="flex flex-col gap-7">
                      {state.operation.user ? (
                        <Message from="user">
                          <MessageContent>{state.operation.user}</MessageContent>
                        </Message>
                      ) : null}
                      {state.operation.blocks.length ? (
                        <AssistantContent
                          content={streamContent(state.operation.blocks)}
                          streaming
                        />
                      ) : (
                        <p className="text-xs text-muted-foreground" role="status">
                          {t("agentChat.status.waiting")}
                        </p>
                      )}
                    </div>
                  ) : null}
                  {state.unsavedTurn ? (
                    <div className="flex flex-col gap-7">
                      {state.unsavedTurn.user ? (
                        <Message from="user">
                          <MessageContent>{state.unsavedTurn.user}</MessageContent>
                        </Message>
                      ) : null}
                      <div className="rounded-md border border-warning/30 p-3">
                        <p className="mb-3 text-xs font-medium text-warning">
                          {t("agentChat.status.notSaved")}
                        </p>
                        <AssistantContent content={state.unsavedTurn.response} />
                      </div>
                    </div>
                  ) : null}
                </div>
              </ScrollArea>

              <div className="border-t p-3">
                <PromptInput
                  className="mx-auto w-full max-w-3xl"
                  onSubmit={(message) => void submit(message)}
                >
                  <PromptInputBody>
                    <PromptInputTextarea
                      aria-label={t("agentChat.composer.label")}
                      disabled={!mutable || busy}
                      onChange={(event) => setDraft(event.currentTarget.value)}
                      placeholder={t("agentChat.composer.placeholder")}
                    />
                  </PromptInputBody>
                  <PromptInputFooter>
                    <PromptInputTools className="flex-wrap">
                      <PromptInputSelect
                        disabled={busy || !modelChangeAllowed}
                        onValueChange={handleModelChange}
                        value={selectedModel?.value ?? null}
                      >
                        <PromptInputSelectTrigger
                          aria-label={t("agentChat.actions.selectModel")}
                        >
                          <PromptInputSelectValue />
                        </PromptInputSelectTrigger>
                        <PromptInputSelectContent>
                          <SelectGroup>
                            {availableModels.map((candidate) => (
                              <PromptInputSelectItem
                                key={candidate.value}
                                value={candidate.value}
                              >
                                {candidate.providerName} · {candidate.model.displayName}
                              </PromptInputSelectItem>
                            ))}
                          </SelectGroup>
                        </PromptInputSelectContent>
                      </PromptInputSelect>
                      <PromptInputSelect
                        disabled={busy || !mutable}
                        onValueChange={handleReasoningChange}
                        value={state.chat.reasoningLevel}
                      >
                        <PromptInputSelectTrigger
                          aria-label={t("agentChat.actions.selectReasoning")}
                        >
                          <PromptInputSelectValue />
                        </PromptInputSelectTrigger>
                        <PromptInputSelectContent>
                          <SelectGroup>
                            {supportedReasoning.map((level) => (
                              <PromptInputSelectItem key={level} value={level}>
                                {t(`agentChat.reasoning.${level}`)}
                              </PromptInputSelectItem>
                            ))}
                          </SelectGroup>
                        </PromptInputSelectContent>
                      </PromptInputSelect>
                      {state.chat.contextWindow ? (
                        <Context
                          maxTokens={state.chat.contextWindow}
                          modelId={state.chat.selectedModelId ?? undefined}
                          usedTokens={state.chat.contextTokens}
                        >
                          <ContextTrigger
                            aria-label={t("agentChat.context.label")}
                            size="sm"
                          />
                          <ContextContent>
                            <ContextContentHeader />
                            <p className="p-3 text-xs text-muted-foreground">
                              {t("agentChat.context.estimated")}
                            </p>
                          </ContextContent>
                        </Context>
                      ) : (
                        <span
                          aria-label={t("agentChat.context.unavailable")}
                          className="text-xs text-muted-foreground"
                          role="status"
                        >
                          {t("agentChat.context.unavailableShort")}
                        </span>
                      )}
                    </PromptInputTools>
                    <PromptInputSubmit
                      aria-label={
                        busy
                          ? t("agentChat.actions.stop")
                          : t("agentChat.actions.send")
                      }
                      disabled={!busy && (!mutable || !draft.trim())}
                      onStop={() => stopOperation()}
                      status={promptStatus}
                    />
                  </PromptInputFooter>
                </PromptInput>
              </div>
            </section>
          </ResizablePanel>
          <ResizableHandle aria-label={t("agentChat.actions.resize")} withHandle />
          <ResizablePanel defaultSize={56} minSize={36}>
            {canvas}
          </ResizablePanel>
        </ResizablePanelGroup>
      </div>
    </div>
  );
}
