use job_radar_lib::{
    acceptance_context_registrations, acceptance_descriptors, compile_acceptance,
    validate_acceptance_context_registrations, validate_acceptance_registration_keys, Acceptance,
    AcceptanceCompileContext, AcceptanceContextRegistration, AcceptanceContextRegistryError,
    AcceptanceField, AcceptancePhase, AcceptanceRegistryError,
};

#[test]
fn acceptance_registry_has_exact_schema_v3_keys_and_detects_parity_faults() {
    let registration_keys = acceptance_descriptors()
        .iter()
        .map(|descriptor| descriptor.key.to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        registration_keys,
        vec![
            "requiredFields".to_string(),
            "minDescriptionLength".to_string(),
            "minResults".to_string(),
        ]
    );
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{}/src/schema/profile-dsl/strategy.schema.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap();
    let acceptance_schema = &schema["$defs"]["acceptance"];
    let schema_keys = acceptance_schema["properties"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(acceptance_schema["additionalProperties"], false);
    assert!(!acceptance_schema["properties"]
        .as_object()
        .unwrap()
        .contains_key("maxErrorRatio"));
    let serde_keys = serde_json::to_value(Acceptance {
        required_fields: Some(vec!["title".into()]),
        min_description_length: Some(1),
        min_results: Some(1),
    })
    .unwrap()
    .as_object()
    .unwrap()
    .keys()
    .cloned()
    .collect::<Vec<_>>();
    assert_eq!(
        validate_acceptance_registration_keys(&schema_keys, &serde_keys, &registration_keys),
        Ok(())
    );
    assert!(matches!(
        validate_acceptance_registration_keys(&schema_keys, &serde_keys[..2], &registration_keys),
        Err(AcceptanceRegistryError::Missing { layer: "serde", .. })
    ));
    assert!(matches!(
        validate_acceptance_registration_keys(
            &schema_keys,
            &serde_keys,
            &[registration_keys[0].clone(), registration_keys[0].clone()]
        ),
        Err(AcceptanceRegistryError::Duplicate {
            layer: "registration",
            ..
        })
    ));

    assert!(serde_json::from_value::<Acceptance>(serde_json::json!({
        "maxErrorRatio": 0.25
    }))
    .is_err());
}

#[test]
fn acceptance_registry_rejects_missing_duplicate_and_cross_phase_contexts() {
    let registrations = acceptance_context_registrations();
    assert_eq!(
        validate_acceptance_context_registrations(&registrations),
        Ok(())
    );

    assert!(matches!(
        validate_acceptance_context_registrations(&registrations[..registrations.len() - 1]),
        Err(AcceptanceContextRegistryError::Missing { .. })
    ));

    let mut duplicate = registrations.clone();
    duplicate.push(registrations[0]);
    assert!(matches!(
        validate_acceptance_context_registrations(&duplicate),
        Err(AcceptanceContextRegistryError::Duplicate { .. })
    ));

    let mut cross_phase = registrations;
    cross_phase.push(AcceptanceContextRegistration {
        key: "minResults",
        phase: AcceptancePhase::Detail,
    });
    assert!(matches!(
        validate_acceptance_context_registrations(&cross_phase),
        Err(AcceptanceContextRegistryError::Extra { .. })
    ));
}

#[test]
fn acceptance_compilation_is_phase_typed_and_rejects_cross_phase_fields() {
    let discovery = Acceptance {
        required_fields: Some(vec!["url".into(), "postingMeta.department".into()]),
        min_description_length: None,
        min_results: Some(1),
    };
    let plan = compile_acceptance(
        &discovery,
        &AcceptanceCompileContext::discovery(["department"]),
    )
    .expect("valid Discovery acceptance");
    assert_eq!(
        plan.required_fields,
        vec![
            AcceptanceField::Url,
            AcceptanceField::PostingMeta("department".into())
        ]
    );

    let error = compile_acceptance(
        &Acceptance {
            required_fields: None,
            min_description_length: None,
            min_results: Some(1),
        },
        &AcceptanceCompileContext::detail(),
    )
    .unwrap_err();
    assert_eq!(error.phase, AcceptancePhase::Detail);
    assert_eq!(error.key, "minResults");

    let error = compile_acceptance(
        &Acceptance {
            required_fields: Some(vec!["url".into()]),
            min_description_length: None,
            min_results: None,
        },
        &AcceptanceCompileContext::detail(),
    )
    .unwrap_err();
    assert_eq!(error.key, "requiredFields");
}
