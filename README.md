# rek0n-parser

Turns source files into structural chunks for RAG ingestion.

## Overview

This crate parses Rust source with tree-sitter and returns `ParsedChunk` values: functions, structs, impl blocks, enums, traits, and related items with line ranges and source text. It does not embed, store vectors, or talk to a database.

Line-based splitters are easy to ship but they cut functions in half. Tree-sitter costs more setup, but chunk boundaries follow real syntax, which makes downstream embeddings much more useful.

## How it works

1. `parse_file(source, "rust")` runs a tree-sitter query over top-level items.
2. Items inside inline `mod { ... }` blocks are extracted separately. File-only `mod foo;` declarations stay one chunk.
3. Leading `#[attributes]` and `inner_attribute_item` siblings are included via AST walks. Doc comments come from a string-aware scan because tree-sitter stores them as extras.
4. Each chunk carries kind, optional name, text, start/end lines, and `has_error` when recovery nodes appear inside that item.
5. `ParseOptions` controls timeout, source size, chunk count, and cooperative cancellation.

## Design

**Tree-sitter over token splitting.** RAG over code needs whole functions and impl blocks, not arbitrary 512-token windows.

**Best effort on broken files.** A syntax error mid-file should not throw away valid chunks above it.

**Hard parser boundary.** This crate answers one question: what are the structural pieces? Embedding and storage live elsewhere.

**Thin stack.** tree-sitter, thiserror, and optional serde.

## Usage

```rust
use rek0n_parser::{parse_file, ChunkKind, ParsedChunk};

let chunks: Vec<ParsedChunk> = parse_file(source, "rust")?;
for chunk in &chunks {
    if chunk.has_error {
        continue;
    }
    // hand off chunk.text to rek0n-embed as IndexedChunk
}
```

Example:

```sh
cargo run --example chunk_file -- path/to/lib.rs
```

## Known gaps

- Rust only. Other languages need their own extractor module on the same pattern.
- Doc comments are lexer-indexed, not tree-walked.
- `has_error` flags recovery inside a chunk, not every subtle parse artifact.

## License

MIT
