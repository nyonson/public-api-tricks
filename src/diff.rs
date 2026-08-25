// Derived from cargo-public-api's public-api crate
// (https://github.com/cargo-public-api/cargo-public-api, MIT licensed).
// Keep this file diffable against upstream to ease cherry-picking fixes.

//! Contains facilities that allows you diff public APIs between releases and
//! commits. [`cargo
//! public-api`](https://github.com/cargo-public-api/cargo-public-api) contains
//! additional helpers for that.

use crate::{
    PublicApi,
    public_item::{PublicItem, PublicItemPath},
};
use std::collections::HashMap;

type ItemsWithPath = HashMap<PublicItemPath, Vec<PublicItem>>;

/// An item has changed in the public API. Two [`PublicItem`]s are considered
/// the same if their `path` is the same.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedPublicItem {
    /// How the item used to look.
    pub old: PublicItem,

    /// How the item looks now.
    pub new: PublicItem,
}

impl ChangedPublicItem {
    /// See [`PublicItem::grouping_cmp`]
    #[must_use]
    pub fn grouping_cmp(&self, other: &Self) -> std::cmp::Ordering {
        match PublicItem::grouping_cmp(&self.old, &other.old) {
            std::cmp::Ordering::Equal => PublicItem::grouping_cmp(&self.new, &other.new),
            ordering => ordering,
        }
    }
}

/// The return value of [`Self::between`]. To quickly get a sense of what it
/// contains, you can pretty-print it:
/// ```txt
/// println!("{:#?}", public_api_diff);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicApiDiff {
    /// Items that have been removed from the public API. A MAJOR change, in
    /// semver terminology. Sorted.
    pub removed: Vec<PublicItem>,

    /// Items in the public API that has been changed. Generally a MAJOR change,
    /// but exceptions exist. For example, if the return value of a method is
    /// changed from `ExplicitType` to `Self` and `Self` is the same as
    /// `ExplicitType`.
    pub changed: Vec<ChangedPublicItem>,

    /// Items that have been added to public API. A MINOR change, in semver
    /// terminology. Sorted.
    pub added: Vec<PublicItem>,
}

impl PublicApiDiff {
    /// Allows you to diff the public API between two arbitrary versions of a
    /// library, e.g. different releases. The input parameters `old` and `new`
    /// is the output of two different invocations of [`crate::build`].
    #[must_use]
    pub fn between(old: PublicApi, new: PublicApi) -> Self {
        // We must count occurrences, because with a HashSet we would lose
        // public items that happen to have the same representation due to
        // limitations or bugs (e.g. an item re-exported at two paths).
        let old = occurrence_counts(old.into_items());
        let new = occurrence_counts(new.into_items());

        // First figure out what items have been removed and what have been
        // added. Later we will match added and removed items with the same path
        // and construct a list of changed items. A changed item is an item with
        // the same path that has been both removed and added.
        let all_removed = multiset_difference(&old, &new);
        let all_added = multiset_difference(&new, &old);

        // Convert the data to make it easier to work with
        let mut removed_paths: ItemsWithPath = bag_to_path_map(all_removed);
        let mut added_paths: ItemsWithPath = bag_to_path_map(all_added);

        // The result we return from this function will be put in these vectors
        let mut removed: Vec<PublicItem> = vec![];
        let mut changed: Vec<ChangedPublicItem> = vec![];
        let mut added: Vec<PublicItem> = vec![];

        // Figure out all paths of items that are either removed or added. Later
        // we will match paths that have been both removed and added (i.e.
        // changed)
        let mut touched_paths: Vec<PublicItemPath> = vec![];
        touched_paths.extend::<Vec<_>>(removed_paths.keys().cloned().collect());
        touched_paths.extend::<Vec<_>>(added_paths.keys().cloned().collect());

        // OK, we are ready to do some actual heavy lifting. Go through all
        // paths and look for changed items. The remaining items are either
        // purely removed or purely added.
        for path in touched_paths {
            let mut removed_items = removed_paths.remove(&path).unwrap_or_default();
            let mut added_items = added_paths.remove(&path).unwrap_or_default();
            loop {
                match (removed_items.pop(), added_items.pop()) {
                    (Some(old), Some(new)) => changed.push(ChangedPublicItem { old, new }),
                    (Some(old), None) => removed.push(old),
                    (None, Some(new)) => added.push(new),
                    (None, None) => break,
                }
            }
        }

        // Make output predictable and stable
        removed.sort_by(PublicItem::grouping_cmp);
        changed.sort_by(ChangedPublicItem::grouping_cmp);
        added.sort_by(PublicItem::grouping_cmp);

        Self {
            removed,
            changed,
            added,
        }
    }

    /// Check whether the diff is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.removed.is_empty() && self.changed.is_empty() && self.added.is_empty()
    }
}

/// Converts a set (read: bag) of public items into a hash map that maps a given
/// path to a vec of public items with that path.
fn bag_to_path_map<'a>(difference: impl Iterator<Item = (&'a PublicItem, usize)>) -> ItemsWithPath {
    let mut map: ItemsWithPath = HashMap::new();
    for (item, occurrences) in difference {
        let items = map.entry(item.sortable_path.clone()).or_default();
        for _ in 0..occurrences {
            items.push(item.clone());
        }
    }
    map
}

/// Count the occurrences of each item (a multiset; [`HashMap`] since
/// [`PublicItem`] is hashable by its tokens).
fn occurrence_counts(items: Vec<PublicItem>) -> HashMap<PublicItem, usize> {
    let mut counts: HashMap<PublicItem, usize> = HashMap::new();
    for item in items {
        *counts.entry(item).or_default() += 1;
    }
    counts
}

/// The items (with occurrence counts) in `a` but not fully covered by `b`.
fn multiset_difference<'a>(
    a: &'a HashMap<PublicItem, usize>,
    b: &'a HashMap<PublicItem, usize>,
) -> impl Iterator<Item = (&'a PublicItem, usize)> {
    a.iter().filter_map(move |(item, &count_a)| {
        let count_b = b.get(item).copied().unwrap_or(0);
        // `then` (lazy), not `then_some` (eager): the subtraction would
        // overflow when count_b >= count_a.
        (count_a > count_b).then(|| (item, count_a - count_b))
    })
}

#[cfg(test)]
mod tests {
    use crate::tokens::Token;

    use super::*;

    #[test]
    fn single_and_only_item_removed() {
        let old = api([item_with_path("foo")]);
        let new = api([]);

        let actual = PublicApiDiff::between(old, new);
        let expected = PublicApiDiff {
            removed: vec![item_with_path("foo")],
            changed: vec![],
            added: vec![],
        };
        assert_eq!(actual, expected);
        assert!(!actual.is_empty());
    }

    #[test]
    fn single_and_only_item_added() {
        let old = api([]);
        let new = api([item_with_path("foo")]);

        let actual = PublicApiDiff::between(old, new);
        let expected = PublicApiDiff {
            removed: vec![],
            changed: vec![],
            added: vec![item_with_path("foo")],
        };
        assert_eq!(actual, expected);
        assert!(!actual.is_empty());
    }

    #[test]
    fn middle_item_added() {
        let old = api([item_with_path("1"), item_with_path("3")]);
        let new = api([
            item_with_path("1"),
            item_with_path("2"),
            item_with_path("3"),
        ]);

        let actual = PublicApiDiff::between(old, new);
        let expected = PublicApiDiff {
            removed: vec![],
            changed: vec![],
            added: vec![item_with_path("2")],
        };
        assert_eq!(actual, expected);
        assert!(!actual.is_empty());
    }

    #[test]
    fn middle_item_removed() {
        let old = api([
            item_with_path("1"),
            item_with_path("2"),
            item_with_path("3"),
        ]);
        let new = api([item_with_path("1"), item_with_path("3")]);

        let actual = PublicApiDiff::between(old, new);
        let expected = PublicApiDiff {
            removed: vec![item_with_path("2")],
            changed: vec![],
            added: vec![],
        };
        assert_eq!(actual, expected);
        assert!(!actual.is_empty());
    }

    #[test]
    fn many_identical_items() {
        let old = api([
            item_with_path("1"),
            item_with_path("2"),
            item_with_path("2"),
            item_with_path("3"),
            item_with_path("3"),
            item_with_path("3"),
            fn_with_param_type(&["a", "b"], "i32"),
            fn_with_param_type(&["a", "b"], "i32"),
        ]);
        let new = api([
            item_with_path("1"),
            item_with_path("2"),
            item_with_path("3"),
            item_with_path("4"),
            item_with_path("4"),
            fn_with_param_type(&["a", "b"], "i64"),
            fn_with_param_type(&["a", "b"], "i64"),
        ]);

        let actual = PublicApiDiff::between(old, new);
        let expected = PublicApiDiff {
            removed: vec![
                item_with_path("2"),
                item_with_path("3"),
                item_with_path("3"),
            ],
            changed: vec![
                ChangedPublicItem {
                    old: fn_with_param_type(&["a", "b"], "i32"),
                    new: fn_with_param_type(&["a", "b"], "i64"),
                },
                ChangedPublicItem {
                    old: fn_with_param_type(&["a", "b"], "i32"),
                    new: fn_with_param_type(&["a", "b"], "i64"),
                },
            ],
            added: vec![item_with_path("4"), item_with_path("4")],
        };
        assert_eq!(actual, expected);
        assert!(!actual.is_empty());
    }

    /// Regression test for
    /// <https://github.com/cargo-public-api/cargo-public-api/issues/50>
    #[test]
    fn no_off_by_one_diff_skewing() {
        let old = api([
            fn_with_param_type(&["a", "b"], "i8"),
            fn_with_param_type(&["a", "b"], "i32"),
            fn_with_param_type(&["a", "b"], "i64"),
        ]);
        // Same as `old` but with a new item with the same path added on top.
        // The diffing algorithm needs to figure out that only one item has been
        // added, rather than showing that of three items changed.
        let new = api([
            fn_with_param_type(&["a", "b"], "u8"), // The only new item!
            fn_with_param_type(&["a", "b"], "i8"),
            fn_with_param_type(&["a", "b"], "i32"),
            fn_with_param_type(&["a", "b"], "i64"),
        ]);
        let expected = PublicApiDiff {
            removed: vec![],
            changed: vec![],
            added: vec![fn_with_param_type(&["a", "b"], "u8")],
        };
        let actual = PublicApiDiff::between(old, new);
        assert_eq!(actual, expected);
        assert!(!actual.is_empty());
    }

    #[test]
    fn no_diff_means_empty_diff() {
        let old = api([item_with_path("foo")]);
        let new = api([item_with_path("foo")]);

        let actual = PublicApiDiff::between(old, new);
        let expected = PublicApiDiff {
            removed: vec![],
            changed: vec![],
            added: vec![],
        };
        assert_eq!(actual, expected);
        assert!(actual.is_empty());
    }

    fn item_with_path(path_str: &str) -> PublicItem {
        new_public_item(
            path_str
                .split("::")
                .map(std::string::ToString::to_string)
                .collect(),
            vec![crate::tokens::Token::identifier(path_str)],
        )
    }

    fn api(items: impl IntoIterator<Item = PublicItem>) -> PublicApi {
        PublicApi {
            items: items.into_iter().collect(),
        }
    }

    fn fn_with_param_type(path_str: &[&str], type_: &str) -> PublicItem {
        let path: Vec<_> = path_str
            .iter()
            .map(std::string::ToString::to_string)
            .collect();

        // Begin with "pub fn "
        let mut tokens = vec![q("pub"), w(), k("fn"), w()];

        // Add path e.g. "a::b"
        let mut first = true;
        for segment in &path {
            if !first {
                tokens.push(Token::symbol("::"));
            }
            first = false;
            tokens.push(Token::identifier(segment.clone()));
        }

        // Append function "(x: usize)"
        tokens.extend(vec![q("("), i("x"), s(":"), w(), t(type_), q(")")]);

        // End result is e.g. "pub fn a::b(x: usize)"
        new_public_item(path, tokens)
    }

    fn new_public_item(path: PublicItemPath, tokens: Vec<Token>) -> PublicItem {
        PublicItem {
            sortable_path: path,
            tokens,
            context: None,
        }
    }

    fn s(s: &str) -> Token {
        Token::symbol(s)
    }

    fn t(s: &str) -> Token {
        Token::type_(s)
    }

    fn q(s: &str) -> Token {
        Token::qualifier(s)
    }

    fn k(s: &str) -> Token {
        Token::kind(s)
    }

    fn i(s: &str) -> Token {
        Token::identifier(s)
    }

    fn w() -> Token {
        Token::Whitespace
    }
}
