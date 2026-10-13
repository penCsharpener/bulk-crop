//! All image logic for pagecrop: model, geometry, decode, render, tone, export, persistence.
//! Nothing in this crate may depend on a window or a file dialog.

pub mod crop_edit;
pub mod decode;
pub mod encode;
pub mod error;
pub mod geometry;
pub mod model;
pub mod persist;
pub mod render;
pub mod scan;
pub mod tone;

/// The version of this crate, taken from `Cargo.toml`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_not_empty() {
        assert!(!version().is_empty());
    }
}
