use super::wire::{AssistantData, Document, Entry, EntryKind};
use super::{
    CompactionSnapshot, ContinuationAssistantBlock, ContinuationBlock, RecoveryNotice,
    SessionAccess, SessionError, SessionErrorCode, SessionId, SessionSnapshot, VisibleBlock,
    VisibleTurn,
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
                        turns.push(VisibleTurn {
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
                        });
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
            } => compactions.push(CompactionSnapshot {
                summary: summary.clone(),
                first_kept_entry_id: first_kept.clone(),
                tokens_before: *tokens,
                reason: reason.clone(),
            }),
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
fn context_entries(path: &[&Entry]) -> Vec<ContinuationBlock> {
    let mut out = Vec::new();
    let mut index = 0;
    while index + 1 < path.len() {
        let user = path[index];
        let assistant = path[index + 1];
        if user.supported && assistant.supported {
            if let (
                EntryKind::User(text),
                EntryKind::Assistant {
                    blocks,
                    response_id,
                    ..
                },
            ) = (&user.kind, &assistant.kind)
            {
                out.push(ContinuationBlock::User(text.clone()));
                out.push(ContinuationBlock::Assistant {
                    blocks: blocks
                        .iter()
                        .map(|block| match block {
                            AssistantData::Text { text, signature } => {
                                ContinuationAssistantBlock::Text {
                                    text: text.clone(),
                                    signature: signature.clone(),
                                }
                            }
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
                    response_id: response_id.clone(),
                });
                index += 2;
                continue;
            }
        }
        index += 1;
    }
    out
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
