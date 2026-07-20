use super::fields::evaluate_string_field;
use super::*;

pub(in crate::profile_dsl::runtime::discovery) fn evaluate_strategy_captures(
    item: &RuntimeItem<'_, '_>,
    capture_rules: Option<&Captures>,
    source_config: &SourceConfig,
    source_name: &str,
    base_path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> Option<BTreeMap<String, String>> {
    let mut captures = BTreeMap::new();
    let Some(capture_rules) = capture_rules else {
        return Some(captures);
    };

    for (key, rule) in capture_rules {
        let path = format!("{base_path}/captures/{key}");
        let context_captures = captures.clone();
        let evaluation = evaluate_string_field(
            item,
            source_config,
            source_name,
            &context_captures,
            &rule.from,
            &format!("{path}/from"),
            strategy_key,
            item_index,
            diagnostics,
        );
        if evaluation.failed {
            return None;
        }
        let Some(value) = evaluation.value else {
            diagnostics.push(runtime_error(
                "capture_source_missing",
                format!("Capture `{key}` source did not resolve to text"),
                &path,
                strategy_key,
                json!({ "captureKey": key, "itemIndex": item_index }),
            ));
            return None;
        };
        let captured = apply_capture_rule(
            key,
            &value,
            rule,
            &path,
            strategy_key,
            item_index,
            diagnostics,
        )?;
        captures.insert(key.clone(), captured);
    }

    Some(captures)
}

fn apply_capture_rule(
    key: &str,
    value: &str,
    rule: &CaptureRule,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> Option<String> {
    let regex = match Regex::new(&rule.pattern) {
        Ok(regex) => regex,
        Err(error) => {
            diagnostics.push(runtime_error(
                "capture_pattern_invalid",
                format!("Capture `{key}` pattern is invalid: {error}"),
                format!("{path}/pattern"),
                strategy_key,
                json!({ "captureKey": key, "itemIndex": item_index, "error": error.to_string() }),
            ));
            return None;
        }
    };
    let Some(regex_captures) = regex.captures(value) else {
        diagnostics.push(runtime_error(
            "capture_not_matched",
            format!("Capture `{key}` pattern did not match runtime text"),
            path,
            strategy_key,
            json!({ "captureKey": key, "itemIndex": item_index }),
        ));
        return None;
    };

    let captured = regex_captures
        .name("value")
        .or_else(|| {
            regex
                .capture_names()
                .flatten()
                .find_map(|name| regex_captures.name(name))
        })
        .or_else(|| regex_captures.get(1))
        .or_else(|| regex_captures.get(0))
        .map(|matched| matched.as_str().trim().to_string())
        .filter(|value| !value.is_empty());

    match captured {
        Some(value) => Some(value),
        None => {
            diagnostics.push(runtime_error(
                "capture_empty",
                format!("Capture `{key}` resolved to empty text"),
                path,
                strategy_key,
                json!({ "captureKey": key, "itemIndex": item_index }),
            ));
            None
        }
    }
}
