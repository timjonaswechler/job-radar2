use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

pub(super) fn location_lookup_keys(input: &str) -> Vec<String> {
    let normalized = normalize_punctuation_and_whitespace(&input.trim().to_lowercase());
    if normalized.is_empty() {
        return Vec::new();
    }

    let mut keys = Vec::new();
    push_unique(&mut keys, german_transliteration_key(&normalized));
    push_unique(&mut keys, ascii_decomposition_key(&normalized));
    push_unique(&mut keys, normalized);
    keys
}

fn german_transliteration_key(input: &str) -> String {
    input
        .replace('ä', "ae")
        .replace('ö', "oe")
        .replace('ü', "ue")
        .replace('ß', "ss")
}

fn ascii_decomposition_key(input: &str) -> String {
    input
        .nfd()
        .filter(|character| !is_combining_mark(*character))
        .collect::<String>()
}

fn normalize_punctuation_and_whitespace(input: &str) -> String {
    input
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || character.is_whitespace() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn push_unique(keys: &mut Vec<String>, key: String) {
    if !keys.contains(&key) {
        keys.push(key);
    }
}
