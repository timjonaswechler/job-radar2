use job_radar_lib::{DetailField, DetailPatch, RequestedFieldDisposition, SourceDetailOutcome};
use serde_json::json;

#[test]
fn closed_source_detail_serialization_keeps_values_and_dispositions_on_completed_only() {
    let completed = SourceDetailOutcome::Completed {
        fields: DetailPatch::default(),
        dispositions: vec![RequestedFieldDisposition::Unsupported {
            field: DetailField::DescriptionText,
        }],
        phase_evidence: None,
    };
    assert_eq!(
        serde_json::to_value(completed).unwrap(),
        json!({
            "type": "completed",
            "fields": {},
            "dispositions": [{
                "type": "unsupported",
                "field": "descriptionText"
            }]
        })
    );

    let mismatch = serde_json::to_value(SourceDetailOutcome::SourceMismatch).unwrap();
    assert_eq!(mismatch, json!({ "type": "source_mismatch" }));
    assert!(mismatch.get("fields").is_none());
    assert!(mismatch.get("dispositions").is_none());
}
