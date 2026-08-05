import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import {
  AlertCircleIcon,
  BrainIcon,
  Minimize2Icon,
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
  type AgentChatApplicationEvent,
  type AgentChatClient,
  type AgentChatContent,
  type AgentChatCreateInput,
  type AgentChatOpenInput,
  type AgentChatProjection,
  type AgentChatReasoningLevel,
} from "@/lib/api/agent-chat";
import {
  agentConfigurationClient,
  type AgentConfigurationClient,
  type AgentConfigurationStatus,
} from "@/lib/api/agent-configuration";
import type { TranslationKey } from "@/lib/i18n/resources";

type AgentChatRequest =
  | ({ type: "create" } & AgentChatCreateInput)
  | ({ type: "open" } & AgentChatOpenInput);

type AgentChatShellProps = {
  request: AgentChatRequest;
  title: string;
  contextLabel: string;
  canvas: ReactNode;
  chatClient?: AgentChatClient;
  configurationClient?: AgentConfigurationClient;
};

type StreamBlock = {
  index: number;
  type: "text" | "reasoning";
  text: string;
};

type SendOperation = {
  type: "send";
  phase: "submitted" | "streaming";
  user: string;
  blocks: StreamBlock[];
};

type CompactOperation = {
  type: "compact";
  phase: "submitted" | "streaming";
  reason: string | null;
};

type ActiveOperation = {
  type: "active";
  phase: "streaming";
};

type ReadyState = {
  type: "ready";
  chat: AgentChatProjection;
  configuration: AgentConfigurationStatus;
  operation: SendOperation | CompactOperation | ActiveOperation | null;
  unsavedTurn: { user: string; response: AgentChatContent[] } | null;
  errorCode: string | null;
  notice: "aborted" | "compacted" | "compaction_cancelled" | null;
};

type ShellState =
  | { type: "loading" }
  | { type: "failed"; code: string }
  | ReadyState;

const reasoningLevels: AgentChatReasoningLevel[] = [
  "off",
  "minimal",
  "low",
  "medium",
  "high",
  "x_high",
  "max",
];

function errorCode(error: unknown) {
  return typeof error === "object" &&
    error !== null &&
    "code" in error &&
    typeof error.code === "string"
    ? error.code
    : "unavailable";
}

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

function noticeKey(notice: ReadyState["notice"]): TranslationKey | null {
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

function streamContent(blocks: StreamBlock[]): AgentChatContent[] {
  return [...blocks]
    .sort((left, right) => left.index - right.index)
    .map((block) => ({ type: block.type, text: block.text }));
}

function reduceEvent(current: ShellState, event: AgentChatApplicationEvent): ShellState {
  if (current.type !== "ready") return current;

  switch (event.type) {
    case "started":
      return current.operation?.type === "send"
        ? {
            ...current,
            operation: { ...current.operation, phase: "streaming" },
          }
        : current;
    case "content_started": {
      if (!current.operation) return current;
      const sendOperation: SendOperation =
        current.operation.type === "send"
          ? current.operation
          : {
              type: "send",
              phase: "streaming",
              user: "",
              blocks: [],
            };
      return {
        ...current,
        operation: {
          ...sendOperation,
          phase: "streaming",
          blocks: sendOperation.blocks.some((block) => block.index === event.index)
            ? sendOperation.blocks
            : [
                ...sendOperation.blocks,
                { index: event.index, type: event.kind, text: "" },
              ],
        },
      };
    }
    case "content_delta":
      if (current.operation?.type !== "send") return current;
      return {
        ...current,
        operation: {
          ...current.operation,
          phase: "streaming",
          blocks: current.operation.blocks.map((block) =>
            block.index === event.index
              ? { ...block, text: block.text + event.delta }
              : block,
          ),
        },
      };
    case "completed":
      return {
        ...current,
        chat: event.chat,
        operation: null,
        unsavedTurn: null,
        errorCode: null,
        notice: null,
      };
    case "failed":
      return {
        ...current,
        operation: null,
        errorCode: event.error.code ?? "unavailable",
      };
    case "not_saved":
      return {
        ...current,
        chat: event.chat,
        operation: null,
        unsavedTurn: {
          user: current.operation?.type === "send" ? current.operation.user : "",
          response: event.response,
        },
        errorCode: event.error.code ?? "not_saved",
        notice: null,
      };
    case "aborted":
      return {
        ...current,
        operation: null,
        errorCode: null,
        notice:
          current.operation?.type === "compact"
            ? "compaction_cancelled"
            : "aborted",
      };
    case "compaction_started":
      return {
        ...current,
        operation: {
          type: "compact",
          phase: "streaming",
          reason: event.reason,
        },
        errorCode: null,
        notice: null,
      };
    case "compaction_completed":
      return {
        ...current,
        chat: event.chat,
        operation: null,
        errorCode: null,
        notice: "compacted",
      };
    case "compaction_cancelled":
      return {
        ...current,
        operation: null,
        errorCode: null,
        notice: "compaction_cancelled",
      };
    case "compaction_failed":
      return {
        ...current,
        operation: null,
        errorCode: event.error.code ?? "provider_failed",
      };
    case "compaction_not_saved":
      return {
        ...current,
        chat: event.chat,
        operation: null,
        errorCode: event.error.code ?? "not_saved",
      };
    default:
      return current;
  }
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
  const [state, setState] = useState<ShellState>({ type: "loading" });
  const [draft, setDraft] = useState("");
  const chatId = useRef<string | null>(null);
  const lastSequence = useRef(0);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    const initialize = async () => {
      try {
        const stopListening = await chatApi.listen((event) => {
          if (
            !active ||
            event.chatId !== chatId.current ||
            event.sequence <= lastSequence.current
          ) {
            return;
          }
          lastSequence.current = event.sequence;
          setState((current) => reduceEvent(current, event));
        });
        if (!active) {
          stopListening();
          return;
        }
        unlisten = stopListening;

        const [chat, configuration] = await Promise.all([
          request.type === "create"
            ? chatApi.create({
                systemPrompt: request.systemPrompt,
                providerId: request.providerId,
                modelId: request.modelId,
                reasoningLevel: request.reasoningLevel,
              })
            : chatApi.open({
                id: request.id,
                systemPrompt: request.systemPrompt,
              }),
          configurationApi.getStatus(),
        ]);
        if (active) {
          chatId.current = chat.id;
          setState({
            type: "ready",
            chat,
            configuration,
            operation:
              chat.status === "running"
                ? { type: "active", phase: "streaming" }
                : null,
            unsavedTurn: null,
            errorCode: null,
            notice: null,
          });
        }
      } catch (error) {
        if (active) setState({ type: "failed", code: errorCode(error) });
      }
    };

    void initialize();
    return () => {
      active = false;
      unlisten?.();
    };
  }, [
    chatApi,
    configurationApi,
    request.type,
    request.systemPrompt,
    request.type === "open" ? request.id : request.providerId,
    request.type === "create" ? request.modelId : null,
    request.type === "create" ? request.reasoningLevel : null,
  ]);

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

  const submit = async (message: PromptInputMessage) => {
    const text = message.text.trim();
    if (!text || !mutable || busy) return;
    setDraft("");
    setState((current) =>
      current.type === "ready"
        ? {
            ...current,
            operation: {
              type: "send",
              phase: "submitted",
              user: text,
              blocks: [],
            },
            unsavedTurn: null,
            errorCode: null,
            notice: null,
          }
        : current,
    );
    try {
      await chatApi.send(state.chat.id, text);
    } catch (error) {
      setState((current) =>
        current.type === "ready"
          ? { ...current, operation: null, errorCode: errorCode(error) }
          : current,
      );
    }
  };

  const stop = async () => {
    try {
      const stopped = await chatApi.stop(state.chat.id);
      if (!stopped) {
        setState((current) =>
          current.type === "ready"
            ? { ...current, operation: null, errorCode: "not_running" }
            : current,
        );
      }
    } catch (error) {
      setState((current) =>
        current.type === "ready"
          ? { ...current, errorCode: errorCode(error) }
          : current,
      );
    }
  };

  const compactChat = async () => {
    if (!mutable || busy) return;
    setState((current) =>
      current.type === "ready"
        ? {
            ...current,
            operation: {
              type: "compact",
              phase: "submitted",
              reason: null,
            },
            errorCode: null,
            notice: null,
          }
        : current,
    );
    try {
      await chatApi.compact(state.chat.id, null);
    } catch (error) {
      setState((current) =>
        current.type === "ready"
          ? { ...current, operation: null, errorCode: errorCode(error) }
          : current,
      );
    }
  };

  const selectModel = async (value: unknown) => {
    const selection = availableModels.find((candidate) => candidate.value === value);
    if (!selection || busy || !modelChangeAllowed) return;
    try {
      const chat = await chatApi.setModel(
        state.chat.id,
        selection.providerId,
        selection.model.id,
      );
      setState((current) =>
        current.type === "ready"
          ? { ...current, chat, errorCode: null, notice: null }
          : current,
      );
    } catch (error) {
      setState((current) =>
        current.type === "ready"
          ? { ...current, errorCode: errorCode(error) }
          : current,
      );
    }
  };

  const selectReasoning = async (value: unknown) => {
    if (
      !reasoningLevels.includes(value as AgentChatReasoningLevel) ||
      busy ||
      !mutable
    ) {
      return;
    }
    try {
      const chat = await chatApi.setReasoningLevel(
        state.chat.id,
        value as AgentChatReasoningLevel,
      );
      setState((current) =>
        current.type === "ready"
          ? { ...current, chat, errorCode: null, notice: null }
          : current,
      );
    } catch (error) {
      setState((current) =>
        current.type === "ready"
          ? { ...current, errorCode: errorCode(error) }
          : current,
      );
    }
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
                        onValueChange={(value) => void selectModel(value)}
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
                        onValueChange={(value) => void selectReasoning(value)}
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
                      onStop={() => void stop()}
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
