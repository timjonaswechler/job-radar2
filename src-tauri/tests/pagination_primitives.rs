use std::collections::{BTreeMap, BTreeSet};

use job_radar_lib::{
    compile_pagination_plan, pagination_descriptors, pagination_parameter_locations,
    validate_pagination_inventories, CompiledPagination, Pagination, PaginationFragment,
    PaginationInventory, PaginationParameterLocation, PaginationRegistryError, ParseType,
};
use serde_json::{json, Value};

fn schema_inventory() -> PaginationInventory {
    let schema: Value = serde_json::from_str(include_str!(
        "../src/schema/profile-dsl/pagination.schema.json"
    ))
    .unwrap();
    let mut variants = Vec::new();
    let mut options = BTreeMap::new();
    for reference in schema["$defs"]["pagination"]["oneOf"].as_array().unwrap() {
        let definition = reference["$ref"]
            .as_str()
            .unwrap()
            .rsplit('/')
            .next()
            .unwrap();
        let properties = schema["$defs"][definition]["properties"]
            .as_object()
            .unwrap();
        let key = properties["type"]["const"].as_str().unwrap().to_string();
        variants.push(key.clone());
        let mut variant_options = properties.keys().cloned().collect::<Vec<_>>();
        variant_options.extend(
            schema["$defs"]["limits"]["properties"]
                .as_object()
                .unwrap()
                .keys()
                .map(|name| format!("limits.{name}")),
        );
        options.insert(key, variant_options);
    }
    let parameter_locations = schema["$defs"]["parameterLocation"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect();
    PaginationInventory {
        variants,
        options,
        parameter_locations,
        fragment_options: descriptor_fragment_options(),
    }
}

fn serde_inventory() -> PaginationInventory {
    let values = [
        json!({ "type": "page", "pageParam": "page", "parameterLocation": "json_body", "firstPage": 0, "pageSizeParam": "size", "pageSize": 1, "totalPath": "$.total", "limits": { "maxRequests": 1, "maxItems": 1, "maxDepth": 0 } }),
        json!({ "type": "offset_limit", "offsetParam": "offset", "limitParam": "limit", "parameterLocation": "json_body", "startOffset": 0, "limit": 1, "totalPath": "$.total", "limits": { "maxRequests": 1, "maxItems": 1, "maxDepth": 0 } }),
        json!({ "type": "cursor", "cursorParam": "cursor", "parameterLocation": "json_body", "nextCursorPath": "$.next", "limits": { "maxRequests": 1, "maxItems": 1, "maxDepth": 0 } }),
        json!({ "type": "sitemap", "childSitemapSelector": { "type": "sitemap_urls" }, "postingUrlSelector": { "type": "sitemap_urls" }, "limits": { "maxRequests": 1, "maxItems": 1, "maxDepth": 0 } }),
    ];
    let mut variants = Vec::new();
    let mut options = BTreeMap::new();
    for value in values {
        let serialized =
            serde_json::to_value(serde_json::from_value::<Pagination>(value).unwrap()).unwrap();
        let object = serialized.as_object().unwrap();
        let key = object["type"].as_str().unwrap().to_string();
        variants.push(key.clone());
        let mut variant_options = object.keys().cloned().collect::<Vec<_>>();
        variant_options.extend(
            object["limits"]
                .as_object()
                .unwrap()
                .keys()
                .map(|name| format!("limits.{name}")),
        );
        options.insert(key, variant_options);
    }
    let parameter_locations = pagination_parameter_locations()
        .iter()
        .map(|key| {
            serde_json::from_value::<PaginationParameterLocation>(json!(key)).unwrap();
            key.to_string()
        })
        .collect();
    assert!(serde_json::from_value::<PaginationParameterLocation>(json!("body")).is_err());
    PaginationInventory {
        variants,
        options,
        parameter_locations,
        fragment_options: serde_fragment_options(),
    }
}

fn fragment_schema_options() -> Vec<String> {
    let schema: Value = serde_json::from_str(include_str!(
        "../src/schema/profile-dsl/fragments.schema.json"
    ))
    .unwrap();
    schema["$defs"]["paginationFragment"]["properties"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect()
}

fn serde_fragment_options() -> Vec<String> {
    let fragment: PaginationFragment = serde_json::from_value(json!({
        "type": "page", "pageParam": "page", "parameterLocation": "query", "firstPage": 0,
        "pageSizeParam": "size", "pageSize": 1, "totalPath": "$.total",
        "offsetParam": "offset", "limitParam": "limit", "startOffset": 0, "limit": 1,
        "cursorParam": "cursor", "nextCursorPath": "$.next",
        "childSitemapSelector": { "type": "sitemap_urls" },
        "postingUrlSelector": { "type": "sitemap_urls" },
        "limits": { "maxRequests": 1, "maxItems": 1, "maxDepth": 0 }
    }))
    .unwrap();
    serde_json::to_value(fragment)
        .unwrap()
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect()
}

fn descriptor_fragment_options() -> Vec<String> {
    pagination_descriptors()
        .iter()
        .flat_map(|descriptor| descriptor.options)
        .filter(|option| !option.starts_with("limits."))
        .map(|option| option.to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[test]
fn canonical_catalogue_matches_real_schema_serde_and_fragment_surfaces() {
    let schema = schema_inventory();
    let serde = serde_inventory();
    let fragment = PaginationInventory {
        fragment_options: fragment_schema_options(),
        ..schema.clone()
    };
    let registration = PaginationInventory::from_descriptors(descriptor_fragment_options());

    validate_pagination_inventories(&schema, &serde, &fragment, &registration).unwrap();
    assert_eq!(pagination_descriptors().len(), 4);
    assert_eq!(pagination_parameter_locations(), ["query", "json_body"]);
}

#[test]
fn catalogue_validation_rejects_nested_missing_and_duplicate_identities() {
    let canonical = schema_inventory();
    let mut duplicate = canonical.clone();
    duplicate
        .options
        .get_mut("page")
        .unwrap()
        .push("pageParam".to_string());
    assert!(matches!(
        validate_pagination_inventories(&canonical, &duplicate, &canonical, &canonical),
        Err(PaginationRegistryError::Duplicate { layer: "serde", .. })
    ));
    let mut missing = canonical.clone();
    missing
        .options
        .get_mut("cursor")
        .unwrap()
        .retain(|key| key != "nextCursorPath");
    assert!(matches!(
        validate_pagination_inventories(&canonical, &missing, &canonical, &canonical),
        Err(PaginationRegistryError::Missing { layer: "serde", .. })
    ));
}

#[test]
fn direct_serde_requires_canonical_bounds() {
    for value in [
        json!({ "type": "page", "pageParam": "page", "limits": {} }),
        json!({ "type": "page", "pageParam": "page", "limits": { "maxRequests": 0 } }),
        json!({ "type": "page", "pageParam": "page", "limits": { "maxRequests": 1001 } }),
        json!({ "type": "sitemap", "limits": { "maxRequests": 1, "maxItems": 0 } }),
        json!({ "type": "sitemap", "limits": { "maxRequests": 1, "maxItems": 100001 } }),
        json!({ "type": "sitemap", "limits": { "maxRequests": 1, "maxDepth": 21 } }),
    ] {
        assert!(serde_json::from_value::<Pagination>(value).is_err());
    }
}

#[test]
fn page_owner_produces_ordered_overlay_and_checked_progression() {
    let authored: Pagination = serde_json::from_value(json!({
        "type": "page", "pageParam": "page", "pageSizeParam": "size", "pageSize": 25,
        "firstPage": 3, "limits": { "maxRequests": 2 }
    }))
    .unwrap();
    let CompiledPagination::Page(plan) =
        compile_pagination_plan(&authored, ParseType::Json, false).unwrap()
    else {
        panic!("expected page plan")
    };
    let mut state = plan.initial_state();
    assert_eq!(
        plan.overlay(&state).query,
        [("page".into(), "3".into()), ("size".into(), "25".into())]
    );
    assert!(plan.advance(&mut state));
    assert_eq!(plan.overlay(&state).query[0], ("page".into(), "4".into()));
}

#[test]
fn json_body_location_rejects_without_post_json_fetch_capability() {
    let authored: Pagination = serde_json::from_value(json!({
        "type": "cursor", "cursorParam": "cursor", "parameterLocation": "json_body",
        "nextCursorPath": "$.next", "limits": { "maxRequests": 2 }
    }))
    .unwrap();
    assert_eq!(
        compile_pagination_plan(&authored, ParseType::Json, false)
            .unwrap_err()
            .path,
        "/parameterLocation"
    );
}
