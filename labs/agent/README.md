# Agent Chat UI prototype (#223)

**Question:** Should persistent Agent Chats primarily use a dedicated page, and how should an editable Agent Canvas fit into that experience?

## Run

```bash
npm run dev
```

Open <http://localhost:1420/labs/agent>.

## Verdict

The resizable split view is the preferred direction: the Agent Chat remains visible while the user reviews or edits the Canvas. The earlier focused-chat/Sheet alternative was removed after comparison.

## Scope

- Mock data and in-memory interactions only.
- No Agent API, persistence, cancellation, or recovery behavior yet.
- Uses AI Elements for reasoning, prompt input, attachments, model selection, and the context-window indicator.
- The Prompt Input supports local file selection, drag-and-drop, screenshot capture, web-search state, and a simulated streaming transition.
- The full `@shadcn-editor/editor-x` registry item was incompatible with this project's Base UI components. The prototype therefore uses a deliberately small Lexical editor with Markdown shortcuts while preserving the intended editable-canvas interaction.

The reference HTML and screenshot remain in `labs/agent/inspo/`.
