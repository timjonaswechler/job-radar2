pub(super) fn to_technical_key(value: &str) -> String {
    let mut key = String::new();
    let mut separator = false;
    for ch in value.to_lowercase().chars() {
        match ch {
            'a'..='z' | '0'..='9' => {
                key.push(ch);
                separator = false;
            }
            'ä' => {
                key.push('a');
                separator = false;
            }
            'ö' => {
                key.push('o');
                separator = false;
            }
            'ü' => {
                key.push('u');
                separator = false;
            }
            'ß' => {
                key.push_str("ss");
                separator = false;
            }
            _ if !separator && !key.is_empty() => {
                key.push('_');
                separator = true;
            }
            _ => {}
        }
    }
    let key = key.trim_matches('_').to_string();
    if key.is_empty() {
        "quelle".to_string()
    } else {
        key
    }
}
pub(super) fn title_case(value: &str) -> String {
    let value = value.replace(['-', '_'], " ");
    let title = value
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| format!("{}{}", first.to_uppercase(), chars.as_str()))
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ");
    if title.is_empty() {
        "Neue Quelle".to_string()
    } else {
        title
    }
}
