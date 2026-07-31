use serde::{Deserialize, Serialize};

use crate::definition::template::{compile_template, CompiledTemplate, TemplateDescriptor};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserCompileError {
    pub path: String,
    pub message: String,
}

impl BrowserCompileError {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

pub const MAX_BROWSER_FETCH_TIMEOUT_MS: u64 = 120_000;
pub const MAX_BROWSER_WAIT_TIMEOUT_MS: u64 = 60_000;
pub const MAX_BROWSER_INTERACTION_COUNT: u64 = 50;
pub const MAX_BROWSER_WAIT_AFTER_MS: u64 = 60_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrowserShapeKind {
    Tagged,
    ParentOption,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BrowserOptionDescriptor {
    pub key: &'static str,
    pub required: bool,
    pub non_empty: bool,
    pub minimum: Option<u64>,
    pub maximum: Option<u64>,
    pub shape: BrowserShapeKind,
    pub compiled_identity: &'static str,
}

const fn option(
    key: &'static str,
    required: bool,
    non_empty: bool,
    minimum: Option<u64>,
    maximum: Option<u64>,
    compiled_identity: &'static str,
) -> BrowserOptionDescriptor {
    BrowserOptionDescriptor {
        key,
        required,
        non_empty,
        minimum,
        maximum,
        shape: BrowserShapeKind::ParentOption,
        compiled_identity,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BrowserPrimitiveDescriptor {
    pub key: &'static str,
    pub owner: &'static str,
    pub canonical_file: &'static str,
    pub shape: BrowserShapeKind,
    pub compiled_identity: &'static str,
    pub options: &'static [BrowserOptionDescriptor],
}

const FETCH_OPTIONS: &[BrowserOptionDescriptor] = &[
    option(
        "url",
        true,
        false,
        None,
        None,
        "ExecutionPlanFetch::Browser.url",
    ),
    option(
        "timeoutMs",
        true,
        false,
        Some(1),
        Some(MAX_BROWSER_FETCH_TIMEOUT_MS),
        "ExecutionPlanFetch::Browser.timeout_ms",
    ),
    option(
        "waits",
        false,
        false,
        None,
        None,
        "ExecutionPlanFetch::Browser.waits",
    ),
    option(
        "interactions",
        false,
        false,
        None,
        None,
        "ExecutionPlanFetch::Browser.interactions",
    ),
];
const SELECTOR_WAIT_OPTIONS: &[BrowserOptionDescriptor] = &[
    option(
        "selector",
        true,
        true,
        None,
        None,
        "ExecutionPlanBrowserWait::Selector.selector",
    ),
    option(
        "timeoutMs",
        true,
        false,
        Some(1),
        Some(MAX_BROWSER_WAIT_TIMEOUT_MS),
        "ExecutionPlanBrowserWait::Selector.timeout_ms",
    ),
];
const NETWORK_IDLE_OPTIONS: &[BrowserOptionDescriptor] = &[option(
    "timeoutMs",
    true,
    false,
    Some(1),
    Some(MAX_BROWSER_WAIT_TIMEOUT_MS),
    "ExecutionPlanBrowserWait::NetworkIdle.timeout_ms",
)];
const CLICK_IF_VISIBLE_OPTIONS: &[BrowserOptionDescriptor] = &[
    option(
        "selector",
        true,
        true,
        None,
        None,
        "ExecutionPlanBrowserInteraction::ClickIfVisible.selector",
    ),
    option(
        "maxCount",
        true,
        false,
        Some(1),
        Some(MAX_BROWSER_INTERACTION_COUNT),
        "ExecutionPlanBrowserInteraction::ClickIfVisible.max_count",
    ),
    option(
        "waitAfterMs",
        false,
        false,
        Some(0),
        Some(MAX_BROWSER_WAIT_AFTER_MS),
        "ExecutionPlanBrowserInteraction::ClickIfVisible.wait_after_ms",
    ),
];
const CLICK_UNTIL_GONE_OPTIONS: &[BrowserOptionDescriptor] = &[
    option(
        "selector",
        true,
        true,
        None,
        None,
        "ExecutionPlanBrowserInteraction::ClickUntilGone.selector",
    ),
    option(
        "maxCount",
        true,
        false,
        Some(1),
        Some(MAX_BROWSER_INTERACTION_COUNT),
        "ExecutionPlanBrowserInteraction::ClickUntilGone.max_count",
    ),
    option(
        "waitAfterMs",
        false,
        false,
        Some(0),
        Some(MAX_BROWSER_WAIT_AFTER_MS),
        "ExecutionPlanBrowserInteraction::ClickUntilGone.wait_after_ms",
    ),
];

pub const BROWSER_FETCH_DESCRIPTOR: BrowserPrimitiveDescriptor = BrowserPrimitiveDescriptor {
    key: "browser",
    owner: "B03a",
    canonical_file: file!(),
    shape: BrowserShapeKind::Tagged,
    compiled_identity: "ExecutionPlanFetch::Browser",
    options: FETCH_OPTIONS,
};
pub const BROWSER_SELECTOR_WAIT_DESCRIPTOR: BrowserPrimitiveDescriptor =
    BrowserPrimitiveDescriptor {
        key: "selector",
        owner: "B03a",
        canonical_file: file!(),
        shape: BrowserShapeKind::Tagged,
        compiled_identity: "ExecutionPlanBrowserWait::Selector",
        options: SELECTOR_WAIT_OPTIONS,
    };
pub const BROWSER_NETWORK_IDLE_WAIT_DESCRIPTOR: BrowserPrimitiveDescriptor =
    BrowserPrimitiveDescriptor {
        key: "network_idle",
        owner: "B03a",
        canonical_file: file!(),
        shape: BrowserShapeKind::Tagged,
        compiled_identity: "ExecutionPlanBrowserWait::NetworkIdle",
        options: NETWORK_IDLE_OPTIONS,
    };
pub const BROWSER_CLICK_IF_VISIBLE_DESCRIPTOR: BrowserPrimitiveDescriptor =
    BrowserPrimitiveDescriptor {
        key: "click_if_visible",
        owner: "B03a",
        canonical_file: file!(),
        shape: BrowserShapeKind::Tagged,
        compiled_identity: "ExecutionPlanBrowserInteraction::ClickIfVisible",
        options: CLICK_IF_VISIBLE_OPTIONS,
    };
pub const BROWSER_CLICK_UNTIL_GONE_DESCRIPTOR: BrowserPrimitiveDescriptor =
    BrowserPrimitiveDescriptor {
        key: "click_until_gone",
        owner: "B03a",
        canonical_file: file!(),
        shape: BrowserShapeKind::Tagged,
        compiled_identity: "ExecutionPlanBrowserInteraction::ClickUntilGone",
        options: CLICK_UNTIL_GONE_OPTIONS,
    };

const BROWSER_DESCRIPTORS: [BrowserPrimitiveDescriptor; 5] = [
    BROWSER_FETCH_DESCRIPTOR,
    BROWSER_SELECTOR_WAIT_DESCRIPTOR,
    BROWSER_NETWORK_IDLE_WAIT_DESCRIPTOR,
    BROWSER_CLICK_IF_VISIBLE_DESCRIPTOR,
    BROWSER_CLICK_UNTIL_GONE_DESCRIPTOR,
];

pub fn browser_primitive_descriptors() -> &'static [BrowserPrimitiveDescriptor] {
    &BROWSER_DESCRIPTORS
}

pub fn validate_browser_primitive_descriptors(
    descriptors: &[BrowserPrimitiveDescriptor],
) -> Result<(), &'static str> {
    let mut actual = descriptors.to_vec();
    actual.sort_by_key(|descriptor| descriptor.key);
    if actual.windows(2).any(|pair| pair[0].key == pair[1].key) {
        return Err("duplicate Browser Primitive descriptor key");
    }
    let mut expected = BROWSER_DESCRIPTORS.to_vec();
    expected.sort_by_key(|descriptor| descriptor.key);
    if actual != expected {
        return Err("Browser Primitive descriptors conflict with the canonical catalogue");
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum BrowserWait {
    Selector {
        #[serde(deserialize_with = "deserialize_non_empty_selector")]
        selector: String,
        #[serde(
            rename = "timeoutMs",
            deserialize_with = "deserialize_browser_wait_timeout"
        )]
        timeout_ms: u64,
    },
    NetworkIdle {
        #[serde(
            rename = "timeoutMs",
            deserialize_with = "deserialize_browser_wait_timeout"
        )]
        timeout_ms: u64,
    },
}

impl BrowserWait {
    pub const fn descriptor(&self) -> &'static BrowserPrimitiveDescriptor {
        match self {
            Self::Selector { .. } => &BROWSER_SELECTOR_WAIT_DESCRIPTOR,
            Self::NetworkIdle { .. } => &BROWSER_NETWORK_IDLE_WAIT_DESCRIPTOR,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum BrowserInteraction {
    ClickIfVisible {
        #[serde(deserialize_with = "deserialize_non_empty_selector")]
        selector: String,
        #[serde(
            rename = "maxCount",
            deserialize_with = "deserialize_browser_interaction_count"
        )]
        max_count: u64,
        #[serde(
            rename = "waitAfterMs",
            default,
            deserialize_with = "deserialize_browser_wait_after",
            skip_serializing_if = "Option::is_none"
        )]
        wait_after_ms: Option<u64>,
    },
    ClickUntilGone {
        #[serde(deserialize_with = "deserialize_non_empty_selector")]
        selector: String,
        #[serde(
            rename = "maxCount",
            deserialize_with = "deserialize_browser_interaction_count"
        )]
        max_count: u64,
        #[serde(
            rename = "waitAfterMs",
            default,
            deserialize_with = "deserialize_browser_wait_after",
            skip_serializing_if = "Option::is_none"
        )]
        wait_after_ms: Option<u64>,
    },
}

impl BrowserInteraction {
    pub const fn descriptor(&self) -> &'static BrowserPrimitiveDescriptor {
        match self {
            Self::ClickIfVisible { .. } => &BROWSER_CLICK_IF_VISIBLE_DESCRIPTOR,
            Self::ClickUntilGone { .. } => &BROWSER_CLICK_UNTIL_GONE_DESCRIPTOR,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExecutionPlanBrowserWait {
    Selector {
        #[serde(deserialize_with = "deserialize_non_empty_selector")]
        selector: String,
        #[serde(
            rename = "timeoutMs",
            deserialize_with = "deserialize_browser_wait_timeout"
        )]
        timeout_ms: u64,
    },
    NetworkIdle {
        #[serde(
            rename = "timeoutMs",
            deserialize_with = "deserialize_browser_wait_timeout"
        )]
        timeout_ms: u64,
    },
}

impl ExecutionPlanBrowserWait {
    pub const fn descriptor(&self) -> &'static BrowserPrimitiveDescriptor {
        match self {
            Self::Selector { .. } => &BROWSER_SELECTOR_WAIT_DESCRIPTOR,
            Self::NetworkIdle { .. } => &BROWSER_NETWORK_IDLE_WAIT_DESCRIPTOR,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExecutionPlanBrowserInteraction {
    ClickIfVisible {
        #[serde(deserialize_with = "deserialize_non_empty_selector")]
        selector: String,
        #[serde(
            rename = "maxCount",
            deserialize_with = "deserialize_browser_interaction_count"
        )]
        max_count: u64,
        #[serde(
            rename = "waitAfterMs",
            default,
            deserialize_with = "deserialize_browser_wait_after",
            skip_serializing_if = "Option::is_none"
        )]
        wait_after_ms: Option<u64>,
    },
    ClickUntilGone {
        #[serde(deserialize_with = "deserialize_non_empty_selector")]
        selector: String,
        #[serde(
            rename = "maxCount",
            deserialize_with = "deserialize_browser_interaction_count"
        )]
        max_count: u64,
        #[serde(
            rename = "waitAfterMs",
            default,
            deserialize_with = "deserialize_browser_wait_after",
            skip_serializing_if = "Option::is_none"
        )]
        wait_after_ms: Option<u64>,
    },
}

impl ExecutionPlanBrowserInteraction {
    pub const fn descriptor(&self) -> &'static BrowserPrimitiveDescriptor {
        match self {
            Self::ClickIfVisible { .. } => &BROWSER_CLICK_IF_VISIBLE_DESCRIPTOR,
            Self::ClickUntilGone { .. } => &BROWSER_CLICK_UNTIL_GONE_DESCRIPTOR,
        }
    }
}

pub fn compile_browser_fetch(
    url: &str,
    timeout_ms: u64,
    waits: Option<&[BrowserWait]>,
    interactions: Option<&[BrowserInteraction]>,
    path: &str,
    descriptor: &TemplateDescriptor,
) -> Result<
    (
        CompiledTemplate,
        u64,
        Vec<ExecutionPlanBrowserWait>,
        Vec<ExecutionPlanBrowserInteraction>,
    ),
    BrowserCompileError,
> {
    Ok((
        compile_template(url, descriptor)
            .map_err(|error| BrowserCompileError::new(format!("{path}/url"), error.to_string()))?,
        require_bounded(
            timeout_ms,
            MAX_BROWSER_FETCH_TIMEOUT_MS,
            &format!("{path}/timeoutMs"),
        )?,
        waits
            .unwrap_or_default()
            .iter()
            .enumerate()
            .map(|(index, wait)| compile_browser_wait(wait, &format!("{path}/waits/{index}")))
            .collect::<Result<Vec<_>, _>>()?,
        interactions
            .unwrap_or_default()
            .iter()
            .enumerate()
            .map(|(index, interaction)| {
                compile_browser_interaction(interaction, &format!("{path}/interactions/{index}"))
            })
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn compile_browser_wait(
    wait: &BrowserWait,
    path: &str,
) -> Result<ExecutionPlanBrowserWait, BrowserCompileError> {
    match wait {
        BrowserWait::Selector {
            selector,
            timeout_ms,
        } => Ok(ExecutionPlanBrowserWait::Selector {
            selector: require_non_empty(selector, &format!("{path}/selector"))?,
            timeout_ms: require_bounded(
                *timeout_ms,
                MAX_BROWSER_WAIT_TIMEOUT_MS,
                &format!("{path}/timeoutMs"),
            )?,
        }),
        BrowserWait::NetworkIdle { timeout_ms } => Ok(ExecutionPlanBrowserWait::NetworkIdle {
            timeout_ms: require_bounded(
                *timeout_ms,
                MAX_BROWSER_WAIT_TIMEOUT_MS,
                &format!("{path}/timeoutMs"),
            )?,
        }),
    }
}

fn compile_browser_interaction(
    interaction: &BrowserInteraction,
    path: &str,
) -> Result<ExecutionPlanBrowserInteraction, BrowserCompileError> {
    let fields = |selector: &str, max_count: u64, wait_after_ms: Option<u64>| {
        Ok((
            require_non_empty(selector, &format!("{path}/selector"))?,
            require_bounded(
                max_count,
                MAX_BROWSER_INTERACTION_COUNT,
                &format!("{path}/maxCount"),
            )?,
            require_optional_max(
                wait_after_ms,
                MAX_BROWSER_WAIT_AFTER_MS,
                &format!("{path}/waitAfterMs"),
            )?,
        ))
    };
    match interaction {
        BrowserInteraction::ClickIfVisible {
            selector,
            max_count,
            wait_after_ms,
        } => {
            let (selector, max_count, wait_after_ms) =
                fields(selector, *max_count, *wait_after_ms)?;
            Ok(ExecutionPlanBrowserInteraction::ClickIfVisible {
                selector,
                max_count,
                wait_after_ms,
            })
        }
        BrowserInteraction::ClickUntilGone {
            selector,
            max_count,
            wait_after_ms,
        } => {
            let (selector, max_count, wait_after_ms) =
                fields(selector, *max_count, *wait_after_ms)?;
            Ok(ExecutionPlanBrowserInteraction::ClickUntilGone {
                selector,
                max_count,
                wait_after_ms,
            })
        }
    }
}

pub fn deserialize_browser_fetch_timeout<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<u64, D::Error> {
    deserialize_bounded_u64(
        deserializer,
        1,
        MAX_BROWSER_FETCH_TIMEOUT_MS,
        "Browser timeoutMs",
    )
}
pub fn deserialize_browser_wait_timeout<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<u64, D::Error> {
    deserialize_bounded_u64(
        deserializer,
        1,
        MAX_BROWSER_WAIT_TIMEOUT_MS,
        "Browser wait timeoutMs",
    )
}
pub fn deserialize_browser_interaction_count<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<u64, D::Error> {
    deserialize_bounded_u64(
        deserializer,
        1,
        MAX_BROWSER_INTERACTION_COUNT,
        "Browser interaction maxCount",
    )
}
pub fn deserialize_browser_wait_after<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<u64>, D::Error> {
    let value = Option::<u64>::deserialize(deserializer)?;
    if value.is_none_or(|value| value <= MAX_BROWSER_WAIT_AFTER_MS) {
        Ok(value)
    } else {
        Err(serde::de::Error::custom(format!(
            "Browser interaction waitAfterMs must be between 0 and {MAX_BROWSER_WAIT_AFTER_MS}"
        )))
    }
}
pub fn deserialize_non_empty_selector<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<String, D::Error> {
    let value = String::deserialize(deserializer)?;
    if value.trim().is_empty() {
        Err(serde::de::Error::custom(
            "Browser selector must not be empty",
        ))
    } else {
        Ok(value)
    }
}
fn deserialize_bounded_u64<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
    min: u64,
    max: u64,
    label: &str,
) -> Result<u64, D::Error> {
    let value = u64::deserialize(deserializer)?;
    if (min..=max).contains(&value) {
        Ok(value)
    } else {
        Err(serde::de::Error::custom(format!(
            "{label} must be between {min} and {max}"
        )))
    }
}
fn require_bounded(value: u64, max: u64, path: &str) -> Result<u64, BrowserCompileError> {
    if (1..=max).contains(&value) {
        Ok(value)
    } else {
        Err(BrowserCompileError::new(
            path,
            format!("bound must be between 1 and {max}"),
        ))
    }
}
fn require_optional_max(
    value: Option<u64>,
    max: u64,
    path: &str,
) -> Result<Option<u64>, BrowserCompileError> {
    if value.is_none_or(|value| value <= max) {
        Ok(value)
    } else {
        Err(BrowserCompileError::new(
            path,
            format!("bound must not exceed {max}"),
        ))
    }
}
fn require_non_empty(value: &str, path: &str) -> Result<String, BrowserCompileError> {
    if value.trim().is_empty() {
        Err(BrowserCompileError::new(path, "selector must not be empty"))
    } else {
        Ok(value.to_string())
    }
}

fn witness_browser_fetch() {
    fn check(v: &crate::definition::execution_plan::capabilities::ExecutionPlanFetch) {
        if let crate::definition::execution_plan::capabilities::ExecutionPlanFetch::Browser {
            ..
        } = v
        {}
    }
    let _ = check as fn(&crate::definition::execution_plan::capabilities::ExecutionPlanFetch);
}
macro_rules! browser_fetch_field_witness {
    ($name:ident,$field:ident) => {
        fn $name() {
            fn check(v: &crate::definition::execution_plan::capabilities::ExecutionPlanFetch) {
                if let crate::definition::execution_plan::capabilities::ExecutionPlanFetch::Browser { $field, .. } = v {
                    let _ = $field;
                }
            }
            let _ = check as fn(&crate::definition::execution_plan::capabilities::ExecutionPlanFetch);
        }
    };
}
browser_fetch_field_witness!(witness_browser_url, url);
browser_fetch_field_witness!(witness_browser_timeout, timeout_ms);
browser_fetch_field_witness!(witness_browser_waits, waits);
browser_fetch_field_witness!(witness_browser_interactions, interactions);
fn witness_wait_selector() {
    fn check(v: &ExecutionPlanBrowserWait) {
        if let ExecutionPlanBrowserWait::Selector { .. } = v {}
    }
    let _ = check as fn(&ExecutionPlanBrowserWait);
}
fn witness_wait_selector_field() {
    fn check(v: &ExecutionPlanBrowserWait) {
        if let ExecutionPlanBrowserWait::Selector { selector, .. } = v {
            let _ = selector;
        }
    }
    let _ = check as fn(&ExecutionPlanBrowserWait);
}
fn witness_wait_selector_timeout() {
    fn check(v: &ExecutionPlanBrowserWait) {
        if let ExecutionPlanBrowserWait::Selector { timeout_ms, .. } = v {
            let _ = timeout_ms;
        }
    }
    let _ = check as fn(&ExecutionPlanBrowserWait);
}
fn witness_wait_idle() {
    fn check(v: &ExecutionPlanBrowserWait) {
        if let ExecutionPlanBrowserWait::NetworkIdle { .. } = v {}
    }
    let _ = check as fn(&ExecutionPlanBrowserWait);
}
fn witness_wait_idle_timeout() {
    fn check(v: &ExecutionPlanBrowserWait) {
        if let ExecutionPlanBrowserWait::NetworkIdle { timeout_ms } = v {
            let _ = timeout_ms;
        }
    }
    let _ = check as fn(&ExecutionPlanBrowserWait);
}
macro_rules! interaction_witness {
    ($name:ident,$variant:ident) => {
        fn $name() {
            fn check(v: &ExecutionPlanBrowserInteraction) {
                if let ExecutionPlanBrowserInteraction::$variant { .. } = v {}
            }
            let _ = check as fn(&ExecutionPlanBrowserInteraction);
        }
    };
    ($name:ident,$variant:ident,$field:ident) => {
        fn $name() {
            fn check(v: &ExecutionPlanBrowserInteraction) {
                if let ExecutionPlanBrowserInteraction::$variant { $field, .. } = v {
                    let _ = $field;
                }
            }
            let _ = check as fn(&ExecutionPlanBrowserInteraction);
        }
    };
}
interaction_witness!(witness_click_visible, ClickIfVisible);
interaction_witness!(witness_click_visible_selector, ClickIfVisible, selector);
interaction_witness!(witness_click_visible_count, ClickIfVisible, max_count);
interaction_witness!(witness_click_visible_wait, ClickIfVisible, wait_after_ms);
interaction_witness!(witness_click_gone, ClickUntilGone);
interaction_witness!(witness_click_gone_selector, ClickUntilGone, selector);
interaction_witness!(witness_click_gone_count, ClickUntilGone, max_count);
interaction_witness!(witness_click_gone_wait, ClickUntilGone, wait_after_ms);

pub fn completeness_compiled_registrations(
) -> Vec<crate::definition::primitives::completeness::CompiledRegistration> {
    use crate::definition::primitives::completeness::{
        AuthoredShapeKind::{ParentOption, Tagged},
        CompiledRegistration,
        Family::Browser,
        Owner::B03a,
        PrimitiveContext::{Detail, DetectionBrowser, Discovery},
    };
    macro_rules! row {
        ($key:literal,$shape:expr,$identity:literal,$witness:expr) => {
            CompiledRegistration {
                family: Browser,
                key: $key,
                contexts: &[Discovery, Detail, DetectionBrowser],
                owner: B03a,
                canonical_file: "src-tauri/crates/source-profile-dsl/src/definition/primitives/fetch/browser.rs",
                shape: $shape,
                compiled_identity: $identity,
                witness: $witness,
                behavior_bearing: false,
            }
        };
    }
    vec![
        row!(
            "browser",
            Tagged,
            "ExecutionPlanFetch::Browser",
            witness_browser_fetch
        ),
        row!(
            "browser.url",
            ParentOption,
            "ExecutionPlanFetch::Browser.url",
            witness_browser_url
        ),
        row!(
            "browser.timeoutMs",
            ParentOption,
            "ExecutionPlanFetch::Browser.timeout_ms",
            witness_browser_timeout
        ),
        row!(
            "browser.waits",
            ParentOption,
            "ExecutionPlanFetch::Browser.waits",
            witness_browser_waits
        ),
        row!(
            "browser.interactions",
            ParentOption,
            "ExecutionPlanFetch::Browser.interactions",
            witness_browser_interactions
        ),
        row!(
            "selector",
            Tagged,
            "ExecutionPlanBrowserWait::Selector",
            witness_wait_selector
        ),
        row!(
            "selector.selector",
            ParentOption,
            "ExecutionPlanBrowserWait::Selector.selector",
            witness_wait_selector_field
        ),
        row!(
            "selector.timeoutMs",
            ParentOption,
            "ExecutionPlanBrowserWait::Selector.timeout_ms",
            witness_wait_selector_timeout
        ),
        row!(
            "network_idle",
            Tagged,
            "ExecutionPlanBrowserWait::NetworkIdle",
            witness_wait_idle
        ),
        row!(
            "network_idle.timeoutMs",
            ParentOption,
            "ExecutionPlanBrowserWait::NetworkIdle.timeout_ms",
            witness_wait_idle_timeout
        ),
        row!(
            "click_if_visible",
            Tagged,
            "ExecutionPlanBrowserInteraction::ClickIfVisible",
            witness_click_visible
        ),
        row!(
            "click_if_visible.selector",
            ParentOption,
            "ExecutionPlanBrowserInteraction::ClickIfVisible.selector",
            witness_click_visible_selector
        ),
        row!(
            "click_if_visible.maxCount",
            ParentOption,
            "ExecutionPlanBrowserInteraction::ClickIfVisible.max_count",
            witness_click_visible_count
        ),
        row!(
            "click_if_visible.waitAfterMs",
            ParentOption,
            "ExecutionPlanBrowserInteraction::ClickIfVisible.wait_after_ms",
            witness_click_visible_wait
        ),
        row!(
            "click_until_gone",
            Tagged,
            "ExecutionPlanBrowserInteraction::ClickUntilGone",
            witness_click_gone
        ),
        row!(
            "click_until_gone.selector",
            ParentOption,
            "ExecutionPlanBrowserInteraction::ClickUntilGone.selector",
            witness_click_gone_selector
        ),
        row!(
            "click_until_gone.maxCount",
            ParentOption,
            "ExecutionPlanBrowserInteraction::ClickUntilGone.max_count",
            witness_click_gone_count
        ),
        row!(
            "click_until_gone.waitAfterMs",
            ParentOption,
            "ExecutionPlanBrowserInteraction::ClickUntilGone.wait_after_ms",
            witness_click_gone_wait
        ),
    ]
}
