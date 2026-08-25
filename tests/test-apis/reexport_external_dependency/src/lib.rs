//! Stand-in for an external crate that is re-exported by the
//! `reexport_external` test crate, e.g. a newer major version re-exported
//! from an older major version line (the "semver trick").

pub struct SomeStruct {
    pub field: u32,
}

impl SomeStruct {
    pub fn new(field: u32) -> Self {
        Self { field }
    }

    pub fn double(&self) -> Self {
        Self::new(self.field * 2)
    }
}

pub fn some_fn(input: &SomeStruct) -> u32 {
    input.field
}

pub mod some_module {
    pub enum SomeEnum {
        A,
        B,
    }
}
