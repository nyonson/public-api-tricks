use std::path::PathBuf;

/// Enumerates all errors that can currently occur within this crate.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Occurs if the rustdoc JSON you provide can't be parsed. Typically
    /// because the rustdoc JSON format that your version of nightly outputs is
    /// too old.
    SerdeJsonError(serde_json::Error),

    /// Some kind of IO error occurred. For example, we might not have read
    /// permissions on the rustdoc JSON input file.
    IoError(std::io::Error),

    /// `cargo rustdoc` (or `cargo metadata`) failed while building rustdoc
    /// JSON.
    Cargo(String),

    /// A crate referenced by `external_crates` could not be matched to a
    /// compiled dependency artifact, or an artifact could not be matched to a
    /// manifest.
    Resolve(String),

    /// The expected rustdoc JSON output file was not produced.
    MissingJson(PathBuf),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::SerdeJsonError(e) => write!(f, "{e}"),
            Error::IoError(e) => write!(f, "{e}"),
            Error::Cargo(msg) => write!(f, "cargo command failed: {msg}"),
            Error::Resolve(msg) => write!(f, "could not resolve external crate: {msg}"),
            Error::MissingJson(path) => {
                write!(f, "rustdoc JSON not found at {}", path.display())
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::SerdeJsonError(e) => Some(e),
            Error::IoError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::SerdeJsonError(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IoError(e)
    }
}

/// Shorthand for [`std::result::Result<T, Error>`].
pub type Result<T> = std::result::Result<T, Error>;
