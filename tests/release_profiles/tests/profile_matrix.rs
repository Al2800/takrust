use std::collections::HashSet;
use std::fs;
use std::path::Path;

use toml::Value;

#[test]
fn profile_matrix_tak_only_is_consistent() {
    let root_manifest = load_root_manifest();
    let tak_only = release_profile(&root_manifest, "tak_only");
    let workspace_members = workspace_members(&root_manifest);

    let included = profile_list(tak_only, "included_crates");
    let forbidden = profile_list(tak_only, "forbidden_crates");
    let commands = profile_list(tak_only, "acceptance_commands");

    assert_contains(&included, "rustak-core");
    assert_contains(&included, "rustak-wire");
    assert_contains(&included, "rustak-record");
    assert_contains(&forbidden, "rustak-sapient");
    assert_contains(&forbidden, "rustak-bridge");
    assert_contains_command(
        &commands,
        "cargo test --manifest-path tests/release_profiles/Cargo.toml -- --exact profile_matrix_tak_only_is_consistent",
    );
    assert_contains_command(
        &commands,
        "cargo test --manifest-path crates/rustak-record/Cargo.toml",
    );

    assert_no_duplicate_entries(&included);
    assert_no_duplicate_entries(&forbidden);
    assert_no_profile_overlap(&included, &forbidden);
    assert_crate_manifests_exist(&included);
    assert_crates_are_workspace_members(&included, &workspace_members);
}

#[test]
fn profile_matrix_tak_sapient_includes_bridge_components() {
    let root_manifest = load_root_manifest();
    let tak_sapient = release_profile(&root_manifest, "tak_sapient");
    let workspace_members = workspace_members(&root_manifest);

    let included = profile_list(tak_sapient, "included_crates");
    let forbidden = profile_list(tak_sapient, "forbidden_crates");
    let commands = profile_list(tak_sapient, "acceptance_commands");

    assert_contains(&included, "rustak-sapient");
    assert_contains(&included, "rustak-bridge");
    assert_contains(&included, "rustak-config");
    assert_contains_command(
        &commands,
        "cargo test --manifest-path tests/release_profiles/Cargo.toml -- --exact profile_matrix_tak_sapient_includes_bridge_components",
    );
    assert_contains_command(
        &commands,
        "cargo test --manifest-path crates/rustak-record/Cargo.toml recovery_",
    );
    assert!(
        forbidden.is_empty(),
        "tak_sapient should not forbid crates; found {forbidden:?}"
    );
    assert_no_duplicate_entries(&included);
    assert_no_duplicate_entries(&commands);
    assert_crate_manifests_exist(&included);
    assert_crates_are_workspace_members(&included, &workspace_members);
}

#[test]
fn conformance_doc_mentions_both_profiles() {
    let root_manifest = load_root_manifest();
    let tak_only = release_profile(&root_manifest, "tak_only");
    let tak_sapient = release_profile(&root_manifest, "tak_sapient");
    let tak_only_commands = profile_list(tak_only, "acceptance_commands");
    let tak_sapient_commands = profile_list(tak_sapient, "acceptance_commands");

    let doc_path = repo_root().join("docs").join("conformance.md");
    let contents = fs::read_to_string(doc_path).expect("conformance.md must exist");
    assert!(
        contents.contains("tak_only"),
        "tak_only profile missing from conformance.md"
    );
    assert!(
        contents.contains("tak_sapient"),
        "tak_sapient profile missing from conformance.md"
    );

    assert!(
        contents.contains("deterministic replay gate command"),
        "conformance.md should document deterministic replay gate command"
    );

    for command in tak_only_commands.iter().chain(tak_sapient_commands.iter()) {
        assert!(
            contents.contains(command),
            "conformance.md missing acceptance command `{command}` from release metadata"
        );
    }
}

#[test]
fn workspace_members_are_resolvable_and_include_release_matrix() {
    let root_manifest = load_root_manifest();
    let members = workspace_members(&root_manifest);

    assert_contains(&members, "xtask");
    assert_contains(&members, "tests/release_profiles");
    assert_no_duplicate_entries(&members);
    assert_workspace_members_exist(&members);
}

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root should be resolvable")
}

fn load_root_manifest() -> Value {
    let root_manifest_path = repo_root().join("Cargo.toml");
    let root_manifest =
        fs::read_to_string(root_manifest_path).expect("root Cargo.toml is readable");
    root_manifest
        .parse::<Value>()
        .expect("root Cargo.toml should parse as TOML")
}

fn release_profile<'a>(
    manifest: &'a Value,
    profile_name: &str,
) -> &'a toml::map::Map<String, Value> {
    manifest
        .get("workspace")
        .and_then(Value::as_table)
        .and_then(|workspace| workspace.get("metadata"))
        .and_then(Value::as_table)
        .and_then(|metadata| metadata.get("release_profiles"))
        .and_then(Value::as_table)
        .and_then(|profiles| profiles.get(profile_name))
        .and_then(Value::as_table)
        .unwrap_or_else(|| panic!("missing workspace.metadata.release_profiles.{profile_name}"))
}

fn workspace_members(manifest: &Value) -> Vec<String> {
    manifest
        .get("workspace")
        .and_then(Value::as_table)
        .and_then(|workspace| workspace.get("members"))
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing workspace.members"))
        .iter()
        .map(|entry| entry.as_str().unwrap_or_default().to_owned())
        .collect()
}

fn profile_list(profile: &toml::map::Map<String, Value>, key: &str) -> Vec<String> {
    profile
        .get(key)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing profile key `{key}`"))
        .iter()
        .map(|entry| entry.as_str().unwrap_or_default().to_owned())
        .collect()
}

fn assert_contains(entries: &[String], value: &str) {
    assert!(
        entries.iter().any(|entry| entry == value),
        "expected entry `{value}` not found; entries={entries:?}"
    );
}

fn assert_contains_command(entries: &[String], command: &str) {
    assert!(
        entries.iter().any(|entry| entry == command),
        "expected acceptance command `{command}` not found; entries={entries:?}"
    );
}

fn assert_no_duplicate_entries(entries: &[String]) {
    let unique: HashSet<&String> = entries.iter().collect();
    assert_eq!(
        unique.len(),
        entries.len(),
        "profile list contains duplicate entries: {entries:?}"
    );
}

fn assert_no_profile_overlap(left: &[String], right: &[String]) {
    let left_set: HashSet<&String> = left.iter().collect();
    let right_set: HashSet<&String> = right.iter().collect();
    let overlap: Vec<&String> = left_set.intersection(&right_set).copied().collect();
    assert!(
        overlap.is_empty(),
        "included and forbidden sets overlap: {overlap:?}"
    );
}

fn assert_crate_manifests_exist(crates: &[String]) {
    for crate_name in crates {
        let manifest = repo_root()
            .join("crates")
            .join(crate_name)
            .join("Cargo.toml");
        assert!(
            manifest.exists(),
            "crate manifest missing for profile entry `{crate_name}` at {}",
            manifest.display()
        );
    }
}

fn assert_crates_are_workspace_members(crates: &[String], workspace_members: &[String]) {
    for crate_name in crates {
        let expected_member = format!("crates/{crate_name}");
        assert!(
            workspace_members
                .iter()
                .any(|member| member == &expected_member),
            "release-profile crate `{crate_name}` must appear in workspace.members as `{expected_member}`; members={workspace_members:?}"
        );
    }
}

fn assert_workspace_members_exist(workspace_members: &[String]) {
    for member in workspace_members {
        let manifest = repo_root().join(member).join("Cargo.toml");
        assert!(
            manifest.exists(),
            "workspace member `{member}` is missing Cargo.toml at {}",
            manifest.display()
        );
    }
}
