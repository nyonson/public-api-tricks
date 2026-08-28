# public-api-tricks

List and diff the public API of a crate, with support for the [semver
trick](https://github.com/dtolnay/semver-trick): public re-exports of external crates (e.g. a newer
major version re-exported from an older major version line) are resolved and inlined.

## Rustdoc Types

`public-api-tricks` builds on the unstable JSON output of `rustdoc`. It runs
`cargo rustdoc` itself with a nightly toolchain, and parses the resulting JSON with
[`rustdoc-types`](https://crates.io/crates/rustdoc-types).

The JSON schema is unstable and versioned by a `format_version` integer that the nightly toolchain
emits. Newer nightlies add fields; parsing stays compatible as long as the changes are additive, but
a breaking schema bump requires a new `rustdoc-types` release. In practice a given release of this
crate supports a range of nightlies.

Rendered output is nightly-sensitive (e.g. auto-trait sets, `Infallible` vs `never`), so snapshots
and diffs are only comparable when built with the same toolchain.

### Compatibility

| Version | `rustdoc-types` | Supported nightlies   |
| ------- | --------------- | --------------------- |
| 0.1.x   | 0.59.x          | nightly-2025-11-22 -- |

## Acknowledgements

`public-api-tricks` is derived from
[`public-api`](https://github.com/cargo-public-api/cargo-public-api). It was spun out in order to
support analysis of the semver-trick which requires some non-trivial plumbing throughout the
codebase. Further extensions are added as well, like implementation contexts.
