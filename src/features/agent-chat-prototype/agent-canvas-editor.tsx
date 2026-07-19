import { useEffect, useState } from "react";
import { CodeNode } from "@lexical/code";
import { $convertFromMarkdownString, TRANSFORMERS } from "@lexical/markdown";
import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { HistoryPlugin } from "@lexical/react/LexicalHistoryPlugin";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { LexicalErrorBoundary } from "@lexical/react/LexicalErrorBoundary";
import { MarkdownShortcutPlugin } from "@lexical/react/LexicalMarkdownShortcutPlugin";
import { RichTextPlugin } from "@lexical/react/LexicalRichTextPlugin";
import { LinkNode } from "@lexical/link";
import {
  INSERT_UNORDERED_LIST_COMMAND,
  ListItemNode,
  ListNode,
  REMOVE_LIST_COMMAND,
} from "@lexical/list";
import { HeadingNode, QuoteNode } from "@lexical/rich-text";
import {
  $createParagraphNode,
  $getRoot,
  FORMAT_TEXT_COMMAND,
  type LexicalEditor,
} from "lexical";
import {
  BoldIcon,
  CodeIcon,
  ItalicIcon,
  ListIcon,
  SparklesIcon,
  TypeIcon,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";

const INITIAL_MARKDOWN = `## Anschreiben für Product Designer

Hallo Team von Example Labs,

Eure Mission, komplexe Arbeitsabläufe verständlich zu machen, passt sehr gut zu meiner Erfahrung in der Gestaltung datenintensiver Produkte.

## Warum ich gut passe

- Fünf Jahre Erfahrung mit B2B-Produkten
- Enge Zusammenarbeit mit Engineering und Research
- Prototyping von KI-gestützten Arbeitsabläufen

Ich freue mich darauf, euch mehr über meine Arbeitsweise zu erzählen.

> **Hinweis:** Der Inhalt dieses Canvas ist nur Mock-Daten und wird nicht gespeichert.
`;

const editorTheme = {
  heading: {
    h2: "mt-6 mb-2 text-base font-semibold first:mt-0",
  },
  link: "text-primary underline underline-offset-4",
  list: {
    listitem: "ml-5 list-disc text-sm leading-7 text-muted-foreground",
    nested: {
      listitem: "list-none",
    },
    ol: "my-2",
    ul: "my-2",
  },
  paragraph: "mb-3 text-sm leading-7 text-muted-foreground",
  quote:
    "my-4 border-l-2 border-border pl-4 text-sm italic leading-7 text-muted-foreground",
  text: {
    bold: "font-semibold text-foreground",
    code: "rounded bg-muted px-1 py-0.5 font-mono text-xs",
    italic: "italic",
  },
};

function formatText(editor: LexicalEditor, format: "bold" | "code" | "italic") {
  editor.dispatchCommand(FORMAT_TEXT_COMMAND, format);
}

function CanvasToolbar() {
  const [editor] = useLexicalComposerContext();
  const [wordCount, setWordCount] = useState(0);

  useEffect(
    () =>
      editor.registerUpdateListener(({ editorState }) => {
        editorState.read(() => {
          const text = $getRoot().getTextContent().trim();
          setWordCount(text ? text.split(/\s+/u).length : 0);
        });
      }),
    [editor],
  );

  return (
    <div className="flex min-h-11 items-center gap-1 border-b pr-12 pl-3">
      <Button
        aria-label="Fett"
        onClick={() => formatText(editor, "bold")}
        size="icon-sm"
        type="button"
        variant="ghost"
      >
        <BoldIcon />
      </Button>
      <Button
        aria-label="Kursiv"
        onClick={() => formatText(editor, "italic")}
        size="icon-sm"
        type="button"
        variant="ghost"
      >
        <ItalicIcon />
      </Button>
      <Button
        aria-label="Code"
        onClick={() => formatText(editor, "code")}
        size="icon-sm"
        type="button"
        variant="ghost"
      >
        <CodeIcon />
      </Button>
      <Button
        aria-label="Aufzählung"
        onClick={() => editor.dispatchCommand(INSERT_UNORDERED_LIST_COMMAND, undefined)}
        size="icon-sm"
        type="button"
        variant="ghost"
      >
        <ListIcon />
      </Button>
      <Button
        aria-label="Aufzählung entfernen"
        onClick={() => editor.dispatchCommand(REMOVE_LIST_COMMAND, undefined)}
        size="icon-sm"
        type="button"
        variant="ghost"
      >
        <TypeIcon />
      </Button>
      <Separator className="mx-2 h-5" orientation="vertical" />
      <Button size="sm" type="button" variant="ghost">
        <SparklesIcon data-icon="inline-start" />
        AI Edit
      </Button>
      <span className="ml-auto text-xs tabular-nums text-muted-foreground">
        {wordCount} Wörter
      </span>
    </div>
  );
}

const initialConfig = {
  editorState: () => {
    $convertFromMarkdownString(INITIAL_MARKDOWN, TRANSFORMERS);
    if ($getRoot().isEmpty()) {
      $getRoot().append($createParagraphNode());
    }
  },
  namespace: "agent-canvas-prototype",
  nodes: [CodeNode, HeadingNode, LinkNode, ListItemNode, ListNode, QuoteNode],
  onError(error: Error) {
    throw error;
  },
  theme: editorTheme,
};

export function AgentCanvasEditor() {
  return (
    <LexicalComposer initialConfig={initialConfig}>
      <div className="flex size-full min-h-0 flex-col bg-background">
        <CanvasToolbar />
        <div className="relative min-h-0 flex-1 overflow-y-auto">
          <RichTextPlugin
            contentEditable={
              <ContentEditable
                aria-label="Bewerbungs-Canvas"
                className="mx-auto min-h-full max-w-3xl px-8 py-8 outline-none"
              />
            }
            ErrorBoundary={LexicalErrorBoundary}
            placeholder={
              <p className="pointer-events-none absolute top-8 left-1/2 w-full max-w-3xl -translate-x-1/2 px-8 text-sm text-muted-foreground">
                Canvas bearbeiten…
              </p>
            }
          />
          <HistoryPlugin />
          <MarkdownShortcutPlugin transformers={TRANSFORMERS} />
        </div>
      </div>
    </LexicalComposer>
  );
}
