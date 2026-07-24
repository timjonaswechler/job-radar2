use std::process::Command;
#[test]
fn top_level_registry_only_declares_family_modules() {
    let source = include_str!("../src/profile_dsl/primitives/mod.rs");
    let declarations = source
        .lines()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(
        declarations,
        [
            "pub mod acceptance;",
            "pub mod capture;",
            "pub mod cardinality;",
            "pub mod completeness;",
            "pub mod fetch;",
            "pub mod pagination;",
            "pub mod parse;",
            "pub mod predicate;",
            "pub mod select;",
            "pub mod transform;",
            "pub mod value;"
        ]
    );
    for marker in [
        "match ",
        "Regex::new",
        "serde_json::from",
        "compile_",
        "execute_",
        "provider",
        "greenhouse",
        "workday",
        "successfactors",
    ] {
        assert!(!source.contains(marker), "registry behavior {marker}");
    }
}
#[test]
fn completeness_assembly_has_no_productive_dependency_or_dispatch() {
    let source = include_str!("../src/profile_dsl/primitives/completeness.rs");
    for forbidden in [
        "profile_dsl::compiler",
        "profile_dsl::runtime",
        "reqwest::",
        "use tauri",
        "greenhouse",
        "workday",
        "successfactors",
        "provider_key",
        "source_key",
    ] {
        assert!(
            !source.contains(forbidden),
            "productive dependency {forbidden}"
        );
    }
    let model = include_str!("../src/profile_dsl/primitives/completeness/model.rs");
    for forbidden_policy in [
        "match owner",
        "(Family::Parse, \"text\")",
        "normalizeWhitespace",
        "maxErrorRatio",
        "src-tauri/src/profile_dsl/primitives/parse/",
    ] {
        assert!(
            !model.contains(forbidden_policy),
            "generic validator embeds Primitive/owner policy {forbidden_policy}"
        );
    }
    for marker in [
        "compile_source",
        "execute_detection_operation",
        "ProfileHttpClient",
        "BrowserAcquisition",
    ] {
        assert!(!source.contains(marker));
    }
}
#[test]
fn frozen_nul_safe_residue_pass_matches_every_checked_in_path_line_classification() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let output = Command::new("bash")
        .arg("scripts/check-primitive-residue.sh")
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest = include_str!("../../docs/agents/primitive-residue-classification.txt");
    let entries = manifest
        .lines()
        .filter(|v| !v.is_empty() && !v.starts_with('#'))
        .collect::<Vec<_>>();
    assert!(entries.len() > 150);
    assert!(entries.iter().all(|v| v.matches('\t').count() == 2
        && v.split('\t').nth(1).is_some_and(|class| matches!(
            class,
            "historical_documentation"
                | "active_contract_documentation"
                | "explicit_negative_or_contract_test"
                | "g02_removed_key_metadata"
                | "unrelated_agent_provider_retry"
                | "unrelated_browser_install_or_admin_retry"
                | "unrelated_frontend_ui_identifier"
                | "active_profile_contract"
                | "final_implementation_or_interface"
        ))));
    assert!(entries.iter().all(|entry| {
        let evidence = entry.split('\t').next().unwrap();
        let fields = evidence.splitn(5, ':').collect::<Vec<_>>();
        fields.len() == 5
            && fields[1].parse::<usize>().is_ok()
            && fields[2].parse::<usize>().is_ok()
            && !fields[3].is_empty()
            && !fields[4].is_empty()
    }));
    let same_line_hits = entries
        .iter()
        .map(|entry| {
            entry
                .split('\t')
                .next()
                .unwrap()
                .split(':')
                .take(2)
                .collect::<Vec<_>>()
                .join(":")
        })
        .collect::<Vec<_>>();
    assert!(same_line_hits.windows(2).any(|pair| pair[0] == pair[1]));
}

#[test]
fn residue_validator_rejects_path_based_historical_auto_classification() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let emitted = Command::new("bash")
        .args(["scripts/check-primitive-residue.sh", "--emit"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(emitted.status.success());
    let source = String::from_utf8(emitted.stdout).unwrap();
    let forged = source
        .lines()
        .map(|line| {
            format!(
                "{}\thistorical_documentation\tautomatically inferred from path",
                line
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let path = std::env::temp_dir().join(format!(
        "job-radar-g02-forged-residue-{}.txt",
        std::process::id()
    ));
    std::fs::write(&path, forged).unwrap();
    let output = Command::new("bash")
        .arg("scripts/check-primitive-residue.sh")
        .env("PRIMITIVE_RESIDUE_MANIFEST", &path)
        .current_dir(root)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(path);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("active, not historical"));
}
