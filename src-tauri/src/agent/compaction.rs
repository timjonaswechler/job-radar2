// Text-only compaction policy ported from Pi dcfe36c79702ec240b146c45f167ab75ecddd205.
use super::sessions::{CompactionReason, ContextEntry, ContextRole, SessionSnapshot};

pub(crate) const RESERVE_TOKENS: u64 = 16_384;
pub(crate) const KEEP_RECENT_TOKENS: u64 = 20_000;
const MIN_COMPACTION_CONTEXT_WINDOW: u64 = RESERVE_TOKENS + KEEP_RECENT_TOKENS;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CompactionPreparation {
    first_kept_entry_id: String,
    tokens_before: u64,
    prompt: String,
    reason: CompactionReason,
    split_prefix_prompt: Option<String>,
}

impl CompactionPreparation {
    pub(crate) fn first_kept_entry_id(&self) -> &str {
        &self.first_kept_entry_id
    }
    pub(crate) fn tokens_before(&self) -> u64 {
        self.tokens_before
    }
    pub(crate) fn prompt(&self) -> &str {
        &self.prompt
    }
    pub(crate) fn reason(&self) -> CompactionReason {
        self.reason
    }
    pub(crate) fn split_prefix_prompt(&self) -> Option<&str> {
        self.split_prefix_prompt.as_deref()
    }
}

pub(crate) fn compaction_capable(context_window: u64) -> bool {
    context_window > MIN_COMPACTION_CONTEXT_WINDOW
}

pub(crate) fn requires_compaction(context_tokens: u64, context_window: u64) -> bool {
    context_window
        .checked_sub(RESERVE_TOKENS)
        .map_or(context_tokens > 0, |safe| context_tokens > safe)
}

pub(crate) fn prepare(
    snapshot: &SessionSnapshot,
    reason: CompactionReason,
    focus: Option<&str>,
) -> Option<CompactionPreparation> {
    // Preparation deliberately uses raw active-path messages. Provider accounting uses
    // `context_entries`, whose synthetic wrapper must never become summary input.
    let entries = &snapshot.compaction_entries;
    let previous = snapshot.compactions().last();
    let preparation_start = previous
        .and_then(|compaction| {
            entries
                .iter()
                .position(|entry| entry.id == compaction.first_kept_entry_id())
                .or_else(|| {
                    entries
                        .iter()
                        .position(|entry| entry.path_index > compaction.path_index)
                })
        })
        .unwrap_or(0);
    let entries = entries.get(preparation_start..)?;
    if entries.len() < 2 {
        return None;
    }

    let tokens_before = snapshot.context_tokens();
    let mut kept = 0_u64;
    let mut crossing = entries.len() - 1;
    let mut keep_budget_reached = false;
    for index in (0..entries.len()).rev() {
        kept = kept.saturating_add(estimate(&entries[index].text));
        crossing = index;
        if kept >= KEEP_RECENT_TOKENS {
            keep_budget_reached = true;
            break;
        }
    }
    // Below budget, compact only complete turns while retaining at least the second
    // turn. Assistant boundaries are reserved for a turn that itself crosses the
    // keep budget.
    let boundary = if keep_budget_reached {
        let mut boundary = (crossing..entries.len())
            .find(|index| entries[*index].role == ContextRole::User)
            .unwrap_or(crossing);
        if boundary == 0 {
            boundary = (1..entries.len())
                .find(|index| entries[*index].role == ContextRole::User)
                .unwrap_or(1);
        }
        boundary
    } else {
        (1..entries.len()).find(|index| entries[*index].role == ContextRole::User)?
    };
    let split_turn_start = (entries[boundary].role == ContextRole::Assistant)
        .then(|| {
            (0..boundary)
                .rev()
                .find(|index| entries[*index].role == ContextRole::User)
        })
        .flatten();
    let history_end = split_turn_start.unwrap_or(boundary);
    if history_end == 0 && split_turn_start.is_none() {
        return None;
    }
    let transcript = serialize(&entries[..history_end]);
    let prompt = summary_prompt(previous.map(|entry| entry.summary()), &transcript, focus);
    let split_prefix_prompt =
        split_turn_start.map(|start| split_prompt(&serialize(&entries[start..boundary])));
    Some(CompactionPreparation {
        first_kept_entry_id: entries[boundary].id.clone(),
        tokens_before,
        prompt,
        reason,
        split_prefix_prompt,
    })
}

fn estimate(text: &str) -> u64 {
    (text.chars().count() as u64).saturating_add(3) / 4
}

fn serialize(entries: &[ContextEntry]) -> String {
    entries
        .iter()
        .map(|entry| {
            let role = match entry.role {
                ContextRole::User => "User",
                ContextRole::Assistant => "Assistant",
            };
            format!("{role}:\n{}", entry.text)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn summary_prompt(previous: Option<&str>, transcript: &str, focus: Option<&str>) -> String {
    let update = previous.map(|summary| format!("\n<previous-summary>\n{summary}\n</previous-summary>\nPreserve existing information while incorporating only the new conversation below.")).unwrap_or_default();
    let focus = focus
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("\nAdditional focus: {value}"))
        .unwrap_or_default();
    format!("Summarize the conversation for a future model. Do not answer or continue it. Return exactly these top-level Markdown sections: Goal; Constraints & Preferences; Progress (with Done, In Progress, and Blocked); Key Decisions; Next Steps; Critical Context.{update}{focus}\n<conversation>\n{transcript}\n</conversation>")
}

fn split_prompt(transcript: &str) -> String {
    format!("Summarize the early part of a split turn. Do not answer it. Return exactly these top-level Markdown sections: Original Request; Early Progress; Context for Suffix.\n<conversation>\n{transcript}\n</conversation>")
}

fn markdown_heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let depth = trimmed.bytes().take_while(|byte| *byte == b'#').count();
    if !(1..=6).contains(&depth)
        || !trimmed
            .as_bytes()
            .get(depth)
            .is_some_and(u8::is_ascii_whitespace)
    {
        return None;
    }
    let title = trimmed[depth..].trim().trim_end_matches('#').trim_end();
    Some((depth, title))
}

pub(crate) fn validate_summary(summary: &str) -> bool {
    const TOP: [&str; 6] = [
        "Goal",
        "Constraints & Preferences",
        "Progress",
        "Key Decisions",
        "Next Steps",
        "Critical Context",
    ];
    const PROGRESS: [&str; 3] = ["Done", "In Progress", "Blocked"];
    if summary.trim().is_empty() {
        return false;
    }
    let headings: Vec<_> = summary.lines().filter_map(markdown_heading).collect();
    let Some(&(top_depth, "Goal")) = headings.first() else {
        return false;
    };
    if top_depth >= 6 {
        return false;
    }
    let mut top_index = 0;
    let mut progress_index = 0;
    let mut current_top = None;
    for (depth, title) in headings {
        if depth <= top_depth {
            if depth != top_depth || TOP.get(top_index) != Some(&title) {
                return false;
            }
            current_top = Some(title);
            top_index += 1;
        } else if depth == top_depth + 1 && current_top == Some("Progress") {
            if PROGRESS.get(progress_index) != Some(&title) {
                return false;
            }
            progress_index += 1;
        } else if matches!(title, "Done" | "In Progress" | "Blocked") {
            // Required progress sections at any other nesting are malformed.
            return false;
        }
    }
    top_index == TOP.len() && progress_index == PROGRESS.len()
}

pub(crate) fn validate_split_summary(summary: &str) -> bool {
    const TOP: [&str; 3] = ["Original Request", "Early Progress", "Context for Suffix"];
    if summary.trim().is_empty() {
        return false;
    }
    let headings: Vec<_> = summary.lines().filter_map(markdown_heading).collect();
    let Some(&(top_depth, "Original Request")) = headings.first() else {
        return false;
    };
    let mut index = 0;
    for (depth, title) in headings {
        if depth <= top_depth {
            if depth != top_depth || TOP.get(index) != Some(&title) {
                return false;
            }
            index += 1;
        }
    }
    index == TOP.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_is_strict_and_small_models_are_not_capable() {
        assert!(!compaction_capable(MIN_COMPACTION_CONTEXT_WINDOW));
        assert!(compaction_capable(MIN_COMPACTION_CONTEXT_WINDOW + 1));
        assert!(!requires_compaction(83_616, 100_000));
        assert!(requires_compaction(83_617, 100_000));
    }

    #[test]
    fn validates_exact_ordered_summary_structure_at_consistent_depth() {
        let valid = "## Goal\nx\n## Constraints & Preferences\nx\n## Progress\n### Done\nx\n### In Progress\nx\n### Blocked\nx\n## Key Decisions\nx\n## Next Steps\nx\n## Critical Context\nx";
        assert!(validate_summary(valid));
        assert!(!validate_summary("# Goal\nnot enough"));
        assert!(!validate_summary(
            &valid.replace("## Key Decisions", "### Key Decisions")
        ));
        assert!(!validate_summary(
            &valid.replace("## Key Decisions", "## Tools\n## Key Decisions")
        ));
        assert!(!validate_summary(&valid.replace("### Done", "#### Done")));
        assert!(!validate_summary(
            &valid.replace("### Blocked", "### Blocked\n### Files")
        ));
    }

    #[test]
    fn validates_exact_ordered_split_structure_at_consistent_depth() {
        let valid = "### Original Request\nx\n### Early Progress\nx\n### Context for Suffix\nx";
        assert!(validate_split_summary(valid));
        assert!(!validate_split_summary(
            &valid.replace("### Early Progress", "## Early Progress")
        ));
        assert!(!validate_split_summary(
            &valid.replace("### Early Progress", "### Tools\n### Early Progress")
        ));
    }
}
