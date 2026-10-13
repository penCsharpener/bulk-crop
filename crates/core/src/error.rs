use std::path::PathBuf;

/// Every error the core library returns. Variants that concern a file carry its path.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("cannot read {}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot decode {}: {message}", path.display())]
    Decode { path: PathBuf, message: String },
}
