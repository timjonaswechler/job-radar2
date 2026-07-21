use std::collections::{BTreeMap, HashMap};

use serde_json::json;

use crate::profile_dsl::{
    diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics},
    occurrence::{
        ContributionOrigin, DetailContributionEvidence, DetailField, DetailPatch, DetailRejection,
        DiscoveryContributionEvidence, DiscoveryRejection, DiscoveryResponsibility,
        PostingOccurrence, PostingOccurrenceIdentity, RequestedDetailFields,
    },
};

#[derive(Clone, Debug)]
pub(crate) struct DiscoveryContribution {
    pub occurrence: PostingOccurrence,
    pub origin: ContributionOrigin,
}

pub(crate) struct ReducedDiscovery {
    pub candidates: Vec<PostingOccurrence>,
    pub provenance: Vec<DiscoveryContributionEvidence>,
    pub conflicts: Vec<DiscoveryContributionEvidence>,
    pub rejections: Vec<DiscoveryRejection>,
    pub diagnostics: Diagnostics,
}

#[derive(Clone, Debug)]
pub(crate) struct DetailContribution {
    pub identity: PostingOccurrenceIdentity,
    pub patch: DetailPatch,
    pub origin: ContributionOrigin,
}

pub(crate) struct ReducedDetail {
    pub patch: DetailPatch,
    pub provenance: Vec<DetailContributionEvidence>,
    pub conflicts: Vec<DetailContributionEvidence>,
    pub rejections: Vec<DetailRejection>,
    pub diagnostics: Diagnostics,
}

#[derive(Clone)]
enum Slot<T> {
    Missing,
    Retained {
        value: T,
        origins: Vec<ContributionOrigin>,
    },
    Conflicted {
        origins: Vec<ContributionOrigin>,
    },
}

impl<T: Eq + Clone> Slot<T> {
    fn contribute(&mut self, value: T, origin: ContributionOrigin) {
        match self {
            Self::Missing => {
                *self = Self::Retained {
                    value,
                    origins: vec![origin],
                }
            }
            Self::Retained {
                value: retained,
                origins,
            } if *retained == value => push_origin(origins, origin),
            Self::Retained { origins, .. } => {
                push_origin(origins, origin);
                *self = Self::Conflicted {
                    origins: origins.clone(),
                };
            }
            Self::Conflicted { origins } => push_origin(origins, origin),
        }
    }

    fn value(&self) -> Option<T> {
        match self {
            Self::Retained { value, .. } => Some(value.clone()),
            Self::Missing | Self::Conflicted { .. } => None,
        }
    }

    fn retained_origins(&self) -> Option<Vec<ContributionOrigin>> {
        match self {
            Self::Retained { origins, .. } => Some(origins.clone()),
            _ => None,
        }
    }

    fn conflict_origins(&self) -> Option<Vec<ContributionOrigin>> {
        match self {
            Self::Conflicted { origins } => Some(origins.clone()),
            _ => None,
        }
    }
}

#[derive(Clone)]
struct DiscoveryGroup {
    identity: PostingOccurrenceIdentity,
    provider_posting_id: Option<String>,
    url: Slot<String>,
    title: Slot<String>,
    company: Slot<String>,
    locations: Slot<Vec<String>>,
    description_text: Slot<String>,
    hints: BTreeMap<String, Slot<crate::profile_dsl::occurrence::DiscoveryHint>>,
    posting_meta: BTreeMap<String, Slot<String>>,
}

pub(crate) fn reduce_discovery(contributions: Vec<DiscoveryContribution>) -> ReducedDiscovery {
    let mut indexes = HashMap::<PostingOccurrenceIdentity, usize>::new();
    let mut groups = Vec::<DiscoveryGroup>::new();
    for contribution in contributions {
        let occurrence = contribution.occurrence;
        let index = match indexes.get(&occurrence.identity).copied() {
            Some(index) => index,
            None => {
                let index = groups.len();
                indexes.insert(occurrence.identity.clone(), index);
                groups.push(DiscoveryGroup {
                    identity: occurrence.identity.clone(),
                    provider_posting_id: occurrence.reference.provider_posting_id.clone(),
                    url: Slot::Missing,
                    title: Slot::Missing,
                    company: Slot::Missing,
                    locations: Slot::Missing,
                    description_text: Slot::Missing,
                    hints: BTreeMap::new(),
                    posting_meta: BTreeMap::new(),
                });
                index
            }
        };
        let group = &mut groups[index];
        let origin = contribution.origin;
        let canonical_url = match &group.identity {
            PostingOccurrenceIdentity::NormalizedUrl { normalized_url, .. } => {
                normalized_url.clone()
            }
            PostingOccurrenceIdentity::ProviderPostingId { .. } => {
                occurrence.reference.provider_url
            }
        };
        group.url.contribute(canonical_url, origin.clone());
        if let Some(value) = occurrence.provider_values.title.filter(|v| !v.is_empty()) {
            group.title.contribute(value, origin.clone());
        }
        if let Some(value) = occurrence.provider_values.company.filter(|v| !v.is_empty()) {
            group.company.contribute(value, origin.clone());
        }
        if !occurrence.provider_values.locations.is_empty() {
            group
                .locations
                .contribute(occurrence.provider_values.locations, origin.clone());
        }
        if let Some(value) = occurrence
            .provider_values
            .description_text
            .filter(|v| !v.is_empty())
        {
            group.description_text.contribute(value, origin.clone());
        }
        for (key, value) in occurrence.hints {
            if !value.value.is_empty() {
                group
                    .hints
                    .entry(key)
                    .or_insert(Slot::Missing)
                    .contribute(value, origin.clone());
            }
        }
        for (key, value) in occurrence.posting_meta {
            if !value.is_empty() {
                group
                    .posting_meta
                    .entry(key)
                    .or_insert(Slot::Missing)
                    .contribute(value, origin.clone());
            }
        }
    }

    let mut candidates = Vec::new();
    let mut provenance = Vec::new();
    let mut conflicts = Vec::new();
    let mut rejections = Vec::new();
    let mut diagnostics = Vec::new();
    for (group_index, group) in groups.into_iter().enumerate() {
        if let Some(origins) = group.url.conflict_origins() {
            diagnostics.push(reducer_diagnostic(
                "discovery_required_provider_url_conflict",
                format!("/discovery/groups/{group_index}/reference/url"),
            ));
            rejections.push(DiscoveryRejection::RequiredProviderUrlConflict {
                group_index,
                contributors: origins,
            });
            continue;
        }
        let provider_url = group.url.value().expect("every Discovery group has a URL");
        push_discovery_slot(
            group_index,
            DiscoveryResponsibility::Url,
            &group.url,
            "/reference/url",
            "discovery_provider_field_conflict",
            &mut provenance,
            &mut conflicts,
            &mut diagnostics,
        );
        let mut provider_values = crate::profile_dsl::occurrence::ProviderValues::default();
        provider_values.title = group.title.value();
        push_discovery_slot(
            group_index,
            DiscoveryResponsibility::Title,
            &group.title,
            "/providerValues/title",
            "discovery_provider_field_conflict",
            &mut provenance,
            &mut conflicts,
            &mut diagnostics,
        );
        provider_values.company = group.company.value();
        push_discovery_slot(
            group_index,
            DiscoveryResponsibility::Company,
            &group.company,
            "/providerValues/company",
            "discovery_provider_field_conflict",
            &mut provenance,
            &mut conflicts,
            &mut diagnostics,
        );
        provider_values.locations = group.locations.value().unwrap_or_default();
        push_discovery_slot(
            group_index,
            DiscoveryResponsibility::Locations,
            &group.locations,
            "/providerValues/locations",
            "discovery_provider_field_conflict",
            &mut provenance,
            &mut conflicts,
            &mut diagnostics,
        );
        provider_values.description_text = group.description_text.value();
        push_discovery_slot(
            group_index,
            DiscoveryResponsibility::DescriptionText,
            &group.description_text,
            "/providerValues/descriptionText",
            "discovery_provider_field_conflict",
            &mut provenance,
            &mut conflicts,
            &mut diagnostics,
        );

        let mut hints = BTreeMap::new();
        for (key, slot) in group.hints {
            let responsibility = DiscoveryResponsibility::Hint { key: key.clone() };
            if let Some(value) = slot.value() {
                hints.insert(key.clone(), value);
            }
            push_discovery_slot(
                group_index,
                responsibility,
                &slot,
                &format!("/hints/{}", pointer(&key)),
                "discovery_hint_conflict",
                &mut provenance,
                &mut conflicts,
                &mut diagnostics,
            );
        }
        let mut posting_meta = BTreeMap::new();
        for (key, slot) in group.posting_meta {
            let responsibility = DiscoveryResponsibility::PostingMeta { key: key.clone() };
            if let Some(value) = slot.value() {
                posting_meta.insert(key.clone(), value);
            }
            push_discovery_slot(
                group_index,
                responsibility,
                &slot,
                &format!("/postingMeta/{}", pointer(&key)),
                "discovery_posting_meta_conflict",
                &mut provenance,
                &mut conflicts,
                &mut diagnostics,
            );
        }
        candidates.push(PostingOccurrence {
            identity: group.identity,
            reference: crate::profile_dsl::occurrence::PostingReference {
                provider_url,
                provider_posting_id: group.provider_posting_id,
            },
            provider_values,
            hints,
            posting_meta,
        });
    }
    ReducedDiscovery {
        candidates,
        provenance,
        conflicts,
        rejections,
        diagnostics,
    }
}

#[allow(clippy::too_many_arguments)]
fn push_discovery_slot<T: Eq + Clone>(
    group_index: usize,
    responsibility: DiscoveryResponsibility,
    slot: &Slot<T>,
    suffix: &str,
    conflict_code: &str,
    provenance: &mut Vec<DiscoveryContributionEvidence>,
    conflicts: &mut Vec<DiscoveryContributionEvidence>,
    diagnostics: &mut Diagnostics,
) {
    if let Some(origins) = slot.retained_origins() {
        provenance.push(evidence(group_index, responsibility.clone(), origins));
    } else if let Some(origins) = slot.conflict_origins() {
        conflicts.push(evidence(group_index, responsibility, origins));
        diagnostics.push(reducer_diagnostic(
            conflict_code,
            format!("/discovery/groups/{group_index}{suffix}"),
        ));
    }
}

fn evidence(
    group_index: usize,
    responsibility: DiscoveryResponsibility,
    contributors: Vec<ContributionOrigin>,
) -> DiscoveryContributionEvidence {
    DiscoveryContributionEvidence {
        group_index,
        responsibility,
        contributors,
    }
}

pub(crate) fn reduce_detail(
    expected_identity: &PostingOccurrenceIdentity,
    requested: &RequestedDetailFields,
    contributions: Vec<DetailContribution>,
) -> ReducedDetail {
    let mut slots: BTreeMap<DetailField, Slot<DetailValue>> = DetailField::ALL
        .into_iter()
        .map(|field| (field, Slot::Missing))
        .collect();
    let mut unrequested = BTreeMap::<DetailField, Vec<ContributionOrigin>>::new();
    let mut identity_mismatches = Vec::new();
    for contribution in contributions {
        if &contribution.identity != expected_identity {
            push_origin(&mut identity_mismatches, contribution.origin);
            continue;
        }
        for (field, value) in detail_values(contribution.patch) {
            let Some(value) = value else { continue };
            if !requested.contains(field) {
                push_origin(
                    unrequested.entry(field).or_default(),
                    contribution.origin.clone(),
                );
                continue;
            }
            slots
                .get_mut(&field)
                .expect("canonical Detail field")
                .contribute(value, contribution.origin.clone());
        }
    }

    let mut patch = DetailPatch::default();
    let mut provenance = Vec::new();
    let mut conflicts = Vec::new();
    let mut rejections = Vec::new();
    let mut diagnostics = Vec::new();
    for field in DetailField::ALL {
        let slot = &slots[&field];
        if let Some(origins) = slot.retained_origins() {
            provenance.push(DetailContributionEvidence {
                field,
                contributors: origins,
            });
            set_detail_value(&mut patch, field, slot.value().expect("retained value"));
        } else if let Some(origins) = slot.conflict_origins() {
            conflicts.push(DetailContributionEvidence {
                field,
                contributors: origins,
            });
            diagnostics.push(reducer_diagnostic(
                "detail_field_conflict",
                format!("/detail/fields/{}", field.as_str()),
            ));
        }
        if let Some(contributors) = unrequested.remove(&field) {
            rejections.push(DetailRejection::UnrequestedField {
                field,
                contributors,
            });
            diagnostics.push(reducer_diagnostic(
                "detail_unrequested_field",
                format!("/detail/fields/{}", field.as_str()),
            ));
        }
    }
    if !identity_mismatches.is_empty() {
        rejections.push(DetailRejection::OccurrenceIdentityMismatch {
            contributors: identity_mismatches,
        });
        diagnostics.push(reducer_diagnostic(
            "detail_occurrence_identity_mismatch",
            "/detail/identity",
        ));
    }
    ReducedDetail {
        patch,
        provenance,
        conflicts,
        rejections,
        diagnostics,
    }
}

#[derive(Clone, Eq, PartialEq)]
enum DetailValue {
    Scalar(String),
    Locations(Vec<String>),
}

fn detail_values(patch: DetailPatch) -> [(DetailField, Option<DetailValue>); 4] {
    [
        (
            DetailField::Title,
            patch
                .title
                .filter(|v| !v.is_empty())
                .map(DetailValue::Scalar),
        ),
        (
            DetailField::Company,
            patch
                .company
                .filter(|v| !v.is_empty())
                .map(DetailValue::Scalar),
        ),
        (
            DetailField::Locations,
            patch
                .locations
                .filter(|v| !v.is_empty())
                .map(DetailValue::Locations),
        ),
        (
            DetailField::DescriptionText,
            patch
                .description_text
                .filter(|v| !v.is_empty())
                .map(DetailValue::Scalar),
        ),
    ]
}

fn set_detail_value(patch: &mut DetailPatch, field: DetailField, value: DetailValue) {
    match (field, value) {
        (DetailField::Title, DetailValue::Scalar(value)) => patch.title = Some(value),
        (DetailField::Company, DetailValue::Scalar(value)) => patch.company = Some(value),
        (DetailField::DescriptionText, DetailValue::Scalar(value)) => {
            patch.description_text = Some(value)
        }
        (DetailField::Locations, DetailValue::Locations(value)) => patch.locations = Some(value),
        _ => unreachable!("Detail field value shape is fixed"),
    }
}

fn push_origin(origins: &mut Vec<ContributionOrigin>, origin: ContributionOrigin) {
    if !origins.contains(&origin) {
        origins.push(origin);
        origins.sort_by(|left, right| {
            (
                left.attempt_index,
                left.provider_item_index,
                left.strategy_key.as_str(),
            )
                .cmp(&(
                    right.attempt_index,
                    right.provider_item_index,
                    right.strategy_key.as_str(),
                ))
        });
    }
}

fn reducer_diagnostic(code: &str, path: impl Into<String>) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: code.to_string(),
        message: "A phase contribution was quarantined by deterministic reduction".to_string(),
        severity: DiagnosticSeverity::Error,
        path: path.into(),
        strategy_key: None,
        details: Some(json!({})),
    }
}

fn pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_dsl::occurrence::{DiscoveryHint, PostingReference, ProviderValues};

    fn identity(id: &str) -> PostingOccurrenceIdentity {
        PostingOccurrenceIdentity::ProviderPostingId {
            source_key: "source".into(),
            provider_posting_id: id.into(),
        }
    }

    fn origin(strategy: &str, attempt: usize, item: usize) -> ContributionOrigin {
        ContributionOrigin {
            strategy_key: strategy.into(),
            attempt_index: attempt,
            provider_item_index: Some(item),
        }
    }

    fn occurrence(id: &str, url: &str, title: Option<&str>) -> PostingOccurrence {
        PostingOccurrence {
            identity: identity(id),
            reference: PostingReference {
                provider_url: url.into(),
                provider_posting_id: Some(id.into()),
            },
            provider_values: ProviderValues {
                title: title.map(str::to_owned),
                ..ProviderValues::default()
            },
            hints: BTreeMap::new(),
            posting_meta: BTreeMap::new(),
        }
    }

    #[test]
    fn discovery_merges_equal_and_complementary_values_with_coordinate_order() {
        let mut second = occurrence("1", "https://example.test/1", Some("Engineer"));
        second.provider_values.company = Some("Example".into());
        second.hints.insert(
            "team".into(),
            DiscoveryHint {
                value: "Core".into(),
                hint_use: None,
            },
        );
        let reduced = reduce_discovery(vec![
            DiscoveryContribution {
                occurrence: second,
                origin: origin("later-key", 1, 0),
            },
            DiscoveryContribution {
                occurrence: occurrence("1", "https://example.test/1", Some("Engineer")),
                origin: origin("earlier-key", 0, 1),
            },
        ]);

        assert_eq!(reduced.candidates.len(), 1);
        assert_eq!(
            reduced.candidates[0].provider_values.company.as_deref(),
            Some("Example")
        );
        let title = reduced
            .provenance
            .iter()
            .find(|evidence| evidence.responsibility == DiscoveryResponsibility::Title)
            .expect("title provenance");
        assert_eq!(
            title
                .contributors
                .iter()
                .map(|origin| origin.attempt_index)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert!(reduced.conflicts.is_empty());
    }

    #[test]
    fn discovery_conflicts_quarantine_one_responsibility_and_never_heal() {
        let reduced = reduce_discovery(vec![
            DiscoveryContribution {
                occurrence: occurrence("1", "https://example.test/1", Some("A")),
                origin: origin("a", 0, 0),
            },
            DiscoveryContribution {
                occurrence: occurrence("1", "https://example.test/1", Some("B")),
                origin: origin("b", 1, 0),
            },
            DiscoveryContribution {
                occurrence: occurrence("1", "https://example.test/1", Some("A")),
                origin: origin("c", 2, 0),
            },
        ]);

        assert_eq!(reduced.candidates.len(), 1);
        assert_eq!(reduced.candidates[0].provider_values.title, None);
        assert_eq!(
            reduced.conflicts[0].responsibility,
            DiscoveryResponsibility::Title
        );
        assert_eq!(reduced.conflicts[0].contributors.len(), 3);
        assert_eq!(
            reduced.diagnostics[0].code,
            "discovery_provider_field_conflict"
        );
    }

    #[test]
    fn provider_id_url_conflict_rejects_the_complete_group() {
        let reduced = reduce_discovery(vec![
            DiscoveryContribution {
                occurrence: occurrence("1", "https://example.test/1", Some("A")),
                origin: origin("a", 0, 0),
            },
            DiscoveryContribution {
                occurrence: occurrence("1", "https://example.test/other", Some("A")),
                origin: origin("b", 0, 1),
            },
        ]);

        assert!(reduced.candidates.is_empty());
        assert_eq!(
            reduced.diagnostics[0].code,
            "discovery_required_provider_url_conflict"
        );
        assert!(reduced.conflicts.is_empty());
        assert!(matches!(
            reduced.rejections[0],
            DiscoveryRejection::RequiredProviderUrlConflict {
                ref contributors,
                ..
            } if contributors.len() == 2
        ));
    }

    #[test]
    fn normalized_url_and_keyed_conflicts_are_canonical_ordered_and_escaped() {
        let normalized_identity = PostingOccurrenceIdentity::NormalizedUrl {
            source_key: "source".into(),
            normalized_url: "https://example.test/jobs/1".into(),
        };
        let mut first = occurrence("unused", "HTTPS://EXAMPLE.TEST:443/jobs/1", Some("A"));
        first.identity = normalized_identity.clone();
        first.reference.provider_posting_id = None;
        first.hints.insert(
            "a~/".into(),
            DiscoveryHint {
                value: "one".into(),
                hint_use: None,
            },
        );
        first.posting_meta.insert("z~/".into(), "one".into());
        let mut second = occurrence("unused", "https://example.test/jobs/1", Some("B"));
        second.identity = normalized_identity.clone();
        second.reference.provider_posting_id = None;
        second.hints.insert(
            "a~/".into(),
            DiscoveryHint {
                value: "two".into(),
                hint_use: None,
            },
        );
        second.posting_meta.insert("z~/".into(), "two".into());

        let reduced = reduce_discovery(vec![
            DiscoveryContribution {
                occurrence: first,
                origin: origin("a", 0, 0),
            },
            DiscoveryContribution {
                occurrence: second,
                origin: origin("b", 0, 1),
            },
        ]);

        assert_eq!(
            reduced.candidates[0].reference.provider_url,
            "https://example.test/jobs/1"
        );
        assert_eq!(
            reduced
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.as_str())
                .collect::<Vec<_>>(),
            vec![
                "discovery_provider_field_conflict",
                "discovery_hint_conflict",
                "discovery_posting_meta_conflict"
            ]
        );
        assert_eq!(
            reduced.diagnostics[1].path,
            "/discovery/groups/0/hints/a~0~1"
        );
        assert_eq!(
            reduced.diagnostics[2].path,
            "/discovery/groups/0/postingMeta/z~0~1"
        );
        assert!(reduced.candidates[0].hints.is_empty());
        assert!(reduced.candidates[0].posting_meta.is_empty());
    }

    #[test]
    fn detail_reduces_fields_independently_and_rejects_unrequested_values() {
        let expected = identity("1");
        let requested =
            RequestedDetailFields::new([DetailField::Title, DetailField::Locations]).unwrap();
        let reduced = reduce_detail(
            &expected,
            &requested,
            vec![DetailContribution {
                identity: expected.clone(),
                patch: DetailPatch {
                    title: Some("Engineer".into()),
                    company: Some("must not leak".into()),
                    locations: Some(vec!["Berlin ".into(), "Berlin".into()]),
                    description_text: None,
                },
                origin: origin("detail", 0, 0),
            }],
        );

        assert_eq!(reduced.patch.title.as_deref(), Some("Engineer"));
        assert_eq!(reduced.patch.company, None);
        assert_eq!(
            reduced.patch.locations,
            Some(vec!["Berlin ".into(), "Berlin".into()])
        );
        assert!(matches!(
            reduced.rejections[0],
            DetailRejection::UnrequestedField {
                field: DetailField::Company,
                ..
            }
        ));
        assert_eq!(reduced.diagnostics[0].code, "detail_unrequested_field");
    }

    #[test]
    fn detail_conflict_never_heals_and_foreign_identity_is_not_exposed() {
        let expected = identity("1");
        let requested = RequestedDetailFields::new([DetailField::Title]).unwrap();
        let contributions = vec![
            DetailContribution {
                identity: expected.clone(),
                patch: DetailPatch {
                    title: Some("A".into()),
                    ..DetailPatch::default()
                },
                origin: origin("a", 0, 0),
            },
            DetailContribution {
                identity: expected.clone(),
                patch: DetailPatch {
                    title: Some("B".into()),
                    ..DetailPatch::default()
                },
                origin: origin("b", 1, 0),
            },
            DetailContribution {
                identity: expected.clone(),
                patch: DetailPatch {
                    title: Some("A".into()),
                    ..DetailPatch::default()
                },
                origin: origin("c", 2, 0),
            },
            DetailContribution {
                identity: identity("foreign-secret"),
                patch: DetailPatch {
                    title: Some("secret value".into()),
                    ..DetailPatch::default()
                },
                origin: origin("d", 3, 0),
            },
        ];
        let reduced = reduce_detail(&expected, &requested, contributions);

        assert_eq!(reduced.patch.title, None);
        assert_eq!(reduced.conflicts[0].contributors.len(), 3);
        assert!(matches!(
            reduced.rejections[0],
            DetailRejection::OccurrenceIdentityMismatch { .. }
        ));
        let serialized = serde_json::to_string(&(
            reduced.patch,
            reduced.provenance,
            reduced.conflicts,
            reduced.rejections,
            reduced.diagnostics,
        ))
        .unwrap();
        assert!(!serialized.contains("foreign-secret"));
        assert!(!serialized.contains("secret value"));
        assert!(serialized.contains("detail_occurrence_identity_mismatch"));
        assert!(serialized.contains("detail_field_conflict"));
    }
}
