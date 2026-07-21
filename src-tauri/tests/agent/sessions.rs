use job_radar_lib::agent::models::{ModelId, ProviderId, ReasoningLevel};
use job_radar_lib::agent::sessions::{
    AssistantBlock, AssistantUsage, CompactionReason, CompactionRecord, CompletedTurn,
    SessionAccess, SessionCheckpoint, SessionErrorCode, SessionId, StopReason,
};
use job_radar_lib::agent::testing::SessionTestHarness;
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, RngSeed};
use std::io::{BufRead, Read, Write};
use std::process::{Child, Command, Stdio};
use std::str::FromStr;
use tempfile::TempDir;
use uuid::Uuid;

const SESSION: &str = "01890f47-e8b0-7cc3-98c4-dc0c0c07398f";
fn id(value: &str) -> Uuid {
    Uuid::parse_str(value).unwrap()
}
fn harness(trash: bool) -> SessionTestHarness {
    let timestamps = (0..20)
        .map(|second| format!("2023-07-01T00:00:{second:02}Z"))
        .collect();
    let mut uuids = vec![id(SESSION)];
    uuids.extend((1_u128..40).map(|value| Uuid::from_u128(value << 96)));
    SessionTestHarness::new(timestamps, uuids, trash)
}
fn root(temp: &TempDir) -> std::path::PathBuf {
    temp.path().canonicalize().unwrap().join("agents")
}
fn install_fixture(temp: &TempDir, fixture: &str) -> std::path::PathBuf {
    let target = root(temp)
        .join("sessions")
        .join(format!("2023-07-01T00-00-00Z_{SESSION}.jsonl"));
    std::fs::copy(format!("tests/fixtures/agent_sessions/{fixture}"), &target).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    target
}

fn turn(text: &str) -> CompletedTurn {
    CompletedTurn::new(
        "synthetic user",
        vec![AssistantBlock::text(text)],
        "synthetic-api",
        ProviderId::new("synthetic-provider").unwrap(),
        ModelId::new("synthetic-model").unwrap(),
        AssistantUsage::default(),
        StopReason::Stop,
    )
}

#[test]
fn draft_is_ephemeral_then_publishes_and_reopens() {
    let temp = TempDir::new().unwrap();
    let harness = harness(true);
    let manager = harness.manager(&root(&temp)).unwrap();
    let mut draft = manager.create().unwrap();
    assert_eq!(
        std::fs::read_dir(root(&temp).join("sessions"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
            .count(),
        0
    );
    draft.set_name("  Synthetic\r\nname ".into()).unwrap();
    draft
        .select_model(
            ProviderId::new("synthetic-provider").unwrap(),
            ModelId::new("synthetic-model").unwrap(),
        )
        .unwrap();
    draft.set_reasoning_level(ReasoningLevel::Low).unwrap();
    draft
        .append_completed_turn(turn("synthetic assistant"))
        .unwrap();
    assert_eq!(draft.snapshot().access(), SessionAccess::Writable);
    assert_eq!(draft.snapshot().display_name(), "Synthetic name");
    assert_eq!(draft.snapshot().turns().len(), 1);
    let id = draft.snapshot().id();
    drop(draft);
    let reopened = manager.open(&id).unwrap();
    assert_eq!(reopened.snapshot().turns().len(), 1);
    assert_eq!(reopened.snapshot().reasoning_level(), ReasoningLevel::Low);
    let bytes = std::fs::read(
        std::fs::read_dir(root(&temp).join("sessions"))
            .unwrap()
            .filter_map(Result::ok)
            .find(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
            .unwrap()
            .path(),
    )
    .unwrap();
    assert!(bytes.ends_with(b"\n"));
    assert!(String::from_utf8(bytes).unwrap().contains("\"version\":3"));
}

#[test]
fn first_publication_is_atomic_no_overwrite_and_reopens_after_sync_failure() {
    let temp = TempDir::new().unwrap();
    let timestamps = vec!["2023-07-01T00:00:00Z".into(); 4];
    let duplicate_session = id(SESSION);
    let first = SessionTestHarness::new(
        timestamps.clone(),
        vec![
            duplicate_session,
            Uuid::from_u128(1 << 96),
            Uuid::from_u128(2 << 96),
            Uuid::from_u128(9 << 96),
        ],
        true,
    );
    let manager = first.manager(&root(&temp)).unwrap();
    let mut original = manager.create().unwrap();
    original
        .append_completed_turn(turn("original answer"))
        .unwrap();
    let path = std::fs::read_dir(root(&temp).join("sessions"))
        .unwrap()
        .find_map(|entry| {
            entry
                .ok()
                .filter(|entry| entry.path().extension().is_some_and(|x| x == "jsonl"))
        })
        .unwrap()
        .path();
    let original_bytes = std::fs::read(&path).unwrap();
    drop(original);

    let collision = SessionTestHarness::new(
        timestamps,
        vec![
            duplicate_session,
            Uuid::from_u128(3 << 96),
            Uuid::from_u128(4 << 96),
            Uuid::from_u128(10 << 96),
        ],
        true,
    );
    let mut colliding = collision.manager(&root(&temp)).unwrap().create().unwrap();
    assert_eq!(
        colliding
            .append_completed_turn(turn("must not replace"))
            .unwrap_err()
            .code(),
        SessionErrorCode::NotSaved
    );
    assert_eq!(std::fs::read(path).unwrap(), original_bytes);
}

#[test]
fn second_open_is_read_only_until_writer_drops() {
    let temp = TempDir::new().unwrap();
    let harness = harness(true);
    let manager = harness.manager(&root(&temp)).unwrap();
    let mut first = manager.create().unwrap();
    first.append_completed_turn(turn("answer")).unwrap();
    let id = first.snapshot().id();
    let mut second = manager.open(&id).unwrap();
    assert_eq!(second.snapshot().access(), SessionAccess::ReadOnlyLocked);
    assert_eq!(
        second.set_name("blocked".into()).unwrap_err().code(),
        SessionErrorCode::Locked
    );
    drop(first);
    let third = manager.open(&id).unwrap();
    assert_eq!(third.snapshot().access(), SessionAccess::Writable);
}

#[test]
fn typed_replay_and_errors_have_redacted_debug() {
    let provider = ProviderId::new("synthetic-provider").unwrap();
    let model = ModelId::new("synthetic-model").unwrap();
    let replay_turn = CompletedTurn::new(
        "CONTENT-CANARY",
        vec![AssistantBlock::signed_text(
            "TEXT-CANARY",
            "SIGNATURE-CANARY",
        )],
        "api",
        provider,
        model,
        AssistantUsage::default(),
        StopReason::Length,
    )
    .with_replay(None, Some("RESPONSE-CANARY".into()));
    let debug = format!("{replay_turn:?}");
    for canary in [
        "CONTENT-CANARY",
        "TEXT-CANARY",
        "SIGNATURE-CANARY",
        "RESPONSE-CANARY",
        "synthetic-provider",
    ] {
        assert!(!debug.contains(canary));
    }
    let temp = TempDir::new().unwrap();
    let harness = harness(true);
    let manager = harness.manager(&root(&temp)).unwrap();
    let mut handle = manager.create().unwrap();
    let persisted = CompletedTurn::new(
        "continuation user",
        vec![
            AssistantBlock::signed_text("SNAPSHOT-CONTENT-CANARY", "PERSISTED-SIGNATURE-CANARY"),
            AssistantBlock::thinking(
                "VISIBLE-THINKING",
                Some("THINKING-SIGNATURE-CANARY".into()),
                false,
            ),
            AssistantBlock::thinking("REDACTED-THINKING-CANARY", None, true),
        ],
        "api",
        ProviderId::new("synthetic-provider").unwrap(),
        ModelId::new("synthetic-model").unwrap(),
        AssistantUsage::default(),
        StopReason::Stop,
    )
    .with_replay(None, Some("PERSISTED-RESPONSE-CANARY".into()));
    handle.append_completed_turn(persisted).unwrap();
    assert!(!format!("{:?}", handle.snapshot()).contains("SNAPSHOT-CONTENT-CANARY"));
    let continuation = harness.continuation(&handle);
    assert_eq!(continuation[1].signature_count(), 2);
    assert!(continuation[1].has_response_id());
    assert_eq!(continuation[1].redacted_thinking_count(), 1);
    assert!(continuation[1]
        .text()
        .iter()
        .any(|text| text == "VISIBLE-THINKING"));
    assert!(!format!("{continuation:?}").contains("REDACTED-THINKING-CANARY"));
    let compaction = CompactionRecord::new(
        "COMPACTION-SUMMARY-CANARY",
        "11111111",
        42,
        Some(CompactionReason::Manual),
    );
    assert!(!format!("{compaction:?}").contains("COMPACTION-SUMMARY-CANARY"));

    let bad = SessionId::from_str("../../private").unwrap_err();
    assert_eq!(bad.code(), SessionErrorCode::InvalidSessionId);
    assert!(!format!("{bad:?}").contains("private"));
}

#[test]
fn trash_failure_preserves_and_success_moves_without_delete_fallback() {
    let temp = TempDir::new().unwrap();
    let fail = harness(false);
    let manager = fail.manager(&root(&temp)).unwrap();
    let mut h = manager.create().unwrap();
    h.append_completed_turn(turn("answer")).unwrap();
    let id = h.snapshot().id();
    drop(h);
    assert_eq!(
        manager.move_to_trash(&id).unwrap_err().code(),
        SessionErrorCode::TrashFailed
    );
    assert!(manager.open(&id).is_ok());
    let success = harness(true);
    let manager = success.manager(&root(&temp)).unwrap();
    manager.move_to_trash(&id).unwrap();
    assert_eq!(success.trashed_paths().len(), 1);
    assert_eq!(
        manager.open(&id).unwrap_err().code(),
        SessionErrorCode::NotFound
    );
}

#[test]
fn pinned_fixtures_classify_through_public_seam() {
    let cases = [
        ("valid-minimal-v3.jsonl", SessionAccess::Writable, 1),
        ("readonly-v1.jsonl", SessionAccess::ReadOnlyUnsupported, 0),
        (
            "readonly-extra-field-v3.jsonl",
            SessionAccess::ReadOnlyUnsupported,
            0,
        ),
        ("damaged-unknown-type-v3.jsonl", SessionAccess::Damaged, 0),
    ];
    for (fixture, access, turns) in cases {
        let temp = TempDir::new().unwrap();
        let harness = harness(true);
        let manager = harness.manager(&root(&temp)).unwrap();
        install_fixture(&temp, fixture);
        let handle = manager
            .open(&SessionId::from_str(SESSION).unwrap())
            .unwrap();
        assert_eq!(handle.snapshot().access(), access, "{fixture}");
        assert_eq!(handle.snapshot().turns().len(), turns, "{fixture}");
    }
}

#[test]
fn existing_turn_ids_retry_collisions_with_every_persisted_id() {
    let temp = TempDir::new().unwrap();
    let timestamps = vec!["2023-07-01T00:00:00Z".into(), "2023-07-01T00:00:01Z".into()];
    let uuid = |prefix: u128| Uuid::from_u128(prefix << 96);
    let harness = SessionTestHarness::new(
        timestamps,
        vec![
            id(SESSION),
            uuid(1),
            uuid(2),
            uuid(3),
            uuid(1),
            uuid(2),
            uuid(4),
            uuid(1),
            uuid(5),
        ],
        true,
    );
    let manager = harness.manager(&root(&temp)).unwrap();
    let mut handle = manager.create().unwrap();
    handle.append_completed_turn(turn("first")).unwrap();
    handle.append_completed_turn(turn("second")).unwrap();
    let path = std::fs::read_dir(root(&temp).join("sessions"))
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "jsonl")
        })
        .unwrap()
        .path();
    let ids: Vec<String> = std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .skip(1)
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line).unwrap()["id"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(ids.len(), unique.len());
    assert_eq!(&ids[2..], &["00000004", "00000005"]);
}

#[test]
fn bounded_final_suffix_recovery_discards_only_the_suffix() {
    use std::io::Write;
    let temp = TempDir::new().unwrap();
    let harness = harness(true);
    let manager = harness.manager(&root(&temp)).unwrap();
    let mut handle = manager.create().unwrap();
    handle.append_completed_turn(turn("answer")).unwrap();
    let id = handle.snapshot().id();
    drop(handle);
    let path = std::fs::read_dir(root(&temp).join("sessions"))
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "jsonl")
        })
        .unwrap()
        .path();
    let original = std::fs::read(&path).unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"{\"type\":\"session_info\",}\n")
        .unwrap();
    let recovered = manager.open(&id).unwrap();
    assert_eq!(recovered.snapshot().access(), SessionAccess::Writable);
    assert_eq!(recovered.snapshot().recovery_notices().len(), 1);
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn large_session_recovery_discards_complete_user_and_truncated_assistant() {
    use std::io::Write;
    #[cfg(unix)]
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let temp = TempDir::new().unwrap();
    let harness = harness(true);
    let manager = harness.manager(&root(&temp)).unwrap();
    let path = root(&temp)
        .join("sessions")
        .join(format!("2023-07-01T00-00-00Z_{SESSION}.jsonl"));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&path).unwrap();
    writeln!(file, "{}", serde_json::json!({"type":"session","version":3,"id":SESSION,"timestamp":"2023-07-01T00:00:00Z","cwd":""})).unwrap();
    let large_text = "x".repeat(11 * 1024 * 1024);
    let mut parent: Option<String> = None;
    for index in 0..3_u8 {
        let user_id = format!("{:08x}", index * 2 + 1);
        let assistant_id = format!("{:08x}", index * 2 + 2);
        writeln!(file, "{}", serde_json::json!({"type":"message","id":user_id,"parentId":parent,"timestamp":"2023-07-01T00:00:01Z","message":{"role":"user","content":"synthetic","timestamp":1688169601000_i64}})).unwrap();
        writeln!(file, "{}", serde_json::json!({"type":"message","id":assistant_id,"parentId":user_id,"timestamp":"2023-07-01T00:00:02Z","message":{"role":"assistant","content":[{"type":"text","text":large_text}],"api":"synthetic-api","provider":"synthetic-provider","model":"synthetic-model","usage":{"input":1,"output":1,"cacheRead":0,"cacheWrite":0,"totalTokens":2,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":1688169602000_i64}})).unwrap();
        parent = Some(assistant_id);
    }
    let valid_len = file.metadata().unwrap().len();
    writeln!(file, "{}", serde_json::json!({"type":"message","id":"00000007","parentId":parent,"timestamp":"2023-07-01T00:00:03Z","message":{"role":"user","content":"discarded user canary","timestamp":1688169603000_i64}})).unwrap();
    file.write_all(b"{\"type\":\"message\",\"id\":\"00000008\",\"discarded-assistant-canary\":}\n")
        .unwrap();
    file.sync_all().unwrap();
    assert!(file.metadata().unwrap().len() > 32 * 1024 * 1024);
    drop(file);
    #[cfg(unix)]
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let recovered = manager
        .open(&SessionId::from_str(SESSION).unwrap())
        .unwrap();
    assert_eq!(recovered.snapshot().access(), SessionAccess::Writable);
    assert_eq!(recovered.snapshot().turns().len(), 3);
    assert_eq!(recovered.snapshot().recovery_notices().len(), 1);
    assert_eq!(std::fs::metadata(path).unwrap().len(), valid_len);
}

#[test]
fn same_length_external_change_poisoning_blocks_mutation() {
    let temp = TempDir::new().unwrap();
    let harness = harness(true);
    let manager = harness.manager(&root(&temp)).unwrap();
    let mut handle = manager.create().unwrap();
    handle.append_completed_turn(turn("answer")).unwrap();
    let path = std::fs::read_dir(root(&temp).join("sessions"))
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "jsonl")
        })
        .unwrap()
        .path();
    let bytes = std::fs::read(&path).unwrap();
    let changed = String::from_utf8(bytes)
        .unwrap()
        .replace("answer", "change");
    std::fs::write(&path, changed).unwrap();
    assert_eq!(
        handle.set_name("blocked".into()).unwrap_err().code(),
        SessionErrorCode::ExternalChange
    );
    assert_eq!(
        handle.set_name("still blocked".into()).unwrap_err().code(),
        SessionErrorCode::NotSaved
    );
}

#[test]
fn conformance_fixtures_cover_reconstruction_and_unsupported_context() {
    let temp = TempDir::new().unwrap();
    let test = harness(true);
    let manager = test.manager(&root(&temp)).unwrap();
    install_fixture(&temp, "active-off-path-v3.jsonl");
    let handle = manager
        .open(&SessionId::from_str(SESSION).unwrap())
        .unwrap();
    assert_eq!(handle.snapshot().turns().len(), 2);
    assert_eq!(handle.snapshot().turns()[1].user(), "Active path");
    assert_eq!(test.continuation(&handle).len(), 4);

    let temp = TempDir::new().unwrap();
    let test = harness(true);
    let manager = test.manager(&root(&temp)).unwrap();
    install_fixture(&temp, "history-name-replay-v3.jsonl");
    let handle = manager
        .open(&SessionId::from_str(SESSION).unwrap())
        .unwrap();
    assert_eq!(handle.snapshot().display_name(), "History question");
    assert_eq!(
        handle.snapshot().selected_provider().unwrap().as_str(),
        "new-provider"
    );
    assert_eq!(handle.snapshot().reasoning_level(), ReasoningLevel::Low);
    let continuation = test.continuation(&handle);
    assert_eq!(continuation[1].signature_count(), 2);
    assert!(continuation[1].has_response_id());

    for fixture in [
        "readonly-assistant-tool-call-v3.jsonl",
        "readonly-tool-use-reason-v3.jsonl",
        "readonly-user-image-v3.jsonl",
        "readonly-tool-result-v3.jsonl",
        "readonly-bash-message-v3.jsonl",
        "readonly-recognized-families-v3.jsonl",
    ] {
        let temp = TempDir::new().unwrap();
        let test = harness(true);
        let manager = test.manager(&root(&temp)).unwrap();
        install_fixture(&temp, fixture);
        let handle = manager
            .open(&SessionId::from_str(SESSION).unwrap())
            .unwrap();
        assert_eq!(
            handle.snapshot().access(),
            SessionAccess::ReadOnlyUnsupported
        );
        if matches!(
            fixture,
            "readonly-assistant-tool-call-v3.jsonl" | "readonly-tool-use-reason-v3.jsonl"
        ) {
            assert_eq!(handle.snapshot().turns().len(), 1);
            assert!(test.continuation(&handle).is_empty());
        } else if fixture == "readonly-user-image-v3.jsonl" {
            assert!(handle.snapshot().turns().is_empty());
            assert!(test.continuation(&handle).is_empty());
        } else {
            assert_eq!(test.continuation(&handle).len(), 2);
        }
    }

    let temp = TempDir::new().unwrap();
    let test = harness(true);
    let manager = test.manager(&root(&temp)).unwrap();
    install_fixture(&temp, "valid-compaction-empty-details-v3.jsonl");
    let handle = manager
        .open(&SessionId::from_str(SESSION).unwrap())
        .unwrap();
    assert_eq!(handle.snapshot().compactions().len(), 1);
    assert_eq!(handle.snapshot().compactions()[0].reason(), None);
    assert_eq!(test.continuation(&handle).len(), 3);
}

#[test]
fn malformed_graph_fixtures_are_damaged_even_with_unsupported_entries() {
    for fixture in [
        "damaged-pair-with-unsupported-v3.jsonl",
        "damaged-duplicate-v3.jsonl",
        "damaged-missing-parent-v3.jsonl",
        "damaged-cycle-v3.jsonl",
        "damaged-recognized-entry-v3.jsonl",
        "damaged-unsupported-message-v3.jsonl",
        "readonly-assistant-image-v3.jsonl",
        "damaged-optional-fields-v3.jsonl",
        "damaged-compaction-from-hook-v3.jsonl",
        "damaged-stop-reason-v3.jsonl",
    ] {
        let temp = TempDir::new().unwrap();
        let test = harness(true);
        let manager = test.manager(&root(&temp)).unwrap();
        install_fixture(&temp, fixture);
        let handle = manager
            .open(&SessionId::from_str(SESSION).unwrap())
            .unwrap();
        assert_eq!(
            handle.snapshot().access(),
            SessionAccess::Damaged,
            "{fixture}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        rng_seed: RngSeed::Fixed(0x2265_3551),
        ..ProptestConfig::default()
    })]

    #[test]
    fn arbitrary_session_bytes_fail_closed_without_mutation(payload in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let temp = TempDir::new().unwrap();
        let manager = harness(true).manager(&root(&temp)).unwrap();
        let path = root(&temp)
            .join("sessions")
            .join(format!("2023-07-01T00-00-00Z_{SESSION}.jsonl"));
        let mut invalid = vec![b'!'];
        invalid.extend_from_slice(&payload);
        std::fs::write(&path, &invalid).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let opened = manager.open(&SessionId::from_str(SESSION).unwrap()).unwrap();
        prop_assert_eq!(opened.snapshot().access(), SessionAccess::Damaged);
        prop_assert_eq!(std::fs::read(path).unwrap(), invalid);
    }

    #[test]
    fn malformed_parent_graphs_are_damaged(parent in "[0-9a-f]{8}") {
        let temp = TempDir::new().unwrap();
        let manager = harness(true).manager(&root(&temp)).unwrap();
        let path = root(&temp)
            .join("sessions")
            .join(format!("2023-07-01T00-00-00Z_{SESSION}.jsonl"));
        let bytes = format!(
            "{{\"type\":\"session\",\"version\":3,\"id\":\"{SESSION}\",\"timestamp\":\"2023-07-01T00:00:00Z\",\"cwd\":\"\"}}\n{{\"type\":\"session_info\",\"id\":\"11111111\",\"parentId\":\"{parent}\",\"timestamp\":\"2023-07-01T00:00:01Z\",\"name\":\"synthetic\"}}\n"
        );
        std::fs::write(&path, &bytes).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let opened = manager.open(&SessionId::from_str(SESSION).unwrap()).unwrap();
        prop_assert_eq!(opened.snapshot().access(), SessionAccess::Damaged);
        prop_assert_eq!(std::fs::read_to_string(path).unwrap(), bytes);
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 3,
        rng_seed: RngSeed::Fixed(0x2265_495a),
        ..ProptestConfig::default()
    })]

    #[test]
    fn oversized_final_frames_fail_closed(excess in 1usize..64) {
        let temp = TempDir::new().unwrap();
        let manager = harness(true).manager(&root(&temp)).unwrap();
        let path = root(&temp)
            .join("sessions")
            .join(format!("2023-07-01T00-00-00Z_{SESSION}.jsonl"));
        let mut bytes = vec![b'x'; 16 * 1024 * 1024 + excess];
        bytes[0] = b'{';
        std::fs::write(&path, &bytes).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let opened = manager.open(&SessionId::from_str(SESSION).unwrap()).unwrap();
        prop_assert_eq!(opened.snapshot().access(), SessionAccess::Damaged);
        prop_assert_eq!(std::fs::metadata(path).unwrap().len(), bytes.len() as u64);
    }
}

#[test]
fn structural_recovery_handles_whitespace_and_invalid_utf8_but_not_ambiguity() {
    let fixture_temp = TempDir::new().unwrap();
    let fixture_test = harness(true);
    let fixture_manager = fixture_test.manager(&root(&fixture_temp)).unwrap();
    install_fixture(&fixture_temp, "recoverable-whitespace-metadata-v3.jsonl");
    let recovered = fixture_manager
        .open(&SessionId::from_str(SESSION).unwrap())
        .unwrap();
    assert_eq!(recovered.snapshot().access(), SessionAccess::Writable);
    assert_eq!(recovered.snapshot().recovery_notices().len(), 1);

    let fixture_temp = TempDir::new().unwrap();
    let fixture_test = harness(true);
    let fixture_manager = fixture_test.manager(&root(&fixture_temp)).unwrap();
    let fixture_path = install_fixture(&fixture_temp, "damaged-ambiguous-final-v3.jsonl");
    let fixture_bytes = std::fs::read(&fixture_path).unwrap();
    let damaged = fixture_manager
        .open(&SessionId::from_str(SESSION).unwrap())
        .unwrap();
    assert_eq!(damaged.snapshot().access(), SessionAccess::Damaged);
    assert_eq!(std::fs::read(fixture_path).unwrap(), fixture_bytes);

    let recoverable_suffixes: Vec<Vec<u8>> = vec![
        b"  { \"type\" : \"session_info\" , \"id\" : \"broken\"  \n".to_vec(),
        [b"{\"type\":\"session_info\",\"name\":\"".as_slice(), &[0xff], b"\n"].concat(),
        [
            b"{\"type\":\"message\",\"id\":\"aaaaaaaa\",\"parentId\":\"00000002\",\"timestamp\":\"2023-07-01T00:00:03Z\",\"message\":{\"role\":\"user\",\"content\":\"discard\",\"timestamp\":1}}\n".as_slice(),
            b" { \"type\" : \"message\", \"message\": \"".as_slice(),
            &[0xff],
            b"\n",
        ]
        .concat(),
    ];
    for suffix in recoverable_suffixes {
        let temp = TempDir::new().unwrap();
        let test = harness(true);
        let manager = test.manager(&root(&temp)).unwrap();
        let mut handle = manager.create().unwrap();
        handle.append_completed_turn(turn("answer")).unwrap();
        let id = handle.snapshot().id();
        drop(handle);
        let path = std::fs::read_dir(root(&temp).join("sessions"))
            .unwrap()
            .find_map(|entry| {
                entry
                    .ok()
                    .filter(|entry| entry.path().extension().is_some_and(|x| x == "jsonl"))
            })
            .unwrap()
            .path();
        let original = std::fs::read(&path).unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(&suffix)
            .unwrap();
        let recovered = manager.open(&id).unwrap();
        assert_eq!(recovered.snapshot().access(), SessionAccess::Writable);
        assert_eq!(std::fs::read(path).unwrap(), original);
    }

    let temp = TempDir::new().unwrap();
    let test = harness(true);
    let manager = test.manager(&root(&temp)).unwrap();
    let mut handle = manager.create().unwrap();
    handle.append_completed_turn(turn("answer")).unwrap();
    let id = handle.snapshot().id();
    drop(handle);
    let path = std::fs::read_dir(root(&temp).join("sessions"))
        .unwrap()
        .find_map(|entry| {
            entry
                .ok()
                .filter(|entry| entry.path().extension().is_some_and(|x| x == "jsonl"))
        })
        .unwrap()
        .path();
    let mut damaged = std::fs::read(&path).unwrap();
    damaged.extend_from_slice(b"{\"payload\":\"ambiguous\",}\n");
    std::fs::write(&path, &damaged).unwrap();
    let opened = manager.open(&id).unwrap();
    assert_eq!(opened.snapshot().access(), SessionAccess::Damaged);
    assert_eq!(std::fs::read(path).unwrap(), damaged);
}

#[test]
fn checkpoint_faults_cover_publication_append_recovery_lock_and_trash() {
    for checkpoint in [
        SessionCheckpoint::TempWrite,
        SessionCheckpoint::TempSync,
        SessionCheckpoint::Publish,
        SessionCheckpoint::Lock,
    ] {
        let temp = TempDir::new().unwrap();
        let test = harness(true).fail_at([checkpoint]);
        let manager = test.manager(&root(&temp)).unwrap();
        let mut draft = manager.create().unwrap();
        assert!(draft.append_completed_turn(turn("answer")).is_err());
        assert_eq!(
            std::fs::read_dir(root(&temp).join("sessions"))
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().is_some_and(|x| x == "jsonl"))
                .count(),
            0
        );
    }

    let temp = TempDir::new().unwrap();
    let test = harness(true).fail_at([SessionCheckpoint::DirectorySync]);
    let manager = test.manager(&root(&temp)).unwrap();
    let mut draft = manager.create().unwrap();
    assert_eq!(
        draft
            .append_completed_turn(turn("answer"))
            .unwrap_err()
            .code(),
        SessionErrorCode::NotSaved
    );
    drop(draft);
    assert_eq!(manager.list().unwrap().len(), 1);

    let temp = TempDir::new().unwrap();
    let setup = harness(true);
    let manager = setup.manager(&root(&temp)).unwrap();
    let mut handle = manager.create().unwrap();
    handle.append_completed_turn(turn("first")).unwrap();
    let id = handle.snapshot().id();
    drop(handle);
    let failing = harness(true).fail_at([SessionCheckpoint::AppendWrite]);
    let manager = failing.manager(&root(&temp)).unwrap();
    let mut handle = manager.open(&id).unwrap();
    assert_eq!(
        handle
            .append_completed_turn(turn("second"))
            .unwrap_err()
            .code(),
        SessionErrorCode::NotSaved
    );
    drop(handle);
    let reopened = setup.manager(&root(&temp)).unwrap().open(&id).unwrap();
    assert_eq!(reopened.snapshot().turns().len(), 1);
    assert_eq!(reopened.snapshot().recovery_notices().len(), 1);

    let temp = TempDir::new().unwrap();
    let setup = harness(true);
    let manager = setup.manager(&root(&temp)).unwrap();
    let mut handle = manager.create().unwrap();
    handle.append_completed_turn(turn("first")).unwrap();
    let id = handle.snapshot().id();
    drop(handle);
    let failing = harness(true).fail_at([SessionCheckpoint::AppendSync]);
    let manager = failing.manager(&root(&temp)).unwrap();
    let mut handle = manager.open(&id).unwrap();
    assert_eq!(
        handle
            .append_completed_turn(turn("complete but unsynchronized"))
            .unwrap_err()
            .code(),
        SessionErrorCode::NotSaved
    );
    drop(handle);
    assert_eq!(
        setup
            .manager(&root(&temp))
            .unwrap()
            .open(&id)
            .unwrap()
            .snapshot()
            .turns()
            .len(),
        2
    );

    for checkpoint in [SessionCheckpoint::Truncate, SessionCheckpoint::TruncateSync] {
        let temp = TempDir::new().unwrap();
        let setup = harness(true);
        let manager = setup.manager(&root(&temp)).unwrap();
        let mut handle = manager.create().unwrap();
        handle.append_completed_turn(turn("first")).unwrap();
        let id = handle.snapshot().id();
        drop(handle);
        let path = std::fs::read_dir(root(&temp).join("sessions"))
            .unwrap()
            .find_map(|entry| {
                entry
                    .ok()
                    .filter(|entry| entry.path().extension().is_some_and(|x| x == "jsonl"))
            })
            .unwrap()
            .path();
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"{\"type\":\"session_info\",}\n")
            .unwrap();
        let failing = harness(true).fail_at([checkpoint]);
        assert_eq!(
            failing
                .manager(&root(&temp))
                .unwrap()
                .open(&id)
                .unwrap_err()
                .code(),
            SessionErrorCode::NotSaved
        );
        if checkpoint == SessionCheckpoint::Truncate {
            assert!(std::fs::read(&path)
                .unwrap()
                .ends_with(b"session_info\",}\n"));
        } else {
            assert_eq!(
                setup
                    .manager(&root(&temp))
                    .unwrap()
                    .open(&id)
                    .unwrap()
                    .snapshot()
                    .turns()
                    .len(),
                1
            );
        }
    }

    let temp = TempDir::new().unwrap();
    let setup = harness(true);
    let manager = setup.manager(&root(&temp)).unwrap();
    let mut handle = manager.create().unwrap();
    handle.append_completed_turn(turn("first")).unwrap();
    let id = handle.snapshot().id();
    drop(handle);
    let failing = harness(true).fail_at([SessionCheckpoint::Trash]);
    assert_eq!(
        failing
            .manager(&root(&temp))
            .unwrap()
            .move_to_trash(&id)
            .unwrap_err()
            .code(),
        SessionErrorCode::TrashFailed
    );
    assert!(setup.manager(&root(&temp)).unwrap().open(&id).is_ok());
}

struct Worker {
    child: Child,
    output: std::io::BufReader<std::process::ChildStdout>,
}

fn spawn_worker(
    mode: &str,
    agents_root: &std::path::Path,
    id: Option<&SessionId>,
    checkpoint: Option<&str>,
) -> Worker {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "sessions::subprocess_worker", "--nocapture"])
        .env("JOB_RADAR_SUBPROCESS_MODE", mode)
        .env("JOB_RADAR_SUBPROCESS_ROOT", agents_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if let Some(id) = id {
        command.env("JOB_RADAR_SUBPROCESS_ID", id.as_str());
    }
    if let Some(checkpoint) = checkpoint {
        command.env("JOB_RADAR_SESSION_CHECKPOINT", checkpoint);
    }
    let mut child = command.spawn().unwrap();
    let output = std::io::BufReader::new(child.stdout.take().unwrap());
    Worker { child, output }
}

fn await_marker(worker: &mut Worker, marker: &str) {
    let mut line = String::new();
    loop {
        line.clear();
        assert_ne!(
            worker.output.read_line(&mut line).unwrap(),
            0,
            "worker exited before {marker}"
        );
        if line.trim() == marker {
            return;
        }
    }
}

#[test]
fn subprocess_worker() {
    let Ok(mode) = std::env::var("JOB_RADAR_SUBPROCESS_MODE") else {
        return;
    };
    let root = std::path::PathBuf::from(std::env::var_os("JOB_RADAR_SUBPROCESS_ROOT").unwrap());
    let manager =
        job_radar_lib::agent::sessions::SessionManager::from_agents_data_root(&root).unwrap();
    match mode.as_str() {
        "publish" => {
            let mut handle = manager.create().unwrap();
            let _ = handle.append_completed_turn(turn("child publication"));
        }
        "partial-append" => {
            let id =
                SessionId::from_str(&std::env::var("JOB_RADAR_SUBPROCESS_ID").unwrap()).unwrap();
            let mut handle = manager.open(&id).unwrap();
            let _ = handle.append_completed_turn(turn("child partial append"));
        }
        "lock-holder" | "snapshot-writer" => {
            let id =
                SessionId::from_str(&std::env::var("JOB_RADAR_SUBPROCESS_ID").unwrap()).unwrap();
            let mut handle = manager.open(&id).unwrap();
            println!("READY");
            std::io::stdout().flush().unwrap();
            let mut release = [0_u8; 1];
            std::io::stdin().read_exact(&mut release).unwrap();
            if mode == "snapshot-writer" {
                handle
                    .append_completed_turn(turn("child complete append"))
                    .unwrap();
                println!("APPENDED");
                std::io::stdout().flush().unwrap();
                std::io::stdin().read_exact(&mut release).unwrap();
            }
        }
        _ => panic!("unknown subprocess mode"),
    }
}

#[test]
fn subprocess_crash_and_snapshot_contracts_use_explicit_ipc() {
    let temp = TempDir::new().unwrap();
    let manager =
        job_radar_lib::agent::sessions::SessionManager::from_agents_data_root(&root(&temp))
            .unwrap();
    drop(manager);
    let mut worker = spawn_worker("publish", &root(&temp), None, Some("publish"));
    await_marker(&mut worker, "CHECKPOINT publish");
    worker.child.kill().unwrap();
    worker.child.wait().unwrap();
    assert!(
        job_radar_lib::agent::sessions::SessionManager::from_agents_data_root(&root(&temp))
            .unwrap()
            .list()
            .unwrap()
            .is_empty()
    );

    let temp = TempDir::new().unwrap();
    let test = harness(true);
    let manager = test.manager(&root(&temp)).unwrap();
    let mut initial = manager.create().unwrap();
    initial.append_completed_turn(turn("initial")).unwrap();
    let id = initial.snapshot().id();
    drop(initial);
    let mut worker = spawn_worker(
        "partial-append",
        &root(&temp),
        Some(&id),
        Some("append-write"),
    );
    await_marker(&mut worker, "CHECKPOINT append-write");
    worker.child.kill().unwrap();
    worker.child.wait().unwrap();
    let reopened = test.manager(&root(&temp)).unwrap().open(&id).unwrap();
    assert_eq!(reopened.snapshot().turns().len(), 1);
    assert_eq!(reopened.snapshot().recovery_notices().len(), 1);
    drop(reopened);

    let mut holder = spawn_worker("lock-holder", &root(&temp), Some(&id), None);
    await_marker(&mut holder, "READY");
    let secondary = test.manager(&root(&temp)).unwrap().open(&id).unwrap();
    assert_eq!(secondary.snapshot().access(), SessionAccess::ReadOnlyLocked);
    drop(secondary);
    holder.child.kill().unwrap();
    holder.child.wait().unwrap();
    let writable = test.manager(&root(&temp)).unwrap().open(&id).unwrap();
    assert_eq!(writable.snapshot().access(), SessionAccess::Writable);
    drop(writable);

    let mut writer = spawn_worker("snapshot-writer", &root(&temp), Some(&id), None);
    await_marker(&mut writer, "READY");
    let mut snapshot = test.manager(&root(&temp)).unwrap().open(&id).unwrap();
    assert_eq!(snapshot.snapshot().access(), SessionAccess::ReadOnlyLocked);
    assert_eq!(snapshot.snapshot().turns().len(), 1);
    writer
        .child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"g")
        .unwrap();
    await_marker(&mut writer, "APPENDED");
    assert_eq!(snapshot.snapshot().turns().len(), 1);
    snapshot.reload().unwrap();
    assert_eq!(snapshot.snapshot().turns().len(), 2);
    writer.child.kill().unwrap();
    writer.child.wait().unwrap();
}

#[cfg(unix)]
#[test]
fn storage_permissions_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let harness = harness(true);
    let manager = harness.manager(&root(&temp)).unwrap();
    let mut h = manager.create().unwrap();
    h.append_completed_turn(turn("answer")).unwrap();
    assert_eq!(
        std::fs::metadata(root(&temp)).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::metadata(root(&temp).join("sessions"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let file = std::fs::read_dir(root(&temp).join("sessions"))
        .unwrap()
        .filter_map(Result::ok)
        .find(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
        .unwrap();
    assert_eq!(file.metadata().unwrap().permissions().mode() & 0o777, 0o600);
}
