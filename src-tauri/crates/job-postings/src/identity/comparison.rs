use std::collections::HashSet;

use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

pub(super) fn key(value: &str) -> String {
    tokens(value).join(" ")
}

pub(super) fn normalized_text_key(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub(super) fn tokens(value: &str) -> Vec<String> {
    let normalized = value.nfkc().collect::<String>().to_lowercase();
    let tokens = normalized
        .unicode_words()
        .map(str::to_string)
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();

    compact_single_letter_runs(tokens)
}

pub(super) fn containment(a: &[String], b: &[String]) -> f64 {
    let set_a = token_set(a);
    let set_b = token_set(b);
    if set_a.is_empty() && set_b.is_empty() {
        return 1.0;
    }
    let shorter_len = set_a.len().min(set_b.len());
    if shorter_len == 0 {
        return 0.0;
    }
    set_a.intersection(&set_b).count() as f64 / shorter_len as f64
}

pub(super) fn jaccard(a: &[String], b: &[String]) -> f64 {
    let set_a = token_set(a);
    let set_b = token_set(b);
    if set_a.is_empty() && set_b.is_empty() {
        return 1.0;
    }
    let union_len = set_a.union(&set_b).count();
    if union_len == 0 {
        return 0.0;
    }
    set_a.intersection(&set_b).count() as f64 / union_len as f64
}

fn token_set(tokens: &[String]) -> HashSet<&str> {
    tokens.iter().map(String::as_str).collect()
}

fn compact_single_letter_runs(tokens: Vec<String>) -> Vec<String> {
    let mut compacted = Vec::new();
    let mut single_letter_run = String::new();
    for token in tokens {
        if is_single_letter_alpha(&token) {
            single_letter_run.push_str(&token);
        } else {
            flush_single_letter_run(&mut compacted, &mut single_letter_run);
            compacted.push(token);
        }
    }
    flush_single_letter_run(&mut compacted, &mut single_letter_run);
    compacted
}

fn flush_single_letter_run(compacted: &mut Vec<String>, single_letter_run: &mut String) {
    if !single_letter_run.is_empty() {
        compacted.push(std::mem::take(single_letter_run));
    }
}

fn is_single_letter_alpha(token: &str) -> bool {
    let mut chars = token.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    chars.next().is_none() && first.is_alphabetic()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_normalize_separators_and_compact_single_letter_runs() {
        assert_eq!(
            tokens("Head Of Laser & Post-Processing Development (m/w/d)*"),
            vec![
                "head",
                "of",
                "laser",
                "post",
                "processing",
                "development",
                "mwd"
            ]
        );
    }

    #[test]
    fn scores_use_unique_token_overlap() {
        let shorter = tokens("Laser Engineer");
        let longer = tokens("Senior Laser Engineer Frontend");
        assert_eq!(containment(&shorter, &longer), 1.0);
        assert_eq!(jaccard(&shorter, &longer), 0.5);
    }
}
