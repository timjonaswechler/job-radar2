use super::fields::evaluate_string_field;
use super::*;

pub(in crate::profile_dsl::runtime::detail) fn evaluate_strategy_captures(
    strategy: &ExecutionPlanDetailStrategy,
    source_config: &SourceConfig,
    source_name: &str,
    posting: &DetailPostingOccurrence,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<BTreeMap<String, String>> {
    let mut captures = BTreeMap::new();
    let empty_document = Value::Null;
    let empty_item = RuntimeItem::Json(&empty_document);
    let Some(capture_rules) = &strategy.captures else {
        return Some(captures);
    };

    for (key, rule) in capture_rules {
        let path = format!("{base_path}/captures/{key}");
        let context_captures = captures.clone();
        let evaluation = evaluate_string_field(
            &empty_item,
            source_config,
            source_name,
            posting,
            &context_captures,
            &rule.from,
            &format!("{path}/from"),
            strategy_key,
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
                json!({ "captureKey": key }),
            ));
            return None;
        };
        let Some(captured) =
            apply_capture_rule(key, &value, rule, &path, strategy_key, diagnostics)
        else {
            return None;
        };
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
                json!({ "captureKey": key, "error": error.to_string() }),
            ));
            return None;
        }
    };
    let Some(captures) = regex.captures(value) else {
        diagnostics.push(runtime_error(
            "capture_not_matched",
            format!("Capture `{key}` pattern did not match runtime text"),
            path,
            strategy_key,
            json!({ "captureKey": key }),
        ));
        return None;
    };

    let captured = captures
        .name("value")
        .or_else(|| {
            regex
                .capture_names()
                .flatten()
                .find_map(|name| captures.name(name))
        })
        .or_else(|| captures.get(1))
        .or_else(|| captures.get(0))
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
                json!({ "captureKey": key }),
            ));
            None
        }
    }
}
