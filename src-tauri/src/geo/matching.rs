use super::{distance::distance_km, GeoDbResolver, ResolvedLocation};

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
        radius_km: f64,
    },
    NotApplied {
        reason: LocationFilterNotAppliedReason,
    },
}

pub async fn prepare_location_filter<RequestLocation>(
    resolver: &GeoDbResolver,
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
    for request_location in request_locations {
        let input = request_location.as_ref();
        let resolved = resolver.resolve(input).await?;
        if resolved.is_empty() {
            return Err(format!(
                "Search Request location could not be resolved: {input}"
            ));
        }
        resolved_request_locations.extend(resolved);
    }

    Ok(PreparedLocationFilter {
        state: PreparedLocationFilterState::Applied {
            request_locations: resolved_request_locations,
            radius_km: radius_km as f64,
        },
    })
}

pub async fn matches_location_filter<RequestLocation, CandidateLocation>(
    resolver: &GeoDbResolver,
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
        resolver: &GeoDbResolver,
        candidate_locations: &[CandidateLocation],
    ) -> Result<LocationMatchOutcome, String>
    where
        CandidateLocation: AsRef<str> + Sync,
    {
        let PreparedLocationFilterState::Applied {
            request_locations,
            radius_km,
        } = &self.state
        else {
            return Ok(LocationMatchOutcome::NotApplied {
                reason: self
                    .not_applied_reason()
                    .expect("not applied state has a reason"),
            });
        };

        for candidate_location in candidate_locations {
            let resolved_candidate_locations =
                resolver.resolve(candidate_location.as_ref()).await?;
            if any_pair_within_radius(request_locations, &resolved_candidate_locations, *radius_km)
            {
                return Ok(LocationMatchOutcome::Applied { matched: true });
            }
        }

        Ok(LocationMatchOutcome::Applied { matched: false })
    }

    pub fn not_applied_reason(&self) -> Option<LocationFilterNotAppliedReason> {
        match &self.state {
            PreparedLocationFilterState::Applied { .. } => None,
            PreparedLocationFilterState::NotApplied { reason } => Some(reason.clone()),
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
