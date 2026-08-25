//! Regression tests over the test fixtures in `tests/test-apis/`.
//!
//! Each fixture is a small standalone crate (excluded from the workspace) that exercises some part
//! of API extraction: item kinds, attributes, re-exports, and so on. `api("name")` builds the
//! fixture's rustdoc JSON with a nightly toolchain and returns its public API.
//!
//! Snapshot tests compare the rendered API against `tests/expected-output/<name>.txt`. When output
//! changes intentionally, re-bless the snapshots:
//!
//! ```sh
//! UPDATE_SNAPSHOTS=1 cargo test
//! ```

use std::path::PathBuf;

use public_api_tricks::PublicApiDiff;
use public_api_tricks::build;

/// The manifest of the test fixture crate `name` in `tests/test-apis/`.
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/test-apis")
        .join(name)
        .join("Cargo.toml")
}

/// Build the public API of the fixture crate `name`, without default features.
fn api(name: &str) -> public_api_tricks::PublicApi {
    build(&fixture(name), &["--no-default-features".to_string()])
        .unwrap_or_else(|e| panic!("building {name}: {e}"))
}

/// Assert the API of a fixture matches its blessed snapshot in `tests/expected-output/`.
fn assert_api_snapshot(fixture_name: &str) {
    let api = api(fixture_name);
    let actual = api.to_string();

    let snapshot_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/expected-output")
        .join(format!("{fixture_name}.txt"));

    if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
        std::fs::create_dir_all(snapshot_path.parent().unwrap()).unwrap();
        std::fs::write(&snapshot_path, &actual).unwrap();
        return;
    }

    let expected = std::fs::read_to_string(&snapshot_path).unwrap_or_else(|e| {
        panic!(
            "reading {}: {e} (run with UPDATE_SNAPSHOTS=1 to create)",
            snapshot_path.display()
        )
    });
    assert_eq!(
        actual, expected,
        "API of {fixture_name} changed; run with UPDATE_SNAPSHOTS=1 to bless"
    );
}

/// Systematically defines public items of all kinds and variants, including re-exports of an
/// external crate.
#[test]
fn comprehensive_api() {
    assert_api_snapshot("comprehensive_api");
}

/// Auto trait impls (Send, Sync, ...) are part of the complete output.
#[test]
fn auto_traits() {
    assert_api_snapshot("auto_traits");
}

/// A `[lib] name = "..."` override is honored when locating the rustdoc JSON and in rendered paths.
#[test]
fn other_lib_name() {
    let api = api("other-lib-name");
    let expected = "pub fn other_name::lib_name_differs_from_package_name() -> usize";
    assert!(api.items().any(|i| i.to_string() == expected), "got: {api}");
}

/// Diffing the same crate against itself is empty.
#[test]
fn example_api_no_diff() {
    let diff = PublicApiDiff::between(api("example_api-v0.1.0"), api("example_api-v0.1.0"));
    assert!(diff.is_empty(), "expected empty diff, got {diff:#?}");
}

/// v0.1.0 -> v0.2.0 adds `StructV2` (and changes existing items).
#[test]
fn example_api_diff_added() {
    let diff = PublicApiDiff::between(api("example_api-v0.1.0"), api("example_api-v0.2.0"));
    assert!(diff.removed.is_empty(), "removed: {:?}", diff.removed);
    assert!(
        diff.added
            .iter()
            .any(|i| i.to_string() == "pub struct example_api::StructV2"),
        "added: {:#?}",
        diff.added
    );
    // Changing an item's representation is detected as a change, not a removal plus an addition.
    assert!(
        diff.changed
            .iter()
            .any(|c| c.old.to_string() == "pub struct example_api::Struct"
                && c.new.to_string() == "#[non_exhaustive] pub struct example_api::Struct"),
        "changed: {:#?}",
        diff.changed
    );
}

/// v0.2.0 -> v0.1.0 removes items.
#[test]
fn example_api_diff_removed() {
    let diff = PublicApiDiff::between(api("example_api-v0.2.0"), api("example_api-v0.1.0"));
    assert!(diff.added.is_empty(), "added: {:?}", diff.added);
    assert!(
        diff.removed
            .iter()
            .any(|i| i.to_string() == "pub struct example_api::StructV2"),
        "removed: {:#?}",
        diff.removed
    );
}
