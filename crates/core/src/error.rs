use std::path::PathBuf;

/// Every error the core library returns. Variants that concern a file carry its path.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("cannot access {}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot decode {}: {message}", path.display())]
    Decode { path: PathBuf, message: String },
    #[error("cannot parse {}: {message}", path.display())]
    Sidecar { path: PathBuf, message: String },
    #[error("the output folder is the photo folder: {}", path.display())]
    OutDirIsSource { path: PathBuf },
    #[error("cannot start worker threads: {0}")]
    Threads(String),
}
