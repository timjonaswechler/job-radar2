use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::definition::{
    documents::ParseType,
    primitives::select::{
        compile_json_path, compile_select, resolve_compiled_json_path, CompiledSelect,
        JsonPathSelectPlan, Select, SelectCompileContext, SelectPhase, SelectPlacement,
        SitemapUrlsSelect,
    },
};

pub const MAX_PAGINATION_REQUESTS: u64 = 1_000;
pub const MAX_PAGINATION_ITEMS: u64 = 100_000;
pub const MAX_SITEMAP_DEPTH: u64 = 20;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PaginationParameterLocation {
    Query,
    JsonBody,
}

impl Default for PaginationParameterLocation {
    fn default() -> Self {
        Self::Query
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Pagination {
    Page {
        #[serde(rename = "pageParam")]
        page_param: String,
        #[serde(
            rename = "parameterLocation",
            default,
            skip_serializing_if = "is_query"
        )]
        parameter_location: PaginationParameterLocation,
        #[serde(rename = "firstPage", skip_serializing_if = "Option::is_none")]
        first_page: Option<u64>,
        #[serde(rename = "pageSizeParam", skip_serializing_if = "Option::is_none")]
        page_size_param: Option<String>,
        #[serde(
            rename = "pageSize",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "positive_optional_u64"
        )]
        page_size: Option<u64>,
        #[serde(rename = "totalPath", skip_serializing_if = "Option::is_none")]
        total_path: Option<String>,
        limits: PaginationLimits,
    },
    OffsetLimit {
        #[serde(rename = "offsetParam")]
        offset_param: String,
        #[serde(rename = "limitParam")]
        limit_param: String,
        #[serde(
            rename = "parameterLocation",
            default,
            skip_serializing_if = "is_query"
        )]
        parameter_location: PaginationParameterLocation,
        #[serde(rename = "startOffset", skip_serializing_if = "Option::is_none")]
        start_offset: Option<u64>,
        #[serde(deserialize_with = "positive_u64")]
        limit: u64,
        #[serde(rename = "totalPath", skip_serializing_if = "Option::is_none")]
        total_path: Option<String>,
        limits: PaginationLimits,
    },
    Cursor {
        #[serde(rename = "cursorParam")]
        cursor_param: String,
        #[serde(
            rename = "parameterLocation",
            default,
            skip_serializing_if = "is_query"
        )]
        parameter_location: PaginationParameterLocation,
        #[serde(rename = "nextCursorPath")]
        next_cursor_path: String,
        limits: PaginationLimits,
    },
    Sitemap {
        #[serde(
            rename = "childSitemapSelector",
            skip_serializing_if = "Option::is_none"
        )]
        child_sitemap_selector: Option<Select>,
        #[serde(rename = "postingUrlSelector", skip_serializing_if = "Option::is_none")]
        posting_url_selector: Option<Select>,
        limits: PaginationLimits,
    },
}

fn is_query(location: &PaginationParameterLocation) -> bool {
    *location == PaginationParameterLocation::Query
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PaginationLimits {
    #[serde(deserialize_with = "pagination_requests")]
    pub max_requests: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "pagination_items"
    )]
    pub max_items: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "sitemap_depth"
    )]
    pub max_depth: Option<u64>,
}

fn positive_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = u64::deserialize(deserializer)?;
    if value == 0 {
        Err(serde::de::Error::custom("value must be positive"))
    } else {
        Ok(value)
    }
}
fn positive_optional_u64<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    bounded_optional_u64(deserializer, 1, u64::MAX, "value")
}
fn pagination_requests<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    bounded_u64(deserializer, 1, MAX_PAGINATION_REQUESTS, "maxRequests")
}
fn pagination_items<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    bounded_optional_u64(deserializer, 1, MAX_PAGINATION_ITEMS, "maxItems")
}
fn sitemap_depth<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    bounded_optional_u64(deserializer, 0, MAX_SITEMAP_DEPTH, "maxDepth")
}
fn bounded_u64<'de, D>(deserializer: D, min: u64, max: u64, name: &str) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = u64::deserialize(deserializer)?;
    if (min..=max).contains(&value) {
        Ok(value)
    } else {
        Err(serde::de::Error::custom(format!(
            "{name} must be between {min} and {max}"
        )))
    }
}
fn bounded_optional_u64<'de, D>(
    deserializer: D,
    min: u64,
    max: u64,
    name: &str,
) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<u64>::deserialize(deserializer)?
        .map(|value| {
            if (min..=max).contains(&value) {
                Ok(value)
            } else {
                Err(serde::de::Error::custom(format!(
                    "{name} must be between {min} and {max}"
                )))
            }
        })
        .transpose()
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompiledPagination {
    Page(PagePaginationPlan),
    OffsetLimit(OffsetLimitPaginationPlan),
    Cursor(CursorPaginationPlan),
    Sitemap(SitemapPaginationPlan),
}

impl CompiledPagination {
    pub const fn limits(&self) -> &PaginationLimits {
        match self {
            Self::Page(plan) => &plan.limits,
            Self::OffsetLimit(plan) => &plan.limits,
            Self::Cursor(plan) => &plan.limits,
            Self::Sitemap(plan) => &plan.limits,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PagePaginationPlan {
    pub page_param: String,
    pub parameter_location: PaginationParameterLocation,
    pub first_page: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size_param: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_path: Option<JsonPathSelectPlan>,
    pub limits: PaginationLimits,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OffsetLimitPaginationPlan {
    pub offset_param: String,
    pub limit_param: String,
    pub parameter_location: PaginationParameterLocation,
    pub start_offset: u64,
    pub limit: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_path: Option<JsonPathSelectPlan>,
    pub limits: PaginationLimits,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorPaginationPlan {
    pub cursor_param: String,
    pub parameter_location: PaginationParameterLocation,
    pub next_cursor_path: JsonPathSelectPlan,
    pub limits: PaginationLimits,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SitemapPaginationPlan {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_sitemap_selector: Option<CompiledSelect>,
    pub posting_url_selector: CompiledSelect,
    pub limits: PaginationLimits,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaginationCompileError {
    pub path: String,
    pub message: String,
}

impl PaginationCompileError {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

pub fn compile_pagination_plan(
    pagination: &Pagination,
    document_type: ParseType,
    supports_json_body: bool,
) -> Result<CompiledPagination, PaginationCompileError> {
    match pagination {
        Pagination::Page {
            page_param,
            parameter_location,
            first_page,
            page_size_param,
            page_size,
            total_path,
            limits,
        } => {
            validate_parameter(page_param, "/pageParam")?;
            if page_size.is_some_and(|value| value == 0) {
                return Err(PaginationCompileError::new(
                    "/pageSize",
                    "pageSize must be positive",
                ));
            }
            validate_location(*parameter_location, supports_json_body)?;
            Ok(CompiledPagination::Page(PagePaginationPlan {
                page_param: page_param.clone(),
                parameter_location: *parameter_location,
                first_page: first_page.unwrap_or(1),
                page_size_param: page_size_param.clone(),
                page_size: *page_size,
                total_path: compile_optional_path(
                    total_path.as_deref(),
                    document_type,
                    "/totalPath",
                )?,
                limits: limits.clone(),
            }))
        }
        Pagination::OffsetLimit {
            offset_param,
            limit_param,
            parameter_location,
            start_offset,
            limit,
            total_path,
            limits,
        } => {
            validate_parameter(offset_param, "/offsetParam")?;
            validate_parameter(limit_param, "/limitParam")?;
            validate_location(*parameter_location, supports_json_body)?;
            Ok(CompiledPagination::OffsetLimit(OffsetLimitPaginationPlan {
                offset_param: offset_param.clone(),
                limit_param: limit_param.clone(),
                parameter_location: *parameter_location,
                start_offset: start_offset.unwrap_or(0),
                limit: *limit,
                total_path: compile_optional_path(
                    total_path.as_deref(),
                    document_type,
                    "/totalPath",
                )?,
                limits: limits.clone(),
            }))
        }
        Pagination::Cursor {
            cursor_param,
            parameter_location,
            next_cursor_path,
            limits,
        } => {
            validate_parameter(cursor_param, "/cursorParam")?;
            validate_location(*parameter_location, supports_json_body)?;
            Ok(CompiledPagination::Cursor(CursorPaginationPlan {
                cursor_param: cursor_param.clone(),
                parameter_location: *parameter_location,
                next_cursor_path: compile_path(next_cursor_path, document_type, "/nextCursorPath")?,
                limits: limits.clone(),
            }))
        }
        Pagination::Sitemap {
            child_sitemap_selector,
            posting_url_selector,
            limits,
        } => {
            let compile = |select: &Select, placement| {
                compile_select(
                    select,
                    SelectCompileContext {
                        document_type,
                        phase: SelectPhase::Discovery,
                        placement,
                    },
                )
                .map_err(|error| PaginationCompileError::new("/selector", error.message))
            };
            Ok(CompiledPagination::Sitemap(SitemapPaginationPlan {
                child_sitemap_selector: child_sitemap_selector
                    .as_ref()
                    .map(|value| compile(value, SelectPlacement::SitemapChild))
                    .transpose()?,
                posting_url_selector: compile(
                    posting_url_selector
                        .as_ref()
                        .unwrap_or(&Select::SitemapUrls(SitemapUrlsSelect::default())),
                    SelectPlacement::SitemapPosting,
                )?,
                limits: limits.clone(),
            }))
        }
    }
}

fn validate_parameter(value: &str, path: &str) -> Result<(), PaginationCompileError> {
    if value.trim().is_empty() {
        Err(PaginationCompileError::new(
            path,
            "pagination parameter name must not be empty",
        ))
    } else {
        Ok(())
    }
}
fn validate_location(
    location: PaginationParameterLocation,
    supports_json_body: bool,
) -> Result<(), PaginationCompileError> {
    if location == PaginationParameterLocation::JsonBody && !supports_json_body {
        Err(PaginationCompileError::new(
            "/parameterLocation",
            "json_body pagination requires an HTTP POST JSON-body Fetch",
        ))
    } else {
        Ok(())
    }
}
fn compile_optional_path(
    value: Option<&str>,
    document_type: ParseType,
    path: &str,
) -> Result<Option<JsonPathSelectPlan>, PaginationCompileError> {
    value
        .map(|value| compile_path(value, document_type, path))
        .transpose()
}
fn compile_path(
    value: &str,
    document_type: ParseType,
    path: &str,
) -> Result<JsonPathSelectPlan, PaginationCompileError> {
    if document_type != ParseType::Json {
        return Err(PaginationCompileError::new(
            path,
            "pagination response paths require JSON Parse",
        ));
    }
    compile_json_path(value).map_err(|message| PaginationCompileError::new(path, message))
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PaginationOverlay {
    pub query: Vec<(String, String)>,
    pub json_body: Vec<(String, String)>,
}
impl PaginationOverlay {
    pub fn from_pairs(
        location: PaginationParameterLocation,
        pairs: impl IntoIterator<Item = (String, String)>,
    ) -> Self {
        let values = pairs.into_iter().collect();
        match location {
            PaginationParameterLocation::Query => Self {
                query: values,
                json_body: Vec::new(),
            },
            PaginationParameterLocation::JsonBody => Self {
                query: Vec::new(),
                json_body: values,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PagePaginationState {
    next_page: u64,
}
impl PagePaginationPlan {
    pub fn initial_state(&self) -> PagePaginationState {
        PagePaginationState {
            next_page: self.first_page,
        }
    }
    pub fn overlay(&self, state: &PagePaginationState) -> PaginationOverlay {
        let mut pairs = vec![(self.page_param.clone(), state.next_page.to_string())];
        if let (Some(name), Some(size)) = (&self.page_size_param, self.page_size) {
            pairs.push((name.clone(), size.to_string()));
        }
        PaginationOverlay::from_pairs(self.parameter_location, pairs)
    }
    pub fn advance(&self, state: &mut PagePaginationState) -> bool {
        match state.next_page.checked_add(1) {
            Some(next) => {
                state.next_page = next;
                true
            }
            None => false,
        }
    }
    pub fn total(&self, document: &Value) -> Option<u64> {
        self.total_path
            .as_ref()
            .and_then(|path| scalar_u64(resolve_compiled_json_path(path, document)?))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OffsetLimitPaginationState {
    next_offset: u64,
}
impl OffsetLimitPaginationPlan {
    pub fn initial_state(&self) -> OffsetLimitPaginationState {
        OffsetLimitPaginationState {
            next_offset: self.start_offset,
        }
    }
    pub fn current_offset(&self, state: &OffsetLimitPaginationState) -> u64 {
        state.next_offset
    }
    pub fn overlay(&self, state: &OffsetLimitPaginationState) -> PaginationOverlay {
        PaginationOverlay::from_pairs(
            self.parameter_location,
            [
                (self.offset_param.clone(), state.next_offset.to_string()),
                (self.limit_param.clone(), self.limit.to_string()),
            ],
        )
    }
    pub fn advance(&self, state: &mut OffsetLimitPaginationState) -> bool {
        match state.next_offset.checked_add(self.limit) {
            Some(next) => {
                state.next_offset = next;
                true
            }
            None => false,
        }
    }
    pub fn total(&self, document: &Value) -> Option<u64> {
        self.total_path
            .as_ref()
            .and_then(|path| scalar_u64(resolve_compiled_json_path(path, document)?))
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CursorPaginationState {
    cursor: Option<String>,
    seen: HashSet<String>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CursorAdvance {
    Advanced,
    MissingOrEmpty,
    Repeated,
}
impl CursorPaginationPlan {
    pub fn initial_state(&self) -> CursorPaginationState {
        CursorPaginationState::default()
    }
    pub fn overlay(&self, state: &CursorPaginationState) -> PaginationOverlay {
        PaginationOverlay::from_pairs(
            self.parameter_location,
            state
                .cursor
                .iter()
                .map(|value| (self.cursor_param.clone(), value.clone())),
        )
    }
    pub fn next_cursor(&self, document: &Value) -> Option<String> {
        resolve_compiled_json_path(&self.next_cursor_path, document).and_then(scalar_string)
    }
    pub fn advance(
        &self,
        state: &mut CursorPaginationState,
        next: Option<String>,
    ) -> CursorAdvance {
        let Some(next) = next else {
            return CursorAdvance::MissingOrEmpty;
        };
        if !state.seen.insert(next.clone()) {
            return CursorAdvance::Repeated;
        }
        state.cursor = Some(next);
        CursorAdvance::Advanced
    }
}
fn scalar_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Number(value) => value.as_u64(),
        Value::String(value) => value.parse().ok(),
        _ => None,
    }
}
fn scalar_string(value: &Value) -> Option<String> {
    let value = match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        _ => return None,
    };
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PaginationDescriptor {
    pub key: &'static str,
    pub options: &'static [&'static str],
}

const COMMON_LIMIT_OPTIONS: [&str; 3] =
    ["limits.maxRequests", "limits.maxItems", "limits.maxDepth"];
const PAGE_OPTIONS: [&str; 11] = [
    "type",
    "pageParam",
    "parameterLocation",
    "firstPage",
    "pageSizeParam",
    "pageSize",
    "totalPath",
    "limits",
    "limits.maxRequests",
    "limits.maxItems",
    "limits.maxDepth",
];
const OFFSET_LIMIT_OPTIONS: [&str; 11] = [
    "type",
    "offsetParam",
    "limitParam",
    "parameterLocation",
    "startOffset",
    "limit",
    "totalPath",
    "limits",
    "limits.maxRequests",
    "limits.maxItems",
    "limits.maxDepth",
];
const CURSOR_OPTIONS: [&str; 8] = [
    "type",
    "cursorParam",
    "parameterLocation",
    "nextCursorPath",
    "limits",
    "limits.maxRequests",
    "limits.maxItems",
    "limits.maxDepth",
];
const SITEMAP_OPTIONS: [&str; 7] = [
    "type",
    "childSitemapSelector",
    "postingUrlSelector",
    "limits",
    "limits.maxRequests",
    "limits.maxItems",
    "limits.maxDepth",
];
const DESCRIPTORS: [PaginationDescriptor; 4] = [
    PaginationDescriptor {
        key: "page",
        options: &PAGE_OPTIONS,
    },
    PaginationDescriptor {
        key: "offset_limit",
        options: &OFFSET_LIMIT_OPTIONS,
    },
    PaginationDescriptor {
        key: "cursor",
        options: &CURSOR_OPTIONS,
    },
    PaginationDescriptor {
        key: "sitemap",
        options: &SITEMAP_OPTIONS,
    },
];
pub fn pagination_descriptors() -> &'static [PaginationDescriptor] {
    &DESCRIPTORS
}
pub fn pagination_limit_options() -> &'static [&'static str] {
    &COMMON_LIMIT_OPTIONS
}
pub fn pagination_parameter_locations() -> &'static [&'static str] {
    &["query", "json_body"]
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaginationInventory {
    pub variants: Vec<String>,
    pub options: BTreeMap<String, Vec<String>>,
    pub parameter_locations: Vec<String>,
    pub fragment_options: Vec<String>,
}

impl PaginationInventory {
    pub fn from_descriptors(fragment_options: Vec<String>) -> Self {
        Self {
            variants: DESCRIPTORS
                .iter()
                .map(|descriptor| descriptor.key.to_string())
                .collect(),
            options: DESCRIPTORS
                .iter()
                .map(|descriptor| {
                    (
                        descriptor.key.to_string(),
                        descriptor
                            .options
                            .iter()
                            .map(|option| option.to_string())
                            .collect(),
                    )
                })
                .collect(),
            parameter_locations: pagination_parameter_locations()
                .iter()
                .map(|key| key.to_string())
                .collect(),
            fragment_options,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PaginationRegistryError {
    Duplicate {
        layer: &'static str,
        keys: Vec<String>,
    },
    Missing {
        layer: &'static str,
        keys: Vec<String>,
    },
    Extra {
        layer: &'static str,
        keys: Vec<String>,
    },
}

pub fn validate_pagination_inventories(
    schema: &PaginationInventory,
    serde: &PaginationInventory,
    fragment: &PaginationInventory,
    registration: &PaginationInventory,
) -> Result<(), PaginationRegistryError> {
    for (layer, inventory) in [
        ("schema", schema),
        ("serde", serde),
        ("fragment", fragment),
        ("registration", registration),
    ] {
        let identities = inventory_identities(inventory);
        let duplicates = duplicates(&identities);
        if !duplicates.is_empty() {
            return Err(PaginationRegistryError::Duplicate {
                layer,
                keys: duplicates,
            });
        }
    }
    let expected = inventory_identities(schema)
        .into_iter()
        .collect::<BTreeSet<_>>();
    for (layer, inventory) in [
        ("serde", serde),
        ("fragment", fragment),
        ("registration", registration),
    ] {
        let actual = inventory_identities(inventory)
            .into_iter()
            .collect::<BTreeSet<_>>();
        let missing = expected.difference(&actual).cloned().collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(PaginationRegistryError::Missing {
                layer,
                keys: missing,
            });
        }
        let extra = actual.difference(&expected).cloned().collect::<Vec<_>>();
        if !extra.is_empty() {
            return Err(PaginationRegistryError::Extra { layer, keys: extra });
        }
    }
    Ok(())
}

fn inventory_identities(inventory: &PaginationInventory) -> Vec<String> {
    let mut identities = inventory
        .variants
        .iter()
        .map(|key| format!("variant:{key}"))
        .collect::<Vec<_>>();
    for (variant, options) in &inventory.options {
        identities.extend(
            options
                .iter()
                .map(|option| format!("option:{variant}.{option}")),
        );
    }
    identities.extend(
        inventory
            .parameter_locations
            .iter()
            .map(|key| format!("location:{key}")),
    );
    identities.extend(
        inventory
            .fragment_options
            .iter()
            .map(|key| format!("fragment:{key}")),
    );
    identities
}
fn duplicates(values: &[String]) -> Vec<String> {
    let mut counts = BTreeMap::new();
    for value in values {
        *counts.entry(value.clone()).or_insert(0usize) += 1;
    }
    counts
        .into_iter()
        .filter_map(|(key, count)| (count > 1).then_some(key))
        .collect()
}

pub fn completeness_serde_shapes() -> Vec<crate::definition::primitives::completeness::SerdeShape> {
    use crate::definition::primitives::completeness::{
        serde_shape,
        AuthoredShapeKind::{ParentOption, Tagged},
        Family::{Pagination, PaginationLocation},
        PrimitiveContext::Discovery,
    };
    let mut out = Vec::new();
    for descriptor in pagination_descriptors() {
        out.push(serde_shape(
            Pagination,
            descriptor.key,
            &[Discovery],
            Tagged,
            "src-tauri/crates/source-profile-dsl/src/definition/documents/pagination.rs",
        ));
        for option in descriptor.options.iter().copied().filter(|v| *v != "type") {
            out.push(serde_shape(
                Pagination,
                format!("{}.{}", descriptor.key, option),
                &[Discovery],
                ParentOption,
                "src-tauri/crates/source-profile-dsl/src/definition/documents/pagination.rs",
            ));
        }
    }
    for key in pagination_parameter_locations() {
        out.push(serde_shape(
            PaginationLocation,
            *key,
            &[Discovery],
            Tagged,
            "src-tauri/crates/source-profile-dsl/src/definition/documents/pagination.rs",
        ));
    }
    out
}

macro_rules! pagination_variant_witness {
    ($name:ident,$variant:ident) => {
        fn $name() {
            fn check(v: &CompiledPagination) {
                if let CompiledPagination::$variant(plan) = v {
                    let _ = plan;
                }
            }
            let _ = check as fn(&CompiledPagination);
        }
    };
}
macro_rules! pagination_field_witness {
    ($name:ident,$variant:ident,$field:ident) => {
        fn $name() {
            fn check(v: &CompiledPagination) {
                if let CompiledPagination::$variant(plan) = v {
                    let _ = &plan.$field;
                }
            }
            let _ = check as fn(&CompiledPagination);
        }
    };
}
macro_rules! pagination_limit_witness {
    ($name:ident,$variant:ident,$field:ident) => {
        fn $name() {
            fn check(v: &CompiledPagination) {
                if let CompiledPagination::$variant(plan) = v {
                    let _ = &plan.limits.$field;
                }
            }
            let _ = check as fn(&CompiledPagination);
        }
    };
}
pagination_variant_witness!(witness_page, Page);
pagination_field_witness!(witness_page_param, Page, page_param);
pagination_field_witness!(witness_page_location, Page, parameter_location);
pagination_field_witness!(witness_page_first, Page, first_page);
pagination_field_witness!(witness_page_size_param, Page, page_size_param);
pagination_field_witness!(witness_page_size, Page, page_size);
pagination_field_witness!(witness_page_total, Page, total_path);
pagination_field_witness!(witness_page_limits, Page, limits);
pagination_limit_witness!(witness_page_requests, Page, max_requests);
pagination_limit_witness!(witness_page_items, Page, max_items);
pagination_limit_witness!(witness_page_depth, Page, max_depth);
pagination_variant_witness!(witness_offset, OffsetLimit);
pagination_field_witness!(witness_offset_param, OffsetLimit, offset_param);
pagination_field_witness!(witness_offset_limit_param, OffsetLimit, limit_param);
pagination_field_witness!(witness_offset_location, OffsetLimit, parameter_location);
pagination_field_witness!(witness_offset_start, OffsetLimit, start_offset);
pagination_field_witness!(witness_offset_limit, OffsetLimit, limit);
pagination_field_witness!(witness_offset_total, OffsetLimit, total_path);
pagination_field_witness!(witness_offset_limits, OffsetLimit, limits);
pagination_limit_witness!(witness_offset_requests, OffsetLimit, max_requests);
pagination_limit_witness!(witness_offset_items, OffsetLimit, max_items);
pagination_limit_witness!(witness_offset_depth, OffsetLimit, max_depth);
pagination_variant_witness!(witness_cursor, Cursor);
pagination_field_witness!(witness_cursor_param, Cursor, cursor_param);
pagination_field_witness!(witness_cursor_location, Cursor, parameter_location);
pagination_field_witness!(witness_cursor_next, Cursor, next_cursor_path);
pagination_field_witness!(witness_cursor_limits, Cursor, limits);
pagination_limit_witness!(witness_cursor_requests, Cursor, max_requests);
pagination_limit_witness!(witness_cursor_items, Cursor, max_items);
pagination_limit_witness!(witness_cursor_depth, Cursor, max_depth);
pagination_variant_witness!(witness_sitemap, Sitemap);
pagination_field_witness!(witness_sitemap_child, Sitemap, child_sitemap_selector);
pagination_field_witness!(witness_sitemap_posting, Sitemap, posting_url_selector);
pagination_field_witness!(witness_sitemap_limits, Sitemap, limits);
pagination_limit_witness!(witness_sitemap_requests, Sitemap, max_requests);
pagination_limit_witness!(witness_sitemap_items, Sitemap, max_items);
pagination_limit_witness!(witness_sitemap_depth, Sitemap, max_depth);
fn witness_location_query() {
    fn check(v: &PaginationParameterLocation) {
        if let PaginationParameterLocation::Query = v {}
    }
    let _ = check as fn(&PaginationParameterLocation);
}
fn witness_location_json() {
    fn check(v: &PaginationParameterLocation) {
        if let PaginationParameterLocation::JsonBody = v {}
    }
    let _ = check as fn(&PaginationParameterLocation);
}

pub fn completeness_compiled_registrations(
) -> Vec<crate::definition::primitives::completeness::CompiledRegistration> {
    use crate::definition::primitives::completeness::{
        AuthoredShapeKind::{ParentOption, Tagged},
        CompiledRegistration,
        Family::{Pagination, PaginationLocation},
        Owner::P10,
        PrimitiveContext::Discovery,
    };
    macro_rules! row {
        ($key:literal,$shape:expr,$identity:literal,$witness:expr) => {
            CompiledRegistration {
                family: Pagination,
                key: $key,
                contexts: &[Discovery],
                owner: P10,
                canonical_file: "src-tauri/crates/source-profile-dsl/src/definition/primitives/pagination/mod.rs",
                shape: $shape,
                compiled_identity: $identity,
                witness: $witness,
                behavior_bearing: false,
            }
        };
    }
    vec![
        row!("page", Tagged, "CompiledPagination::Page", witness_page),
        row!(
            "page.pageParam",
            ParentOption,
            "CompiledPagination::Page.page_param",
            witness_page_param
        ),
        row!(
            "page.parameterLocation",
            ParentOption,
            "CompiledPagination::Page.parameter_location",
            witness_page_location
        ),
        row!(
            "page.firstPage",
            ParentOption,
            "CompiledPagination::Page.first_page",
            witness_page_first
        ),
        row!(
            "page.pageSizeParam",
            ParentOption,
            "CompiledPagination::Page.page_size_param",
            witness_page_size_param
        ),
        row!(
            "page.pageSize",
            ParentOption,
            "CompiledPagination::Page.page_size",
            witness_page_size
        ),
        row!(
            "page.totalPath",
            ParentOption,
            "CompiledPagination::Page.total_path",
            witness_page_total
        ),
        row!(
            "page.limits",
            ParentOption,
            "CompiledPagination::Page.limits",
            witness_page_limits
        ),
        row!(
            "page.limits.maxRequests",
            ParentOption,
            "CompiledPagination::Page.limits.max_requests",
            witness_page_requests
        ),
        row!(
            "page.limits.maxItems",
            ParentOption,
            "CompiledPagination::Page.limits.max_items",
            witness_page_items
        ),
        row!(
            "page.limits.maxDepth",
            ParentOption,
            "CompiledPagination::Page.limits.max_depth",
            witness_page_depth
        ),
        row!(
            "offset_limit",
            Tagged,
            "CompiledPagination::OffsetLimit",
            witness_offset
        ),
        row!(
            "offset_limit.offsetParam",
            ParentOption,
            "CompiledPagination::OffsetLimit.offset_param",
            witness_offset_param
        ),
        row!(
            "offset_limit.limitParam",
            ParentOption,
            "CompiledPagination::OffsetLimit.limit_param",
            witness_offset_limit_param
        ),
        row!(
            "offset_limit.parameterLocation",
            ParentOption,
            "CompiledPagination::OffsetLimit.parameter_location",
            witness_offset_location
        ),
        row!(
            "offset_limit.startOffset",
            ParentOption,
            "CompiledPagination::OffsetLimit.start_offset",
            witness_offset_start
        ),
        row!(
            "offset_limit.limit",
            ParentOption,
            "CompiledPagination::OffsetLimit.limit",
            witness_offset_limit
        ),
        row!(
            "offset_limit.totalPath",
            ParentOption,
            "CompiledPagination::OffsetLimit.total_path",
            witness_offset_total
        ),
        row!(
            "offset_limit.limits",
            ParentOption,
            "CompiledPagination::OffsetLimit.limits",
            witness_offset_limits
        ),
        row!(
            "offset_limit.limits.maxRequests",
            ParentOption,
            "CompiledPagination::OffsetLimit.limits.max_requests",
            witness_offset_requests
        ),
        row!(
            "offset_limit.limits.maxItems",
            ParentOption,
            "CompiledPagination::OffsetLimit.limits.max_items",
            witness_offset_items
        ),
        row!(
            "offset_limit.limits.maxDepth",
            ParentOption,
            "CompiledPagination::OffsetLimit.limits.max_depth",
            witness_offset_depth
        ),
        row!(
            "cursor",
            Tagged,
            "CompiledPagination::Cursor",
            witness_cursor
        ),
        row!(
            "cursor.cursorParam",
            ParentOption,
            "CompiledPagination::Cursor.cursor_param",
            witness_cursor_param
        ),
        row!(
            "cursor.parameterLocation",
            ParentOption,
            "CompiledPagination::Cursor.parameter_location",
            witness_cursor_location
        ),
        row!(
            "cursor.nextCursorPath",
            ParentOption,
            "CompiledPagination::Cursor.next_cursor_path",
            witness_cursor_next
        ),
        row!(
            "cursor.limits",
            ParentOption,
            "CompiledPagination::Cursor.limits",
            witness_cursor_limits
        ),
        row!(
            "cursor.limits.maxRequests",
            ParentOption,
            "CompiledPagination::Cursor.limits.max_requests",
            witness_cursor_requests
        ),
        row!(
            "cursor.limits.maxItems",
            ParentOption,
            "CompiledPagination::Cursor.limits.max_items",
            witness_cursor_items
        ),
        row!(
            "cursor.limits.maxDepth",
            ParentOption,
            "CompiledPagination::Cursor.limits.max_depth",
            witness_cursor_depth
        ),
        row!(
            "sitemap",
            Tagged,
            "CompiledPagination::Sitemap",
            witness_sitemap
        ),
        row!(
            "sitemap.childSitemapSelector",
            ParentOption,
            "CompiledPagination::Sitemap.child_sitemap_selector",
            witness_sitemap_child
        ),
        row!(
            "sitemap.postingUrlSelector",
            ParentOption,
            "CompiledPagination::Sitemap.posting_url_selector",
            witness_sitemap_posting
        ),
        row!(
            "sitemap.limits",
            ParentOption,
            "CompiledPagination::Sitemap.limits",
            witness_sitemap_limits
        ),
        row!(
            "sitemap.limits.maxRequests",
            ParentOption,
            "CompiledPagination::Sitemap.limits.max_requests",
            witness_sitemap_requests
        ),
        row!(
            "sitemap.limits.maxItems",
            ParentOption,
            "CompiledPagination::Sitemap.limits.max_items",
            witness_sitemap_items
        ),
        row!(
            "sitemap.limits.maxDepth",
            ParentOption,
            "CompiledPagination::Sitemap.limits.max_depth",
            witness_sitemap_depth
        ),
        CompiledRegistration {
            family: PaginationLocation,
            key: "query",
            contexts: &[Discovery],
            owner: P10,
            canonical_file:
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/pagination/mod.rs",
            shape: Tagged,
            compiled_identity: "PaginationParameterLocation::Query",
            witness: witness_location_query,
            behavior_bearing: false,
        },
        CompiledRegistration {
            family: PaginationLocation,
            key: "json_body",
            contexts: &[Discovery],
            owner: P10,
            canonical_file:
                "src-tauri/crates/source-profile-dsl/src/definition/primitives/pagination/mod.rs",
            shape: Tagged,
            compiled_identity: "PaginationParameterLocation::JsonBody",
            witness: witness_location_json,
            behavior_bearing: false,
        },
    ]
}
