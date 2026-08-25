//! Integration tests against the semver-trick-style test fixtures.

use std::path::PathBuf;

use public_api_tricks::{PublicApi, PublicApiDiff};

/// Build the public API of the fixture crate `name` with the given cargo args.
fn api(name: &str, args: &[String]) -> PublicApi {
    public_api_tricks::build(&fixture(name), args).unwrap()
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/test-apis")
        .join(name)
        .join("Cargo.toml")
}

/// A crate that re-exports an external crate (glob, module, and a rename) has
/// the external items inlined at their re-exported paths instead of an opaque
/// `pub use`.
#[test]
fn reexport_external() {
    let api = api("reexport_external", &["--no-default-features".to_string()]);

    let text: Vec<String> = api.items().map(|i| i.to_string()).collect();

    // The re-exported struct appears at its re-exported path, granularly.
    assert!(
        text.iter()
            .any(|l| l == "pub struct reexport_external::SomeStruct"),
        "missing re-exported struct; got:\n{}",
        text.join("\n")
    );
    // And the glob re-export is not left opaque.
    assert!(
        !text
            .iter()
            .any(|l| l.contains("<<reexport_external_dependency")),
        "opaque re-export left behind"
    );
    // The crate's own item is present.
    assert!(
        text.iter()
            .any(|l| l == "pub fn reexport_external::own_fn() -> u32")
    );
}

/// Diffing a crate against itself yields an empty diff (and, more generally,
/// a semver-tricked backport compares equal to the native API it replaces).
#[test]
fn empty_diff_for_identical_api() {
    let args = vec!["--no-default-features".to_string()];
    let a = api("reexport_external", &args);
    let b = api("reexport_external", &args);
    let diff = PublicApiDiff::between(a, b);
    assert!(diff.is_empty(), "expected empty diff, got {diff:#?}");
}
