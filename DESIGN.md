# Design

This crate is a minimal mix of two projects which could possibly encompass its scope in the future.

The overall flow of information is asking `cargo` for docs on the current crate, then asking it for
docs on any crate the current one re-exports. The doc JSON is then deserialized and operated on to
output API snapshots and diffs.

Getting `cargo` to generate all the correct JSON is handled by the `build.rs` module. But ideally
this is done with a tool like [rdxcr] once the `cargo` interface is more stabilized.

The bulk of the deserialization logic is handled by the `render.rs` module. This was taken straight
from the [cargo-public-api] project, but then non-trivially augmented to support the semver-trick.

[rdxcr]: https://codeberg.org/adot/rdxcr
[cargo-public-api]: https://github.com/cargo-public-api/cargo-public-api
