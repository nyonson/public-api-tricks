// Derived from cargo-public-api's public-api crate (as crate_wrapper.rs)
// (https://github.com/cargo-public-api/cargo-public-api, MIT licensed).
// Keep this file diffable against upstream to ease cherry-picking fixes.

use std::collections::HashMap;
use std::path::PathBuf;

use rustdoc_types::{Crate, Id, Item};

/// Identifies which crate's rustdoc JSON an item or [`Id`] belongs to.
///
/// [`Id`]s are only unique within a single crate's rustdoc JSON, so all
/// lookups must be qualified with the crate they belong to.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CrateTag {
    /// The crate for which the public API is being generated.
    Main,
    /// An external crate whose rustdoc JSON was provided, so that re-exports
    /// of its items can be inlined. The index is into [`CrateSet`]'s
    /// externals.
    External(usize),
}

/// A set of [`Crate`]s (deserialized rustdoc JSON): the crate for which the
/// public API is generated, plus the rustdoc JSON of external crates that
/// have been provided so that re-exports of their items can be resolved.
pub struct CrateSet<'c> {
    /// The crate for which the public API is being generated.
    main: &'c Crate,

    /// The provided external crates. Each was compiled to the artifact path
    /// passed alongside it to [`CrateSet::new`].
    externals: Vec<&'c Crate>,

    /// Maps the key of an entry in the main crate's `external_crates` (the
    /// `crate_id` of `paths` map entries) to the tag of the corresponding
    /// provided external crate.
    external_tags: HashMap<u32, CrateTag>,
}

impl<'c> CrateSet<'c> {
    /// `externals` and `artifact_paths` are parallel: `artifact_paths[i]` is
    /// the compiled artifact of `externals[i]`, used to match against the
    /// main crate's `external_crates[].path` (the crate identity added to
    /// rustdoc JSON in <https://github.com/rust-lang/rust/pull/149043>).
    pub fn new(main: &'c Crate, externals: Vec<&'c Crate>, artifact_paths: Vec<PathBuf>) -> Self {
        debug_assert_eq!(externals.len(), artifact_paths.len());
        let mut external_tags = HashMap::new();
        for (crate_id, external) in &main.external_crates {
            if let Some(pos) = artifact_paths.iter().position(|p| *p == external.path) {
                external_tags.insert(*crate_id, CrateTag::External(pos));
            }
        }

        Self {
            main,
            externals,
            external_tags,
        }
    }

    /// The crate for which the public API is being generated.
    pub fn main(&self) -> &'c Crate {
        self.main
    }

    /// The crate with the given tag.
    pub fn get(&self, tag: CrateTag) -> &'c Crate {
        match tag {
            CrateTag::Main => self.main,
            CrateTag::External(pos) => self.externals[pos],
        }
    }

    /// The tag of the provided external crate that corresponds to `crate_id`
    /// in the main crate's `external_crates`, if its rustdoc JSON was
    /// provided.
    pub fn external_tag(&self, crate_id: u32) -> Option<CrateTag> {
        self.external_tags.get(&crate_id).copied()
    }

    pub fn get_item(&self, tag: CrateTag, id: Id) -> Option<&'c Item> {
        self.get(tag).index.get(&id)
    }
}
