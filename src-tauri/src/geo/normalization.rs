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

pub(super) fn postal_lookup_keys(input: &str) -> Vec<String> {
    let numeric_key = input.trim().to_lowercase();
    if !numeric_key.is_empty()
        && numeric_key
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        return vec![numeric_key];
    }

    let compact = input
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect::<String>();

    let mut keys = Vec::new();
    if let Some((outward_code, sector_digit, inward_code)) = split_uk_full_postcode(&compact) {
        push_unique(
            &mut keys,
            format!("{outward_code} {sector_digit}{inward_code}"),
        );
        push_unique(&mut keys, format!("{outward_code} {sector_digit}"));
        push_unique(&mut keys, outward_code);
    } else if let Some((outward_code, sector_digit)) = split_uk_postcode_sector(&compact) {
        push_unique(&mut keys, format!("{outward_code} {sector_digit}"));
        push_unique(&mut keys, outward_code);
    } else if is_uk_outward_code(&compact) {
        push_unique(&mut keys, compact);
    }

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

fn split_uk_full_postcode(input: &str) -> Option<(String, char, String)> {
    if !(5..=7).contains(&input.len()) {
        return None;
    }

    let outward_code_end = input.len() - 3;
    let outward_code = &input[..outward_code_end];
    let inward_code = &input[outward_code_end..];
    let mut inward_characters = inward_code.chars();
    let sector_digit = inward_characters.next()?;
    let inward_suffix = inward_characters.collect::<String>();

    if is_uk_outward_code(outward_code)
        && sector_digit.is_ascii_digit()
        && inward_suffix
            .chars()
            .all(|character| character.is_ascii_alphabetic())
    {
        Some((outward_code.to_string(), sector_digit, inward_suffix))
    } else {
        None
    }
}

fn split_uk_postcode_sector(input: &str) -> Option<(String, char)> {
    if !(3..=5).contains(&input.len()) {
        return None;
    }

    let outward_code_end = input.len() - 1;
    let outward_code = &input[..outward_code_end];
    let sector_digit = input.chars().last()?;

    if is_uk_outward_code(outward_code) && sector_digit.is_ascii_digit() {
        Some((outward_code.to_string(), sector_digit))
    } else {
        None
    }
}

fn is_uk_outward_code(input: &str) -> bool {
    (2..=4).contains(&input.len())
        && input
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic())
        && input
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
        && input.chars().any(|character| character.is_ascii_digit())
}

fn push_unique(keys: &mut Vec<String>, key: String) {
    if !keys.contains(&key) {
        keys.push(key);
    }
}

#[cfg(test)]
mod tests {
    use super::postal_lookup_keys;

    #[test]
    fn derives_uk_postcode_fallback_keys() {
        assert_eq!(
            postal_lookup_keys("SW1A 1AA"),
            ["sw1a 1aa", "sw1a 1", "sw1a"]
        );
        assert_eq!(
            postal_lookup_keys("sw1a1aa"),
            ["sw1a 1aa", "sw1a 1", "sw1a"]
        );
        assert_eq!(postal_lookup_keys("AL3 8"), ["al3 8", "al3"]);
        assert_eq!(postal_lookup_keys("AL3"), ["al3"]);
    }

    #[test]
    fn keeps_numeric_postal_codes() {
        assert_eq!(postal_lookup_keys("55116"), ["55116"]);
    }

    #[test]
    fn ignores_regular_place_names() {
        assert!(postal_lookup_keys("Mainz").is_empty());
    }
}
