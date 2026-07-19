# Research: Pi persistent sessions/chats at `dcfe36c79702ec240b146c45f167ab75ecddd205`

## Summary

Pi persists an append-only, versioned JSONL event tree per session: the file header supplies stable session identity and ancestry, while entry `id`/`parentId` links define the active branch; reopening reconstructs messages plus the latest model/reasoning settings from the final persisted leaf. Job Radar should reuse the event-log and deterministic reconstruction ideas, but introduce a product-level **Agent Chat** above the existing ephemeral `AgentConversation`, use app-data storage and stronger commit/recovery guarantees, and initially omit Pi’s tools, skills/extensions, branch UI, and compaction.

## Findings

1. **Identity and files.** Pi’s header is `{type:"session", version, id, timestamp, cwd, parentSession?}`; v3 is current, v1/v2 are migrated on open. Default files are `~/.pi/agent/sessions/--<encoded-cwd>--/<timestamp>_<session-id>.jsonl`; a session ID is UUIDv7 unless explicitly supplied, while entry IDs are collision-checked short UUID fragments. The header is metadata, not a tree node. [types/IDs](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/src/core/session-manager.ts#L18-L143) [path and file creation](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/src/core/session-manager.ts#L365-L372) [new session](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/src/core/session-manager.ts#L718-L742)

2. **Append/resume semantics.** Every non-header entry has `type`, `id`, `parentId`, and ISO timestamp. Appending adds a child of the in-memory leaf and advances it; reopening parses the file, migrates if needed, indexes entries, and treats the last parsed entry as the leaf. `continueRecent` selects the newest file by filesystem mtime (optionally CWD-filtered), while an explicitly opened missing/empty path initializes a session and a non-empty invalid file fails rather than being overwritten. [append/index/open](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/src/core/session-manager.ts#L667-L794) [recent selection](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/src/core/session-manager.ts#L467-L491) [constructors](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/src/core/session-manager.ts#L1117-L1166)

3. **Branching and forking are distinct.** `branch(id)` only moves the leaf; the next append creates an alternate child in the same file without rewriting history. `createBranchedSession(leaf)` extracts one root-to-leaf path into a new session ID/file and records `parentSession`; `forkFrom` copies all source entries to a new file/CWD. Pi’s `/tree` is same-file navigation, `/fork` is a new file from an earlier prompt, and `/clone` duplicates the active path. [tree operations](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/src/core/session-manager.ts#L937-L1110) [cross-project fork](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/src/core/session-manager.ts#L1175-L1238) [official sessions doc](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/docs/sessions.md#L61-L117)

4. **Messages and metadata.** Entry kinds include messages, model/reasoning changes, compactions, branch summaries, labels, session names, and extension state/messages. Base messages support user text/images; assistants persist text, thinking, tool calls, provider/API/model, usage/cost, stop reason/error, and timestamp; tool results are separate messages. `session_info` names are append-only metadata and the latest value wins. [entry union](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/src/core/session-manager.ts#L37-L120) [official format](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/docs/session-format.md#L35-L214)

5. **Model/reasoning persistence is event-based.** Model and thinking changes are explicit entries. Context reconstruction walks the full active path; latest change wins, but an assistant message also establishes its actual provider/model. Default thinking is `off`; model is initially absent. Thus historical assistant turns retain their effective model, while the path-level latest setting controls the next turn. [setting reconstruction](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/src/core/session-manager.ts#L276-L293) [append change events](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/src/core/session-manager.ts#L831-L859)

6. **Compaction/context reconstruction.** Pi walks leaf-to-root, then, if a compaction exists, replaces older context with the latest summary while retaining entries from `firstKeptEntryId` and entries after compaction; branch summaries become context messages, plain extension state does not. Auto-compaction triggers above `contextWindow - reserveTokens`, keeps a recent budget, appends rather than deletes, and supports iterative summaries. [context algorithm](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/src/core/session-manager.ts#L295-L363) [official compaction design](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/docs/compaction.md#L20-L113)

7. **Failure and atomicity are weak reference behavior, not a contract to copy.** Pi delays creating a normal session file until an assistant message exists, then writes all accumulated entries; later entries use direct synchronous append. This avoids saving user-only starts, but a crash can leave a truncated final line. Parsing silently skips malformed lines; migrations and some branch creation rewrite directly with `"w"`, with no temp-file/rename, fsync, file lock, checksum, or transaction boundary. A partially persisted turn can therefore survive (e.g. user without assistant after later-session append), unlike Job Radar’s current transactional turn contract. [persistence](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/src/core/session-manager.ts#L746-L812) [lenient parsing/loading](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/src/core/session-manager.ts#L162-L180) [file rewrite](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/src/core/session-manager.ts#L744-L756)

8. **Selection/UI.** `/resume`/`pi -r` offers current-folder or all-session scope, search, threaded/recent/relevance sorting, named-only filtering, rename, path toggle, and confirmed deletion; rows use name or first user message plus message count and relative activity age. Parent-file links produce a threaded session-family view. The active file cannot be deleted. [official UX](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/docs/sessions.md#L29-L59) [selector tree/list behavior](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/src/modes/interactive/components/session-selector.ts#L134-L376)

## Proposed Job Radar destination

- Add **Agent Chat** as the persisted, user-visible aggregate; retain **Agent Conversation** as the ephemeral turn engine exactly as defined in `CONTEXT.md`, `docs/adr/0011-minimal-agent-conversation-contract.md`, and `src-tauri/src/agent/conversation.rs`. Do not rename or inflate `AgentConversation` into persistence.
- Explicitly build on issue 208: store chats under the same app-data `agents/` root (proposed `agents/chats/<chat-id>.jsonl` plus a rebuildable index only if needed); each chat stores provider/model IDs and effective Reasoning Level, while credentials and provider transport configuration remain solely in `agents/auth.json` and `agents/models.json`. New chats preselect issue 208’s last-used model; resumed chats resolve their persisted IDs against the current immutable registry snapshot and show a recoverable “model unavailable” state rather than silently substituting.
- Start with schema-versioned header plus append-only entries: `user_assistant_turn` (one atomic logical record containing the completed pair), `model_change`, `reasoning_change`, and `chat_info` (name). Persist assistant content blocks, effective provider/model, usage, finish reason, and opaque replay metadata only if the provider adapter requires it for correct replay; encrypt or otherwise protect account-linked opaque data consistently with `docs/security/agent-credential-containment.md`.
- Preserve current semantics: `Completed` alone commits; failed, aborted, malformed, or dropped streams commit neither (`docs/agent-conversation-core.md`; `src-tauri/src/agent/conversation.rs`). Publish a completed turn with temp-write + durable atomic rename or an equivalently tested framed append/recovery protocol—do **not** copy Pi’s best-effort line append.
- On resume, validate the complete header/entry schema and reconstruct only committed turns plus latest model/reasoning selections. Unknown entry versions, broken parent links, duplicate IDs, or non-final corruption should quarantine/read-only the chat with redacted diagnostics; only a demonstrably incomplete final frame may be recoverable.
- Initial UI: Chats list (name/first prompt, updated time, completed-turn count, selected model/provider status), create/open/rename/delete-with-confirmation, and clear unavailable-model remediation. Keep provider auth/settings in issue 208’s Settings surface.
- **Do not transfer initially:** CWD-derived storage, cost accounting, tool/tool-result/bash entries, tools/skills/extensions/custom entries, file tracking, branch/fork/clone/tree navigation, labels, HTML share/export, or automatic/manual compaction. These are outside issue 208 and ADR 0011; tools/skills are explicitly excluded by this task. Reserve entry-type/version extensibility rather than implementing them speculatively.

## Decisions still requiring the user

1. Whether opaque provider replay metadata may be persisted at all; if yes, required at-rest protection, retention, export, and deletion behavior.
2. Whether first release needs cross-restart continuation only, or also same-chat branching/forking; recommendation: linear chats first, preserving a future `parentId` field only if its semantics are specified now.
3. Chat retention/deletion policy (trash vs permanent), export/privacy expectations, and whether chats can contain images.
4. Recovery UX and durability target (atomic whole-file rewrite is simplest but scales poorly; framed append plus checksum/commit marker is more complex).
5. Whether compaction is intentionally deferred until real context-window pressure appears, and what user disclosure/consent a future lossy summary requires.

## Sources

- Kept: [Pi session manager](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/src/core/session-manager.ts) — authoritative storage, traversal, append, fork, migration, and reconstruction code.
- Kept: [Pi session format](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/docs/session-format.md), [sessions](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/docs/sessions.md), and [compaction](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/docs/compaction.md) — official repository documentation at the pin.
- Kept: [Pi session selector](https://github.com/earendil-works/pi/blob/dcfe36c79702ec240b146c45f167ab75ecddd205/packages/coding-agent/src/modes/interactive/components/session-selector.ts) — authoritative picker behavior.
- Kept (local): `CONTEXT.md`, `docs/adr/0011-minimal-agent-conversation-contract.md`, `docs/agent-conversation-core.md`, `src-tauri/src/agent/conversation.rs`, `docs/research/pi-auth-model-registry-behavior.md`, `docs/security/agent-credential-containment.md` — Job Radar vocabulary, lifecycle, issue-208 destination, and security constraints.
- Dropped: moving branches, third-party commentary, mirrors, and later Pi commits — not authoritative for the requested pin.

## Gaps

Issue 208 is private/inaccessible to web tooling; its destination and decisions were supplied by the supervising session and cross-checked against local `docs/research/pi-auth-model-registry-behavior.md`, but comments/history could not be independently cited. No Job Radar persistent-chat schema or UI contract exists yet; the destination above is therefore a proposal, not an accepted architecture.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Only the configured research artifact was written; no project/source files were modified, and tools/skills were explicitly excluded from the proposed destination."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "The brief cites immutable commit-and-line GitHub permalinks for Pi claims and names the local Job Radar contracts used for the recommendation."
    }
  ],
  "changedFiles": [
    ".pi-subagents/artifacts/outputs/91f5c432-d464-4ace-86f4-d77d630f1e18/docs/research/pi-persistent-agent-chat-behavior.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [],
  "validationOutput": [
    "Primary Pi source and official repository docs were inspected at commit dcfe36c79702ec240b146c45f167ab75ecddd205.",
    "Local Agent Conversation/domain documents and implementation were inspected read-only.",
    "Artifact write completed at the exact configured output path."
  ],
  "residualRisks": [
    "Issue 208 body/comments were not directly accessible in this worker; supervisor-provided assumptions were cross-checked against the existing local issue-208 research document.",
    "No shell/git capability was available to independently inspect repository status or execute Markdown/link checks.",
    "Persistent replay-metadata security, durability protocol, branching scope, retention, and future compaction require user decisions."
  ],
  "noStagedFiles": true,
  "diffSummary": "Added one research-only Markdown artifact describing pinned Pi session behavior and a scoped Job Radar Agent Chat destination.",
  "reviewFindings": [
    "no blockers found in the research artifact; review gate remains required"
  ],
  "manualNotes": "No tests were appropriate for a research-only artifact. noStagedFiles means this worker performed no staging operation; git status could not be queried with the available tools."
}
```
