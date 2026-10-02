//! Typed scalar-property descriptors and zero-copy runtime bindings.

pub(crate) mod registry;

#[cfg(test)]
mod tests;

mod binding;
mod property_spec;
mod scalar_property;

pub use binding::ScalarPropertyBinding;
pub use property_spec::PropertySpec;
pub use scalar_property::ScalarProperty;
