use geo::{
    prepare_location_filter, GeoResolver, LocationFilterError, LocationFilterMatchReport,
    LocationMatchOutcome, PreparedLocationFilter,
};
use regex::{Regex, RegexBuilder};

use crate::rules::{SearchRule, SearchRuleKind};

#[derive(Clone, Debug)]
pub struct Requirements<'a> {
    include: Vec<CompiledRule>,
    exclude: Vec<CompiledRule>,
    pub(crate) geo: Option<GeoRequirements<'a>>,
    pub(crate) missing_radius: bool,
    pub(crate) geo_failure: Option<LocationFilterError>,
}

#[derive(Clone)]
pub(crate) struct GeoRequirements<'a> {
    pub(crate) filter: PreparedLocationFilter,
    resolver: &'a dyn GeoResolver,
}

impl std::fmt::Debug for GeoRequirements<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GeoRequirements")
            .field("filter", &self.filter)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
struct CompiledRule {
    matcher: CompiledRuleMatcher,
}

#[derive(Clone, Debug)]
enum CompiledRuleMatcher {
    Text(String),
    Regex(Regex),
}

impl<'a> Requirements<'a> {
    /// Compiles matching when no radius is configured. Configured locations without a radius
    /// deliberately do not apply a location filter, preserving established Search Run semantics.
    pub fn compile(
        include: &[SearchRule],
        exclude: &[SearchRule],
        locations: &[String],
        radius_km: Option<i64>,
    ) -> Result<Self, RequirementsCompilationFailure> {
        if radius_km.is_some() {
            return Err(RequirementsCompilationFailure::RadiusRequiresGeoResolver);
        }
        Ok(Self {
            include: compile_rules(include, false)?,
            exclude: compile_rules(exclude, true)?,
            geo: None,
            missing_radius: !locations.is_empty(),
            geo_failure: None,
        })
    }

    /// Compiles explicit-radius matching and retains the prepared Geo filter for Resolution.
    pub async fn compile_with_geo(
        include: &[SearchRule],
        exclude: &[SearchRule],
        locations: &[String],
        radius_km: Option<i64>,
        resolver: &'a dyn GeoResolver,
    ) -> Result<Self, RequirementsCompilationFailure> {
        let (geo, geo_failure) = match prepare_location_filter(resolver, locations, radius_km).await
        {
            Ok(filter) => (Some(GeoRequirements { filter, resolver }), None),
            Err(error @ LocationFilterError::UnresolvedRequestLocation { .. }) => {
                return Err(RequirementsCompilationFailure::Geo(error));
            }
            Err(error @ LocationFilterError::ResolverFailure { .. }) => (None, Some(error)),
        };
        Ok(Self {
            include: compile_rules(include, false)?,
            exclude: compile_rules(exclude, true)?,
            geo,
            missing_radius: false,
            geo_failure,
        })
    }

    pub(crate) fn matches_title(&self, title: &str) -> bool {
        let included = self.include.iter().any(|rule| rule.matches(title));
        included && !self.exclude.iter().any(|rule| rule.matches(title))
    }

    pub(crate) async fn matches_locations(
        &self,
        locations: &[String],
    ) -> Result<(bool, Option<LocationFilterMatchReport>), LocationFilterError> {
        if let Some(geo) = &self.geo {
            let report = geo
                .filter
                .matches_candidate_with_report(geo.resolver, locations)
                .await?;
            let matched = matches!(
                report.outcome,
                LocationMatchOutcome::Applied { matched: true }
                    | LocationMatchOutcome::NotApplied { .. }
            );
            return Ok((matched, Some(report)));
        }
        Ok((true, None))
    }

    pub(crate) fn requires_locations(&self) -> bool {
        self.geo
            .as_ref()
            .is_some_and(|geo| geo.filter.not_applied_reason().is_none())
    }
}

impl CompiledRule {
    fn matches(&self, value: &str) -> bool {
        match &self.matcher {
            CompiledRuleMatcher::Text(needle) => value.to_lowercase().contains(needle),
            CompiledRuleMatcher::Regex(regex) => regex.is_match(value),
        }
    }
}

fn compile_rules(
    rules: &[SearchRule],
    case_insensitive_regex: bool,
) -> Result<Vec<CompiledRule>, RequirementsCompilationFailure> {
    rules
        .iter()
        .map(|rule| {
            let matcher = match rule.kind {
                SearchRuleKind::Text => CompiledRuleMatcher::Text(rule.value.to_lowercase()),
                SearchRuleKind::Regex => CompiledRuleMatcher::Regex(
                    RegexBuilder::new(&rule.value)
                        .case_insensitive(case_insensitive_regex)
                        .build()
                        .map_err(|_| RequirementsCompilationFailure::InvalidRegex)?,
                ),
            };
            Ok(CompiledRule { matcher })
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub enum RequirementsCompilationFailure {
    InvalidRegex,
    RadiusRequiresGeoResolver,
    Geo(LocationFilterError),
}

impl std::fmt::Display for RequirementsCompilationFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Geo(error) => error.fmt(formatter),
            failure => write!(
                formatter,
                "Search Request matching requirements are invalid: {failure:?}"
            ),
        }
    }
}

impl std::error::Error for RequirementsCompilationFailure {}
