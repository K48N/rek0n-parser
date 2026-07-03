use std::sync::atomic::Ordering;
use tree_sitter::{Node, Parser, Query, QueryCursor};

use crate::extractors::doc_scan::{leading_doc_start, scan_doc_comments, ByteRange};
use crate::types::{ChunkKind, ParserError, SemanticChunk};
use crate::ParseOptions;

const RUST_QUERY: &str = r#"
(
  [
    (function_item) @function
    (function_signature_item) @function
    (struct_item) @struct
    (enum_item) @enum
    (impl_item) @impl
    (trait_item) @trait
    (mod_item) @mod
    (const_item) @const
    (static_item) @static
    (type_item) @type_alias
    (macro_definition) @macro
  ]
)
"#;

pub fn extract_rust_chunks(
    source_code: &str,
    options: ParseOptions<'_>,
) -> Result<Vec<SemanticChunk>, ParserError> {
    let language = tree_sitter_rust::language();

    let mut parser = Parser::new();
    parser
        .set_language(language)
        .map_err(|err| ParserError::LanguageSetup(err.to_string()))?;
    configure_parser(&mut parser, options);

    if is_parse_cancelled(options) {
        return Err(ParserError::ParseCancelled);
    }

    let tree = match parser.parse(source_code, None) {
        Some(tree) => tree,
        None => return Err(parse_interrupted_error(options)),
    };

    if is_parse_cancelled(options) {
        return Err(ParserError::ParseCancelled);
    }

    let query = Query::new(language, RUST_QUERY)
        .map_err(|err| ParserError::InvalidQuery(err.to_string()))?;
    let mut cursor = QueryCursor::new();
    let source_bytes = source_code.as_bytes();
    let line_starts = build_line_starts(source_code);
    let doc_comments = scan_doc_comments(source_code);

    let mut chunks = Vec::new();

    for query_match in cursor.matches(&query, tree.root_node(), source_bytes) {
        for capture in query_match.captures {
            let node = capture.node;
            if node.is_missing() || !is_extractable_item(node) {
                continue;
            }

            let kind = match query.capture_names().get(capture.index as usize) {
                Some(name) => chunk_kind_from_capture(name),
                None => ChunkKind::Unknown,
            };
            let name = extract_name(node, source_bytes);
            let has_error = node.has_error();
            let (start_byte, end_byte) = chunk_byte_range(node, source_code, &doc_comments);
            let text = source_code[start_byte..end_byte].to_string();
            let start_line = byte_offset_to_line(&line_starts, start_byte);
            let end_line = node.end_position().row + 1;

            chunks.push(SemanticChunk {
                kind,
                name,
                text,
                start_line,
                end_line,
                has_error,
            });
        }
    }

    Ok(chunks)
}

fn configure_parser(parser: &mut Parser, options: ParseOptions<'_>) {
    parser.set_timeout_micros(options.timeout.as_micros() as u64);
    if let Some(flag) = options.cancellation {
        unsafe {
            parser.set_cancellation_flag(Some(flag));
        }
    }
}

fn is_parse_cancelled(options: ParseOptions<'_>) -> bool {
    options
        .cancellation
        .is_some_and(|flag| flag.load(Ordering::Relaxed) != 0)
}

fn parse_interrupted_error(options: ParseOptions<'_>) -> ParserError {
    if is_parse_cancelled(options) {
        ParserError::ParseCancelled
    } else {
        ParserError::ParseTimedOut
    }
}

fn is_extractable_item(node: Node) -> bool {
    let mut current = node;
    loop {
        let Some(parent) = current.parent() else {
            return false;
        };
        match parent.kind() {
            "source_file" => return true,
            "declaration_list" => {
                return parent.parent().is_some_and(|container| {
                    matches!(container.kind(), "mod_item" | "foreign_mod_item")
                });
            }
            "ERROR" => {
                // A badly broken file can leave the root node as ERROR instead of
                // source_file; treat that like a top-level module body.
                if parent.parent().is_none()
                    || parent
                        .parent()
                        .is_some_and(|grandparent| grandparent.kind() == "source_file")
                {
                    return true;
                }
                current = parent;
            }
            _ => return false,
        }
    }
}

fn chunk_kind_from_capture(capture_name: &str) -> ChunkKind {
    match capture_name {
        "function" => ChunkKind::Function,
        "struct" => ChunkKind::Struct,
        "enum" => ChunkKind::Enum,
        "impl" => ChunkKind::Impl,
        "trait" => ChunkKind::Trait,
        "mod" => ChunkKind::Mod,
        "const" => ChunkKind::Const,
        "static" => ChunkKind::Static,
        "type_alias" => ChunkKind::TypeAlias,
        "macro" => ChunkKind::Macro,
        _ => ChunkKind::Unknown,
    }
}

fn extract_name(node: Node, source: &[u8]) -> Option<String> {
    match node.kind() {
        "impl_item" => node
            .child_by_field_name("type")
            .and_then(|type_node| type_name_from_type_node(type_node, source)),
        _ => node
            .child_by_field_name("name")
            .and_then(|name_node| identifier_text(name_node, source)),
    }
}

fn type_name_from_type_node(node: Node, source: &[u8]) -> Option<String> {
    match node.kind() {
        "type_identifier" => node.utf8_text(source).ok().map(str::to_string),
        "generic_type" => node
            .child_by_field_name("type")
            .and_then(|inner| type_name_from_type_node(inner, source)),
        "scoped_type_identifier" => node
            .child_by_field_name("name")
            .and_then(|inner| type_name_from_type_node(inner, source)),
        "reference_type" => node
            .child_by_field_name("type")
            .and_then(|inner| type_name_from_type_node(inner, source)),
        _ => identifier_text(node, source),
    }
}

fn identifier_text(node: Node, source: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" | "type_identifier" => node.utf8_text(source).ok().map(str::to_string),
        _ => None,
    }
}

fn chunk_byte_range(node: Node, source: &str, doc_comments: &[ByteRange]) -> (usize, usize) {
    let mut start = node.start_byte();
    let end = node.end_byte();

    let mut sibling = node.prev_sibling();
    while let Some(sib) = sibling {
        if sib.kind() == "attribute_item" {
            start = sib.start_byte();
            sibling = sib.prev_sibling();
        } else {
            break;
        }
    }

    start = leading_doc_start(doc_comments, source, start);

    (start, end)
}

fn build_line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, byte) in source.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(index + 1);
        }
    }
    starts
}

fn byte_offset_to_line(line_starts: &[usize], byte_offset: usize) -> usize {
    match line_starts.binary_search(&byte_offset) {
        Ok(line) => line + 1,
        Err(line) => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ChunkKind;
    use crate::ParseOptions;

    fn parse(source: &str) -> Result<Vec<SemanticChunk>, ParserError> {
        extract_rust_chunks(source, ParseOptions::default())
    }

    #[test]
    fn extracts_function_struct_and_impl_chunks() {
        let source = r#"
struct Widget {
    value: i32,
}

impl Widget {
    fn new(value: i32) -> Self {
        Self { value }
    }
}

fn standalone() -> i32 {
    42
}
"#;

        let chunks = parse(source).expect("parse should succeed");
        assert_eq!(chunks.len(), 3);

        assert_eq!(chunks[0].kind, ChunkKind::Struct);
        assert_eq!(chunks[0].name.as_deref(), Some("Widget"));

        assert_eq!(chunks[1].kind, ChunkKind::Impl);
        assert_eq!(chunks[1].name.as_deref(), Some("Widget"));

        assert_eq!(chunks[2].kind, ChunkKind::Function);
        assert_eq!(chunks[2].name.as_deref(), Some("standalone"));
    }

    #[test]
    fn chunk_text_matches_source_slice() {
        let source = "fn hello() {}\n";
        let chunks = parse(source).expect("parse should succeed");

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "fn hello() {}");
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 1);
    }

    #[test]
    fn empty_source_returns_no_chunks() {
        let chunks = parse("").expect("parse should succeed");
        assert!(chunks.is_empty());
    }

    #[test]
    fn impl_methods_are_not_extracted_as_separate_functions() {
        let source = r#"
impl Widget {
    fn new() -> Self {
        Self
    }

    fn reset(&mut self) {}
}
"#;

        let chunks = parse(source).expect("parse should succeed");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].kind, ChunkKind::Impl);
        assert_eq!(chunks[0].name.as_deref(), Some("Widget"));
        assert!(chunks[0].text.contains("fn new"));
        assert!(chunks[0].text.contains("fn reset"));
    }

    #[test]
    fn module_scoped_items_are_still_extracted() {
        let source = r#"
mod inner {
    struct InnerWidget {
        value: i32,
    }

    fn inner_fn() -> i32 {
        1
    }
}
"#;

        let chunks = parse(source).expect("parse should succeed");
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].kind, ChunkKind::Mod);
        assert_eq!(chunks[0].name.as_deref(), Some("inner"));
        assert_eq!(chunks[1].kind, ChunkKind::Struct);
        assert_eq!(chunks[1].name.as_deref(), Some("InnerWidget"));
        assert_eq!(chunks[2].kind, ChunkKind::Function);
        assert_eq!(chunks[2].name.as_deref(), Some("inner_fn"));
    }

    #[test]
    fn trait_impl_name_uses_implemented_type_not_trait() {
        let source = r#"
impl std::fmt::Display for Widget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "widget")
    }
}
"#;

        let chunks = parse(source).expect("parse should succeed");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].kind, ChunkKind::Impl);
        assert_eq!(chunks[0].name.as_deref(), Some("Widget"));
    }

    #[test]
    fn generic_impl_name_resolves_base_type() {
        let source = r#"
impl<T: Default> Container<T> {
    fn new() -> Self {
        Self { value: T::default() }
    }
}
"#;

        let chunks = parse(source).expect("parse should succeed");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].kind, ChunkKind::Impl);
        assert_eq!(chunks[0].name.as_deref(), Some("Container"));
    }

    #[test]
    fn trait_methods_are_not_extracted_as_separate_functions() {
        let source = r#"
trait Drawable {
    fn draw(&self);

    fn area(&self) -> f64;
}
"#;

        let chunks = parse(source).expect("parse should succeed");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].kind, ChunkKind::Trait);
        assert_eq!(chunks[0].name.as_deref(), Some("Drawable"));
        assert!(chunks[0].text.contains("fn draw"));
        assert!(chunks[0].text.contains("fn area"));
    }

    #[test]
    fn extracts_valid_items_despite_parse_errors_elsewhere() {
        let source = r#"
struct Good {
    value: i32,
}

fn broken( {

fn also_good() -> i32 {
    42
}
"#;

        let chunks = parse(source).expect("best-effort parse should succeed");
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].kind, ChunkKind::Struct);
        assert_eq!(chunks[0].name.as_deref(), Some("Good"));
        assert_eq!(chunks[1].kind, ChunkKind::Function);
        assert_eq!(chunks[1].name.as_deref(), Some("also_good"));
    }

    #[test]
    fn doc_comments_and_attributes_are_included_in_chunk_text() {
        let source = r#"/// Widget docs.
#[derive(Debug, Clone)]
struct Widget {
    value: i32,
}
"#;

        let chunks = parse(source).expect("parse should succeed");
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("/// Widget docs."));
        assert!(chunks[0].text.contains("#[derive(Debug, Clone)]"));
        assert!(chunks[0].text.contains("struct Widget"));
        assert_eq!(chunks[0].start_line, 1);
    }

    #[test]
    fn function_attributes_are_included_in_chunk_text() {
        let source = r#"/// Runs the app.
#[test]
fn run_smoke_test() {
    assert!(true);
}
"#;

        let chunks = parse(source).expect("parse should succeed");
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("#[test]"));
        assert!(chunks[0].text.contains("/// Runs the app."));
        assert!(chunks[0].text.contains("fn run_smoke_test"));
    }

    #[test]
    fn block_doc_comments_are_included_in_chunk_text() {
        let source = r#"/**
 * Widget documentation.
 */
struct Widget {
    value: i32,
}
"#;

        let chunks = parse(source).expect("parse should succeed");
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("Widget documentation."));
        assert!(chunks[0].text.contains("struct Widget"));
    }

    #[test]
    fn extern_block_declarations_are_extracted() {
        let source = r#"
extern "C" {
    fn ffi_fn(x: i32) -> i32;
    static FFI_FLAG: i32;
}
"#;

        let chunks = parse(source).expect("parse should succeed");
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].kind, ChunkKind::Function);
        assert_eq!(chunks[0].name.as_deref(), Some("ffi_fn"));
        assert_eq!(chunks[1].kind, ChunkKind::Static);
        assert_eq!(chunks[1].name.as_deref(), Some("FFI_FLAG"));
    }

    #[test]
    fn doc_like_text_inside_raw_string_does_not_extend_chunk() {
        let source = r##"r#" /** not a doc comment "# struct Widget { value: i32 }"##;
        let chunks = parse(source).expect("parse should succeed");
        assert_eq!(chunks.len(), 1);
        assert!(!chunks[0].text.contains("not a doc comment"));
        assert!(chunks[0].text.starts_with("struct Widget"));
    }

    #[test]
    fn doc_comments_after_lifetime_annotations_are_included() {
        let source = r#"
fn foo<'a>(x: &'a str) -> &'a str {
    x
}

/// Docs for another.
struct Other {
    value: i32,
}
"#;

        let chunks = parse(source).expect("parse should succeed");
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].kind, ChunkKind::Function);
        assert_eq!(chunks[0].name.as_deref(), Some("foo"));
        assert_eq!(chunks[1].kind, ChunkKind::Struct);
        assert_eq!(chunks[1].name.as_deref(), Some("Other"));
        assert!(chunks[1].text.contains("/// Docs for another."));
    }

    #[test]
    fn healthy_chunks_have_no_error_flag() {
        let source = "fn healthy() -> i32 { 42 }";
        let chunks = parse(source).expect("parse should succeed");
        assert_eq!(chunks.len(), 1);
        assert!(!chunks[0].has_error);
    }

    #[test]
    fn extracts_extended_item_kinds() {
        let source = r#"
enum Color {
    Red,
    Green,
}

trait Drawable {
    fn draw(&self);
}

const MAX: usize = 100;

static VERSION: &str = "1.0";

type UserId = u64;

macro_rules! vec_of {
    ($($x:expr),*) => { vec![$($x),*] };
}
"#;

        let chunks = parse(source).expect("parse should succeed");
        assert_eq!(chunks.len(), 6);

        assert_eq!(chunks[0].kind, ChunkKind::Enum);
        assert_eq!(chunks[0].name.as_deref(), Some("Color"));

        assert_eq!(chunks[1].kind, ChunkKind::Trait);
        assert_eq!(chunks[1].name.as_deref(), Some("Drawable"));

        assert_eq!(chunks[2].kind, ChunkKind::Const);
        assert_eq!(chunks[2].name.as_deref(), Some("MAX"));

        assert_eq!(chunks[3].kind, ChunkKind::Static);
        assert_eq!(chunks[3].name.as_deref(), Some("VERSION"));

        assert_eq!(chunks[4].kind, ChunkKind::TypeAlias);
        assert_eq!(chunks[4].name.as_deref(), Some("UserId"));

        assert_eq!(chunks[5].kind, ChunkKind::Macro);
        assert_eq!(chunks[5].name.as_deref(), Some("vec_of"));
    }
}
