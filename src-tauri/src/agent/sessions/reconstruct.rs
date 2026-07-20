use super::wire::{AssistantData, Document, Entry, EntryKind};
use super::{
    CompactionSnapshot, ContextEntry, ContextRole, ContinuationAssistantBlock, ContinuationBlock,
    RecoveryNotice, SessionAccess, SessionError, SessionErrorCode, SessionId, SessionSnapshot,
    VisibleBlock, VisibleHistoryEntry, VisibleTurn,
};
use crate::agent::models::{ModelId, ProviderId, ReasoningLevel};
use std::collections::HashMap;

pub(super) fn reconstruct(
    doc: &Document,
    locked: bool,
    notice: Option<RecoveryNotice>,
) -> Result<(SessionSnapshot, Vec<ContinuationBlock>, Option<String>), SessionError> {
    let id = SessionId(doc.header.id.to_string());
    if doc.header.version != 3 {
        return Ok((
            SessionSnapshot::empty(
                id,
                doc.header.timestamp.clone(),
                SessionAccess::ReadOnlyUnsupported,
            ),
            Vec::new(),
            None,
        ));
    }
    let path = active_path(&doc.entries)?;
    validate_compactions(&path)?;
    let access = if doc.unsupported {
        SessionAccess::ReadOnlyUnsupported
    } else if locked {
        SessionAccess::ReadOnlyLocked
    } else {
        SessionAccess::Writable
    };
    let mut turns = Vec::new();
    let mut visible_history = Vec::new();
    let mut first_user = None;
    let mut provider = None;
    let mut model = None;
    let mut reasoning = ReasoningLevel::Off;
    let mut explicit_name: Option<String> = None;
    let mut compactions = Vec::new();
    let mut index = 0;
    while index < path.len() {
        match &path[index].kind {
            EntryKind::User(text) => {
                if first_user.is_none() {
                    first_user = Some(text.clone());
                }
                if let Some(next) = path.get(index + 1) {
                    if let EntryKind::Assistant {
                        blocks,
                        provider: p,
                        model: m,
                        ..
                    } = &next.kind
                    {
                        let turn = VisibleTurn {
                            user: text.clone(),
                            assistant: blocks
                                .iter()
                                .map(|block| match block {
                                    AssistantData::Text { text, .. } => {
                                        VisibleBlock::Text(text.clone())
                                    }
                                    AssistantData::Thinking { redacted: true, .. } => {
                                        VisibleBlock::RedactedThinking
                                    }
                                    AssistantData::Thinking { thinking, .. } => {
                                        VisibleBlock::Thinking(thinking.clone())
                                    }
                                })
                                .collect(),
                        };
                        visible_history.push(VisibleHistoryEntry::Turn(turn.clone()));
                        turns.push(turn);
                        provider = ProviderId::new(p.clone()).ok();
                        model = ModelId::new(m.clone()).ok();
                        index += 2;
                        continue;
                    }
                }
            }
            EntryKind::Model {
                provider: p,
                model: m,
            } => {
                provider = ProviderId::new(p.clone()).ok();
                model = ModelId::new(m.clone()).ok();
            }
            EntryKind::Reasoning(level) => reasoning = parse_reasoning(level),
            EntryKind::Name(Some(name)) => explicit_name = Some(name.clone()),
            EntryKind::Name(None) => {}
            EntryKind::Compaction {
                summary,
                first_kept,
                tokens,
                reason,
            } => {
                let compaction = CompactionSnapshot {
                    summary: summary.clone(),
                    first_kept_entry_id: first_kept.clone(),
                    tokens_before: *tokens,
                    reason: reason.clone(),
                    path_index: index,
                };
                visible_history.push(VisibleHistoryEntry::Compaction(compaction.clone()));
                compactions.push(compaction);
            }
            EntryKind::Assistant { .. } | EntryKind::Unsupported | EntryKind::UnsupportedUser => {}
        }
        index += 1;
    }
    // Pairing is an integrity property of supported message entries, not of the
    // document's aggregate compatibility classification. An unrelated recognized
    // extension must never suppress validation of an otherwise malformed pair.
    validate_pairs(&path)?;
    let display_name = match explicit_name {
        Some(ref name) if !name.is_empty() => name.clone(),
        Some(_) => first_user
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Untitled session".into()),
        None => first_user
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Untitled session".into()),
    };
    let modified = path
        .last()
        .map(|e| e.timestamp.clone())
        .unwrap_or_else(|| doc.header.timestamp.clone());
    let continuation = continuation(&path);
    let context_entries = context_metadata(&path);
    let compaction_entries = raw_context_metadata(&path);
    let leaf = path.last().map(|e| e.id.clone());
    Ok((
        SessionSnapshot {
            id,
            access,
            display_name,
            created_at: doc.header.timestamp.clone(),
            modified_at: modified,
            turns,
            selected_provider: provider,
            selected_model: model,
            reasoning_level: reasoning,
            compactions,
            recovery_notices: notice.into_iter().collect(),
            visible_history,
            context_entries,
            compaction_entries,
        },
        continuation,
        leaf,
    ))
}

fn active_path(entries: &[Entry]) -> Result<Vec<&Entry>, SessionError> {
    let Some(mut current) = entries.last() else {
        return Ok(Vec::new());
    };
    let by_id: HashMap<&str, &Entry> = entries.iter().map(|e| (e.id.as_str(), e)).collect();
    let mut reversed = vec![current];
    while let Some(parent) = current.parent.as_deref() {
        current = by_id
            .get(parent)
            .copied()
            .ok_or_else(|| SessionError::new(SessionErrorCode::Damaged))?;
        reversed.push(current);
    }
    reversed.reverse();
    Ok(reversed)
}
fn validate_pairs(path: &[&Entry]) -> Result<(), SessionError> {
    let mut pending: Option<&Entry> = None;
    for entry in path {
        match entry.kind {
            EntryKind::User(_) | EntryKind::UnsupportedUser => {
                if pending.is_some() {
                    return Err(SessionError::diagnostic(
                        SessionErrorCode::Damaged,
                        Some(entry.line),
                        Some("message"),
                        Some("message"),
                    ));
                }
                pending = Some(entry);
            }
            EntryKind::Assistant { .. } => {
                let Some(user) = pending.take() else {
                    return Err(SessionError::diagnostic(
                        SessionErrorCode::Damaged,
                        Some(entry.line),
                        Some("message"),
                        Some("message"),
                    ));
                };
                if entry.parent.as_deref() != Some(user.id.as_str()) {
                    return Err(SessionError::diagnostic(
                        SessionErrorCode::Damaged,
                        Some(entry.line),
                        Some("message"),
                        Some("parentId"),
                    ));
                }
            }
            _ if pending.is_some() => {
                return Err(SessionError::diagnostic(
                    SessionErrorCode::Damaged,
                    Some(entry.line),
                    Some("message"),
                    Some("message"),
                ));
            }
            _ => {}
        }
    }
    if pending.is_some() {
        return Err(SessionError::new(SessionErrorCode::IncompleteFinalSuffix));
    }
    Ok(())
}
fn validate_compactions(path: &[&Entry]) -> Result<(), SessionError> {
    for (i, e) in path.iter().enumerate() {
        if let EntryKind::Compaction { first_kept, .. } = &e.kind {
            let valid = path[..i].iter().any(|prior| {
                prior.id == *first_kept
                    && matches!(prior.kind, EntryKind::User(_) | EntryKind::Assistant { .. })
            });
            if !valid {
                return Err(SessionError::diagnostic(
                    SessionErrorCode::Damaged,
                    Some(e.line),
                    Some("compaction"),
                    Some("firstKeptEntryId"),
                ));
            }
        }
    }
    Ok(())
}
fn continuation(path: &[&Entry]) -> Vec<ContinuationBlock> {
    let newest = path.iter().enumerate().rev().find_map(|(i, e)| {
        if e.supported {
            if let EntryKind::Compaction {
                summary,
                first_kept,
                ..
            } = &e.kind
            {
                return Some((i, summary, first_kept));
            }
        }
        None
    });
    let Some((compaction_index, summary, first)) = newest else {
        return context_entries(path);
    };
    let mut out=vec![ContinuationBlock::User(format!("The conversation history before this point was compacted into the following summary:\n\n<summary>\n{summary}\n</summary>"))];
    let first_index = path
        .iter()
        .position(|e| e.id == *first)
        .unwrap_or(compaction_index);
    out.extend(context_entries(&path[first_index..compaction_index]));
    out.extend(context_entries(&path[compaction_index + 1..]));
    out
}
fn context_metadata(path: &[&Entry]) -> Vec<ContextEntry> {
    let newest = path.iter().enumerate().rev().find_map(|(index, entry)| {
        if let EntryKind::Compaction {
            summary,
            first_kept,
            ..
        } = &entry.kind
        {
            Some((index, summary, first_kept))
        } else {
            None
        }
    });
    let mut selected: Vec<(usize, &Entry)> = Vec::new();
    let mut output = Vec::new();
    if let Some((compaction_index, summary, first_kept)) = newest {
        output.push(ContextEntry {
            id: path[compaction_index].id.clone(),
            role: ContextRole::User,
            text: format!("The conversation history before this point was compacted into the following summary:\n\n<summary>\n{summary}\n</summary>"),
            authoritative_tokens: None,
            path_index: compaction_index,
        });
        if let Some(first_index) = path.iter().position(|entry| entry.id == *first_kept) {
            selected.extend(
                path[first_index..compaction_index]
                    .iter()
                    .map(|entry| (0, *entry)),
            );
        }
        selected.extend(
            path[compaction_index + 1..]
                .iter()
                .enumerate()
                .map(|(offset, entry)| (compaction_index + 1 + offset, *entry)),
        );
    } else {
        selected.extend(
            path.iter()
                .enumerate()
                .map(|(index, entry)| (index, *entry)),
        );
    }
    for (index, entry) in selected {
        match &entry.kind {
            EntryKind::User(text) => output.push(ContextEntry {
                id: entry.id.clone(),
                role: ContextRole::User,
                text: text.clone(),
                authoritative_tokens: None,
                path_index: index,
            }),
            EntryKind::Assistant {
                blocks,
                authoritative_tokens,
                ..
            } => output.push(ContextEntry {
                id: entry.id.clone(),
                role: ContextRole::Assistant,
                text: blocks
                    .iter()
                    .filter_map(|block| match block {
                        AssistantData::Text { text, .. } => Some(text.as_str()),
                        AssistantData::Thinking {
                            thinking,
                            redacted: false,
                            ..
                        } => Some(thinking.as_str()),
                        AssistantData::Thinking { redacted: true, .. } => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                authoritative_tokens: newest
                    .as_ref()
                    .and_then(|(compaction_index, _, _)| {
                        (index > *compaction_index)
                            .then_some(*authoritative_tokens)
                            .flatten()
                    })
                    .or_else(|| newest.is_none().then_some(*authoritative_tokens).flatten()),
                path_index: index,
            }),
            _ => {}
        }
    }
    output
}

fn raw_context_metadata(path: &[&Entry]) -> Vec<ContextEntry> {
    let mut output = Vec::new();
    for (path_index, entry) in path.iter().enumerate() {
        match &entry.kind {
            EntryKind::User(text) => output.push(ContextEntry {
                id: entry.id.clone(),
                role: ContextRole::User,
                text: text.clone(),
                authoritative_tokens: None,
                path_index,
            }),
            EntryKind::Assistant { blocks, .. } => output.push(ContextEntry {
                id: entry.id.clone(),
                role: ContextRole::Assistant,
                text: blocks
                    .iter()
                    .filter_map(|block| match block {
                        AssistantData::Text { text, .. } => Some(text.as_str()),
                        AssistantData::Thinking {
                            thinking,
                            redacted: false,
                            ..
                        } => Some(thinking.as_str()),
                        AssistantData::Thinking { redacted: true, .. } => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                authoritative_tokens: None,
                path_index,
            }),
            _ => {}
        }
    }
    output
}

fn context_entries(path: &[&Entry]) -> Vec<ContinuationBlock> {
    let mut out = Vec::new();
    let mut index = 0;
    // A compaction may deliberately retain an Assistant suffix from an oversized
    // turn. This is the only supported unpaired context entry; durable pairing is
    // still validated on the full active path.
    if let Some(first) = path.first().filter(|entry| entry.supported) {
        if matches!(first.kind, EntryKind::Assistant { .. }) {
            if let Some(block) = continuation_entry(first) {
                out.push(block);
            }
            index = 1;
        }
    }
    while index + 1 < path.len() {
        let user = path[index];
        let assistant = path[index + 1];
        if user.supported
            && assistant.supported
            && matches!(user.kind, EntryKind::User(_))
            && matches!(assistant.kind, EntryKind::Assistant { .. })
        {
            out.push(continuation_entry(user).expect("supported user"));
            out.push(continuation_entry(assistant).expect("supported assistant"));
            index += 2;
        } else {
            index += 1;
        }
    }
    out
}

fn continuation_entry(entry: &Entry) -> Option<ContinuationBlock> {
    match &entry.kind {
        EntryKind::User(text) => Some(ContinuationBlock::User(text.clone())),
        EntryKind::Assistant {
            blocks,
            provider,
            model,
            response_id,
            ..
        } => Some(ContinuationBlock::Assistant {
            blocks: blocks
                .iter()
                .map(|block| match block {
                    AssistantData::Text { text, signature } => ContinuationAssistantBlock::Text {
                        text: text.clone(),
                        signature: signature.clone(),
                    },
                    AssistantData::Thinking {
                        thinking,
                        signature,
                        redacted,
                    } => ContinuationAssistantBlock::Thinking {
                        thinking: thinking.clone(),
                        signature: signature.clone(),
                        redacted: *redacted,
                    },
                })
                .collect(),
            provider: provider.clone(),
            model: model.clone(),
            response_id: response_id.clone(),
        }),
        _ => None,
    }
}
fn parse_reasoning(s: &str) -> ReasoningLevel {
    match s {
        "minimal" => ReasoningLevel::Minimal,
        "low" => ReasoningLevel::Low,
        "medium" => ReasoningLevel::Medium,
        "high" => ReasoningLevel::High,
        "xhigh" => ReasoningLevel::XHigh,
        "max" => ReasoningLevel::Max,
        _ => ReasoningLevel::Off,
    }
}
