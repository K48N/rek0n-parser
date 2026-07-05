use rek0n_parser::{parse_file, ChunkKind, ParsedChunk, ParserError};

fn healthy(chunks: &[ParsedChunk]) -> Vec<&ParsedChunk> {
    chunks.iter().filter(|chunk| !chunk.has_error).collect()
}

#[test]
fn parse_file_extracts_struct_and_function() {
    let source = r#"
struct User {
    id: u64,
}

fn authenticate(token: &str) -> bool {
    !token.is_empty()
}
"#;

    let chunks = parse_file(source, "rust").expect("rust parse should succeed");
    assert_eq!(chunks.len(), 2);

    assert_eq!(chunks[0].kind, ChunkKind::Struct);
    assert_eq!(chunks[0].name.as_deref(), Some("User"));
    assert!(chunks[0].text.contains("id: u64"));

    assert_eq!(chunks[1].kind, ChunkKind::Function);
    assert_eq!(chunks[1].name.as_deref(), Some("authenticate"));
    assert!(chunks[1].text.contains("token"));
}

#[test]
fn parse_file_rejects_unsupported_language() {
    let err = parse_file("print('hi')", "python").expect_err("python should be rejected");
    assert!(matches!(
        err,
        ParserError::UnsupportedLanguage(ref language) if language == "python"
    ));
}

#[test]
fn parse_file_includes_doc_comments_in_chunk_text() {
    let source = r#"/// Authenticates a bearer token.
fn authenticate(token: &str) -> bool {
    !token.is_empty()
}
"#;

    let chunks = parse_file(source, "rust").expect("rust parse should succeed");
    assert_eq!(chunks.len(), 1);
    assert!(chunks[0].text.contains("/// Authenticates a bearer token."));
}

#[test]
fn consumer_pattern_skips_error_flagged_chunks() {
    let source = r#"
struct Good {
    value: i32,
}

fn broken( {

fn also_good() -> i32 {
    42
}
"#;

    let chunks = parse_file(source, "rust").expect("best-effort parse should succeed");
    let usable = healthy(&chunks);

    assert_eq!(usable.len(), 2);
    assert_eq!(usable[0].name.as_deref(), Some("Good"));
    assert_eq!(usable[1].name.as_deref(), Some("also_good"));
}
