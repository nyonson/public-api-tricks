// Derived from cargo-public-api's public-api crate
// (https://github.com/cargo-public-api/cargo-public-api, MIT licensed).
// Keep this file diffable against upstream to ease cherry-picking fixes.

use std::cmp::Ordering;
use std::fmt::Display;
use std::hash::Hash;

use crate::intermediate_public_item::IntermediatePublicItem;
use crate::render::RenderingContext;
use crate::tokens::Token;
use crate::tokens::tokens_to_string;

/// Each public item (except `impl`s) have a path that is displayed like
/// `first::second::third`. Internally we represent that with a `vec!["first",
/// "second", "third"]`. This is a type alias for that internal representation
/// to make the code easier to read.
pub(crate) type PublicItemPath = Vec<String>;

/// Represent a public item of an analyzed crate, i.e. an item that forms part
/// of the public API of a crate. Implements [`Display`] so it can be printed.
/// Ordered for logical grouping (struct fields after their struct, etc) via
/// [`Self::grouping_cmp`].
#[derive(Clone)]
pub struct PublicItem {
    /// Read [`crate::item_processor::sorting_prefix()`] docs for more info
    pub(crate) sortable_path: PublicItemPath,

    /// The rendered item as a stream of [`Token`]s
    pub(crate) tokens: Vec<Token>,

    /// Display-only context: for items of a trait impl, the rendered impl
    /// header (e.g. `[impl: impl<T> Foo<T> for Bar]`). Not part of the API
    /// identity, so excluded from `Eq` and `Hash`.
    pub(crate) context: Option<String>,
}

impl PublicItem {
    pub(crate) fn from_intermediate_public_item(
        context: &RenderingContext,
        public_item: &IntermediatePublicItem<'_>,
    ) -> PublicItem {
        // For items of a trait impl, capture the rendered impl header as
        // display-only context. The parent's tokens would also work for the
        // "is a trait impl" check, but inspecting the item kind is structural
        // and does not depend on rendering details.
        let item_context = public_item
            .parent()
            .and_then(|parent| context.id_to_items.get(&parent)?.first().copied())
            .filter(|parent| {
                matches!(
                    parent.item().inner,
                    rustdoc_types::ItemEnum::Impl(ref impl_) if impl_.trait_.is_some()
                )
            })
            .map(|parent| {
                format!(
                    "[impl: {}]",
                    tokens_to_string(&parent.render_token_stream(context))
                )
            });

        PublicItem {
            sortable_path: public_item.sortable_path(context),
            tokens: public_item.render_token_stream(context),
            context: item_context,
        }
    }

    /// The display-only context of this item (e.g. the trait impl header it
    /// belongs to), if any.
    #[must_use]
    pub fn context(&self) -> Option<&str> {
        self.context.as_deref()
    }

    /// Display the item with its context appended, if any.
    #[must_use]
    pub fn display_with_context(&self) -> impl Display + '_ {
        struct WithContext<'a>(&'a PublicItem);
        impl Display for WithContext<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match &self.0.context {
                    Some(ctx) => write!(f, "{} {}", self.0, ctx),
                    None => write!(f, "{}", self.0),
                }
            }
        }
        WithContext(self)
    }

    /// Special version of [`cmp`](Ord::cmp) that is used to sort public items in a way that
    /// makes them grouped logically. For example, struct fields will be put
    /// right after the struct they are part of.
    #[must_use]
    pub fn grouping_cmp(&self, other: &Self) -> std::cmp::Ordering {
        // This will make e.g. struct and struct fields be grouped together.
        if let Some(ordering) = different_or_none(&self.sortable_path, &other.sortable_path) {
            return ordering;
        }

        // Fall back to lexical sorting if the above is not sufficient
        self.to_string().cmp(&other.to_string())
    }
}

impl PartialEq for PublicItem {
    fn eq(&self, other: &Self) -> bool {
        self.tokens == other.tokens
    }
}

impl Eq for PublicItem {}

impl Hash for PublicItem {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.tokens.hash(state);
    }
}

/// We want pretty-printing (`"{:#?}"`) of diffs to print each public item as
/// `Display`, so implement `Debug` with `Display`.
impl std::fmt::Debug for PublicItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

/// One of the basic uses cases is printing a sorted `Vec` of `PublicItem`s. So
/// we implement `Display` for it.
impl Display for PublicItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", tokens_to_string(&self.tokens))
    }
}

/// Returns `None` if two items are equal. Otherwise their ordering is returned.
fn different_or_none<T: Ord>(a: &T, b: &T) -> Option<Ordering> {
    match a.cmp(b) {
        Ordering::Equal => None,
        c => Some(c),
    }
}
