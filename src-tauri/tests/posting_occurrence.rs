use job_radar_lib::{
    validate_posting_reference, OccurrenceReferenceError, PostingOccurrenceIdentity,
};

#[test]
fn provider_id_identity_is_source_local_case_sensitive_and_independent_of_url() {
    let (_, first) = validate_posting_reference(
        "source-a",
        "https://example.test/jobs/old#section",
        Some("Req-42".to_string()),
    )
    .unwrap();
    let (_, moved) = validate_posting_reference(
        "source-a",
        "https://jobs.example.test/new",
        Some("Req-42".to_string()),
    )
    .unwrap();
    let (_, other_case) = validate_posting_reference(
        "source-a",
        "https://jobs.example.test/new",
        Some("REQ-42".to_string()),
    )
    .unwrap();
    let (_, other_source) = validate_posting_reference(
        "source-b",
        "https://example.test/jobs/old#section",
        Some("Req-42".to_string()),
    )
    .unwrap();

    assert_eq!(first, moved);
    assert_ne!(first, other_case);
    assert_ne!(first, other_source);
}

#[test]
fn url_fallback_uses_conservative_parser_serialization() {
    let (reference, identity) = validate_posting_reference(
        "source-a",
        " \thTTps://BÜCHER.example:443/a/../jobs/?b=2&a=1&a=0 ",
        None,
    )
    .unwrap();

    assert_eq!(
        reference.provider_url,
        "hTTps://BÜCHER.example:443/a/../jobs/?b=2&a=1&a=0"
    );
    assert_eq!(
        identity,
        PostingOccurrenceIdentity::NormalizedUrl {
            source_key: "source-a".to_string(),
            normalized_url: "https://xn--bcher-kva.example/jobs/?b=2&a=1&a=0".to_string(),
        }
    );

    let (_, without_trailing_slash) = validate_posting_reference(
        "source-a",
        "https://xn--bcher-kva.example/jobs?b=2&a=1&a=0",
        None,
    )
    .unwrap();
    assert_ne!(identity, without_trailing_slash);
}

#[test]
fn reference_validation_rejects_unsafe_or_ambiguous_identity_inputs() {
    assert_eq!(
        validate_posting_reference("source", "https://user:secret@example.test/jobs", None),
        Err(OccurrenceReferenceError::UserInfo)
    );
    assert_eq!(
        validate_posting_reference("source", "https://example.test/jobs#one", None),
        Err(OccurrenceReferenceError::FragmentWithoutProviderPostingId)
    );
    assert_eq!(
        validate_posting_reference("source", "https://example.test/jobs", Some(String::new()),),
        Err(OccurrenceReferenceError::EmptyProviderPostingId)
    );
    assert_eq!(
        validate_posting_reference("source", "javascript:alert(1)", None),
        Err(OccurrenceReferenceError::InvalidUrl)
    );
}

#[test]
fn provider_ids_are_not_trimmed_or_case_folded() {
    let (reference, identity) = validate_posting_reference(
        "source",
        "https://example.test/jobs/42",
        Some(" Req-42 ".to_string()),
    )
    .unwrap();
    assert_eq!(reference.provider_posting_id.as_deref(), Some(" Req-42 "));
    assert_eq!(
        identity,
        PostingOccurrenceIdentity::ProviderPostingId {
            source_key: "source".to_string(),
            provider_posting_id: " Req-42 ".to_string(),
        }
    );
}

#[test]
fn id_and_url_fallback_identities_never_correlate_by_url() {
    let (_, by_id) = validate_posting_reference(
        "source",
        "https://example.test/jobs/42",
        Some("42".to_string()),
    )
    .unwrap();
    let (_, by_url) =
        validate_posting_reference("source", "https://example.test/jobs/42", None).unwrap();

    assert_ne!(by_id, by_url);
}
