use std::sync::atomic::{AtomicUsize, Ordering};

use geo::{
    prepare_location_filter, GeoPoint, GeoResolveFuture, GeoResolver, LocationFilterError,
    LocationFilterNotAppliedReason, ResolvedLocation,
};

struct Resolver {
    calls: AtomicUsize,
    outcome: Outcome,
}

enum Outcome {
    Locations(Vec<ResolvedLocation>),
    Failure(&'static str),
}

impl Resolver {
    fn locations(locations: Vec<ResolvedLocation>) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            outcome: Outcome::Locations(locations),
        }
    }

    fn failure(message: &'static str) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            outcome: Outcome::Failure(message),
        }
    }
}

impl GeoResolver for Resolver {
    fn resolve<'a>(&'a self, _input: &'a str) -> GeoResolveFuture<'a> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            match &self.outcome {
                Outcome::Locations(locations) => Ok(locations.clone()),
                Outcome::Failure(message) => Err((*message).to_string()),
            }
        })
    }
}

#[tokio::test]
async fn no_authored_locations_do_not_resolve_and_are_not_applied() {
    let resolver = Resolver::failure("must not be called");
    let filter = prepare_location_filter(&resolver, &[] as &[&str], None)
        .await
        .unwrap();

    assert_eq!(
        filter.not_applied_reason(),
        Some(LocationFilterNotAppliedReason::NoRequestLocations)
    );
    assert_eq!(resolver.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn locations_without_radius_do_not_resolve_and_are_not_applied() {
    let resolver = Resolver::failure("must not be called");

    let filter = prepare_location_filter(&resolver, &["Berlin"], None)
        .await
        .unwrap();

    assert_eq!(
        filter.not_applied_reason(),
        Some(LocationFilterNotAppliedReason::MissingRadiusKm)
    );
    assert_eq!(resolver.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn unresolved_authored_location_is_a_typed_input_error() {
    let resolver = Resolver::locations(vec![]);

    let error = prepare_location_filter(&resolver, &["Atlantis"], Some(25))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        LocationFilterError::UnresolvedRequestLocation {
            input: "Atlantis".to_string(),
        }
    );
}

#[tokio::test]
async fn resolver_failure_is_distinct_from_authored_input_failure() {
    let resolver = Resolver::failure("database unavailable");

    let error = prepare_location_filter(&resolver, &["Berlin"], Some(25))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        LocationFilterError::ResolverFailure {
            message: "database unavailable".to_string(),
        }
    );
}

#[tokio::test]
async fn ambiguous_authored_location_remains_applied_with_ambiguity_evidence() {
    let resolver = Resolver::locations(vec![
        location("Berlin", "Berlin, DE", 52.52, 13.405),
        location("Berlin", "Berlin, US", 44.47, -71.19),
    ]);

    let filter = prepare_location_filter(&resolver, &["Berlin"], Some(25))
        .await
        .unwrap();

    assert_eq!(filter.not_applied_reason(), None);
    assert_eq!(filter.request_ambiguities().len(), 1);
    assert_eq!(filter.request_ambiguities()[0].input, "Berlin");
}

fn location(input: &str, label: &str, latitude: f64, longitude: f64) -> ResolvedLocation {
    ResolvedLocation {
        input: input.to_string(),
        label: label.to_string(),
        point: GeoPoint {
            latitude,
            longitude,
        },
    }
}
