use job_radar_lib::agent::api::ApiKind;
use job_radar_lib::agent::models::{ModelId, ModelInput, ProviderId, ReasoningLevel};
use job_radar_lib::agent::registry::{ModelRegistry, ProviderAvailability};
use job_radar_lib::agent::AgentErrorCategory;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

fn registry_root() -> (tempfile::TempDir, std::path::PathBuf) {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("agents");
    fs::create_dir(&root).unwrap();
    (temporary, root)
}

fn write_models(root: &std::path::Path, document: &str) {
    let path = root.join("models.json");
    fs::write(&path, document).unwrap();
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

#[test]
fn absent_models_document_publishes_pinned_builtins_and_value_free_availability() {
    let (_temporary, root) = registry_root();
    let registry = ModelRegistry::from_agents_data_root(&root).unwrap();
    let snapshot = registry.snapshot();
    let provider = ProviderId::new("openai-codex").unwrap();

    assert!(!registry.last_reload_failed());
    assert_eq!(snapshot.providers().len(), 1);
    assert_eq!(snapshot.models().len(), 7);
    assert_eq!(
        snapshot.provider(&provider).unwrap().api(),
        ApiKind::OpenAiResponses
    );
    let pinned = snapshot
        .model(&provider, &ModelId::new("gpt-5.4").unwrap())
        .unwrap();
    assert_eq!(pinned.base_url(), "https://chatgpt.com/backend-api");
    assert_eq!(pinned.input(), &[ModelInput::Text, ModelInput::Image]);
    assert_eq!(pinned.context_window(), 272_000);
    assert_eq!(pinned.max_tokens(), 128_000);
    assert_eq!(pinned.cost().input().as_f64(), Some(2.5));
    assert_eq!(
        pinned.cost().tiers().unwrap()[0]
            .input_tokens_above()
            .as_u64(),
        Some(272_000)
    );
    assert_eq!(pinned.compat()["supportsToolSearch"], true);
    assert!(snapshot
        .available_models(&ProviderAvailability::default())
        .is_empty());

    let availability = ProviderAvailability::new([provider]);
    assert_eq!(snapshot.available_models(&availability).len(), 7);
}

#[test]
fn comments_custom_providers_overrides_and_whole_model_upserts_follow_pi_merge_order() {
    let (_temporary, root) = registry_root();
    write_models(
        &root,
        r#"
        {
          // Built-in provider metadata is modified before model overrides.
          "providers": {
            "openai-codex": {
              "name": "Synthetic Codex",
              "baseUrl": "https://synthetic.invalid/responses",
              "headers": {"x-provider": "provider"},
              "compat": {
                "openRouterRouting": {"order": ["first"], "allow_fallbacks": true},
                "vercelGatewayRouting": {"only": ["first"]},
                "chatTemplateKwargs": {"effort": {"$var": "thinking.effort"}}
              },
              "modelOverrides": {
                "gpt-5.4": {
                  "name": "Overridden before replacement",
                  "cost": {"input": 9},
                  "contextWindow": 999,
                  "headers": {"x-replaced-override": "absent"}
                },
                "gpt-5.4-mini": {
                  "name": "Overridden model",
                  "reasoning": true,
                  "thinkingLevelMap": {"low": "small", "high": "large"},
                  "cost": {"input": 1.25},
                  "contextWindow": 999,
                  "headers": {"x-model": "override"},
                  "compat": {
                    "openRouterRouting": {"allow_fallbacks": false},
                    "vercelGatewayRouting": {"order": ["second"]},
                    "chatTemplateKwargs": {"enabled": {"$var": "thinking.enabled", "omitWhenOff": true}}
                  }
                }
              },
              "models": [
                {
                  "id": "gpt-5.4",
                  "name": "Custom replacement",
                  "reasoning": false,
                  "input": ["text", "image"],
                  "cost": {"input": 0, "output": 2.5, "cacheRead": 0, "cacheWrite": 0},
                  "maxTokens": 321,
                  "headers": {"x-custom": "replacement"}
                }
              ]
            },
            /* A custom provider inherits its model API and base URL. */
            "synthetic-provider": {
              "name": "Synthetic Provider",
              "api": "openai-responses",
              "baseUrl": "https://models.invalid/v1",
              "apiKey": "$SYNTHETIC_API_KEY",
              "models": [
                {
                  "id": "synthetic-model",
                  "reasoning": true,
                  "thinkingLevelMap": {"off": null, "medium": "balanced"},
                  "contextWindow": 64000,
                  "maxTokens": 2048
                }
              ]
            }
          }
        }
        "#,
    );

    let registry = ModelRegistry::from_agents_data_root_with_environment_names(
        &root,
        ["SYNTHETIC_API_KEY".to_owned()],
    )
    .unwrap();
    let snapshot = registry.snapshot();
    let codex = ProviderId::new("openai-codex").unwrap();
    let replacement = snapshot
        .model(&codex, &ModelId::new("gpt-5.4").unwrap())
        .unwrap();

    assert_eq!(
        snapshot.provider(&codex).unwrap().display_name(),
        "Synthetic Codex"
    );
    assert_eq!(replacement.display_name(), "Custom replacement");
    assert_eq!(
        replacement.base_url(),
        "https://synthetic.invalid/responses"
    );
    assert_eq!(
        replacement.supported_reasoning_levels(),
        &[ReasoningLevel::Off]
    );
    assert_eq!(replacement.input(), &[ModelInput::Text, ModelInput::Image]);
    assert_eq!(replacement.context_window(), 128_000);
    assert_eq!(replacement.max_tokens(), 321);
    assert_eq!(replacement.cost().input().as_u64(), Some(0));
    assert_eq!(replacement.cost().output().as_f64(), Some(2.5));
    assert!(replacement.headers().is_empty());
    assert!(!replacement.headers().contains_key("x-replaced-override"));
    assert_eq!(
        replacement.compat()["openRouterRouting"]["order"][0],
        "first"
    );
    assert_eq!(
        replacement.compat()["openRouterRouting"]["allow_fallbacks"],
        true
    );

    let overridden = snapshot
        .model(&codex, &ModelId::new("gpt-5.4-mini").unwrap())
        .unwrap();
    assert_eq!(overridden.display_name(), "Overridden model");
    assert_eq!(overridden.context_window(), 999);
    assert_eq!(overridden.cost().input().as_f64(), Some(1.25));
    assert_eq!(overridden.cost().output().as_f64(), Some(4.5));
    assert_eq!(
        overridden.supported_reasoning_levels(),
        &[
            ReasoningLevel::Off,
            ReasoningLevel::Minimal,
            ReasoningLevel::Low,
            ReasoningLevel::Medium,
            ReasoningLevel::High,
            ReasoningLevel::XHigh,
        ]
    );
    assert_eq!(
        overridden.compat()["openRouterRouting"]["order"][0],
        "first"
    );
    assert_eq!(
        overridden.compat()["openRouterRouting"]["allow_fallbacks"],
        false
    );
    assert!(overridden.headers().is_empty());

    let custom_provider = ProviderId::new("synthetic-provider").unwrap();
    let custom = snapshot
        .model(&custom_provider, &ModelId::new("synthetic-model").unwrap())
        .unwrap();
    assert_eq!(custom.api(), ApiKind::OpenAiResponses);
    assert_eq!(custom.base_url(), "https://models.invalid/v1");
    assert_eq!(custom.display_name(), "synthetic-model");
    assert_eq!(
        custom.supported_reasoning_levels(),
        &[
            ReasoningLevel::Off,
            ReasoningLevel::Minimal,
            ReasoningLevel::Low,
            ReasoningLevel::Medium,
            ReasoningLevel::High,
            ReasoningLevel::XHigh,
        ]
    );
    assert_eq!(custom.context_window(), 64_000);
    assert_eq!(custom.max_tokens(), 2_048);
    assert_eq!(
        snapshot
            .available_models(&ProviderAvailability::default())
            .iter()
            .filter(|model| model.provider() == &custom_provider)
            .count(),
        1
    );
    assert!(!format!("{snapshot:?}").contains("SYNTHETIC_API_KEY"));
}

#[test]
fn explicit_reload_is_transactional_and_preserves_old_immutable_snapshots() {
    let (_temporary, root) = registry_root();
    write_models(
        &root,
        r#"{"providers":{"synthetic-provider":{"api":"openai-responses","baseUrl":"https://one.invalid/v1","apiKey":"synthetic-key-one","models":[{"id":"synthetic-model","name":"First"}]}}}"#,
    );
    let registry = ModelRegistry::from_agents_data_root(&root).unwrap();
    let first = registry.snapshot();

    write_models(
        &root,
        r#"{"providers":{"synthetic-provider":{"api":"openai-responses","baseUrl":"https://two.invalid/v1","apiKey":"synthetic-key-two","models":[{"id":"synthetic-model","name":"Second"}]}}}"#,
    );
    let second = registry.reload().unwrap();
    assert!(!Arc::ptr_eq(&first, &second));
    let provider = ProviderId::new("synthetic-provider").unwrap();
    let model = ModelId::new("synthetic-model").unwrap();
    assert_eq!(
        first.model(&provider, &model).unwrap().display_name(),
        "First"
    );
    assert_eq!(
        second.model(&provider, &model).unwrap().display_name(),
        "Second"
    );

    write_models(
        &root,
        r#"{"providers":{"synthetic-provider":{"api":"openai-responses","baseUrl":"https://three.invalid/v1","apiKey":"!synthetic-command","models":[{"id":"synthetic-model"}]}}}"#,
    );
    let error = registry.reload().unwrap_err();
    let retained = registry.snapshot();

    assert_eq!(error.category, AgentErrorCategory::InvalidConfiguration);
    assert_eq!(error.message, "agent model configuration is invalid");
    assert!(!format!("{error:?}").contains("synthetic-command"));
    assert!(registry.last_reload_failed());
    assert!(Arc::ptr_eq(&second, &retained));
    assert_eq!(
        retained.model(&provider, &model).unwrap().display_name(),
        "Second"
    );
}

#[test]
fn invalid_startup_and_unsupported_api_fail_closed_without_losing_builtins() {
    let invalid_documents = [
        r#"{"providers":{"custom":{"api":"anthropic-messages","baseUrl":"https://models.invalid","models":[{"id":"model"}]}}}"#,
        r#"{"providers":{"custom":{"api":"openai-responses","models":[{"id":"model"}]}}}"#,
        r#"{"providers":{"openai-codex":{"modelOverrides":{"missing-model":{"name":"Missing"}}}}}"#,
        r#"{"providers":{"custom":{"api":"openai-responses","baseUrl":"https://models.invalid","models":[{"id":"model","contextWindow":0}]}}}"#,
        r#"{"providers":{},"unknown":true}"#,
        r#"{"providers":{"openai-codex":{"compat":{"unknownCapability":true}}}}"#,
        r#"{"providers":{"custom":{"api":"openai-responses","baseUrl":"https://models.invalid","models":[{"id":"model","cost":{"input":1}}]}}}"#,
        r#"{"providers":{"openai-codex":{"baseUrl":null}}}"#,
        r#"{"providers":{"openai-codex":{"modelOverrides":{"gpt-5.4":{"cost":{"tiers":null}}}}}}"#,
    ];

    for document in invalid_documents {
        let (_temporary, root) = registry_root();
        write_models(&root, document);
        let registry = ModelRegistry::from_agents_data_root(&root).unwrap();
        let snapshot = registry.snapshot();
        assert!(registry.last_reload_failed());
        assert_eq!(snapshot.models().len(), 7);
        assert!(snapshot
            .model(
                &ProviderId::new("openai-codex").unwrap(),
                &ModelId::new("gpt-5.4").unwrap(),
            )
            .is_some());
    }
}

#[test]
fn availability_resolves_direct_and_environment_references_without_exposing_values() {
    let (_temporary, root) = registry_root();
    write_models(
        &root,
        r#"{
          "providers": {
            "direct-provider": {
              "api": "openai-responses",
              "baseUrl": "https://direct.invalid/v1",
              "apiKey": "synthetic-direct-secret",
              "headers": {"Authorization": "synthetic-authorization-value"},
              "models": [{"id": "direct-model", "headers": {"x-vendor-secret": "synthetic-model-secret"}}]
            },
            "present-provider": {
              "api": "openai-responses",
              "baseUrl": "https://present.invalid/v1",
              "apiKey": "$SYNTHETIC_PRESENT_KEY",
              "models": [{"id": "present-model"}]
            },
            "missing-provider": {
              "api": "openai-responses",
              "baseUrl": "https://missing.invalid/v1",
              "apiKey": "${SYNTHETIC_MISSING_KEY}",
              "models": [{"id": "missing-model"}]
            }
          }
        }"#,
    );

    let registry = ModelRegistry::from_agents_data_root_with_environment_names(
        &root,
        ["SYNTHETIC_PRESENT_KEY".to_owned()],
    )
    .unwrap();
    let snapshot = registry.snapshot();
    let available = snapshot.available_models(&ProviderAvailability::default());

    assert_eq!(available.len(), 2);
    assert!(available
        .iter()
        .any(|model| model.id().as_str() == "direct-model"));
    assert!(available
        .iter()
        .any(|model| model.id().as_str() == "present-model"));
    assert!(!available
        .iter()
        .any(|model| model.id().as_str() == "missing-model"));
    let direct = snapshot
        .model(
            &ProviderId::new("direct-provider").unwrap(),
            &ModelId::new("direct-model").unwrap(),
        )
        .unwrap();
    assert!(direct.headers().is_empty());

    let debug = format!("{snapshot:?}");
    for secret in [
        "synthetic-direct-secret",
        "synthetic-authorization-value",
        "synthetic-model-secret",
    ] {
        assert!(!debug.contains(secret));
    }
}

#[test]
fn typed_compat_accepts_nested_merges_and_nullable_thinking_values() {
    let (_temporary, root) = registry_root();
    write_models(
        &root,
        r#"{
          "providers": {
            "openai-codex": {
              "compat": {
                "openRouterRouting": {"order": ["first"], "max_price": {"prompt": "1.5"}},
                "vercelGatewayRouting": {"only": ["first"]},
                "chatTemplateKwargs": {"effort": {"$var": "thinking.effort"}}
              },
              "modelOverrides": {
                "gpt-5.4": {
                  "thinkingLevelMap": {"off": null, "high": "high"},
                  "compat": {
                    "openRouterRouting": {"only": ["second"]},
                    "vercelGatewayRouting": {"order": ["second"]},
                    "chatTemplateKwargs": {"enabled": {"$var": "thinking.enabled", "omitWhenOff": true}}
                  }
                }
              }
            }
          }
        }"#,
    );

    let snapshot = ModelRegistry::from_agents_data_root(&root)
        .unwrap()
        .snapshot();
    let model = snapshot
        .model(
            &ProviderId::new("openai-codex").unwrap(),
            &ModelId::new("gpt-5.4").unwrap(),
        )
        .unwrap();

    assert_eq!(model.compat()["openRouterRouting"]["order"][0], "first");
    assert_eq!(model.compat()["openRouterRouting"]["only"][0], "second");
    assert_eq!(model.compat()["vercelGatewayRouting"]["only"][0], "first");
    assert_eq!(model.compat()["vercelGatewayRouting"]["order"][0], "second");
    assert!(model.compat()["chatTemplateKwargs"]["effort"].is_object());
    assert!(model.compat()["chatTemplateKwargs"]["enabled"].is_object());
    assert_eq!(model.thinking_level_map()[&ReasoningLevel::Off], None);
}

#[cfg(unix)]
#[test]
fn protected_loading_rejects_symlinked_ancestors_and_model_files() {
    use std::os::unix::fs::symlink;

    let temporary = tempfile::tempdir().unwrap();
    let real_app = temporary.path().join("real-app");
    fs::create_dir(&real_app).unwrap();
    let linked_app = temporary.path().join("linked-app");
    symlink(&real_app, &linked_app).unwrap();
    assert!(ModelRegistry::from_agents_data_root(linked_app.join("agents")).is_err());

    let (_temporary, root) = registry_root();
    let target = root.parent().unwrap().join("synthetic-models-target.json");
    fs::write(&target, r#"{"providers":{}}"#).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
    symlink(&target, root.join("models.json")).unwrap();
    let registry = ModelRegistry::from_agents_data_root(&root).unwrap();
    assert!(registry.last_reload_failed());
    assert_eq!(registry.snapshot().models().len(), 7);

    let (_temporary, dangling_root) = registry_root();
    symlink(
        dangling_root.join("missing-target.json"),
        dangling_root.join("models.json"),
    )
    .unwrap();
    let dangling = ModelRegistry::from_agents_data_root(&dangling_root).unwrap();
    assert!(dangling.last_reload_failed());
    assert_eq!(dangling.snapshot().models().len(), 7);
}

#[cfg(unix)]
#[test]
fn reload_rejects_an_agents_root_replaced_by_a_symlink_and_retains_the_snapshot() {
    use std::os::unix::fs::symlink;

    let (_temporary, root) = registry_root();
    write_models(
        &root,
        r#"{"providers":{"synthetic-provider":{"api":"openai-responses","baseUrl":"https://models.invalid","models":[{"id":"synthetic-model"}]}}}"#,
    );
    let registry = ModelRegistry::from_agents_data_root(&root).unwrap();
    let published = registry.snapshot();
    let original_root = root.with_file_name("original-agents");
    fs::rename(&root, &original_root).unwrap();
    symlink(&original_root, &root).unwrap();

    assert!(registry.reload().is_err());
    assert!(registry.last_reload_failed());
    assert!(Arc::ptr_eq(&published, &registry.snapshot()));
}

#[cfg(unix)]
#[test]
fn protected_loading_rejects_insecure_model_file_mode() {
    let (_temporary, root) = registry_root();
    let path = root.join("models.json");
    fs::write(&path, r#"{"providers":{}}"#).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

    let registry = ModelRegistry::from_agents_data_root(&root).unwrap();
    assert!(registry.last_reload_failed());
    assert_eq!(registry.snapshot().models().len(), 7);
}
