use super::document::RuntimeItem;
use super::*;
use crate::profile_dsl::primitives::{
    capture::evaluate_compiled_captures,
    value::{evaluate_detail_capture_value, DetailCaptureValueContext, SourceValueView},
};
use crate::profile_dsl::template::json_pointer_segment;

mod fields;

pub(super) use fields::{evaluate_predicate, evaluate_value_list, evaluate_value_scalar};
use fields::{posting_view, push_value_error};

pub(super) fn evaluate_strategy_captures(
    strategy: &ExecutionPlanDetailStrategy,
    source_config: &SourceConfig,
    source_name: &str,
    posting: &PostingOccurrence,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<BTreeMap<String, String>> {
    let Some(plan) = &strategy.captures else {
        return Some(BTreeMap::new());
    };
    let context = DetailCaptureValueContext {
        source: SourceValueView {
            source_name,
            source_config,
        },
        posting: posting_view(posting),
    };
    match evaluate_compiled_captures(plan, |value| evaluate_detail_capture_value(value, &context)) {
        Ok(outputs) => Some(
            outputs
                .into_iter()
                .map(|output| (output.key, output.value))
                .collect(),
        ),
        Err(errors) => {
            for error in errors {
                let path = format!(
                    "{base_path}/captures/{}",
                    json_pointer_segment(&error.capture_key)
                );
                if let Some(value_error) = error.value_error {
                    push_value_error(
                        value_error,
                        &format!("{path}/from"),
                        strategy_key,
                        diagnostics,
                    );
                    continue;
                }
                let (code, message) = error.kind.diagnostic();
                diagnostics.push(runtime_error(
                    code,
                    message,
                    path,
                    strategy_key,
                    json!({ "captureKey": error.capture_key }),
                ));
            }
            None
        }
    }
}
