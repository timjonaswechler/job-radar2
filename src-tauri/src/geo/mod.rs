mod db;
mod distance;
mod matching;
mod normalization;

pub use db::GeoDbResolver;
pub use distance::distance_km;
pub use matching::{
    matches_location_filter, prepare_location_filter, LocationFilterNotAppliedReason,
    LocationMatchOutcome, PreparedLocationFilter,
};

#[derive(Clone, Debug, PartialEq)]
pub struct GeoPoint {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedLocation {
    pub input: String,
    pub label: String,
    pub point: GeoPoint,
}
