use std::path::Path;

use job_radar_lib::{matches_location_filter, GeoDbResolver, LocationMatchOutcome};

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
