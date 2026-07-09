use super::{distance::distance_km, GeoResolver, ResolvedLocation};

#[derive(Clone, Debug, PartialEq)]
pub enum LocationMatchOutcome {
    Applied {
        matched: bool,
    },
    NotApplied {
        reason: LocationFilterNotAppliedReason,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum LocationFilterNotAppliedReason {
    NoRequestLocations,
    MissingRadiusKm,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedLocationFilter {
    state: PreparedLocationFilterState,
}

#[derive(Clone, Debug, PartialEq)]
enum PreparedLocationFilterState {
    Applied {
        request_locations: Vec<ResolvedLocation>,
        request_ambiguities: Vec<LocationResolutionAmbiguity>,
        radius_km: f64,
    },
    NotApplied {
        reason: LocationFilterNotAppliedReason,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocationFilterMatchReport {
    pub outcome: LocationMatchOutcome,
    pub unresolved_candidate_locations: Vec<String>,
    pub candidate_ambiguities: Vec<LocationResolutionAmbiguity>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocationResolutionAmbiguity {
    pub input: String,
    pub resolved_labels: Vec<String>,
}

pub async fn prepare_location_filter<RequestLocation>(
    resolver: &dyn GeoResolver,
    request_locations: &[RequestLocation],
    radius_km: Option<i64>,
) -> Result<PreparedLocationFilter, String>
where
    RequestLocation: AsRef<str> + Sync,
{
    if request_locations.is_empty() {
        return Ok(PreparedLocationFilter {
            state: PreparedLocationFilterState::NotApplied {
                reason: LocationFilterNotAppliedReason::NoRequestLocations,
            },
        });
    }

    let Some(radius_km) = radius_km else {
        return Ok(PreparedLocationFilter {
            state: PreparedLocationFilterState::NotApplied {
                reason: LocationFilterNotAppliedReason::MissingRadiusKm,
            },
        });
    };

    let mut resolved_request_locations = Vec::new();
    let mut request_ambiguities = Vec::new();
    for request_location in request_locations {
        let input = request_location.as_ref();
        let resolved = resolver.resolve(input).await?;
        if resolved.is_empty() {
            return Err(format!(
                "Search Request location could not be resolved: {input}"
            ));
        }
        if resolved.len() > 1 {
            request_ambiguities.push(location_ambiguity(input, &resolved));
        }
        resolved_request_locations.extend(resolved);
    }

    Ok(PreparedLocationFilter {
        state: PreparedLocationFilterState::Applied {
            request_locations: resolved_request_locations,
            request_ambiguities,
            radius_km: radius_km as f64,
        },
    })
}

pub async fn matches_location_filter<RequestLocation, CandidateLocation>(
    resolver: &dyn GeoResolver,
    request_locations: &[RequestLocation],
    radius_km: Option<i64>,
    candidate_locations: &[CandidateLocation],
) -> Result<LocationMatchOutcome, String>
where
    RequestLocation: AsRef<str> + Sync,
    CandidateLocation: AsRef<str> + Sync,
{
    let filter = prepare_location_filter(resolver, request_locations, radius_km).await?;
    filter
        .matches_candidate(resolver, candidate_locations)
        .await
}

impl PreparedLocationFilter {
    pub async fn matches_candidate<CandidateLocation>(
        &self,
        resolver: &dyn GeoResolver,
        candidate_locations: &[CandidateLocation],
    ) -> Result<LocationMatchOutcome, String>
    where
        CandidateLocation: AsRef<str> + Sync,
    {
        Ok(self
            .matches_candidate_with_report(resolver, candidate_locations)
            .await?
            .outcome)
    }

    pub async fn matches_candidate_with_report<CandidateLocation>(
        &self,
        resolver: &dyn GeoResolver,
        candidate_locations: &[CandidateLocation],
    ) -> Result<LocationFilterMatchReport, String>
    where
        CandidateLocation: AsRef<str> + Sync,
    {
        let PreparedLocationFilterState::Applied {
            request_locations,
            radius_km,
            ..
        } = &self.state
        else {
            return Ok(LocationFilterMatchReport {
                outcome: LocationMatchOutcome::NotApplied {
                    reason: self
                        .not_applied_reason()
                        .expect("not applied state has a reason"),
                },
                unresolved_candidate_locations: Vec::new(),
                candidate_ambiguities: Vec::new(),
            });
        };

        let mut unresolved_candidate_locations = Vec::new();
        let mut candidate_ambiguities = Vec::new();
        for candidate_location in candidate_locations {
            let input = candidate_location.as_ref();
            let resolved_candidate_locations = resolver.resolve(input).await?;
            if resolved_candidate_locations.is_empty() {
                unresolved_candidate_locations.push(input.to_string());
                continue;
            }
            if resolved_candidate_locations.len() > 1 {
                candidate_ambiguities
                    .push(location_ambiguity(input, &resolved_candidate_locations));
            }
            if any_pair_within_radius(request_locations, &resolved_candidate_locations, *radius_km)
            {
                return Ok(LocationFilterMatchReport {
                    outcome: LocationMatchOutcome::Applied { matched: true },
                    unresolved_candidate_locations,
                    candidate_ambiguities,
                });
            }
        }

        Ok(LocationFilterMatchReport {
            outcome: LocationMatchOutcome::Applied { matched: false },
            unresolved_candidate_locations,
            candidate_ambiguities,
        })
    }

    pub fn not_applied_reason(&self) -> Option<LocationFilterNotAppliedReason> {
        match &self.state {
            PreparedLocationFilterState::Applied { .. } => None,
            PreparedLocationFilterState::NotApplied { reason } => Some(reason.clone()),
        }
    }

    pub fn request_ambiguities(&self) -> &[LocationResolutionAmbiguity] {
        match &self.state {
            PreparedLocationFilterState::Applied {
                request_ambiguities,
                ..
            } => request_ambiguities,
            PreparedLocationFilterState::NotApplied { .. } => &[],
        }
    }
}

fn any_pair_within_radius(
    request_locations: &[ResolvedLocation],
    candidate_locations: &[ResolvedLocation],
    radius_km: f64,
) -> bool {
    request_locations.iter().any(|request_location| {
        candidate_locations.iter().any(|candidate_location| {
            distance_km(&request_location.point, &candidate_location.point) <= radius_km
        })
    })
}

fn location_ambiguity(input: &str, resolved: &[ResolvedLocation]) -> LocationResolutionAmbiguity {
    LocationResolutionAmbiguity {
        input: input.to_string(),
        resolved_labels: resolved
            .iter()
            .map(|location| location.label.clone())
            .collect(),
    }
}
