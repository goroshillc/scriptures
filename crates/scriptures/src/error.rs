//! Error type for every fallible step of fetch, build and serve.
//!
//! > *"But seek ye first the kingdom of God, and his righteousness; and all these
//! > things shall be added unto you."* - Matthew 6:33, KJV

use std::path::PathBuf;

/// Everything that can go wrong, named so a failed build says exactly where.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A file could not be read or written.
    #[error("{op} {path}: {source}")]
    Io {
        /// What was being done ("read", "write", "create dir").
        op: &'static str,
        /// The path involved.
        path: PathBuf,
        /// The underlying OS error.
        source: std::io::Error,
    },
    /// JSON did not parse into the expected shape.
    #[error("json {what}: {source}")]
    Json {
        /// Which document failed.
        what: String,
        /// The parser error.
        source: serde_json::Error,
    },
    /// A network request failed.
    #[error("http {url}: {reason}")]
    Http {
        /// The URL requested.
        url: String,
        /// Why it failed.
        reason: String,
    },
    /// Input data broke an invariant the site depends on.
    #[error("data: {0}")]
    Data(String),
    /// An image could not be decoded, resized or encoded.
    #[error("image {path}: {reason}")]
    Image {
        /// The image path.
        path: PathBuf,
        /// What failed.
        reason: String,
    },
    /// A licence gate refused a file: absence of a free licence is not permission.
    #[error("licence refused for {file}: {reason}")]
    Licence {
        /// The Commons file title.
        file: String,
        /// The licence string the source reported.
        reason: String,
    },
    /// The command line was wrong.
    #[error("usage: {0}")]
    Usage(String),
}

impl Error {
    /// Wraps an I/O error with the operation and path, so no error is ever pathless.
    pub fn io(op: &'static str, path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            op,
            path: path.into(),
            source,
        }
    }

    /// Wraps a JSON error with the document it came from.
    pub fn json(what: impl Into<String>, source: serde_json::Error) -> Self {
        Self::Json {
            what: what.into(),
            source,
        }
    }
}
