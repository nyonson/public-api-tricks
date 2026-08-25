// Derived from cargo-public-api's public-api crate
// (https://github.com/cargo-public-api/cargo-public-api, MIT licensed).
// Keep this file diffable against upstream to ease cherry-picking fixes.

use rustdoc_types::{Id, Item};

use crate::crate_set::CrateTag;
use crate::nameable_item::NameableItem;
use crate::path_component::PathComponent;
use crate::public_item::PublicItemPath;
use crate::render::RenderingContext;
use crate::tokens::Token;

/// This struct represents one public item of a crate, but in intermediate form.
/// Conceptually it wraps a single [`Item`] even though the path to the item
/// consists of many [`Item`]s. Later, one [`Self`] will be converted to exactly
/// one [`crate::PublicItem`].
#[derive(Clone, Debug)]
pub struct IntermediatePublicItem<'c> {
    path: Vec<PathComponent<'c>>,
    parent: Option<(CrateTag, Id)>,
    id: (CrateTag, Id),
}

impl<'c> IntermediatePublicItem<'c> {
    pub fn new(
        path: Vec<PathComponent<'c>>,
        parent: Option<(CrateTag, Id)>,
        id: (CrateTag, Id),
    ) -> Self {
        Self { path, parent, id }
    }

    /// Which crate's rustdoc JSON this item (and its [`Id`]) belongs to.
    #[must_use]
    pub fn tag(&self) -> CrateTag {
        self.id.0
    }

    #[must_use]
    pub fn item(&self) -> &'c Item {
        self.path()
            .last()
            .expect("path must not be empty")
            .item
            .item
    }

    #[must_use]
    pub fn path(&self) -> &[PathComponent<'c>] {
        &self.path
    }

    /// The [`Id`] of this item's logical parent (if any), qualified with the
    /// crate it belongs to.
    #[must_use]
    pub fn parent(&self) -> Option<(CrateTag, Id)> {
        self.parent
    }

    /// See [`crate::item_processor::sorting_prefix()`] docs for an explanation why we have this.
    #[must_use]
    pub fn sortable_path(&self, context: &RenderingContext) -> PublicItemPath {
        self.path()
            .iter()
            .map(|p| NameableItem::sortable_name(&p.item, context))
            .collect()
    }

    #[must_use]
    pub fn path_contains_renamed_item(&self) -> bool {
        self.path().iter().any(|m| m.item.overridden_name.is_some())
    }

    pub fn render_token_stream(&self, context: &RenderingContext) -> Vec<Token> {
        context.token_stream(self)
    }
}
