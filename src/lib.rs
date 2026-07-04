//! Tree-sitter structural chunking for [rek0n](https://github.com/K48N/rek0n).

mod extractors;
mod types;

use std::sync::atomic::AtomicUsize;
use std::time::Duration;

pub use types::{ChunkKind, ParserError, SemanticChunk};
pub use rek0n_chunk::ParsedChunk;

pub const DEFAULT_PARSE_TIMEOUT: Duration = Duration::from_secs(5);
pub const DEFAULT_MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;
pub const DEFAULT_MAX_CHUNKS: usize = 100_000;
pub const MAX_SOURCE_BYTES_LIMIT: usize = 16 * 1024 * 1024;
pub const MAX_CHUNKS_LIMIT: usize = 1_000_000;
pub const MAX_PARSE_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone, Copy)]
pub struct ParseOptions<'a> {
    pub timeout: Duration,
    pub max_source_bytes: usize,
    pub max_chunks: usize,
    pub cancellation: Option<&'a AtomicUsize>,
}

impl Default for ParseOptions<'_> {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_PARSE_TIMEOUT,
            max_source_bytes: DEFAULT_MAX_SOURCE_BYTES,
            max_chunks: DEFAULT_MAX_CHUNKS,
            cancellation: None,
        }
    }
}

impl<'a> ParseOptions<'a> {
    pub fn validate(&self) -> Result<(), ParserError> {
        if self.max_source_bytes == 0 {
            return Err(ParserError::InvalidOptions(
                "max_source_bytes must be at least 1".into(),
            ));
        }
        if self.max_source_bytes > MAX_SOURCE_BYTES_LIMIT {
            return Err(ParserError::InvalidOptions(format!(
                "max_source_bytes {0} exceeds limit of {MAX_SOURCE_BYTES_LIMIT}",
                self.max_source_bytes
            )));
        }

        if self.max_chunks == 0 {
            return Err(ParserError::InvalidOptions(
                "max_chunks must be at least 1".into(),
            ));
        }
        if self.max_chunks > MAX_CHUNKS_LIMIT {
            return Err(ParserError::InvalidOptions(format!(
                "max_chunks {0} exceeds limit of {MAX_CHUNKS_LIMIT}",
                self.max_chunks
            )));
        }

        if self.timeout.is_zero() {
            return Err(ParserError::InvalidOptions(
                "parse timeout must be non-zero".into(),
            ));
        }
        if self.timeout > MAX_PARSE_TIMEOUT {
            return Err(ParserError::InvalidOptions(
                "parse timeout exceeds 300s limit".into(),
            ));
        }

        Ok(())
    }
}

pub fn parse_file(source: &str, language: &str) -> Result<Vec<SemanticChunk>, ParserError> {
    parse_file_with_options(source, language, ParseOptions::default())
}

pub fn parse_file_with_options(
    source: &str,
    language: &str,
    options: ParseOptions<'_>,
) -> Result<Vec<SemanticChunk>, ParserError> {
    options.validate()?;

    if source.len() > options.max_source_bytes {
        return Err(ParserError::SourceTooLarge {
            len: source.len(),
            max: options.max_source_bytes,
        });
    }

    let chunks = match language {
        "rust" => extractors::rust::extract_rust_chunks(source, options)?,
        other => return Err(ParserError::UnsupportedLanguage(other.to_string())),
    };

    if chunks.len() > options.max_chunks {
        return Err(ParserError::TooManyChunks {
            count: chunks.len(),
            max: options.max_chunks,
        });
    }

    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_file_routes_rust_language() {
        let source = "fn main() {}";
        let chunks = parse_file(source, "rust").expect("rust parsing should succeed");

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].kind, ChunkKind::Function);
        assert_eq!(chunks[0].name.as_deref(), Some("main"));
    }

    #[test]
    fn rejects_oversized_source() {
        let options = ParseOptions {
            max_source_bytes: 4,
            ..Default::default()
        };
        let err = parse_file_with_options("fn main() {}", "rust", options).unwrap_err();
        assert!(matches!(err, ParserError::SourceTooLarge { .. }));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn semantic_chunks_round_trip_through_serde_json() {
        let chunks = parse_file("fn main() {}", "rust").expect("parse should succeed");
        let json = serde_json::to_string(&chunks).expect("serialize should succeed");
        let restored: Vec<SemanticChunk> =
            serde_json::from_str(&json).expect("deserialize should succeed");
        assert_eq!(chunks, restored);
    }
}
