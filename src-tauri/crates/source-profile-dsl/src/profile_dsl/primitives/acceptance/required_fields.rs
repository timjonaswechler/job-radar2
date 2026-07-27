use serde_json::json;

use super::*;

pub(super) const DESCRIPTOR: AcceptanceDescriptor = AcceptanceDescriptor {
    key: "requiredFields",
    phases: &[AcceptancePhase::Discovery, AcceptancePhase::Detail],
};

impl AcceptanceField {
    pub(super) fn authored_name(&self) -> String {
        match self {
            Self::Url => "url".into(),
            Self::Title => "title".into(),
            Self::Company => "company".into(),
            Self::Locations => "locations".into(),
            Self::DescriptionText => "descriptionText".into(),
            Self::PostingMeta(key) => format!("postingMeta.{key}"),
        }
    }

    fn detail_field(&self) -> Option<DetailField> {
        match self {
            Self::Title => Some(DetailField::Title),
            Self::Company => Some(DetailField::Company),
            Self::Locations => Some(DetailField::Locations),
            Self::DescriptionText => Some(DetailField::DescriptionText),
            Self::Url | Self::PostingMeta(_) => None,
        }
    }
}

pub(super) fn compile(
    authored: Option<&[String]>,
    context: &AcceptanceCompileContext,
) -> Result<Vec<AcceptanceField>, AcceptanceCompileError> {
    let mut required_fields = Vec::new();
    for field in authored.unwrap_or_default() {
        let compiled = match field.as_str() {
            "url" if context.phase == AcceptancePhase::Discovery => AcceptanceField::Url,
            "title" => AcceptanceField::Title,
            "company" => AcceptanceField::Company,
            "locations" => AcceptanceField::Locations,
            "descriptionText" => AcceptanceField::DescriptionText,
            value if context.phase == AcceptancePhase::Discovery => {
                let Some(key) = value.strip_prefix("postingMeta.") else {
                    return Err(invalid_field(context.phase, field));
                };
                if key.is_empty() || !context.posting_meta_keys.contains(key) {
                    return Err(invalid_field(context.phase, field));
                }
                AcceptanceField::PostingMeta(key.to_string())
            }
            _ => return Err(invalid_field(context.phase, field)),
        };
        if !required_fields.contains(&compiled) {
            required_fields.push(compiled);
        }
    }
    Ok(required_fields)
}

fn invalid_field(phase: AcceptancePhase, field: &str) -> AcceptanceCompileError {
    AcceptanceCompileError {
        phase,
        key: DESCRIPTOR.key,
        field: Some(field.to_string()),
        message: format!("required field `{field}` is not available in {phase:?} acceptance"),
    }
}

pub(super) fn evaluate_discovery(
    candidates: &[PostingOccurrence],
    phase: Option<&CompiledAcceptance>,
    strategy: Option<&CompiledAcceptance>,
    strategy_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> bool {
    for rule in required_rules(phase, strategy, "/discovery", strategy_path) {
        if let Some(item_index) = candidates
            .iter()
            .position(|candidate| !discovery_field_present(candidate, rule.value))
        {
            diagnostics.push(acceptance_diagnostic(
                "acceptance_required_field_missing",
                "Discovery occurrence is missing a required field",
                format!("{}/acceptWhen/{}", rule.owner_path, DESCRIPTOR.key),
                strategy_key,
                json!({ "field": rule.value.authored_name(), "itemIndex": item_index }),
            ));
            return false;
        }
    }
    true
}

pub(super) fn evaluate_detail(
    patch: &DetailPatch,
    phase: Option<&CompiledAcceptance>,
    strategy: Option<&CompiledAcceptance>,
    strategy_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> bool {
    for rule in required_rules(phase, strategy, "/detail", strategy_path) {
        let present = match rule.value {
            AcceptanceField::Title => patch.title.is_some(),
            AcceptanceField::Company => patch.company.is_some(),
            AcceptanceField::Locations => patch.locations.is_some(),
            AcceptanceField::DescriptionText => patch.description_text.is_some(),
            AcceptanceField::Url | AcceptanceField::PostingMeta(_) => false,
        };
        if !present {
            diagnostics.push(acceptance_diagnostic(
                "acceptance_required_field_missing",
                "Detail patch is missing a required field",
                format!("{}/acceptWhen/{}", rule.owner_path, DESCRIPTOR.key),
                strategy_key,
                json!({ "field": rule.value.authored_name() }),
            ));
            return false;
        }
    }
    true
}

pub(super) fn validate_detail_request(
    plan: &CompiledAcceptance,
    path: &str,
    strategy_key: Option<&str>,
    requested: &RequestedDetailFields,
) -> Option<Diagnostic> {
    for field in &plan.required_fields {
        if field
            .detail_field()
            .is_some_and(|field| !requested.contains(field))
        {
            return Some(acceptance_diagnostic(
                "acceptance_field_not_requested",
                "Detail acceptance references a field not requested by this invocation",
                format!("{path}/acceptWhen/{}", DESCRIPTOR.key),
                strategy_key,
                json!({ "field": field.authored_name() }),
            ));
        }
    }
    None
}

fn discovery_field_present(candidate: &PostingOccurrence, field: &AcceptanceField) -> bool {
    match field {
        AcceptanceField::Url => !candidate.reference.provider_url.is_empty(),
        AcceptanceField::Title => candidate
            .provider_values
            .title
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()),
        AcceptanceField::Company => candidate
            .provider_values
            .company
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()),
        AcceptanceField::Locations => !candidate.provider_values.locations.is_empty(),
        AcceptanceField::DescriptionText => candidate
            .provider_values
            .description_text
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()),
        AcceptanceField::PostingMeta(key) => candidate
            .posting_meta
            .get(key)
            .is_some_and(|value| !value.trim().is_empty()),
    }
}
