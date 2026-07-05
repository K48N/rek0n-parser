pub use rek0n_chunk::{ChunkKind, ParsedChunk};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ParserError {
    #[error("invalid parse options: {0}")]
    InvalidOptions(String),

    #[error("unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("failed to configure tree-sitter language: {0}")]
    LanguageSetup(String),

    #[error("invalid tree-sitter query: {0}")]
    InvalidQuery(String),

    #[error("parse timed out")]
    ParseTimedOut,

    #[error("parse was cancelled")]
    ParseCancelled,

    #[error("source is {len} bytes, exceeding limit of {max}")]
    SourceTooLarge { len: usize, max: usize },

    #[error("extracted {count} chunks, exceeding limit of {max}")]
    TooManyChunks { count: usize, max: usize },

    #[error("invalid utf-8 in source slice")]
    InvalidUtf8(#[from] std::str::Utf8Error),
}
