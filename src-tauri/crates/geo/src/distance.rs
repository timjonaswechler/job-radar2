use super::GeoPoint;

const EARTH_RADIUS_KM: f64 = 6_371.0;

pub fn distance_km(from: &GeoPoint, to: &GeoPoint) -> f64 {
    let from_lat = from.latitude.to_radians();
    let to_lat = to.latitude.to_radians();
    let delta_lat = (to.latitude - from.latitude).to_radians();
    let delta_lon = (to.longitude - from.longitude).to_radians();

    let a = (delta_lat / 2.0).sin().powi(2)
        + from_lat.cos() * to_lat.cos() * (delta_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

    EARTH_RADIUS_KM * c
}
