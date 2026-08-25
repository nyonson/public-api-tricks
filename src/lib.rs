//! List and diff the public API of crates, resolving re-exports (the "semver trick").

mod build;
mod crate_set;
mod diff;
mod error;
mod intermediate_public_item;
mod item_processor;
mod nameable_item;
mod path_component;
mod public_item;
mod render;
mod tokens;

pub use build::build;
pub use diff::{ChangedPublicItem, PublicApiDiff};
pub use error::{Error, Result};
pub use public_item::PublicItem;

use std::fmt;

/// The public API of a crate for one feature configuration.
pub struct PublicApi {
    pub(crate) items: Vec<PublicItem>,
}

impl PublicApi {
    pub(crate) fn new(mut items: Vec<PublicItem>) -> Self {
        items.sort_by(PublicItem::grouping_cmp);
        Self { items }
    }

    /// All public items, sorted for logical grouping (struct fields right after their struct, etc).
    pub fn items(&self) -> impl Iterator<Item = &PublicItem> {
        self.items.iter()
    }

    /// Like [`Self::items`], but transfers ownership.
    pub fn into_items(self) -> Vec<PublicItem> {
        self.items
    }
}

impl fmt::Display for PublicApi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for item in &self.items {
            writeln!(f, "{}", item.display_with_context())?;
        }
        Ok(())
    }
}
