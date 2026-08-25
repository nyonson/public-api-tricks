//! Test crate that re-exports items of an external crate, like a crate
//! applying the "semver trick" does.

pub use reexport_external_dependency::SomeStruct as RenamedStruct;
pub use reexport_external_dependency::some_module;
pub use reexport_external_dependency::*;

pub fn own_fn() -> u32 {
    1
}
