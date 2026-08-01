use std::{fs, path::Path};

#[test]
fn source_behavior_schemas_are_valid_json_with_unique_ids() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("schema");
    let mut ids = std::collections::BTreeSet::new();
    for path in schema_files(&root) {
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let id = value
            .get("$id")
            .and_then(|value| value.as_str())
            .expect("schema $id");
        assert!(ids.insert(id.to_owned()), "duplicate schema id {id}");
    }
    assert!(!ids.is_empty());
}

fn schema_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(schema_files(&path));
        } else if path.extension().and_then(|value| value.to_str()) == Some("json") {
            files.push(path);
        }
    }
    files
}
