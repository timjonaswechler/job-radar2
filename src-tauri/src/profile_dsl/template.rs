use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

/// Immutable, context-neutral template plan. It contains authored literals and
/// typed references only; values are supplied exclusively while rendering.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct CompiledTemplate(pub(crate) Vec<TemplateSegment>);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TemplateSegment {
    Literal { value: String },
    Reference { reference: TemplateReference },
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateReference {
    pub namespace: Option<String>,
    pub key: String,
}

/// Exact names admitted at one authored placement. A descriptor is input to
/// compilation and is deliberately not retained by the compiled plan.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TemplateDescriptor {
    bare_keys: BTreeSet<String>,
    namespaces: BTreeMap<String, BTreeSet<String>>,
    admit_all: bool,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum TemplatePlacement {
    DiscoveryValue,
    DetailValue,
    DiscoveryHttpUrl,
    DiscoveryHttpHeader,
    DiscoveryHttpBody,
    DetailHttpUrl,
    DetailHttpHeader,
    DetailHttpBody,
    DiscoveryBrowserUrl,
    DetailBrowserUrl,
    DetectionHttpUrl,
    DetectionBrowserUrl,
    DetectionProposal,
}

impl TemplatePlacement {
    const ALL: [Self; 13] = [
        Self::DiscoveryValue,
        Self::DetailValue,
        Self::DiscoveryHttpUrl,
        Self::DiscoveryHttpHeader,
        Self::DiscoveryHttpBody,
        Self::DetailHttpUrl,
        Self::DetailHttpHeader,
        Self::DetailHttpBody,
        Self::DiscoveryBrowserUrl,
        Self::DetailBrowserUrl,
        Self::DetectionHttpUrl,
        Self::DetectionBrowserUrl,
        Self::DetectionProposal,
    ];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TemplateAdmissionPolicy {
    DiscoveryValue,
    DetailValue,
    DiscoveryFetch,
    DetailFetch,
    DetectionBase,
    DetectionBrowser,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TemplateAdmissionKeys {
    pub source_config: BTreeSet<String>,
    pub captures: BTreeSet<String>,
    pub posting_meta: BTreeSet<String>,
}

pub(crate) fn descriptor_for_placement(
    placement: TemplatePlacement,
    keys: &TemplateAdmissionKeys,
) -> TemplateDescriptor {
    descriptor_from_inventory(placement, keys, &TEMPLATE_INFRASTRUCTURE_INVENTORY).expect(
        "production Template infrastructure inventory must have exactly one placement owner",
    )
}

fn descriptor_from_inventory(
    placement: TemplatePlacement,
    keys: &TemplateAdmissionKeys,
    entries: &[TemplateInfrastructureEntry],
) -> Result<TemplateDescriptor, &'static str> {
    let mut matches = entries.iter().filter(|entry| entry.placement == placement);
    let entry = matches
        .next()
        .ok_or("missing Template infrastructure placement")?;
    if matches.next().is_some() {
        return Err("duplicate Template infrastructure placement");
    }
    let source = TemplateDescriptor::new()
        .allow_namespace("sourceConfig", keys.source_config.iter().cloned())
        .allow_namespace("source", ["name"]);
    let detail_fetch = source
        .clone()
        .allow_namespace("postingMeta", keys.posting_meta.iter().cloned())
        .allow_namespace(
            "posting",
            ["title", "company", "url", "locations", "descriptionText"],
        );
    Ok(match entry.admission {
        TemplateAdmissionPolicy::DiscoveryValue => {
            source.allow_namespace("captures", keys.captures.iter().cloned())
        }
        TemplateAdmissionPolicy::DetailValue => {
            detail_fetch.allow_namespace("captures", keys.captures.iter().cloned())
        }
        TemplateAdmissionPolicy::DiscoveryFetch => source,
        TemplateAdmissionPolicy::DetailFetch => detail_fetch,
        TemplateAdmissionPolicy::DetectionBase => TemplateDescriptor::new()
            .allow_bare("inputUrl")
            .allow_namespace("capture", keys.captures.iter().cloned()),
        TemplateAdmissionPolicy::DetectionBrowser => TemplateDescriptor::new()
            .allow_bare("inputUrl")
            .allow_namespace("capture", keys.captures.iter().cloned())
            .allow_namespace("sourceConfig", keys.source_config.iter().cloned()),
    })
}

impl TemplateDescriptor {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn allow_bare(mut self, key: impl Into<String>) -> Self {
        self.bare_keys.insert(key.into());
        self
    }
    pub fn allow_namespace<I, S>(mut self, namespace: impl Into<String>, keys: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.namespaces
            .entry(namespace.into())
            .or_default()
            .extend(keys.into_iter().map(Into::into));
        self
    }
    fn admits(&self, reference: &TemplateReference) -> Result<(), TemplateCompileErrorKind> {
        if self.admit_all {
            return Ok(());
        }
        match &reference.namespace {
            None if self.bare_keys.contains(&reference.key) => Ok(()),
            None => Err(TemplateCompileErrorKind::UnknownKey),
            Some(namespace) => match self.namespaces.get(namespace) {
                None => Err(TemplateCompileErrorKind::UnknownNamespace),
                Some(keys) if keys.contains(&reference.key) => Ok(()),
                Some(_) => Err(TemplateCompileErrorKind::UnknownKey),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemplateCompileErrorKind {
    UnmatchedOpeningDelimiter,
    UnmatchedClosingDelimiter,
    EmptyReference,
    NestedReference,
    InvalidReference,
    TransformPipeUnsupported,
    UnknownNamespace,
    UnknownKey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateCompileError {
    pub kind: TemplateCompileErrorKind,
    pub offset: usize,
    pub reference: Option<TemplateReference>,
}

impl fmt::Display for TemplateCompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at byte {}", match self.kind {
            TemplateCompileErrorKind::UnmatchedOpeningDelimiter => "unmatched template opening delimiter",
            TemplateCompileErrorKind::UnmatchedClosingDelimiter => "unmatched template closing delimiter",
            TemplateCompileErrorKind::EmptyReference => "template reference must not be empty",
            TemplateCompileErrorKind::NestedReference => "template reference must not contain delimiters",
            TemplateCompileErrorKind::InvalidReference => "template reference must use namespace:key syntax",
            TemplateCompileErrorKind::TransformPipeUnsupported => "template transform pipes are not supported; transforms must be declared in transforms[]",
            TemplateCompileErrorKind::UnknownNamespace => "template namespace is not available at this placement",
            TemplateCompileErrorKind::UnknownKey => "template key is not available at this placement",
        }, self.offset)
    }
}

pub fn compile_template(
    input: &str,
    descriptor: &TemplateDescriptor,
) -> Result<CompiledTemplate, TemplateCompileError> {
    compile_template_all(input, descriptor).map_err(|mut errors| errors.remove(0))
}

pub(crate) fn compile_template_all(
    input: &str,
    descriptor: &TemplateDescriptor,
) -> Result<CompiledTemplate, Vec<TemplateCompileError>> {
    let parsed = compile_template_once(
        input,
        &TemplateDescriptor {
            admit_all: true,
            ..TemplateDescriptor::default()
        },
    )
    .map_err(|error| vec![error])?;
    let mut reference_offsets = parsed.reference_offsets.iter();
    let errors = parsed
        .template
        .0
        .iter()
        .filter_map(|segment| match segment {
            TemplateSegment::Reference { reference } => {
                let offset = *reference_offsets
                    .next()
                    .expect("every parsed reference retains its offset");
                descriptor
                    .admits(reference)
                    .err()
                    .map(|kind| error(kind, offset, Some(reference.clone())))
            }
            TemplateSegment::Literal { .. } => None,
        })
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(parsed.template)
    } else {
        Err(errors)
    }
}

struct ParsedTemplate {
    template: CompiledTemplate,
    reference_offsets: Vec<usize>,
}

fn compile_template_once(
    input: &str,
    descriptor: &TemplateDescriptor,
) -> Result<ParsedTemplate, TemplateCompileError> {
    let mut segments = Vec::new();
    let mut reference_offsets = Vec::new();
    let mut literal = String::new();
    let mut offset = 0;
    while offset < input.len() {
        let remainder = &input[offset..];
        if remainder.starts_with("{{{{") {
            literal.push_str("{{");
            offset += 4;
            continue;
        }
        if remainder.starts_with("}}}}") {
            literal.push_str("}}");
            offset += 4;
            continue;
        }
        if remainder.starts_with("}}") {
            return Err(error(
                TemplateCompileErrorKind::UnmatchedClosingDelimiter,
                offset,
                None,
            ));
        }
        if !remainder.starts_with("{{") {
            let ch = remainder.chars().next().expect("non-empty remainder");
            literal.push(ch);
            offset += ch.len_utf8();
            continue;
        }
        if !literal.is_empty() {
            segments.push(TemplateSegment::Literal {
                value: std::mem::take(&mut literal),
            });
        }
        let start = offset;
        let body_start = offset + 2;
        let Some(relative_end) = input[body_start..].find("}}") else {
            return Err(error(
                TemplateCompileErrorKind::UnmatchedOpeningDelimiter,
                start,
                None,
            ));
        };
        let end = body_start + relative_end;
        let body = input[body_start..end].trim();
        let kind = if body.is_empty() {
            Some(TemplateCompileErrorKind::EmptyReference)
        } else if body.contains("{{") || body.contains(['{', '}']) {
            Some(TemplateCompileErrorKind::NestedReference)
        } else if body.contains('|') {
            Some(TemplateCompileErrorKind::TransformPipeUnsupported)
        } else {
            None
        };
        if let Some(kind) = kind {
            return Err(error(kind, start, None));
        }
        let reference = if let Some((namespace, key)) = body.split_once(':') {
            if namespace.is_empty()
                || key.is_empty()
                || key.contains(':')
                || namespace.chars().any(char::is_whitespace)
                || key.chars().any(char::is_whitespace)
            {
                return Err(error(
                    TemplateCompileErrorKind::InvalidReference,
                    start,
                    None,
                ));
            }
            TemplateReference {
                namespace: Some(namespace.to_string()),
                key: key.to_string(),
            }
        } else if body.chars().any(char::is_whitespace) || body.contains('.') {
            return Err(error(
                TemplateCompileErrorKind::InvalidReference,
                start,
                None,
            ));
        } else {
            TemplateReference {
                namespace: None,
                key: body.to_string(),
            }
        };
        if let Err(kind) = descriptor.admits(&reference) {
            return Err(error(kind, start, Some(reference)));
        }
        segments.push(TemplateSegment::Reference { reference });
        reference_offsets.push(start);
        offset = end + 2;
    }
    if !literal.is_empty() || segments.is_empty() {
        segments.push(TemplateSegment::Literal { value: literal });
    }
    Ok(ParsedTemplate {
        template: CompiledTemplate(segments),
        reference_offsets,
    })
}

fn error(
    kind: TemplateCompileErrorKind,
    offset: usize,
    reference: Option<TemplateReference>,
) -> TemplateCompileError {
    TemplateCompileError {
        kind,
        offset,
        reference,
    }
}

pub trait TemplateValueView {
    fn resolve(&self, reference: &TemplateReference) -> Option<String>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateRenderError {
    pub reference: TemplateReference,
}
impl fmt::Display for TemplateRenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "template value `{}` is not available",
            reference_label(&self.reference)
        )
    }
}

pub fn render_template(
    template: &CompiledTemplate,
    values: &dyn TemplateValueView,
) -> Result<String, TemplateRenderError> {
    let mut rendered = String::new();
    for segment in &template.0 {
        match segment {
            TemplateSegment::Literal { value } => rendered.push_str(value),
            TemplateSegment::Reference { reference } => {
                rendered.push_str(&values.resolve(reference).ok_or_else(|| {
                    TemplateRenderError {
                        reference: reference.clone(),
                    }
                })?)
            }
        }
    }
    Ok(rendered)
}

pub(crate) fn json_pointer_segment(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

pub fn reference_label(reference: &TemplateReference) -> String {
    reference
        .namespace
        .as_ref()
        .map(|namespace| format!("{namespace}:{}", reference.key))
        .unwrap_or_else(|| reference.key.clone())
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum TemplateFamily {
    Value,
    Http,
    Browser,
    Detection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TemplateInfrastructureEntry {
    placement: TemplatePlacement,
    family: TemplateFamily,
    admission: TemplateAdmissionPolicy,
}

const TEMPLATE_INFRASTRUCTURE_INVENTORY: [TemplateInfrastructureEntry; 13] = [
    TemplateInfrastructureEntry {
        placement: TemplatePlacement::DiscoveryValue,
        family: TemplateFamily::Value,
        admission: TemplateAdmissionPolicy::DiscoveryValue,
    },
    TemplateInfrastructureEntry {
        placement: TemplatePlacement::DetailValue,
        family: TemplateFamily::Value,
        admission: TemplateAdmissionPolicy::DetailValue,
    },
    TemplateInfrastructureEntry {
        placement: TemplatePlacement::DiscoveryHttpUrl,
        family: TemplateFamily::Http,
        admission: TemplateAdmissionPolicy::DiscoveryFetch,
    },
    TemplateInfrastructureEntry {
        placement: TemplatePlacement::DiscoveryHttpHeader,
        family: TemplateFamily::Http,
        admission: TemplateAdmissionPolicy::DiscoveryFetch,
    },
    TemplateInfrastructureEntry {
        placement: TemplatePlacement::DiscoveryHttpBody,
        family: TemplateFamily::Http,
        admission: TemplateAdmissionPolicy::DiscoveryFetch,
    },
    TemplateInfrastructureEntry {
        placement: TemplatePlacement::DetailHttpUrl,
        family: TemplateFamily::Http,
        admission: TemplateAdmissionPolicy::DetailFetch,
    },
    TemplateInfrastructureEntry {
        placement: TemplatePlacement::DetailHttpHeader,
        family: TemplateFamily::Http,
        admission: TemplateAdmissionPolicy::DetailFetch,
    },
    TemplateInfrastructureEntry {
        placement: TemplatePlacement::DetailHttpBody,
        family: TemplateFamily::Http,
        admission: TemplateAdmissionPolicy::DetailFetch,
    },
    TemplateInfrastructureEntry {
        placement: TemplatePlacement::DiscoveryBrowserUrl,
        family: TemplateFamily::Browser,
        admission: TemplateAdmissionPolicy::DiscoveryFetch,
    },
    TemplateInfrastructureEntry {
        placement: TemplatePlacement::DetailBrowserUrl,
        family: TemplateFamily::Browser,
        admission: TemplateAdmissionPolicy::DetailFetch,
    },
    TemplateInfrastructureEntry {
        placement: TemplatePlacement::DetectionHttpUrl,
        family: TemplateFamily::Detection,
        admission: TemplateAdmissionPolicy::DetectionBase,
    },
    TemplateInfrastructureEntry {
        placement: TemplatePlacement::DetectionBrowserUrl,
        family: TemplateFamily::Detection,
        admission: TemplateAdmissionPolicy::DetectionBrowser,
    },
    TemplateInfrastructureEntry {
        placement: TemplatePlacement::DetectionProposal,
        family: TemplateFamily::Detection,
        admission: TemplateAdmissionPolicy::DetectionBase,
    },
];

fn validate_template_inventory(
    entries: &[TemplateInfrastructureEntry],
) -> Result<(), &'static str> {
    let placements = entries
        .iter()
        .map(|entry| entry.placement)
        .collect::<BTreeSet<_>>();
    let expected = TemplatePlacement::ALL.into_iter().collect::<BTreeSet<_>>();
    if entries.len() != placements.len() {
        return Err("duplicate Template infrastructure placement");
    }
    if placements != expected {
        return Err("missing Template infrastructure placement");
    }
    let families = entries
        .iter()
        .map(|entry| entry.family)
        .collect::<BTreeSet<_>>();
    if families
        != [
            TemplateFamily::Value,
            TemplateFamily::Http,
            TemplateFamily::Browser,
            TemplateFamily::Detection,
        ]
        .into_iter()
        .collect()
    {
        return Err("missing Template consuming family");
    }
    Ok(())
}

#[cfg(test)]
mod inventory_tests {
    use super::*;

    #[test]
    fn production_inventory_is_complete_and_drives_descriptor_dispatch() {
        assert_eq!(
            validate_template_inventory(&TEMPLATE_INFRASTRUCTURE_INVENTORY),
            Ok(())
        );
        let descriptor = descriptor_for_placement(
            TemplatePlacement::DetailHttpUrl,
            &TemplateAdmissionKeys {
                captures: ["late".to_string()].into_iter().collect(),
                ..Default::default()
            },
        );
        assert_eq!(
            compile_template("{{captures:late}}", &descriptor)
                .unwrap_err()
                .kind,
            TemplateCompileErrorKind::UnknownNamespace
        );
    }

    #[test]
    fn synthetic_missing_and_duplicate_inventories_fail_validation_and_dispatch() {
        let missing = &TEMPLATE_INFRASTRUCTURE_INVENTORY[1..];
        assert!(validate_template_inventory(missing).is_err());
        assert!(descriptor_from_inventory(
            TemplatePlacement::DiscoveryValue,
            &Default::default(),
            missing
        )
        .is_err());

        let mut duplicate = TEMPLATE_INFRASTRUCTURE_INVENTORY.to_vec();
        duplicate.push(TEMPLATE_INFRASTRUCTURE_INVENTORY[0]);
        assert!(validate_template_inventory(&duplicate).is_err());
        assert!(descriptor_from_inventory(
            TemplatePlacement::DiscoveryValue,
            &Default::default(),
            &duplicate
        )
        .is_err());
    }
}
