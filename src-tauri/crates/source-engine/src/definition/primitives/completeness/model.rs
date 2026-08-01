use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InventoryLayer {
    Schema,
    Serde,
    Compiled,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Owner {
    P01,
    P02,
    P03,
    P04,
    P05,
    P06a,
    P06bc,
    P07,
    P08,
    P09,
    P10,
    P11,
    B01,
    B03a,
    D02,
    D03,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Template,
    Parse,
    Select,
    Cardinality,
    Transform,
    ValuePlacement,
    Value,
    Predicate,
    Capture,
    Fetch,
    Pagination,
    PaginationLocation,
    Acceptance,
    Browser,
    Detection,
    PhaseLimit,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrimitiveContext {
    Discovery,
    Detail,
    Detection,
    DetectionHttp,
    DetectionBrowser,
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
    DiscoveryCaptureSource,
    DiscoveryFilterOutput,
    DetailCaptureSource,
    DetailMatchFilterOutput,
    DetailMatch,
    DiscoveryWhere,
    DetailWhere,
    DetectionHttpStatus,
    DetectionHttpContains,
    DetectionBrowserContains,
    DetectionHttpRegex,
    DetectionBrowserRegex,
    DiscoveryPhaseLimit,
    DetailPhaseLimit,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthoredShapeKind {
    Tagged,
    Keyed,
    Untagged,
    ParentOption,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaPointerContext {
    /// Exact executable occurrence where traversal starts.
    pub occurrence: String,
    /// Exact definition or property classified by this inventory record.
    pub pointer: String,
    /// Connected, ordered traversal chain from `occurrence` through local `$ref` edges and
    /// structural children to `pointer` (both endpoints included).
    pub chain: Vec<String>,
    pub context: PrimitiveContext,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SchemaShape {
    pub family: Family,
    pub key: String,
    pub contexts: BTreeSet<PrimitiveContext>,
    pub owner: Owner,
    pub canonical_file: String,
    pub compiled_identity: String,
    pub shape: AuthoredShapeKind,
    pub pointers: BTreeSet<String>,
    pub evidence: BTreeSet<SchemaPointerContext>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SerdeShape {
    pub family: Family,
    pub key: String,
    pub contexts: BTreeSet<PrimitiveContext>,
    pub owner: Owner,
    pub canonical_file: String,
    pub compiled_identity: String,
    pub shape: AuthoredShapeKind,
    pub authored_file: &'static str,
}

pub fn missing_witness() {}

#[allow(unpredictable_function_pointer_comparisons)]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CompiledRegistration {
    pub family: Family,
    pub key: &'static str,
    pub contexts: &'static [PrimitiveContext],
    pub owner: Owner,
    pub canonical_file: &'static str,
    pub shape: AuthoredShapeKind,
    pub compiled_identity: &'static str,
    pub witness: fn(),
    pub behavior_bearing: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrimitiveCompletenessRecord {
    pub family: Family,
    pub key: String,
    pub contexts: BTreeSet<PrimitiveContext>,
    pub owner: Owner,
    pub canonical_file: String,
    pub shape: AuthoredShapeKind,
    pub compiled_identity: String,
    pub schema_pointers: BTreeSet<String>,
    pub schema_evidence: BTreeSet<SchemaPointerContext>,
    pub serde_file: String,
}

impl PrimitiveCompletenessRecord {
    pub fn identity(&self) -> (Family, &str, &BTreeSet<PrimitiveContext>) {
        (self.family, &self.key, &self.contexts)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PrimitiveCompletenessInventories {
    pub schema: Vec<SchemaShape>,
    pub serde: Vec<SerdeShape>,
    pub compiled: Vec<CompiledRegistration>,
    /// Descriptive, non-executable policy supplied by canonical owners and the frozen absence
    /// catalogue. The generic validator contains no Primitive- or owner-specific dispatch.
    pub policy: CompletenessPolicy,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompletenessPolicy {
    pub forbidden: BTreeSet<(Family, String)>,
    pub canonical_files: BTreeMap<Owner, BTreeSet<String>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum PrimitiveCompletenessError {
    Duplicate {
        layer: InventoryLayer,
        family: Family,
        key: String,
        contexts: BTreeSet<PrimitiveContext>,
    },
    Missing {
        layer: InventoryLayer,
        family: Family,
        key: String,
        contexts: BTreeSet<PrimitiveContext>,
    },
    SchemaContextEvidenceMismatch {
        family: Family,
        key: String,
        contexts: BTreeSet<PrimitiveContext>,
        evidence_contexts: BTreeSet<PrimitiveContext>,
    },
    MetadataMismatch {
        layer: InventoryLayer,
        family: Family,
        key: String,
        contexts: BTreeSet<PrimitiveContext>,
        field: String,
        expected: String,
        actual: String,
    },
    ShapeMismatch {
        layer: InventoryLayer,
        family: Family,
        key: String,
        contexts: BTreeSet<PrimitiveContext>,
        expected: AuthoredShapeKind,
        actual: AuthoredShapeKind,
    },
    DuplicateOwner {
        family: Family,
        key: String,
        contexts: BTreeSet<PrimitiveContext>,
        owners: BTreeSet<Owner>,
    },
    RemovedKey {
        layer: InventoryLayer,
        family: Family,
        key: String,
    },
    NoncanonicalFile {
        family: Family,
        key: String,
        owner: Owner,
        file: String,
    },
    BehaviorBearingRegistration {
        family: Family,
        key: String,
    },
    MissingCompiledWitness {
        family: Family,
        key: String,
    },
    EmptyCompiledIdentity {
        family: Family,
        key: String,
    },
    DuplicateCompiledIdentity {
        compiled_identity: String,
        registrations: BTreeSet<String>,
    },
}

pub fn contexts(values: &[PrimitiveContext]) -> BTreeSet<PrimitiveContext> {
    values.iter().copied().collect()
}

#[derive(Clone)]
pub struct OwnerClassification {
    pub owner: Owner,
    pub canonical_file: String,
    pub compiled_identity: String,
}

/// Independently frozen owner/file/identity classification. This checked-in catalogue is not
/// assembled from the runtime registration slice, so changing a compiled registration alone makes
/// schema/Serde/compiled parity fail.
pub fn frozen_owner_classification(
    layer: InventoryLayer,
    family: Family,
    key: &str,
    expected_contexts: &BTreeSet<PrimitiveContext>,
) -> OwnerClassification {
    let catalogue = match layer {
        InventoryLayer::Schema => {
            include_str!("../../../../tests/fixtures/primitive_completeness/primitive-schema-owner-catalogue.txt")
        }
        InventoryLayer::Serde => {
            include_str!("../../../../tests/fixtures/primitive_completeness/primitive-serde-owner-catalogue.txt")
        }
        InventoryLayer::Compiled => panic!("compiled registrations carry owner metadata directly"),
    };
    for line in catalogue.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        assert_eq!(fields.len(), 7, "invalid frozen owner classification row");
        if parse_family(fields[0]) == family
            && fields[1] == key
            && parse_contexts(fields[2]) == *expected_contexts
        {
            return OwnerClassification {
                owner: parse_owner(fields[3]),
                canonical_file: fields[4].to_owned(),
                compiled_identity: fields[6].to_owned(),
            };
        }
    }
    panic!("missing frozen owner classification for {family:?}.{key}:{expected_contexts:?}")
}

fn parse_family(value: &str) -> Family {
    use Family::*;
    match value {
        "Template" => Template,
        "Parse" => Parse,
        "Select" => Select,
        "Cardinality" => Cardinality,
        "Transform" => Transform,
        "ValuePlacement" => ValuePlacement,
        "Value" => Value,
        "Predicate" => Predicate,
        "Capture" => Capture,
        "Fetch" => Fetch,
        "Pagination" => Pagination,
        "PaginationLocation" => PaginationLocation,
        "Acceptance" => Acceptance,
        "Browser" => Browser,
        "Detection" => Detection,
        "PhaseLimit" => PhaseLimit,
        _ => panic!("unknown frozen family {value}"),
    }
}
fn parse_owner(value: &str) -> Owner {
    use Owner::*;
    match value {
        "P01" => P01,
        "P02" => P02,
        "P03" => P03,
        "P04" => P04,
        "P05" => P05,
        "P06a" => P06a,
        "P06bc" => P06bc,
        "P07" => P07,
        "P08" => P08,
        "P09" => P09,
        "P10" => P10,
        "P11" => P11,
        "B01" => B01,
        "B03a" => B03a,
        "D02" => D02,
        "D03" => D03,
        _ => panic!("unknown frozen owner {value}"),
    }
}
fn parse_contexts(value: &str) -> BTreeSet<PrimitiveContext> {
    let inner = value
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .expect("frozen contexts use brackets");
    if inner.is_empty() {
        return BTreeSet::new();
    }
    inner.split(", ").map(parse_context).collect()
}
fn parse_context(value: &str) -> PrimitiveContext {
    use PrimitiveContext::*;
    match value {
        "Discovery" => Discovery,
        "Detail" => Detail,
        "Detection" => Detection,
        "DetectionHttp" => DetectionHttp,
        "DetectionBrowser" => DetectionBrowser,
        "DiscoveryValue" => DiscoveryValue,
        "DetailValue" => DetailValue,
        "DiscoveryHttpUrl" => DiscoveryHttpUrl,
        "DiscoveryHttpHeader" => DiscoveryHttpHeader,
        "DiscoveryHttpBody" => DiscoveryHttpBody,
        "DetailHttpUrl" => DetailHttpUrl,
        "DetailHttpHeader" => DetailHttpHeader,
        "DetailHttpBody" => DetailHttpBody,
        "DiscoveryBrowserUrl" => DiscoveryBrowserUrl,
        "DetailBrowserUrl" => DetailBrowserUrl,
        "DetectionHttpUrl" => DetectionHttpUrl,
        "DetectionBrowserUrl" => DetectionBrowserUrl,
        "DetectionProposal" => DetectionProposal,
        "DiscoveryCaptureSource" => DiscoveryCaptureSource,
        "DiscoveryFilterOutput" => DiscoveryFilterOutput,
        "DetailCaptureSource" => DetailCaptureSource,
        "DetailMatchFilterOutput" => DetailMatchFilterOutput,
        "DetailMatch" => DetailMatch,
        "DiscoveryWhere" => DiscoveryWhere,
        "DetailWhere" => DetailWhere,
        "DetectionHttpStatus" => DetectionHttpStatus,
        "DetectionHttpContains" => DetectionHttpContains,
        "DetectionBrowserContains" => DetectionBrowserContains,
        "DetectionHttpRegex" => DetectionHttpRegex,
        "DetectionBrowserRegex" => DetectionBrowserRegex,
        "DiscoveryPhaseLimit" => DiscoveryPhaseLimit,
        "DetailPhaseLimit" => DetailPhaseLimit,
        _ => panic!("unknown frozen context {value}"),
    }
}

pub fn serde_shape(
    family: Family,
    key: impl Into<String>,
    values: &[PrimitiveContext],
    shape: AuthoredShapeKind,
    authored_file: &'static str,
) -> SerdeShape {
    let key = key.into();
    let contexts = contexts(values);
    let metadata = frozen_owner_classification(InventoryLayer::Serde, family, &key, &contexts);
    SerdeShape {
        family,
        key,
        contexts,
        owner: metadata.owner,
        canonical_file: metadata.canonical_file,
        compiled_identity: metadata.compiled_identity,
        shape,
        authored_file,
    }
}

pub fn validate_primitive_completeness(
    inventories: &PrimitiveCompletenessInventories,
) -> Result<Vec<PrimitiveCompletenessRecord>, Vec<PrimitiveCompletenessError>> {
    type Id = (Family, String, BTreeSet<PrimitiveContext>);
    let mut errors = Vec::new();
    let mut schema = BTreeMap::<Id, &SchemaShape>::new();
    let mut serde = BTreeMap::<Id, &SerdeShape>::new();
    let mut compiled = BTreeMap::<Id, &CompiledRegistration>::new();
    for value in &inventories.schema {
        let evidence_contexts = value
            .evidence
            .iter()
            .map(|entry| entry.context)
            .collect::<BTreeSet<_>>();
        if evidence_contexts != value.contexts {
            errors.push(PrimitiveCompletenessError::SchemaContextEvidenceMismatch {
                family: value.family,
                key: value.key.clone(),
                contexts: value.contexts.clone(),
                evidence_contexts,
            });
        }
        let id = (value.family, value.key.clone(), value.contexts.clone());
        if schema.insert(id.clone(), value).is_some() {
            errors.push(duplicate(InventoryLayer::Schema, id));
        }
    }
    for value in &inventories.serde {
        let id = (value.family, value.key.clone(), value.contexts.clone());
        if serde.insert(id.clone(), value).is_some() {
            errors.push(duplicate(InventoryLayer::Serde, id));
        }
    }
    let mut owners = BTreeMap::<Id, BTreeSet<Owner>>::new();
    let mut compiled_identities = BTreeMap::<&str, BTreeSet<String>>::new();
    for value in &inventories.compiled {
        let id = (value.family, value.key.to_owned(), contexts(value.contexts));
        owners.entry(id.clone()).or_default().insert(value.owner);
        if compiled.insert(id.clone(), value).is_some() {
            errors.push(duplicate(InventoryLayer::Compiled, id));
        }
        if std::ptr::fn_addr_eq(value.witness, missing_witness as fn()) {
            errors.push(PrimitiveCompletenessError::MissingCompiledWitness {
                family: value.family,
                key: value.key.into(),
            });
        }
        if value.compiled_identity.is_empty() {
            errors.push(PrimitiveCompletenessError::EmptyCompiledIdentity {
                family: value.family,
                key: value.key.into(),
            });
        } else {
            compiled_identities
                .entry(value.compiled_identity)
                .or_default()
                .insert(format!(
                    "{:?}.{}:{:?}",
                    value.family, value.key, value.contexts
                ));
        }
        if value.behavior_bearing {
            errors.push(PrimitiveCompletenessError::BehaviorBearingRegistration {
                family: value.family,
                key: value.key.into(),
            });
        }
        if !canonical_file(&inventories.policy, value.owner, value.canonical_file) {
            errors.push(PrimitiveCompletenessError::NoncanonicalFile {
                family: value.family,
                key: value.key.into(),
                owner: value.owner,
                file: value.canonical_file.into(),
            });
        }
    }
    for (compiled_identity, registrations) in compiled_identities {
        if registrations.len() != 1 {
            errors.push(PrimitiveCompletenessError::DuplicateCompiledIdentity {
                compiled_identity: compiled_identity.into(),
                registrations,
            });
        }
    }
    for (id, set) in owners {
        if set.len() != 1 {
            errors.push(PrimitiveCompletenessError::DuplicateOwner {
                family: id.0,
                key: id.1,
                contexts: id.2,
                owners: set,
            });
        }
    }
    for (layer, ids) in [
        (
            InventoryLayer::Schema,
            schema.keys().cloned().collect::<BTreeSet<_>>(),
        ),
        (InventoryLayer::Serde, serde.keys().cloned().collect()),
        (InventoryLayer::Compiled, compiled.keys().cloned().collect()),
    ] {
        for id in &ids {
            if inventories.policy.forbidden.contains(&(id.0, id.1.clone())) {
                errors.push(PrimitiveCompletenessError::RemovedKey {
                    layer,
                    family: id.0,
                    key: id.1.clone(),
                });
            }
        }
    }
    let all = schema
        .keys()
        .chain(serde.keys())
        .chain(compiled.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut records = Vec::new();
    for id in all {
        let Some(s) = schema.get(&id) else {
            errors.push(missing(InventoryLayer::Schema, id));
            continue;
        };
        let Some(d) = serde.get(&id) else {
            errors.push(missing(InventoryLayer::Serde, id));
            continue;
        };
        let Some(c) = compiled.get(&id) else {
            errors.push(missing(InventoryLayer::Compiled, id));
            continue;
        };
        for (layer, owner, file, identity) in [
            (
                InventoryLayer::Schema,
                s.owner,
                s.canonical_file.as_str(),
                s.compiled_identity.as_str(),
            ),
            (
                InventoryLayer::Serde,
                d.owner,
                d.canonical_file.as_str(),
                d.compiled_identity.as_str(),
            ),
        ] {
            compare_metadata(
                layer,
                &id,
                "owner",
                format!("{:?}", c.owner),
                format!("{:?}", owner),
                &mut errors,
            );
            compare_metadata(
                layer,
                &id,
                "canonical_file",
                c.canonical_file.to_owned(),
                file.to_owned(),
                &mut errors,
            );
            compare_metadata(
                layer,
                &id,
                "compiled_identity",
                c.compiled_identity.to_owned(),
                identity.to_owned(),
                &mut errors,
            );
        }
        if s.shape != c.shape {
            errors.push(shape_mismatch(
                InventoryLayer::Schema,
                &id,
                c.shape,
                s.shape,
            ));
        }
        if d.shape != c.shape {
            errors.push(shape_mismatch(InventoryLayer::Serde, &id, c.shape, d.shape));
        }
        records.push(PrimitiveCompletenessRecord {
            family: id.0,
            key: id.1,
            contexts: id.2,
            owner: c.owner,
            canonical_file: c.canonical_file.into(),
            shape: c.shape,
            compiled_identity: c.compiled_identity.into(),
            schema_pointers: s.pointers.clone(),
            schema_evidence: s.evidence.clone(),
            serde_file: d.authored_file.into(),
        });
    }
    errors.sort_by_key(|error| serde_json::to_string(error).unwrap());
    records.sort();
    if errors.is_empty() {
        Ok(records)
    } else {
        Err(errors)
    }
}

fn compare_metadata(
    layer: InventoryLayer,
    id: &(Family, String, BTreeSet<PrimitiveContext>),
    field: &str,
    expected: String,
    actual: String,
    errors: &mut Vec<PrimitiveCompletenessError>,
) {
    if expected != actual {
        errors.push(PrimitiveCompletenessError::MetadataMismatch {
            layer,
            family: id.0,
            key: id.1.clone(),
            contexts: id.2.clone(),
            field: field.to_owned(),
            expected,
            actual,
        });
    }
}

fn duplicate(
    layer: InventoryLayer,
    id: (Family, String, BTreeSet<PrimitiveContext>),
) -> PrimitiveCompletenessError {
    PrimitiveCompletenessError::Duplicate {
        layer,
        family: id.0,
        key: id.1,
        contexts: id.2,
    }
}
fn missing(
    layer: InventoryLayer,
    id: (Family, String, BTreeSet<PrimitiveContext>),
) -> PrimitiveCompletenessError {
    PrimitiveCompletenessError::Missing {
        layer,
        family: id.0,
        key: id.1,
        contexts: id.2,
    }
}
fn shape_mismatch(
    layer: InventoryLayer,
    id: &(Family, String, BTreeSet<PrimitiveContext>),
    expected: AuthoredShapeKind,
    actual: AuthoredShapeKind,
) -> PrimitiveCompletenessError {
    PrimitiveCompletenessError::ShapeMismatch {
        layer,
        family: id.0,
        key: id.1.clone(),
        contexts: id.2.clone(),
        expected,
        actual,
    }
}

fn canonical_file(policy: &CompletenessPolicy, owner: Owner, file: &str) -> bool {
    file.starts_with("src-tauri/")
        && !file.contains("..")
        && policy
            .canonical_files
            .get(&owner)
            .is_some_and(|files| files.contains(file))
}
