import { useEffect, useRef, useState } from "react";
import {
  CopyIcon,
  GlobeIcon,
  RefreshCcwIcon,
  SparklesIcon,
  ThumbsDownIcon,
  ThumbsUpIcon,
} from "lucide-react";

import {
  Context,
  ContextCacheUsage,
  ContextContent,
  ContextContentBody,
  ContextContentFooter,
  ContextContentHeader,
  ContextInputUsage,
  ContextOutputUsage,
  ContextReasoningUsage,
  ContextTrigger,
} from "@/components/ai-elements/context";
import {
  PromptInput,
  PromptInputActionAddAttachments,
  PromptInputActionAddScreenshot,
  PromptInputActionMenu,
  PromptInputActionMenuContent,
  PromptInputActionMenuTrigger,
  PromptInputBody,
  PromptInputButton,
  PromptInputFooter,
  PromptInputHeader,
  type PromptInputMessage,
  PromptInputSelect,
  PromptInputSelectContent,
  PromptInputSelectItem,
  PromptInputSelectTrigger,
  PromptInputSelectValue,
  PromptInputSubmit,
  PromptInputTextarea,
  PromptInputTools,
  usePromptInputAttachments,
} from "@/components/ai-elements/prompt-input";
import {
  Reasoning,
  ReasoningContent,
  ReasoningTrigger,
} from "@/components/ai-elements/reasoning";
import { Action, Actions } from "@/components/ai/actions";
import {
  Attachment,
  AttachmentInfo,
  AttachmentPreview,
  AttachmentRemove,
  Attachments,
} from "@/components/ai/attachments";
import {
  Message,
  MessageContent,
  MessageResponse,
} from "@/components/ai/message";
import {
  Source,
  Sources,
  SourcesContent,
  SourcesTrigger,
} from "@/components/ai/sources";
import { Badge } from "@/components/ui/badge";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { DropdownMenuGroup } from "@/components/ui/dropdown-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import { SelectGroup } from "@/components/ui/select";

import { AgentCanvasEditor } from "./agent-canvas-editor";

type MockChatStatus = "ready" | "streaming" | "submitted";

type ChatMessage = {
  id: number;
  from: "assistant" | "user";
  content: string;
};

const MODELS = [
  {
    id: "anthropic:claude-sonnet-4-20250514",
    maxTokens: 200_000,
    name: "Claude Sonnet",
  },
  { id: "openai:gpt-5", maxTokens: 272_000, name: "GPT-5" },
  { id: "google:gemini-2.5-pro", maxTokens: 128_000, name: "Gemini Pro" },
] as const;

type ModelId = (typeof MODELS)[number]["id"];

const MOCK_USAGE = {
  inputTokenDetails: {
    cacheReadTokens: 740,
    cacheWriteTokens: 0,
    noCacheTokens: 3640,
  },
  inputTokens: 4380,
  outputTokenDetails: {
    reasoningTokens: 510,
    textTokens: 410,
  },
  outputTokens: 920,
  totalTokens: 5300,
};

const initialMessages: ChatMessage[] = [
  {
    id: 1,
    from: "user",
    content:
      "Entwirf ein kurzes Anschreiben für die Product-Designer-Stelle. Nutze meine Portfolio-Notizen und öffne den Entwurf im Canvas.",
  },
  {
    id: 2,
    from: "assistant",
    content:
      "Ich habe einen ersten Entwurf im Canvas erstellt. Er betont deine Erfahrung mit **datenintensiven B2B-Produkten** und die Zusammenarbeit mit Engineering.",
  },
];

function PromptAttachments() {
  const attachments = usePromptInputAttachments();

  if (attachments.files.length === 0) return null;

  return (
    <Attachments variant="inline">
      {attachments.files.map((attachment) => (
        <Attachment
          data={attachment}
          key={attachment.id}
          onRemove={() => attachments.remove(attachment.id)}
        >
          <AttachmentPreview />
          <AttachmentInfo />
          <AttachmentRemove />
        </Attachment>
      ))}
    </Attachments>
  );
}

function ContextIndicator({ model }: { model: ModelId }) {
  const maxTokens =
    MODELS.find((candidate) => candidate.id === model)?.maxTokens ?? 200_000;

  return (
    <Context
      maxTokens={maxTokens}
      modelId={model}
      usage={MOCK_USAGE}
      usedTokens={5300}
    >
      <ContextTrigger aria-label="Kontextnutzung" />
      <ContextContent>
        <ContextContentHeader />
        <ContextContentBody className="flex flex-col gap-2">
          <ContextInputUsage />
          <ContextOutputUsage />
          <ContextReasoningUsage />
          <ContextCacheUsage />
        </ContextContentBody>
        <ContextContentFooter />
      </ContextContent>
    </Context>
  );
}

function AssistantMessage({
  content,
  isStreaming = false,
}: {
  content: string;
  isStreaming?: boolean;
}) {
  return (
    <Message from="assistant">
      <MessageContent>
        <Reasoning duration={isStreaming ? undefined : 8} isStreaming={isStreaming}>
          <ReasoningTrigger
            getThinkingMessage={(streaming, duration) => (
              <span>
                {streaming
                  ? "Entwurf wird vorbereitet…"
                  : `Entwurf in ${duration ?? 8} Sekunden vorbereitet`}
              </span>
            )}
          />
          <ReasoningContent>
            Ich verknüpfe die Anforderungen aus der Stellenbeschreibung mit den
            stärksten Beispielen aus den Portfolio-Notizen. Der Text bleibt kurz
            und vermeidet nicht belegbare Behauptungen.
          </ReasoningContent>
        </Reasoning>
        <MessageResponse>{content}</MessageResponse>
        <Sources>
          <SourcesTrigger count={2} />
          <SourcesContent>
            <Source href="https://example.com/job" title="Stellenbeschreibung" />
            <Source href="https://example.com/portfolio" title="Portfolio-Notizen" />
          </SourcesContent>
        </Sources>
      </MessageContent>
      <Actions>
        <Action aria-label="Antwort kopieren">
          <CopyIcon />
        </Action>
        <Action aria-label="Antwort neu erzeugen">
          <RefreshCcwIcon />
        </Action>
        <Action aria-label="Gute Antwort">
          <ThumbsUpIcon />
        </Action>
        <Action aria-label="Schlechte Antwort">
          <ThumbsDownIcon />
        </Action>
      </Actions>
    </Message>
  );
}

function ComposerSubmit({
  draft,
  onStop,
  status,
}: {
  draft: string;
  onStop: () => void;
  status: MockChatStatus;
}) {
  const attachments = usePromptInputAttachments();
  const hasContent = Boolean(draft.trim() || attachments.files.length);

  return (
    <PromptInputSubmit
      disabled={!hasContent && status === "ready"}
      onStop={onStop}
      status={status}
    />
  );
}

function ChatPane() {
  const [draft, setDraft] = useState("");
  const [messages, setMessages] = useState(initialMessages);
  const [model, setModel] = useState<ModelId>(MODELS[0].id);
  const [status, setStatus] = useState<MockChatStatus>("ready");
  const [useWebSearch, setUseWebSearch] = useState(false);
  const pendingTimers = useRef<number[]>([]);

  function clearPendingTimers() {
    for (const timer of pendingTimers.current) window.clearTimeout(timer);
    pendingTimers.current = [];
  }

  useEffect(() => clearPendingTimers, []);

  function stopGeneration() {
    clearPendingTimers();
    setStatus("ready");
  }

  function submitMessage(message: PromptInputMessage) {
    const content = message.text.trim();
    if (!content && message.files.length === 0) return;

    setMessages((current) => [
      ...current,
      {
        content: content || "Nachricht mit Anhängen",
        from: "user",
        id: Date.now(),
      },
    ]);
    setDraft("");
    clearPendingTimers();
    setStatus("submitted");

    pendingTimers.current = [
      window.setTimeout(() => {
        setMessages((current) => [
          ...current,
          {
            content:
              "Ich aktualisiere den Entwurf im Canvas und gleiche ihn mit den bereitgestellten Unterlagen ab.",
            from: "assistant",
            id: Date.now() + 1,
          },
        ]);
        setStatus("streaming");
      }, 400),
      window.setTimeout(() => {
        pendingTimers.current = [];
        setStatus("ready");
      }, 2200),
    ];
  }

  return (
    <section
      aria-label="Agent Chat"
      className="flex size-full min-h-0 flex-col bg-background"
    >
      <header className="flex min-h-11 items-center gap-2 border-b px-4">
        <SparklesIcon className="text-muted-foreground" />
        <div className="min-w-0">
          <h1 className="truncate text-sm font-medium">Bewerbung vorbereiten</h1>
          <p className="text-xs text-muted-foreground">
            Nicht gespeichert · Prototyp
          </p>
        </div>
        <Badge className="ml-auto" variant="secondary">
          Mock
        </Badge>
      </header>

      <ScrollArea className="min-h-0 flex-1">
        <div className="mx-auto flex w-full max-w-3xl flex-col gap-7 px-5 py-8">
          {messages.map((message, index) =>
            message.from === "user" ? (
              <Message from="user" key={message.id}>
                <MessageContent>{message.content}</MessageContent>
              </Message>
            ) : (
              <AssistantMessage
                content={message.content}
                isStreaming={
                  status === "streaming" && index === messages.length - 1
                }
                key={message.id}
              />
            ),
          )}
        </div>
      </ScrollArea>

      <div className="border-t p-3">
        <PromptInput
          className="mx-auto w-full max-w-3xl"
          globalDrop
          multiple
          onSubmit={submitMessage}
        >
          <PromptInputHeader>
            <PromptAttachments />
          </PromptInputHeader>
          <PromptInputBody>
            <PromptInputTextarea
              onChange={(event) => setDraft(event.currentTarget.value)}
              placeholder="Nachricht an den Agenten…"
              value={draft}
            />
          </PromptInputBody>
          <PromptInputFooter>
            <PromptInputTools>
              <PromptInputActionMenu>
                <PromptInputActionMenuTrigger aria-label="Anhänge hinzufügen" />
                <PromptInputActionMenuContent>
                  <DropdownMenuGroup>
                    <PromptInputActionAddAttachments label="Datei hinzufügen" />
                    <PromptInputActionAddScreenshot label="Screenshot aufnehmen" />
                  </DropdownMenuGroup>
                </PromptInputActionMenuContent>
              </PromptInputActionMenu>
              <PromptInputButton
                aria-pressed={useWebSearch}
                onClick={() => setUseWebSearch((current) => !current)}
                tooltip="Websuche verwenden"
                variant={useWebSearch ? "default" : "ghost"}
              >
                <GlobeIcon data-icon="inline-start" />
                Websuche
              </PromptInputButton>
              <PromptInputSelect
                onValueChange={(value) => {
                  if (MODELS.some((candidate) => candidate.id === value)) {
                    setModel(value as ModelId);
                  }
                }}
                value={model}
              >
                <PromptInputSelectTrigger aria-label="Modell auswählen">
                  <PromptInputSelectValue />
                </PromptInputSelectTrigger>
                <PromptInputSelectContent>
                  <SelectGroup>
                    {MODELS.map((candidate) => (
                      <PromptInputSelectItem
                        key={candidate.id}
                        value={candidate.id}
                      >
                        {candidate.name}
                      </PromptInputSelectItem>
                    ))}
                  </SelectGroup>
                </PromptInputSelectContent>
              </PromptInputSelect>
              <ContextIndicator model={model} />
            </PromptInputTools>
            <ComposerSubmit
              draft={draft}
              onStop={stopGeneration}
              status={status}
            />
          </PromptInputFooter>
        </PromptInput>
      </div>
    </section>
  );
}

export function AgentChatPrototype() {
  return (
    <div className="size-full bg-muted/30 p-4">
      <div className="size-full overflow-hidden rounded-lg border bg-background shadow-sm">
        <ResizablePanelGroup orientation="horizontal">
          <ResizablePanel defaultSize={44} minSize={32}>
            <ChatPane />
          </ResizablePanel>
          <ResizableHandle withHandle />
          <ResizablePanel defaultSize={56} minSize={36}>
            <AgentCanvasEditor />
          </ResizablePanel>
        </ResizablePanelGroup>
      </div>
    </div>
  );
}
