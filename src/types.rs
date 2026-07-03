#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ChunkKind {
    Function,
    Struct,
    Enum,
    Impl,
    Trait,
    Mod,
    Const,
    Static,
    TypeAlias,
    Macro,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SemanticChunk {
    pub kind: ChunkKind,
    pub name: Option<String>,
    pub text: String,
    pub start_line: usize,
    pub end_line: usize,
    pub has_error: bool,
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ParserError {
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

    #[error("invalid utf-8 in source slice")]
    InvalidUtf8(#[from] std::str::Utf8Error),
}
