use super::{distance::distance_km, GeoDbResolver, ResolvedLocation};

#[derive(Clone, Debug, PartialEq)]
pub enum LocationMatchOutcome {
    Applied { matched: bool },
    NotApplied { reason: LocationFilterNotAppliedReason },
}

#[derive(Clone, Debug, PartialEq)]
pub enum LocationFilterNotAppliedReason {
    NoRequestLocations,
    MissingRadiusKm,
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
    if request_locations.is_empty() {
        return Ok(LocationMatchOutcome::NotApplied {
            reason: LocationFilterNotAppliedReason::NoRequestLocations,
        });
    }

    let Some(radius_km) = radius_km else {
        return Ok(LocationMatchOutcome::NotApplied {
            reason: LocationFilterNotAppliedReason::MissingRadiusKm,
        });
    };

    let mut resolved_request_locations = Vec::new();
    for request_location in request_locations {
        let input = request_location.as_ref();
        let resolved = resolver.resolve(input).await?;
        if resolved.is_empty() {
            return Err(format!("Search Request location could not be resolved: {input}"));
        }
        resolved_request_locations.extend(resolved);
    }

    for candidate_location in candidate_locations {
        let resolved_candidate_locations = resolver.resolve(candidate_location.as_ref()).await?;
        if any_pair_within_radius(
            &resolved_request_locations,
            &resolved_candidate_locations,
            radius_km as f64,
        ) {
            return Ok(LocationMatchOutcome::Applied { matched: true });
        }
    }

    Ok(LocationMatchOutcome::Applied { matched: false })
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
