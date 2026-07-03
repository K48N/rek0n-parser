//! Tree-sitter structural chunking for [rek0n](https://github.com/K48N/rek0n).

mod extractors;
mod types;

use std::sync::atomic::AtomicUsize;
use std::time::Duration;

pub use types::{ChunkKind, ParserError, SemanticChunk};

pub const DEFAULT_PARSE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy)]
pub struct ParseOptions<'a> {
    pub timeout: Duration,
    pub cancellation: Option<&'a AtomicUsize>,
}

impl Default for ParseOptions<'_> {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_PARSE_TIMEOUT,
            cancellation: None,
        }
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
    match language {
        "rust" => extractors::rust::extract_rust_chunks(source, options),
        other => Err(ParserError::UnsupportedLanguage(other.to_string())),
    }
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
