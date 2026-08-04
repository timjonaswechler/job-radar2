use job_postings::identity::{decide, merge_unique_locations, same, Comparison};

#[test]
fn exact_identity_wins_before_semantic_equivalence() {
    assert_eq!(decide(&[7], &[2, 3]).unwrap(), Some(7));
}

#[test]
fn several_exact_identities_may_resolve_to_one_posting() {
    assert_eq!(decide(&[7, 7], &[2]).unwrap(), Some(7));
}

#[test]
fn conflicting_exact_identities_are_structured() {
    let conflict = decide(&[9, 3, 9], &[1]).unwrap_err();
    assert_eq!(conflict.posting_ids(), &[3, 9]);
}

#[test]
fn semantic_fallback_selects_the_lowest_id_only_without_an_exact_hit() {
    assert_eq!(decide(&[], &[9, 3, 7]).unwrap(), Some(3));
    assert_eq!(decide(&[], &[]).unwrap(), None);
}

#[test]
fn equivalence_preserves_title_company_and_location_policy() {
    assert!(same(
        Comparison {
            title: "Senior Laser Engineer",
            company: "ACME GmbH",
            locations: &["Berlin, Germany".into()],
        },
        Comparison {
            title: "Laser Engineer Senior",
            company: "acme gmbh",
            locations: &["Berlin".into()],
        },
    ));
    assert!(!same(
        Comparison {
            title: "Senior Laser Engineer",
            company: "ACME GmbH",
            locations: &["Berlin".into()],
        },
        Comparison {
            title: "Senior Laser Engineer",
            company: "Other GmbH",
            locations: &["Berlin".into()],
        },
    ));
}

#[test]
fn locations_merge_additively_without_changing_the_existing_normalization_policy() {
    assert_eq!(
        merge_unique_locations(
            vec!["Berlin".into()],
            &[" berlin ".into(), "Ｍｕｎｉｃｈ".into(), "Munich".into()]
        ),
        vec!["Berlin", "Ｍｕｎｉｃｈ", "Munich"]
    );
}
