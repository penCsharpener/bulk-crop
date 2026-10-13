//! All image logic for pagecrop: model, geometry, decode, render, tone, export, persistence.
//! Nothing in this crate may depend on a window or a file dialog.

pub mod error;
pub mod model;
pub mod scan;

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
