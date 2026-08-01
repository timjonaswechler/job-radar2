use sources::installed::{
    CreateDraft, ErrorKind, InactiveStatus, Revision, Snapshot, SourceDocument, SourceStatus, Store,
};
use std::fs;

const PROFILE: &str =
    include_str!("../../../tests/fixtures/source-behavior/valid/simple-source-profile.json");
const SOURCE: &str = include_str!("fixtures/sources/valid/source-selecting-access-path.json");
const OWNED: &str = include_str!("fixtures/sources/valid/source-owned-access-path.json");

fn install(root: &std::path::Path, profile: bool, source: &str) {
    if profile {
        fs::create_dir_all(root.join("source-profiles")).unwrap();
        fs::write(root.join("source-profiles/example_jobs.json"), PROFILE).unwrap();
    }
    fs::create_dir_all(root.join("sources")).unwrap();
    let document: SourceDocument = serde_json::from_str(source).unwrap();
    fs::write(root.join(format!("sources/{}.json", document.key)), source).unwrap();
}

fn create_draft(document: SourceDocument) -> CreateDraft {
    CreateDraft {
        key: document.key,
        name: document.name,
        source_config: document.source_config,
        selected_access_path: document.selected_access_path,
        access_paths: document.access_paths,
        source_support: document.source_support,
    }
}

#[test]
fn snapshot_prepares_profile_selected_and_source_owned_sources_once_behind_an_opaque_view() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Snapshot>();

    let root = tempfile::tempdir().unwrap();
    install(root.path(), true, SOURCE);
    install(root.path(), false, OWNED);
    let snapshot = Snapshot::load(root.path()).unwrap();
    let selected = snapshot.source("example_source").unwrap();
    let owned = snapshot.source("owned_source").unwrap();
    let compiled_selected = selected.compiled().unwrap();
    assert_eq!(
        compiled_selected
            .materialized_profile()
            .unwrap()
            .access_paths[0]
            .discovery
            .strategies[0]
            .accept_when
            .as_ref()
            .and_then(|acceptance| acceptance.min_results),
        Some(0),
        "Direct Source Specialization must be retained in the exact preparation"
    );
    assert!(owned.compiled().is_some());
    assert!(std::ptr::eq(
        compiled_selected,
        selected.compiled().unwrap()
    ));
    let view = serde_json::to_value(snapshot.view()).unwrap();
    let text = view.to_string();
    assert!(!text.contains("generation"));
    assert!(!text.contains("effectiveProfile"));
    assert!(!text.contains("compileOutcome"));
    assert!(!text.contains(root.path().to_string_lossy().as_ref()));
}

#[test]
fn mutations_enforce_draft_creation_preserved_revision_and_inactive_transitions() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("source-profiles")).unwrap();
    fs::write(
        root.path().join("source-profiles/example_jobs.json"),
        PROFILE,
    )
    .unwrap();
    let store = Store::new(root.path());
    let authored: SourceDocument = serde_json::from_str(SOURCE).unwrap();
    let created = store
        .create(CreateDraft {
            key: authored.key.clone(),
            name: authored.name.clone(),
            source_config: authored.source_config.clone(),
            selected_access_path: authored.selected_access_path.clone(),
            access_paths: authored.access_paths.clone(),
            source_support: authored.source_support.clone(),
        })
        .unwrap();
    assert_eq!(created.document.status, SourceStatus::Draft);
    let disabled = store
        .set_inactive(&authored.key, InactiveStatus::Disabled)
        .unwrap();
    assert_eq!(disabled.document.status, SourceStatus::Disabled);
    let revised = store
        .revise(Revision {
            key: authored.key.clone(),
            name: "Revised".into(),
            source_config: authored.source_config,
            selected_access_path: authored.selected_access_path,
            access_paths: authored.access_paths,
            source_support: authored.source_support,
        })
        .unwrap();
    assert_eq!(revised.document.status, SourceStatus::Disabled);
    assert_eq!(revised.document.name, "Revised");
}

#[test]
fn checked_admission_rejects_source_or_selected_profile_changes_but_ignores_unrelated_profiles() {
    let root = tempfile::tempdir().unwrap();
    let mut source: serde_json::Value = serde_json::from_str(SOURCE).unwrap();
    source["status"] = serde_json::json!("draft");
    install(
        root.path(),
        true,
        &serde_json::to_string_pretty(&source).unwrap(),
    );
    let store = Store::new(root.path());
    let first = store.snapshot().unwrap();
    let generation = first.source("example_source").unwrap().generation().clone();

    let mut unrelated: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    unrelated["key"] = serde_json::json!("unrelated");
    unrelated["name"] = serde_json::json!("Unrelated");
    fs::write(
        root.path().join("source-profiles/unrelated.json"),
        serde_json::to_vec_pretty(&unrelated).unwrap(),
    )
    .unwrap();
    assert_eq!(
        store
            .snapshot()
            .unwrap()
            .source("example_source")
            .unwrap()
            .generation(),
        &generation
    );

    source["name"] = serde_json::json!("Manually revised");
    fs::write(
        root.path().join("sources/example_source.json"),
        serde_json::to_vec_pretty(&source).unwrap(),
    )
    .unwrap();
    let error = store
        .admit_checked("example_source", &generation)
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::GenerationMismatch);

    let current = store
        .snapshot()
        .unwrap()
        .source("example_source")
        .unwrap()
        .generation()
        .clone();
    let mut profile: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    profile["accessPaths"][0]["discovery"]["strategies"][0]["fetch"]["url"] =
        serde_json::json!("https://changed.test/jobs");
    fs::write(
        root.path().join("source-profiles/example_jobs.json"),
        serde_json::to_vec_pretty(&profile).unwrap(),
    )
    .unwrap();
    assert_eq!(
        store
            .admit_checked("example_source", &current)
            .unwrap_err()
            .kind,
        ErrorKind::GenerationMismatch
    );
}

#[test]
fn cloned_store_serializes_competing_creates_without_overwrite() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::new(root.path());
    let authored: SourceDocument = serde_json::from_str(SOURCE).unwrap();
    let draft = CreateDraft {
        key: authored.key,
        name: authored.name,
        source_config: authored.source_config,
        selected_access_path: authored.selected_access_path,
        access_paths: authored.access_paths,
        source_support: authored.source_support,
    };
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let first = {
        let store = store.clone();
        let draft = draft.clone();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            store.create(draft)
        })
    };
    let second = {
        let store = store.clone();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            store.create(draft)
        })
    };
    let results = [first.join().unwrap(), second.join().unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter_map(|result| result.as_ref().err())
            .map(|error| error.kind)
            .collect::<Vec<_>>(),
        vec![ErrorKind::Duplicate]
    );
    assert_eq!(store.snapshot().unwrap().view().sources.len(), 1);
}

#[cfg(unix)]
#[test]
fn atomic_revision_failure_preserves_the_previous_document_bytes() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir().unwrap();
    install(root.path(), true, SOURCE);
    let store = Store::new(root.path());
    let source_path = root.path().join("sources/example_source.json");
    let previous = fs::read(&source_path).unwrap();
    let authored: SourceDocument = serde_json::from_str(SOURCE).unwrap();
    let revision = Revision {
        key: authored.key,
        name: "Must Not Persist".into(),
        source_config: authored.source_config,
        selected_access_path: authored.selected_access_path,
        access_paths: authored.access_paths,
        source_support: authored.source_support,
    };
    let source_directory = root.path().join("sources");
    let original_permissions = fs::metadata(&source_directory).unwrap().permissions();
    let mut read_only = original_permissions.clone();
    read_only.set_mode(0o555);
    fs::set_permissions(&source_directory, read_only).unwrap();
    let result = store.revise(revision);
    fs::set_permissions(&source_directory, original_permissions).unwrap();

    assert_eq!(result.unwrap_err().kind, ErrorKind::Storage);
    assert_eq!(fs::read(source_path).unwrap(), previous);
}

#[test]
fn manual_file_changes_are_visible_on_the_next_operation() {
    let root = tempfile::tempdir().unwrap();
    install(root.path(), true, SOURCE);
    let store = Store::new(root.path());
    assert_eq!(
        store
            .snapshot()
            .unwrap()
            .source("example_source")
            .unwrap()
            .document()
            .name,
        "Example Source"
    );
    let mut source: serde_json::Value = serde_json::from_str(SOURCE).unwrap();
    source["name"] = serde_json::json!("Manual Name");
    fs::write(
        root.path().join("sources/example_source.json"),
        serde_json::to_vec_pretty(&source).unwrap(),
    )
    .unwrap();
    assert_eq!(
        store
            .snapshot()
            .unwrap()
            .source("example_source")
            .unwrap()
            .document()
            .name,
        "Manual Name"
    );
}

#[test]
fn loader_and_mutations_reject_schema_invalid_authored_sources_without_writing() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("sources")).unwrap();
    let store = Store::new(root.path());

    let base: serde_json::Value = serde_json::from_str(SOURCE).unwrap();
    let owned: serde_json::Value = serde_json::from_str(OWNED).unwrap();
    let cases = [
        (
            "_leading",
            {
                let mut value = base.clone();
                value["key"] = serde_json::json!("_leading");
                value
            },
            "invalid_document_key",
            ErrorKind::InvalidKey,
        ),
        (
            "empty_name",
            {
                let mut value = base.clone();
                value["key"] = serde_json::json!("empty_name");
                value["name"] = serde_json::json!("");
                value
            },
            "invalid_source_name",
            ErrorKind::InvalidInput,
        ),
        (
            "profile_support",
            {
                let mut value = base.clone();
                value["key"] = serde_json::json!("profile_support");
                value["sourceSupport"] = serde_json::json!({"level": "experimental"});
                value
            },
            "profile_selected_source_support_forbidden",
            ErrorKind::InvalidInput,
        ),
        (
            "owned_missing_support",
            {
                let mut value = owned.clone();
                value["key"] = serde_json::json!("owned_missing_support");
                value.as_object_mut().unwrap().remove("sourceSupport");
                value
            },
            "source_owned_support_required",
            ErrorKind::InvalidInput,
        ),
        (
            "owned_direct_paths",
            {
                let mut value = owned.clone();
                value["key"] = serde_json::json!("owned_direct_paths");
                value["accessPaths"] = serde_json::json!([]);
                value
            },
            "source_owned_direct_access_paths_forbidden",
            ErrorKind::InvalidInput,
        ),
        (
            "owned_bad_key",
            {
                let mut value = owned.clone();
                value["key"] = serde_json::json!("owned_bad_key");
                value["selectedAccessPath"]["key"] = serde_json::json!("_bad");
                value
            },
            "invalid_source_owned_access_path_key",
            ErrorKind::InvalidInput,
        ),
        (
            "owned_empty_name",
            {
                let mut value = owned.clone();
                value["key"] = serde_json::json!("owned_empty_name");
                value["selectedAccessPath"]["name"] = serde_json::json!("");
                value
            },
            "invalid_source_owned_access_path_name",
            ErrorKind::InvalidInput,
        ),
        (
            "profile_bad_key",
            {
                let mut value = base.clone();
                value["key"] = serde_json::json!("profile_bad_key");
                value["selectedAccessPath"]["profileKey"] = serde_json::json!("_bad");
                value
            },
            "invalid_selected_profile_key",
            ErrorKind::InvalidInput,
        ),
    ];

    for (key, value, diagnostic_code, mutation_kind) in cases {
        let path = root.path().join(format!("sources/{key}.json"));
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        let snapshot = store.snapshot().unwrap();
        assert!(snapshot.source(key).is_none(), "{key} must not be admitted");
        assert!(
            snapshot
                .view()
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == diagnostic_code),
            "missing {diagnostic_code} for {key}: {:?}",
            snapshot.view().diagnostics
        );
        fs::remove_file(&path).unwrap();

        let document: SourceDocument = serde_json::from_value(value).unwrap();
        let error = store.create(create_draft(document)).unwrap_err();
        assert_eq!(error.kind, mutation_kind, "{key}");
        assert!(!path.exists(), "invalid mutation must not write {key}");
    }
}

#[test]
fn mutation_limits_are_checked_before_any_source_bytes_are_replaced() {
    use sources::installed::{
        MAX_AGGREGATE_SOURCE_BYTES, MAX_CUSTOM_SOURCE_DOCUMENTS, MAX_SOURCE_BYTES,
    };

    let oversized_root = tempfile::tempdir().unwrap();
    install(oversized_root.path(), true, SOURCE);
    let oversized_store = Store::new(oversized_root.path());
    let source_path = oversized_root.path().join("sources/example_source.json");
    let previous = fs::read(&source_path).unwrap();
    let authored: SourceDocument = serde_json::from_str(SOURCE).unwrap();
    let mut oversized_config = authored.source_config.clone();
    oversized_config.insert(
        "payload".to_string(),
        serde_json::Value::String("x".repeat(MAX_SOURCE_BYTES)),
    );
    let oversized = oversized_store
        .revise(Revision {
            key: authored.key.clone(),
            name: authored.name.clone(),
            source_config: oversized_config,
            selected_access_path: authored.selected_access_path.clone(),
            access_paths: authored.access_paths.clone(),
            source_support: authored.source_support.clone(),
        })
        .unwrap_err();
    assert_eq!(oversized.kind, ErrorKind::LimitExceeded);
    assert_eq!(fs::read(&source_path).unwrap(), previous);

    let count_root = tempfile::tempdir().unwrap();
    let count_dir = count_root.path().join("sources");
    fs::create_dir_all(&count_dir).unwrap();
    for index in 0..MAX_CUSTOM_SOURCE_DOCUMENTS {
        fs::write(count_dir.join(format!("occupied_{index:04}.json")), []).unwrap();
    }
    let count_store = Store::new(count_root.path());
    let count_error = count_store
        .create(create_draft(authored.clone()))
        .unwrap_err();
    assert_eq!(count_error.kind, ErrorKind::LimitExceeded);
    assert!(!count_dir.join("example_source.json").exists());

    let aggregate_create_root = tempfile::tempdir().unwrap();
    let aggregate_create_dir = aggregate_create_root.path().join("sources");
    fs::create_dir_all(&aggregate_create_dir).unwrap();
    fs::File::create(aggregate_create_dir.join("occupied.json"))
        .unwrap()
        .set_len(MAX_AGGREGATE_SOURCE_BYTES as u64)
        .unwrap();
    let aggregate_create_error = Store::new(aggregate_create_root.path())
        .create(create_draft(authored.clone()))
        .unwrap_err();
    assert_eq!(aggregate_create_error.kind, ErrorKind::LimitExceeded);
    assert!(!aggregate_create_dir.join("example_source.json").exists());

    let aggregate_revise_root = tempfile::tempdir().unwrap();
    install(aggregate_revise_root.path(), true, SOURCE);
    let aggregate_revise_path = aggregate_revise_root
        .path()
        .join("sources/example_source.json");
    let previous = fs::read(&aggregate_revise_path).unwrap();
    fs::File::create(
        aggregate_revise_root
            .path()
            .join("sources/zz_aggregate_fill.json"),
    )
    .unwrap()
    .set_len((MAX_AGGREGATE_SOURCE_BYTES - previous.len()) as u64)
    .unwrap();
    let aggregate_revise_error = Store::new(aggregate_revise_root.path())
        .revise(Revision {
            key: authored.key,
            name: format!("{} expanded", authored.name),
            source_config: authored.source_config,
            selected_access_path: authored.selected_access_path,
            access_paths: authored.access_paths,
            source_support: authored.source_support,
        })
        .unwrap_err();
    assert_eq!(aggregate_revise_error.kind, ErrorKind::LimitExceeded);
    assert_eq!(fs::read(aggregate_revise_path).unwrap(), previous);
}

#[test]
fn aggregate_read_budget_charges_oversized_file_probes() {
    use sources::installed::{MAX_AGGREGATE_SOURCE_BYTES, MAX_SOURCE_BYTES};
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("sources");
    fs::create_dir_all(&directory).unwrap();
    let oversized_count = MAX_AGGREGATE_SOURCE_BYTES / (MAX_SOURCE_BYTES + 1) + 2;
    for index in 0..oversized_count {
        fs::File::create(directory.join(format!("oversized_{index:03}.json")))
            .unwrap()
            .set_len((MAX_SOURCE_BYTES + 1) as u64)
            .unwrap();
    }

    let snapshot = Snapshot::load(root.path()).unwrap();
    let per_file_diagnostics = snapshot
        .view()
        .diagnostics
        .iter()
        .filter(|item| item.code == "source_bytes_limit_exceeded")
        .count();
    assert!(per_file_diagnostics < oversized_count);
    assert!(snapshot
        .view()
        .diagnostics
        .iter()
        .any(|item| item.code == "aggregate_source_bytes_limit_exceeded"));
}

#[test]
fn source_count_bytes_and_document_diagnostics_are_bounded() {
    use sources::installed::{
        MAX_CUSTOM_SOURCE_DOCUMENTS, MAX_SOURCE_BYTES, MAX_SOURCE_DIAGNOSTICS_PER_DOCUMENT,
    };
    let count_root = tempfile::tempdir().unwrap();
    fs::create_dir_all(count_root.path().join("sources")).unwrap();
    for index in (0..=MAX_CUSTOM_SOURCE_DOCUMENTS).rev() {
        let key = format!("source_{index:04}");
        fs::write(
            count_root.path().join(format!("sources/{key}.json")),
            serde_json::to_vec(&serde_json::json!({
                "schemaVersion": 3,
                "key": key,
                "name": "Source",
                "status": "draft",
                "sourceConfig": {},
                "selectedAccessPath": {
                    "type": "profile_access_path",
                    "profileKey": "missing",
                    "pathKey": "missing"
                }
            }))
            .unwrap(),
        )
        .unwrap();
    }
    let snapshot = Snapshot::load(count_root.path()).unwrap();
    let diagnostic = snapshot
        .view()
        .diagnostics
        .iter()
        .find(|item| item.code == "custom_source_document_limit_exceeded")
        .unwrap();
    assert_eq!(
        diagnostic.details.as_ref().unwrap()["actual"],
        MAX_CUSTOM_SOURCE_DOCUMENTS + 1
    );
    assert_eq!(snapshot.view().sources.len(), MAX_CUSTOM_SOURCE_DOCUMENTS);
    assert_eq!(
        snapshot.view().sources.first().unwrap().document.key,
        "source_0000"
    );
    assert_eq!(
        snapshot.view().sources.last().unwrap().document.key,
        "source_4095"
    );
    assert!(snapshot.source("source_4096").is_none());

    let bytes_root = tempfile::tempdir().unwrap();
    fs::create_dir_all(bytes_root.path().join("sources")).unwrap();
    fs::write(
        bytes_root.path().join("sources/oversized.json"),
        vec![b' '; MAX_SOURCE_BYTES + 1],
    )
    .unwrap();
    let snapshot = Snapshot::load(bytes_root.path()).unwrap();
    assert!(snapshot
        .view()
        .diagnostics
        .iter()
        .any(|item| item.code == "source_bytes_limit_exceeded"));

    let diagnostics_root = tempfile::tempdir().unwrap();
    install(diagnostics_root.path(), true, SOURCE);
    let mut source: serde_json::Value = serde_json::from_str(SOURCE).unwrap();
    source["diagnostics"] = serde_json::Value::Array((0..=MAX_SOURCE_DIAGNOSTICS_PER_DOCUMENT).map(|index| serde_json::json!({
        "category": "registry", "code": format!("authored_{index}"), "message": "authored", "severity": "warning", "path": ""
    })).collect());
    fs::write(
        diagnostics_root.path().join("sources/example_source.json"),
        serde_json::to_vec_pretty(&source).unwrap(),
    )
    .unwrap();
    let snapshot = Snapshot::load(diagnostics_root.path()).unwrap();
    assert!(snapshot
        .view()
        .diagnostics
        .iter()
        .any(|item| item.code == "source_diagnostics_truncated"));
    assert!(snapshot.view().diagnostics.len() <= MAX_SOURCE_DIAGNOSTICS_PER_DOCUMENT);
}

#[test]
fn aggregate_source_bytes_and_snapshot_diagnostics_are_bounded() {
    use sources::installed::{
        MAX_AGGREGATE_SOURCE_BYTES, MAX_SOURCE_BYTES, MAX_SOURCE_DIAGNOSTICS_PER_SNAPSHOT,
    };
    let bytes_root = tempfile::tempdir().unwrap();
    fs::create_dir_all(bytes_root.path().join("sources")).unwrap();
    for index in 0..=(MAX_AGGREGATE_SOURCE_BYTES / MAX_SOURCE_BYTES) {
        fs::write(
            bytes_root
                .path()
                .join(format!("sources/aggregate_{index:03}.json")),
            vec![b' '; MAX_SOURCE_BYTES],
        )
        .unwrap();
    }
    fs::write(
        bytes_root.path().join("sources/zzz_oversized.json"),
        vec![b' '; MAX_SOURCE_BYTES + 1],
    )
    .unwrap();
    let snapshot = Snapshot::load(bytes_root.path()).unwrap();
    assert!(snapshot
        .view()
        .diagnostics
        .iter()
        .any(|item| item.code == "aggregate_source_bytes_limit_exceeded"));
    assert!(
        snapshot
            .view()
            .diagnostics
            .iter()
            .all(|item| item.code != "source_bytes_limit_exceeded"),
        "files after aggregate exhaustion must not be read"
    );

    let diagnostics_root = tempfile::tempdir().unwrap();
    fs::create_dir_all(diagnostics_root.path().join("sources")).unwrap();
    let template: serde_json::Value = serde_json::from_str(SOURCE).unwrap();
    let authored = (0..100).map(|index| serde_json::json!({
        "category": "registry", "code": format!("authored_{index}"), "message": "authored", "severity": "warning", "path": ""
    })).collect::<Vec<_>>();
    for index in 0..165 {
        let mut document = template.clone();
        let key = format!("diagnostics_{index:03}");
        document["key"] = serde_json::json!(key);
        document["diagnostics"] = serde_json::Value::Array(authored.clone());
        fs::write(
            diagnostics_root.path().join(format!("sources/{key}.json")),
            serde_json::to_vec(&document).unwrap(),
        )
        .unwrap();
    }
    let snapshot = Snapshot::load(diagnostics_root.path()).unwrap();
    assert_eq!(
        snapshot.view().diagnostics.len(),
        MAX_SOURCE_DIAGNOSTICS_PER_SNAPSHOT
    );
    assert_eq!(
        snapshot.view().diagnostics.last().unwrap().code,
        "source_snapshot_diagnostics_truncated"
    );
}
