use std::{collections::BTreeMap, path::Path};

use job_radar_lib::{
    matches_location_filter, prepare_location_filter, GeoDbResolver, GeoPoint, GeoResolveFuture,
    GeoResolver, LocationMatchOutcome, ResolvedLocation,
};

#[test]
fn resolves_mainz_from_bundled_geo_database() {
    tauri::async_runtime::block_on(async {
        let db_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/geo_loc.sqlite");
        let resolver = GeoDbResolver::connect(&db_path).await.unwrap();

        let resolved = resolver.resolve("Mainz").await.unwrap();

        let mainz = resolved
            .iter()
            .find(|location| location.label == "Mainz")
            .expect("Mainz should resolve through the bundled geo database");
        assert!((mainz.point.latitude - 49.9926).abs() < 0.01);
        assert!((mainz.point.longitude - 8.2489).abs() < 0.01);
    });
}

#[test]
fn resolves_postal_code_55116_to_postal_code_coordinates() {
    tauri::async_runtime::block_on(async {
        let db_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/geo_loc.sqlite");
        let resolver = GeoDbResolver::connect(&db_path).await.unwrap();

        let resolved = resolver.resolve("55116").await.unwrap();

        let mainz = resolved
            .iter()
            .find(|location| location.label == "Mainz")
            .expect("55116 should resolve to Mainz through postal-code lookup");
        assert!((mainz.point.latitude - 50.001).abs() < 0.0001);
        assert!((mainz.point.longitude - 8.2688).abs() < 0.0001);
    });
}

#[test]
fn resolves_uk_full_postcode_through_sector_fallback() {
    tauri::async_runtime::block_on(async {
        let db_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/geo_loc.sqlite");
        let resolver = GeoDbResolver::connect(&db_path).await.unwrap();

        let resolved = resolver.resolve("SW1A 1AA").await.unwrap();

        assert!(
            resolved.iter().any(|location| location.label == "London"),
            "SW1A 1AA should resolve through the bundled GB postcode sector lookup"
        );
    });
}

#[test]
fn resolves_german_umlaut_transliteration_variants() {
    tauri::async_runtime::block_on(async {
        let db_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/geo_loc.sqlite");
        let resolver = GeoDbResolver::connect(&db_path).await.unwrap();

        for input in ["München", "Muenchen", "Munchen"] {
            let resolved = resolver.resolve(input).await.unwrap();
            assert!(
                resolved.iter().any(|location| location.label == "München"),
                "{input} should resolve to München"
            );
        }
    });
}

#[test]
fn matches_candidates_by_radius_between_resolved_locations() {
    tauri::async_runtime::block_on(async {
        let db_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/geo_loc.sqlite");
        let resolver = GeoDbResolver::connect(&db_path).await.unwrap();

        let wiesbaden = matches_location_filter(&resolver, &["Mainz"], Some(30), &["Wiesbaden"])
            .await
            .unwrap();
        let koeln = matches_location_filter(&resolver, &["Mainz"], Some(30), &["Köln"])
            .await
            .unwrap();

        assert_eq!(wiesbaden, LocationMatchOutcome::Applied { matched: true });
        assert_eq!(koeln, LocationMatchOutcome::Applied { matched: false });
    });
}

#[test]
fn prepared_location_filter_reports_unresolved_and_ambiguous_locations_through_resolver_seam() {
    tauri::async_runtime::block_on(async {
        let resolver = FixtureGeoResolver::new([
            (
                "Mainz",
                vec![
                    resolved_location("Mainz", "Mainz", 49.99, 8.24),
                    resolved_location("Mainz", "Mainz-Bretzenheim", 49.98, 8.23),
                ],
            ),
            ("Atlantis", vec![]),
            (
                "Twin City",
                vec![
                    resolved_location("Twin City", "Twin City North", 50.0, 8.25),
                    resolved_location("Twin City", "Twin City South", 60.0, 9.25),
                ],
            ),
        ]);

        let filter = prepare_location_filter(&resolver, &["Mainz"], Some(30))
            .await
            .unwrap();
        let report = filter
            .matches_candidate_with_report(&resolver, &["Atlantis", "Twin City"])
            .await
            .unwrap();

        assert_eq!(
            report.outcome,
            LocationMatchOutcome::Applied { matched: true }
        );
        assert_eq!(filter.request_ambiguities().len(), 1);
        assert_eq!(filter.request_ambiguities()[0].input, "Mainz");
        assert_eq!(report.unresolved_candidate_locations, vec!["Atlantis"]);
        assert_eq!(report.candidate_ambiguities.len(), 1);
        assert_eq!(report.candidate_ambiguities[0].input, "Twin City");
    });
}

struct FixtureGeoResolver {
    locations: BTreeMap<String, Vec<ResolvedLocation>>,
}

impl FixtureGeoResolver {
    fn new(locations: impl IntoIterator<Item = (&'static str, Vec<ResolvedLocation>)>) -> Self {
        Self {
            locations: locations
                .into_iter()
                .map(|(input, locations)| (input.to_string(), locations))
                .collect(),
        }
    }
}

impl GeoResolver for FixtureGeoResolver {
    fn resolve<'a>(&'a self, input: &'a str) -> GeoResolveFuture<'a> {
        Box::pin(async move { Ok(self.locations.get(input).cloned().unwrap_or_default()) })
    }
}

fn resolved_location(input: &str, label: &str, latitude: f64, longitude: f64) -> ResolvedLocation {
    ResolvedLocation {
        input: input.to_string(),
        label: label.to_string(),
        point: GeoPoint {
            latitude,
            longitude,
        },
    }
}
