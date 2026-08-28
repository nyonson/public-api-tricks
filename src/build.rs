//! Builds rustdoc JSON for a crate and its re-exported external dependencies, then analyzes it
//! into a [`PublicApi`].

use std::path::{Path, PathBuf};
use std::process::Command;

use rustdoc_types::{Crate, Id};

use crate::PublicApi;
use crate::error::{Error, Result};

/// Relevant `cargo metadata` output.
#[derive(serde::Deserialize)]
struct Metadata {
    target_directory: String,
    packages: Vec<MetadataPackage>,
}

#[derive(serde::Deserialize)]
struct MetadataPackage {
    manifest_path: String,
    targets: Vec<MetadataTarget>,
}

#[derive(serde::Deserialize)]
struct MetadataTarget {
    kind: Vec<String>,
    name: String,
}

/// Drives the cargo builds necessary to produce rustdoc JSON for a crate and its re-exports.
struct BuildDriver {
    /// Manifest of the crate whose public API we want.
    manifest_path: PathBuf,
    /// Cargo feature arguments (e.g. `--no-default-features`, `--features=..`).
    cargo_args: Vec<String>,
    /// The library target name (`-` becomes `_`), which names the JSON output file.
    lib_name: String,
    /// Directory rustdoc JSON is written to (`<target>/doc`).
    doc_dir: PathBuf,
}

/// A dependency of the crate under analysis.
struct DepArtifact {
    /// The crate (library) name.
    name: String,
    /// Path to the compiled `.rmeta`/`.rlib`.
    artifact: PathBuf,
    /// The dependency's manifest.
    manifest_path: PathBuf,
    /// Features the dependency is compiled with in this configuration.
    features: Vec<String>,
}

impl BuildDriver {
    fn new(manifest_path: &Path, cargo_args: &[&str]) -> Result<Self> {
        let metadata = Self::cargo_metadata(manifest_path, cargo_args)?;
        let lib_name = metadata
            .packages
            .iter()
            .find(|p| p.manifest_path == manifest_path.to_string_lossy())
            .and_then(|p| {
                p.targets
                    .iter()
                    .find(|t| t.kind.iter().any(|k| k == "lib"))
                    .map(|t| t.name.replace('-', "_"))
            })
            .ok_or_else(|| {
                Error::Resolve(format!("no lib target in {}", manifest_path.display()))
            })?;
        Ok(Self {
            manifest_path: manifest_path.to_path_buf(),
            cargo_args: cargo_args.iter().map(|s| s.to_string()).collect(),
            lib_name,
            doc_dir: PathBuf::from(metadata.target_directory).join("doc"),
        })
    }

    fn run(&self) -> Result<PublicApi> {
        // Build the crate's rustdoc JSON, capturing artifact messages to identify dependencies.
        let (main_json, deps) = self.build_main_json()?;
        let main_crate = Self::parse_rustdoc_json(&main_json)?;

        // The `crate_id`s of external crates that the crate's rustdoc JSON `paths` map references
        // but whose items are missing from `index`, i.e. that items are re-exported from.
        let index_ids: std::collections::HashSet<Id> = main_crate.index.keys().copied().collect();
        let mut wanted = Vec::new();
        for (id, summary) in &main_crate.paths {
            if !index_ids.contains(id)
                && summary.crate_id != 0
                && !wanted.contains(&summary.crate_id)
            {
                wanted.push(summary.crate_id);
            }
        }

        // Build rustdoc JSON for each re-exported external crate that is an actual cargo dependency
        // (we have its compiled artifact and manifest), with the features it is actually compiled
        // with. Keep the compiled artifact path alongside, so the analyzer can match each external
        // crate to its `external_crates` entry by identity (the rmeta path).
        let mut external_crates = Vec::new();
        let mut artifact_paths = Vec::new();
        for crate_id in wanted {
            let external = &main_crate.external_crates[&crate_id];
            // Only cargo dependencies have a compiled artifact in the deps dir. Standard library
            // crates (core, std, alloc, ...) are referenced by `paths` but are not dependencies to
            // document.
            let Some(dep) = deps.iter().find(|d| d.artifact == external.path) else {
                continue;
            };
            let json = self.build_dep_json(dep)?;
            external_crates.push(Self::parse_rustdoc_json(&json)?);
            artifact_paths.push(dep.artifact.clone());
        }

        // Analyze, expanding re-exports of the external crates.
        let external_refs = external_crates.iter().collect::<Vec<_>>();
        let items =
            crate::item_processor::public_api_in_crate(&main_crate, external_refs, artifact_paths);
        Ok(PublicApi::new(items))
    }

    /// Build the rustdoc JSON for the crate under analysis, returning its path and the dependency
    /// artifacts discovered from the compiler messages.
    fn build_main_json(&self) -> Result<(PathBuf, Vec<DepArtifact>)> {
        let args: Vec<&str> = self.cargo_args.iter().map(|s| s.as_str()).collect();
        let output = Self::rustdoc_command(&self.manifest_path, &args, true)
            .output()
            .map_err(|e| Error::Cargo(e.to_string()))?;
        if !output.status.success() {
            return Err(Error::Cargo(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ));
        }

        // Parse cargo's JSON messages for compiled library dependencies. This is cargo's
        // documented machine interface (`--message-format=json`), relied on by rust-analyzer et
        // al: https://doc.rust-lang.org/cargo/reference/external-tools.html#json-messages
        let mut deps = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if msg["reason"] != "compiler-artifact" {
                continue;
            }
            let target = &msg["target"];
            if target["kind"]
                .as_array()
                .map(|k| k.iter().any(|v| v == "lib"))
                != Some(true)
            {
                continue;
            }
            let Some(manifest) = msg["manifest_path"].as_str() else {
                continue;
            };
            // Skip the crate itself; we only want its dependencies.
            if Path::new(manifest) == self.manifest_path {
                continue;
            }
            let features: Vec<String> = msg["features"]
                .as_array()
                .map(|f| {
                    f.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            for file in msg["filenames"].as_array().into_iter().flatten() {
                let Some(f) = file.as_str() else { continue };
                if f.ends_with(".rmeta") || f.ends_with(".rlib") {
                    deps.push(DepArtifact {
                        name: target["name"].as_str().unwrap_or_default().to_string(),
                        artifact: PathBuf::from(f),
                        manifest_path: PathBuf::from(manifest),
                        features: features.clone(),
                    });
                }
            }
        }

        Ok((self.doc_dir.join(format!("{}.json", self.lib_name)), deps))
    }

    /// Build the rustdoc JSON for a re-exported dependency, with exactly the features it is
    /// compiled with in this configuration.
    fn build_dep_json(&self, dep: &DepArtifact) -> Result<PathBuf> {
        let mut args: Vec<String> = vec!["--no-default-features".into()];
        if !dep.features.is_empty() {
            args.push(format!("--features={}", dep.features.join(",")));
        }
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let status = Self::rustdoc_command(&dep.manifest_path, &args, false)
            // Write the JSON into the main crate's target dir, not the dependency's own (which may
            // be the read-only cargo registry).
            .env("CARGO_TARGET_DIR", self.doc_dir.parent().unwrap())
            .status()
            .map_err(|e| Error::Cargo(e.to_string()))?;
        if !status.success() {
            return Err(Error::Cargo(format!("documenting dependency {}", dep.name)));
        }
        let name = dep.name.replace('-', "_");
        let json = self.doc_dir.join(format!("{name}.json"));
        if json.exists() {
            Ok(json)
        } else {
            Err(Error::MissingJson(json))
        }
    }
    /// A `cargo rustdoc` command emitting rustdoc JSON for a lib target, plus cargo's feature
    /// arguments. Passing `--message-format=json` also produces cargo's artifact messages on stdout
    /// (used for dependency discovery). `RUSTDOCFLAGS` suppresses warnings-only doc errors (e.g.
    /// broken intra-doc links with limited feature sets) so they do not fail the JSON build.
    fn rustdoc_command(manifest: &Path, cargo_args: &[&str], message_json: bool) -> Command {
        let mut cmd = Command::new("cargo");
        cmd.arg("rustdoc")
            .arg("--lib")
            .arg("--manifest-path")
            .arg(manifest);
        for arg in cargo_args {
            cmd.arg(arg);
        }
        if message_json {
            cmd.arg("--message-format=json");
        }
        cmd.args(["--", "-Z", "unstable-options", "--output-format", "json"]);
        cmd.env("RUSTDOCFLAGS", "-A rustdoc::broken_intra_doc_links");
        cmd
    }

    /// Deserialize rustdoc JSON, with the recursion limit disabled for large crates.
    fn parse_rustdoc_json(path: &Path) -> Result<Crate> {
        let json_str = std::fs::read_to_string(path)?;
        let mut deserializer = serde_json::Deserializer::from_str(&json_str);
        deserializer.disable_recursion_limit();
        Ok(serde::de::Deserialize::deserialize(&mut deserializer)?)
    }

    /// Run `cargo metadata` for the manifest with the given feature arguments.
    fn cargo_metadata(manifest_path: &Path, cargo_args: &[&str]) -> Result<Metadata> {
        let mut cmd = Command::new("cargo");
        cmd.arg("metadata")
            .arg("--format-version")
            .arg("1")
            .arg("--manifest-path")
            .arg(manifest_path);
        for arg in cargo_args {
            cmd.arg(arg);
        }
        let output = cmd.output().map_err(|e| Error::Cargo(e.to_string()))?;
        if !output.status.success() {
            return Err(Error::Cargo(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ));
        }
        Ok(serde_json::from_slice(&output.stdout)?)
    }
}

/// Build the public API of the crate at `manifest_path` for the given cargo feature arguments
/// (e.g. `["--no-default-features"]`).
///
/// Rustdoc JSON is generated for the crate. If it publicly re-exports items of an external crate
/// (e.g. the "semver trick"), rustdoc JSON is also generated for that external crate with
/// exactly the features it is compiled with in this configuration.
pub fn build(manifest_path: &Path, cargo_args: &[&str]) -> Result<PublicApi> {
    let builder = BuildDriver::new(manifest_path, cargo_args)?;
    builder.run()
}
