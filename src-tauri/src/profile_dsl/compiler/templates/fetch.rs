use super::validation::validate_template_string;
use super::*;

pub(super) fn validate_fetch_templates(
    fetch: &Fetch,
    path: &str,
    strategy_key: &str,
    context: TemplateContext,
    source_config_keys: &HashSet<String>,
    captures: &HashSet<String>,
    posting_meta_keys: &HashSet<String>,
    diagnostics: &mut Diagnostics,
) {
    match fetch {
        Fetch::Http {
            url, headers, body, ..
        } => {
            validate_template_string(
                url,
                &format!("{path}/url"),
                strategy_key,
                context,
                source_config_keys,
                captures,
                posting_meta_keys,
                diagnostics,
            );
            if let Some(headers) = headers {
                for (header, value) in headers {
                    validate_template_string(
                        value,
                        &format!("{path}/headers/{header}"),
                        strategy_key,
                        context,
                        source_config_keys,
                        captures,
                        posting_meta_keys,
                        diagnostics,
                    );
                }
            }
            if let Some(body) = body {
                validate_request_body_templates(
                    body,
                    &format!("{path}/body"),
                    strategy_key,
                    context,
                    source_config_keys,
                    captures,
                    posting_meta_keys,
                    diagnostics,
                );
            }
        }
        Fetch::Browser { url, .. } => {
            validate_template_string(
                url,
                &format!("{path}/url"),
                strategy_key,
                context,
                source_config_keys,
                captures,
                posting_meta_keys,
                diagnostics,
            );
        }
    }
}

fn validate_request_body_templates(
    body: &RequestBody,
    path: &str,
    strategy_key: &str,
    context: TemplateContext,
    source_config_keys: &HashSet<String>,
    captures: &HashSet<String>,
    posting_meta_keys: &HashSet<String>,
    diagnostics: &mut Diagnostics,
) {
    match body {
        RequestBody::Json { value } => validate_json_object_templates(
            value,
            path,
            strategy_key,
            context,
            source_config_keys,
            captures,
            posting_meta_keys,
            diagnostics,
        ),
        RequestBody::Text { value } => validate_template_string(
            value,
            &format!("{path}/value"),
            strategy_key,
            context,
            source_config_keys,
            captures,
            posting_meta_keys,
            diagnostics,
        ),
        RequestBody::Form { fields } => {
            for (key, value) in fields {
                validate_template_string(
                    value,
                    &format!("{path}/fields/{key}"),
                    strategy_key,
                    context,
                    source_config_keys,
                    captures,
                    posting_meta_keys,
                    diagnostics,
                );
            }
        }
    }
}

fn validate_json_object_templates(
    value: &JsonObject,
    path: &str,
    strategy_key: &str,
    context: TemplateContext,
    source_config_keys: &HashSet<String>,
    captures: &HashSet<String>,
    posting_meta_keys: &HashSet<String>,
    diagnostics: &mut Diagnostics,
) {
    for (key, value) in value {
        validate_json_value_templates(
            value,
            &format!("{path}/{key}"),
            strategy_key,
            context,
            source_config_keys,
            captures,
            posting_meta_keys,
            diagnostics,
        );
    }
}

fn validate_json_value_templates(
    value: &serde_json::Value,
    path: &str,
    strategy_key: &str,
    context: TemplateContext,
    source_config_keys: &HashSet<String>,
    captures: &HashSet<String>,
    posting_meta_keys: &HashSet<String>,
    diagnostics: &mut Diagnostics,
) {
    match value {
        serde_json::Value::String(value) => validate_template_string(
            value,
            path,
            strategy_key,
            context,
            source_config_keys,
            captures,
            posting_meta_keys,
            diagnostics,
        ),
        serde_json::Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_json_value_templates(
                    value,
                    &format!("{path}/{index}"),
                    strategy_key,
                    context,
                    source_config_keys,
                    captures,
                    posting_meta_keys,
                    diagnostics,
                );
            }
        }
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                validate_json_value_templates(
                    value,
                    &format!("{path}/{key}"),
                    strategy_key,
                    context,
                    source_config_keys,
                    captures,
                    posting_meta_keys,
                    diagnostics,
                );
            }
        }
        _ => {}
    }
}
