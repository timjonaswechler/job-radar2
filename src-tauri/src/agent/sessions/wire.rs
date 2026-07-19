// Session format semantics are derived from Pi at commit
// dcfe36c79702ec240b146c45f167ab75ecddd205 (MIT; see THIRD_PARTY_NOTICES.md).
use super::{
    AssistantBlockKind, AssistantUsage, SessionError, SessionErrorCode, StopReason, UsageCost,
};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

pub(super) const MAX_LINE: usize = 16 * 1024 * 1024;
pub(super) const MAX_BATCH: usize = 32 * 1024 * 1024;

#[derive(Clone)]
pub(super) struct Header {
    pub id: Uuid,
    pub timestamp: String,
    pub version: u64,
}
#[derive(Clone)]
pub(super) struct Entry {
    pub kind: EntryKind,
    pub id: String,
    pub parent: Option<String>,
    pub timestamp: String,
    pub line: u32,
    /// Whether this individual entry belongs to Job Radar's writable/context-safe subset.
    /// `Document::unsupported` is only the aggregate read-only classification.
    pub supported: bool,
}
#[derive(Clone)]
pub(super) enum EntryKind {
    User(String),
    Assistant {
        blocks: Vec<AssistantData>,
        provider: String,
        model: String,
        response_id: Option<String>,
    },
    Model {
        provider: String,
        model: String,
    },
    Reasoning(String),
    Name(Option<String>),
    Compaction {
        summary: String,
        first_kept: String,
        tokens: u64,
        reason: Option<String>,
    },
    Unsupported,
    UnsupportedUser,
}
#[derive(Clone)]
pub(super) enum AssistantData {
    Text {
        text: String,
        signature: Option<String>,
    },
    Thinking {
        thinking: String,
        signature: Option<String>,
        redacted: bool,
    },
}
#[derive(Clone)]
pub(super) struct Document {
    pub header: Header,
    pub entries: Vec<Entry>,
    pub unsupported: bool,
}

fn err(
    code: SessionErrorCode,
    line: Option<u32>,
    ty: Option<&str>,
    field: Option<&str>,
) -> SessionError {
    SessionError::diagnostic(code, line, ty, field)
}
fn object<'a>(
    v: &'a Value,
    line: u32,
    ty: Option<&str>,
) -> Result<&'a Map<String, Value>, SessionError> {
    v.as_object()
        .ok_or_else(|| err(SessionErrorCode::Damaged, Some(line), ty, None))
}
fn string(
    m: &Map<String, Value>,
    k: &'static str,
    line: u32,
    ty: &str,
) -> Result<String, SessionError> {
    m.get(k)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| err(SessionErrorCode::Damaged, Some(line), Some(ty), Some(k)))
}
fn timestamp(s: &str, line: u32, ty: &str) -> Result<(), SessionError> {
    let t = OffsetDateTime::parse(s, &Rfc3339).map_err(|_| {
        err(
            SessionErrorCode::Damaged,
            Some(line),
            Some(ty),
            Some("timestamp"),
        )
    })?;
    if t.offset() != time::UtcOffset::UTC {
        return Err(err(
            SessionErrorCode::Damaged,
            Some(line),
            Some(ty),
            Some("timestamp"),
        ));
    }
    Ok(())
}
fn exact(m: &Map<String, Value>, allowed: &[&str]) -> bool {
    m.keys().all(|k| allowed.contains(&k.as_str()))
}
fn valid_entry_id(s: &str) -> bool {
    (s.len() == 8
        && s.bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
        || (s.len() == 36 && Uuid::parse_str(s).is_ok() && s == s.to_ascii_lowercase())
}

fn valid_text_or_image_block(value: &Value) -> bool {
    let Some(block) = value.as_object() else {
        return false;
    };
    match block.get("type").and_then(Value::as_str) {
        Some("text") => {
            block.get("text").is_some_and(Value::is_string)
                && block.get("textSignature").is_none_or(Value::is_string)
        }
        Some("image") => {
            block.get("data").is_some_and(Value::is_string)
                && block.get("mimeType").is_some_and(Value::is_string)
        }
        _ => false,
    }
}

fn valid_string_or_content(value: &Value) -> bool {
    value.is_string()
        || value
            .as_array()
            .is_some_and(|blocks| blocks.iter().all(valid_text_or_image_block))
}

fn valid_tool_call_block(block: &Map<String, Value>) -> bool {
    block.get("id").is_some_and(Value::is_string)
        && block.get("name").is_some_and(Value::is_string)
        && block.get("arguments").is_some_and(Value::is_object)
        && block.get("thoughtSignature").is_none_or(Value::is_string)
}

fn validate_pinned_unsupported(
    m: &Map<String, Value>,
    line: u32,
    ty: &str,
) -> Result<(), SessionError> {
    let valid = match ty {
        "branch_summary" => {
            m.get("fromId").is_some_and(Value::is_string)
                && m.get("summary").is_some_and(Value::is_string)
                && m.get("fromHook").is_none_or(Value::is_boolean)
        }
        "label" => {
            m.get("targetId").is_some_and(Value::is_string)
                && m.get("label").is_none_or(Value::is_string)
        }
        "custom" => m.get("customType").is_some_and(Value::is_string),
        "custom_message" => {
            m.get("customType").is_some_and(Value::is_string)
                && m.get("content").is_some_and(valid_string_or_content)
                && m.get("display").is_some_and(Value::is_boolean)
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(err(SessionErrorCode::Damaged, Some(line), Some(ty), None))
    }
}

fn validate_unsupported_message(
    message: &Map<String, Value>,
    role: &str,
    line: u32,
) -> Result<(), SessionError> {
    let valid = match role {
        "toolResult" => {
            message.get("toolCallId").is_some_and(Value::is_string)
                && message.get("toolName").is_some_and(Value::is_string)
                && message
                    .get("content")
                    .and_then(Value::as_array)
                    .is_some_and(|blocks| blocks.iter().all(valid_text_or_image_block))
                && message.get("addedToolNames").is_none_or(|value| {
                    value
                        .as_array()
                        .is_some_and(|names| names.iter().all(Value::is_string))
                })
                && message.get("isError").is_some_and(Value::is_boolean)
                && message.get("timestamp").and_then(Value::as_i64).is_some()
        }
        "bashExecution" => {
            message.get("command").is_some_and(Value::is_string)
                && message.get("output").is_some_and(Value::is_string)
                && message
                    .get("exitCode")
                    .is_none_or(|value| value.as_i64().is_some())
                && message.get("cancelled").is_some_and(Value::is_boolean)
                && message.get("truncated").is_some_and(Value::is_boolean)
                && message.get("fullOutputPath").is_none_or(Value::is_string)
                && message
                    .get("excludeFromContext")
                    .is_none_or(Value::is_boolean)
                && message.get("timestamp").and_then(Value::as_i64).is_some()
        }
        "custom" => {
            message.get("customType").is_some_and(Value::is_string)
                && message.get("content").is_some_and(valid_string_or_content)
                && message.get("display").is_some_and(Value::is_boolean)
                && message.get("timestamp").and_then(Value::as_i64).is_some()
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(err(
            SessionErrorCode::Damaged,
            Some(line),
            Some("message"),
            Some("content"),
        ))
    }
}

pub(super) fn parse(bytes: &[u8], expected: Option<Uuid>) -> Result<Document, SessionError> {
    if bytes.is_empty() {
        return Err(err(
            SessionErrorCode::Damaged,
            Some(1),
            Some("session"),
            None,
        ));
    }
    let mut values = Vec::new();
    let mut start = 0;
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'\n' {
            if i - start > MAX_LINE {
                return Err(err(
                    SessionErrorCode::SizeLimit,
                    Some((values.len() + 1) as u32),
                    None,
                    None,
                ));
            }
            let slice = &bytes[start..i];
            if slice.is_empty() {
                return Err(err(
                    SessionErrorCode::Damaged,
                    Some((values.len() + 1) as u32),
                    None,
                    None,
                ));
            }
            match serde_json::from_slice(slice) {
                Ok(v) => values.push(v),
                Err(_) => {
                    // Whether a malformed final frame is provably an interrupted append is
                    // decided by storage's bounded, byte-structural recovery routine. Parsing
                    // itself never guesses from formatting or UTF-8 text.
                    return Err(err(
                        SessionErrorCode::Damaged,
                        Some((values.len() + 1) as u32),
                        None,
                        None,
                    ));
                }
            };
            start = i + 1;
        }
    }
    if start != bytes.len() {
        let code = if bytes.len() - start > MAX_LINE {
            SessionErrorCode::SizeLimit
        } else {
            SessionErrorCode::IncompleteFinalSuffix
        };
        return Err(err(code, Some((values.len() + 1) as u32), None, None));
    }
    if values.is_empty() {
        return Err(err(
            SessionErrorCode::Damaged,
            Some(1),
            Some("session"),
            None,
        ));
    }
    let hm = object(&values[0], 1, Some("session"))?;
    if hm.get("type").and_then(Value::as_str) != Some("session") {
        return Err(err(
            SessionErrorCode::Damaged,
            Some(1),
            Some("session"),
            Some("type"),
        ));
    }
    if !exact(
        hm,
        &["type", "version", "id", "timestamp", "cwd", "parentSession"],
    ) {
        return Err(err(
            SessionErrorCode::Damaged,
            Some(1),
            Some("session"),
            None,
        ));
    }
    let version = hm.get("version").and_then(Value::as_u64).ok_or_else(|| {
        err(
            SessionErrorCode::Damaged,
            Some(1),
            Some("session"),
            Some("version"),
        )
    })?;
    let id_s = string(hm, "id", 1, "session")?;
    let id = Uuid::parse_str(&id_s).map_err(|_| {
        err(
            SessionErrorCode::Damaged,
            Some(1),
            Some("session"),
            Some("id"),
        )
    })?;
    if id.get_version_num() != 7 || expected.is_some_and(|x| x != id) {
        return Err(err(
            SessionErrorCode::Damaged,
            Some(1),
            Some("session"),
            Some("id"),
        ));
    }
    let ts = string(hm, "timestamp", 1, "session")?;
    timestamp(&ts, 1, "session")?;
    if hm.get("cwd").and_then(Value::as_str) != Some("") {
        return Err(err(
            SessionErrorCode::Damaged,
            Some(1),
            Some("session"),
            Some("cwd"),
        ));
    }
    if let Some(p) = hm.get("parentSession") {
        if !p.is_string() {
            return Err(err(
                SessionErrorCode::Damaged,
                Some(1),
                Some("session"),
                Some("parentSession"),
            ));
        }
    }
    let header = Header {
        id,
        timestamp: ts,
        version,
    };
    if version != 3 {
        return Ok(Document {
            header,
            entries: Vec::new(),
            unsupported: true,
        });
    }
    let mut entries = Vec::new();
    let mut ids = HashSet::new();
    let mut unsupported = false;
    let pinned = ["branch_summary", "label", "custom", "custom_message"];
    for (idx, v) in values.iter().enumerate().skip(1) {
        let line = (idx + 1) as u32;
        let m = object(v, line, None)?;
        let ty = string(m, "type", line, "entry")?;
        if pinned.contains(&ty.as_str()) {
            unsupported = true;
            parse_identity(m, line, &ty, &mut ids, &entries)?;
            validate_pinned_unsupported(m, line, &ty)?;
            entries.push(identity_entry(m, line, EntryKind::Unsupported, false)?);
            continue;
        }
        if ![
            "message",
            "model_change",
            "thinking_level_change",
            "session_info",
            "compaction",
        ]
        .contains(&ty.as_str())
        {
            return Err(err(
                SessionErrorCode::Damaged,
                Some(line),
                Some(&ty),
                Some("type"),
            ));
        }
        parse_identity(m, line, &ty, &mut ids, &entries)?;
        let (kind, supported) = match ty.as_str() {
            "message" => parse_message(m, line)?,
            "model_change" => {
                let ok = exact(
                    m,
                    &["type", "id", "parentId", "timestamp", "provider", "modelId"],
                );
                (
                    EntryKind::Model {
                        provider: string(m, "provider", line, &ty)?,
                        model: string(m, "modelId", line, &ty)?,
                    },
                    ok,
                )
            }
            "thinking_level_change" => {
                let level = string(m, "thinkingLevel", line, &ty)?;
                let valid = ["off", "minimal", "low", "medium", "high", "xhigh", "max"]
                    .contains(&level.as_str());
                if !valid {
                    return Err(err(
                        SessionErrorCode::Damaged,
                        Some(line),
                        Some(&ty),
                        Some("thinkingLevel"),
                    ));
                }
                (
                    EntryKind::Reasoning(level),
                    exact(m, &["type", "id", "parentId", "timestamp", "thinkingLevel"]),
                )
            }
            "session_info" => {
                let name = match m.get("name") {
                    None => None,
                    Some(Value::String(name)) => Some(name.clone()),
                    Some(_) => {
                        return Err(err(
                            SessionErrorCode::Damaged,
                            Some(line),
                            Some(&ty),
                            Some("name"),
                        ))
                    }
                };
                (
                    EntryKind::Name(name),
                    exact(m, &["type", "id", "parentId", "timestamp", "name"]),
                )
            }
            "compaction" => parse_compaction(m, line)?,
            _ => unreachable!(),
        };
        unsupported |= !supported;
        entries.push(identity_entry(m, line, kind, supported)?);
    }
    validate_graph(&entries)?;
    validate_message_graph(&entries)?;
    validate_all_compactions(&entries)?;
    Ok(Document {
        header,
        entries,
        unsupported,
    })
}
fn parse_identity(
    m: &Map<String, Value>,
    line: u32,
    ty: &str,
    ids: &mut HashSet<String>,
    prior: &[Entry],
) -> Result<(), SessionError> {
    let id = string(m, "id", line, ty)?;
    if !valid_entry_id(&id) || !ids.insert(id.clone()) {
        return Err(err(
            SessionErrorCode::Damaged,
            Some(line),
            Some(ty),
            Some("id"),
        ));
    }
    match m.get("parentId") {
        Some(Value::Null) => {
            if !prior.is_empty() {
                return Err(err(
                    SessionErrorCode::Damaged,
                    Some(line),
                    Some(ty),
                    Some("parentId"),
                ));
            }
        }
        Some(Value::String(p)) => {
            if !prior.iter().any(|e| e.id == *p) {
                return Err(err(
                    SessionErrorCode::Damaged,
                    Some(line),
                    Some(ty),
                    Some("parentId"),
                ));
            }
        }
        _ => {
            return Err(err(
                SessionErrorCode::Damaged,
                Some(line),
                Some(ty),
                Some("parentId"),
            ))
        }
    };
    timestamp(&string(m, "timestamp", line, ty)?, line, ty)
}
fn identity_entry(
    m: &Map<String, Value>,
    line: u32,
    kind: EntryKind,
    supported: bool,
) -> Result<Entry, SessionError> {
    let ty = m.get("type").and_then(Value::as_str).unwrap_or("entry");
    Ok(Entry {
        kind,
        id: string(m, "id", line, ty)?,
        parent: m.get("parentId").and_then(Value::as_str).map(str::to_owned),
        timestamp: string(m, "timestamp", line, ty)?,
        line,
        supported,
    })
}
fn parse_message(m: &Map<String, Value>, line: u32) -> Result<(EntryKind, bool), SessionError> {
    let msg = m.get("message").and_then(Value::as_object).ok_or_else(|| {
        err(
            SessionErrorCode::Damaged,
            Some(line),
            Some("message"),
            Some("message"),
        )
    })?;
    let role = string(msg, "role", line, "message")?;
    let outer = exact(m, &["type", "id", "parentId", "timestamp", "message"]);
    if role == "user" {
        let mut canonical = true;
        let mut unsupported_user = false;
        let text = match msg.get("content") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Array(a)) => {
                canonical = false;
                let mut out = String::new();
                for b in a {
                    let o = b.as_object().ok_or_else(|| {
                        err(
                            SessionErrorCode::Damaged,
                            Some(line),
                            Some("message"),
                            Some("content"),
                        )
                    })?;
                    match o.get("type").and_then(Value::as_str) {
                        Some("text") if valid_text_or_image_block(b) => {
                            canonical &= exact(o, &["type", "text"]);
                            out.push_str(o.get("text").and_then(Value::as_str).unwrap_or_default());
                        }
                        Some("image") if valid_text_or_image_block(b) => {
                            unsupported_user = true;
                        }
                        _ => {
                            return Err(err(
                                SessionErrorCode::Damaged,
                                Some(line),
                                Some("message"),
                                Some("content"),
                            ));
                        }
                    }
                }
                out
            }
            _ => {
                return Err(err(
                    SessionErrorCode::Damaged,
                    Some(line),
                    Some("message"),
                    Some("content"),
                ))
            }
        };
        let ts = msg
            .get("timestamp")
            .and_then(Value::as_i64)
            .ok_or_else(|| {
                err(
                    SessionErrorCode::Damaged,
                    Some(line),
                    Some("message"),
                    Some("timestamp"),
                )
            })?;
        let _ = ts;
        canonical &= exact(msg, &["role", "content", "timestamp"]);
        let kind = if unsupported_user {
            EntryKind::UnsupportedUser
        } else {
            EntryKind::User(text)
        };
        return Ok((kind, outer && canonical && !unsupported_user));
    }
    if role != "assistant" {
        return if matches!(role.as_str(), "toolResult" | "bashExecution" | "custom") {
            validate_unsupported_message(msg, &role, line)?;
            Ok((EntryKind::Unsupported, false))
        } else {
            Err(err(
                SessionErrorCode::Damaged,
                Some(line),
                Some("message"),
                Some("message"),
            ))
        };
    }
    let required = [
        "role",
        "content",
        "api",
        "provider",
        "model",
        "usage",
        "stopReason",
        "timestamp",
    ];
    if required.iter().any(|k| !msg.contains_key(*k)) {
        return Err(err(
            SessionErrorCode::Damaged,
            Some(line),
            Some("message"),
            None,
        ));
    }
    let _api = string(msg, "api", line, "message")?;
    let provider = string(msg, "provider", line, "message")?;
    let model = string(msg, "model", line, "message")?;
    let _message_timestamp = msg
        .get("timestamp")
        .and_then(Value::as_i64)
        .ok_or_else(|| {
            err(
                SessionErrorCode::Damaged,
                Some(line),
                Some("message"),
                Some("timestamp"),
            )
        })?;
    for optional in ["responseModel", "responseId", "errorMessage"] {
        if msg.get(optional).is_some_and(|value| !value.is_string()) {
            return Err(err(
                SessionErrorCode::Damaged,
                Some(line),
                Some("message"),
                Some(optional),
            ));
        }
    }
    if msg
        .get("diagnostics")
        .is_some_and(|value| !value.is_array())
    {
        return Err(err(
            SessionErrorCode::Damaged,
            Some(line),
            Some("message"),
            Some("message"),
        ));
    }
    let stop = string(msg, "stopReason", line, "message")?;
    if !["stop", "length", "toolUse", "error", "aborted"].contains(&stop.as_str()) {
        return Err(err(
            SessionErrorCode::Damaged,
            Some(line),
            Some("message"),
            Some("message"),
        ));
    }
    let stop_ok = stop == "stop" || stop == "length";
    let content = msg
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            err(
                SessionErrorCode::Damaged,
                Some(line),
                Some("message"),
                Some("content"),
            )
        })?;
    let mut blocks = Vec::new();
    let mut content_ok = true;
    for b in content {
        let Some(o) = b.as_object() else {
            return Err(err(
                SessionErrorCode::Damaged,
                Some(line),
                Some("message"),
                Some("content"),
            ));
        };
        match o.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(t) = o.get("text").and_then(Value::as_str) {
                    blocks.push(AssistantData::Text {
                        text: t.to_owned(),
                        signature: o
                            .get("textSignature")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                    })
                } else {
                    return Err(err(
                        SessionErrorCode::Damaged,
                        Some(line),
                        Some("message"),
                        Some("content"),
                    ));
                };
                content_ok &= exact(o, &["type", "text", "textSignature"]);
                if o.get("textSignature")
                    .is_some_and(|value| !value.is_string())
                {
                    return Err(err(
                        SessionErrorCode::Damaged,
                        Some(line),
                        Some("message"),
                        Some("content"),
                    ));
                }
            }
            Some("thinking") => {
                content_ok &= exact(o, &["type", "thinking", "thinkingSignature", "redacted"])
                    && o.get("thinking").is_some_and(Value::is_string);
                if o.get("thinkingSignature")
                    .is_some_and(|value| !value.is_string())
                    || o.get("redacted").is_some_and(|value| !value.is_boolean())
                {
                    return Err(err(
                        SessionErrorCode::Damaged,
                        Some(line),
                        Some("message"),
                        Some("content"),
                    ));
                }
                blocks.push(AssistantData::Thinking {
                    thinking: o
                        .get("thinking")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    signature: o
                        .get("thinkingSignature")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    redacted: o.get("redacted").and_then(Value::as_bool).unwrap_or(false),
                });
            }
            Some("image") => {
                // ImageContent is pinned only for User and ToolResult messages.
                return Err(err(
                    SessionErrorCode::Damaged,
                    Some(line),
                    Some("message"),
                    Some("content"),
                ));
            }
            Some("toolCall") => {
                if !valid_tool_call_block(o) {
                    return Err(err(
                        SessionErrorCode::Damaged,
                        Some(line),
                        Some("message"),
                        Some("content"),
                    ));
                }
                content_ok = false;
            }
            _ => {
                return Err(err(
                    SessionErrorCode::Damaged,
                    Some(line),
                    Some("message"),
                    Some("content"),
                ))
            }
        }
    }
    let usage = msg.get("usage").and_then(Value::as_object).ok_or_else(|| {
        err(
            SessionErrorCode::Damaged,
            Some(line),
            Some("message"),
            Some("usage"),
        )
    })?;
    for k in ["input", "output", "cacheRead", "cacheWrite", "totalTokens"] {
        if usage.get(k).and_then(Value::as_u64).is_none() {
            return Err(err(
                SessionErrorCode::Damaged,
                Some(line),
                Some("message"),
                Some("usage"),
            ));
        }
    }
    for k in ["cacheWrite1h", "reasoning"] {
        if usage.get(k).is_some_and(|value| value.as_u64().is_none()) {
            return Err(err(
                SessionErrorCode::Damaged,
                Some(line),
                Some("message"),
                Some("usage"),
            ));
        }
    }
    let cost = usage
        .get("cost")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            err(
                SessionErrorCode::Damaged,
                Some(line),
                Some("message"),
                Some("usage"),
            )
        })?;
    for k in ["input", "output", "cacheRead", "cacheWrite", "total"] {
        if cost.get(k).and_then(Value::as_f64).is_none() {
            return Err(err(
                SessionErrorCode::Damaged,
                Some(line),
                Some("message"),
                Some("usage"),
            ));
        }
    }
    let usage_ok = exact(
        usage,
        &[
            "input",
            "output",
            "cacheRead",
            "cacheWrite",
            "cacheWrite1h",
            "reasoning",
            "totalTokens",
            "cost",
        ],
    ) && exact(
        cost,
        &["input", "output", "cacheRead", "cacheWrite", "total"],
    );
    let msg_ok = exact(
        msg,
        &[
            "role",
            "content",
            "api",
            "provider",
            "model",
            "responseModel",
            "responseId",
            "usage",
            "stopReason",
            "timestamp",
        ],
    );
    Ok((
        EntryKind::Assistant {
            blocks,
            provider,
            model,
            response_id: msg
                .get("responseId")
                .and_then(Value::as_str)
                .map(str::to_owned),
        },
        outer && msg_ok && usage_ok && content_ok && stop_ok,
    ))
}
fn parse_compaction(m: &Map<String, Value>, line: u32) -> Result<(EntryKind, bool), SessionError> {
    if m.get("fromHook").is_some_and(|value| !value.is_boolean()) {
        return Err(err(
            SessionErrorCode::Damaged,
            Some(line),
            Some("compaction"),
            Some("details"),
        ));
    }
    let summary = string(m, "summary", line, "compaction")?;
    let first = string(m, "firstKeptEntryId", line, "compaction")?;
    let tokens = m
        .get("tokensBefore")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            err(
                SessionErrorCode::Damaged,
                Some(line),
                Some("compaction"),
                Some("tokensBefore"),
            )
        })?;
    let mut ok = exact(
        m,
        &[
            "type",
            "id",
            "parentId",
            "timestamp",
            "summary",
            "firstKeptEntryId",
            "tokensBefore",
            "details",
        ],
    );
    let reason = match m.get("details") {
        None => None,
        Some(Value::Object(details)) => match details.get("reason") {
            None => {
                // Pi permits arbitrary extension details. They are preserved but are
                // outside the writable Job Radar subset unless the object is empty.
                ok &= details.is_empty();
                None
            }
            Some(Value::String(reason))
                if ["manual", "threshold", "overflow"].contains(&reason.as_str()) =>
            {
                ok &= exact(details, &["reason"]);
                Some(reason.clone())
            }
            Some(_) => {
                // A non-Job-Radar extension payload is recognized and preserved
                // read-only rather than interpreted as a trigger.
                ok = false;
                None
            }
        },
        Some(_) => {
            // `details` is generic in pinned Pi. Non-object values are compatible
            // opaque extension data, but not writable by this Core.
            ok = false;
            None
        }
    };
    Ok((
        EntryKind::Compaction {
            summary,
            first_kept: first,
            tokens,
            reason,
        },
        ok,
    ))
}
fn validate_message_graph(entries: &[Entry]) -> Result<(), SessionError> {
    let by_id: HashMap<&str, &Entry> = entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect();
    for entry in entries {
        match &entry.kind {
            EntryKind::Assistant { .. } => {
                let parent = entry
                    .parent
                    .as_deref()
                    .and_then(|id| by_id.get(id).copied());
                if !parent.is_some_and(|parent| {
                    matches!(parent.kind, EntryKind::User(_) | EntryKind::UnsupportedUser)
                }) {
                    return Err(err(
                        SessionErrorCode::Damaged,
                        Some(entry.line),
                        Some("message"),
                        Some("parentId"),
                    ));
                }
            }
            EntryKind::User(_) | EntryKind::UnsupportedUser => {
                let paired = entries.iter().any(|child| {
                    child.parent.as_deref() == Some(entry.id.as_str())
                        && matches!(child.kind, EntryKind::Assistant { .. })
                });
                if !paired {
                    if std::ptr::eq(entry, entries.last().expect("nonempty entries")) {
                        return Err(SessionError::new(SessionErrorCode::IncompleteFinalSuffix));
                    }
                    return Err(err(
                        SessionErrorCode::Damaged,
                        Some(entry.line),
                        Some("message"),
                        Some("message"),
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_all_compactions(entries: &[Entry]) -> Result<(), SessionError> {
    let by_id: HashMap<&str, &Entry> = entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect();
    for entry in entries {
        let EntryKind::Compaction { first_kept, .. } = &entry.kind else {
            continue;
        };
        let mut parent = entry.parent.as_deref();
        let mut valid = false;
        while let Some(id) = parent {
            let Some(ancestor) = by_id.get(id).copied() else {
                break;
            };
            if ancestor.id == *first_kept
                && matches!(
                    ancestor.kind,
                    EntryKind::User(_) | EntryKind::Assistant { .. }
                )
            {
                valid = true;
                break;
            }
            parent = ancestor.parent.as_deref();
        }
        if !valid {
            return Err(err(
                SessionErrorCode::Damaged,
                Some(entry.line),
                Some("compaction"),
                Some("firstKeptEntryId"),
            ));
        }
    }
    Ok(())
}

fn validate_graph(entries: &[Entry]) -> Result<(), SessionError> {
    if entries.is_empty() {
        return Ok(());
    }
    let map: HashMap<&str, usize> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.as_str(), i))
        .collect();
    for (i, e) in entries.iter().enumerate() {
        if i == 0 && e.parent.is_some() {
            return Err(err(
                SessionErrorCode::Damaged,
                Some(e.line),
                None,
                Some("parentId"),
            ));
        }
        if i > 0 && e.parent.is_none() {
            return Err(err(
                SessionErrorCode::Damaged,
                Some(e.line),
                None,
                Some("parentId"),
            ));
        }
        if let Some(p) = &e.parent {
            if map.get(p.as_str()).is_none_or(|j| *j >= i) {
                return Err(err(
                    SessionErrorCode::Damaged,
                    Some(e.line),
                    None,
                    Some("parentId"),
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn line(v: Value) -> Result<Vec<u8>, SessionError> {
    let mut b =
        serde_json::to_vec(&v).map_err(|_| SessionError::new(SessionErrorCode::NotSaved))?;
    b.push(b'\n');
    if b.len() - 1 > MAX_LINE {
        Err(SessionError::new(SessionErrorCode::SizeLimit))
    } else {
        Ok(b)
    }
}
pub(super) fn header(id: Uuid, ts: &str) -> Value {
    json!({"type":"session","version":3,"id":id.to_string(),"timestamp":ts,"cwd":""})
}
pub(super) fn model(id: &str, parent: Option<&str>, ts: &str, p: &str, m: &str) -> Value {
    json!({"type":"model_change","id":id,"parentId":parent,"timestamp":ts,"provider":p,"modelId":m})
}
pub(super) fn reasoning(id: &str, parent: Option<&str>, ts: &str, l: &str) -> Value {
    json!({"type":"thinking_level_change","id":id,"parentId":parent,"timestamp":ts,"thinkingLevel":l})
}
pub(super) fn name(id: &str, parent: Option<&str>, ts: &str, n: &str) -> Value {
    json!({"type":"session_info","id":id,"parentId":parent,"timestamp":ts,"name":n})
}
pub(super) fn user(id: &str, parent: Option<&str>, ts: &str, text: &str, ms: i128) -> Value {
    json!({"type":"message","id":id,"parentId":parent,"timestamp":ts,"message":{"role":"user","content":text,"timestamp":ms}})
}
pub(super) fn assistant(
    id: &str,
    parent: &str,
    ts: &str,
    turn: &super::CompletedTurn,
    ms: i128,
) -> Value {
    let blocks: Vec<Value> = turn
        .assistant_blocks
        .iter()
        .map(|b| match &b.0 {
            AssistantBlockKind::Text {
                text,
                text_signature,
            } => {
                let mut v = json!({"type":"text","text":text});
                if let Some(s) = text_signature {
                    v.as_object_mut()
                        .unwrap()
                        .insert("textSignature".into(), json!(s));
                }
                v
            }
            AssistantBlockKind::Thinking {
                thinking,
                thinking_signature,
                redacted,
            } => {
                let mut v = json!({"type":"thinking","thinking":thinking,"redacted":redacted});
                if let Some(s) = thinking_signature {
                    v.as_object_mut()
                        .unwrap()
                        .insert("thinkingSignature".into(), json!(s));
                }
                v
            }
        })
        .collect();
    let AssistantUsage {
        input,
        output,
        cache_read,
        cache_write,
        cache_write_1h,
        reasoning,
        total_tokens,
        cost,
    } = &turn.usage;
    let UsageCost {
        input: ci,
        output: co,
        cache_read: cr,
        cache_write: cw,
        total,
    } = cost;
    let mut usage = json!({"input":input,"output":output,"cacheRead":cache_read,"cacheWrite":cache_write,"totalTokens":total_tokens,"cost":{"input":ci,"output":co,"cacheRead":cr,"cacheWrite":cw,"total":total}});
    if let Some(x) = cache_write_1h {
        usage
            .as_object_mut()
            .unwrap()
            .insert("cacheWrite1h".into(), json!(x));
    }
    if let Some(x) = reasoning {
        usage
            .as_object_mut()
            .unwrap()
            .insert("reasoning".into(), json!(x));
    }
    let mut msg = json!({"role":"assistant","content":blocks,"api":turn.api,"provider":turn.provider.as_str(),"model":turn.model.as_str(),"usage":usage,"stopReason":match turn.stop_reason{StopReason::Stop=>"stop",StopReason::Length=>"length"},"timestamp":ms});
    if let Some(x) = &turn.response_model {
        msg.as_object_mut()
            .unwrap()
            .insert("responseModel".into(), json!(x));
    }
    if let Some(x) = &turn.response_id {
        msg.as_object_mut()
            .unwrap()
            .insert("responseId".into(), json!(x));
    }
    json!({"type":"message","id":id,"parentId":parent,"timestamp":ts,"message":msg})
}
pub(super) fn compaction(
    id: &str,
    parent: Option<&str>,
    ts: &str,
    r: &super::CompactionRecord,
) -> Value {
    let mut v = json!({"type":"compaction","id":id,"parentId":parent,"timestamp":ts,"summary":r.summary,"firstKeptEntryId":r.first_kept_entry_id,"tokensBefore":r.tokens_before});
    if let Some(reason) = r.reason {
        v.as_object_mut()
            .unwrap()
            .insert("details".into(), json!({"reason":reason.as_str()}));
    }
    v
}
