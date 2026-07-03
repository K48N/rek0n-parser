# rek0n-parser

Part of my personal project rek0n. Parses source into intact structural blocks for RAG ingestion.

## What it is

A Rust library that turns source files into `SemanticChunk` values (functions, structs, impl blocks, enums, traits, and the rest) ready for embedding downstream. No vectors, models, or databases here; just parse and return text + metadata.

Most chunking for RAG pipelines is line-based or token-based, and libraries for that already exist. This crate exists because those approaches cut functions and impl blocks in half, which produces embeddings that do not mean much. Tree-sitter takes more setup than a text splitter, but it is the only reliable way to guarantee a chunk boundary lines up with a real structural unit. Rust is the only language wired up today; every other language needs its own extractor built on the same approach.

## How it works

1. `parse_file(source, "rust")` runs tree-sitter with a query over top-level item nodes.
2. Items nested inside `impl`/`trait` bodies stay in the parent chunk. Items inside `mod`/`extern` blocks are extracted on their own.
3. Leading `#[attributes]` come from AST sibling nodes. `///` / `/**` docs come from a string-aware source scan (tree-sitter stores comments as extras, not tree nodes).
4. Each chunk carries kind, name, line range, text, and a `has_error` flag when tree-sitter inserted recovery nodes inside that item.
5. Parse timeouts and cancellation are configurable via `ParseOptions`. Errors elsewhere in the file do not block extraction of valid items.

## Why it's built this way

**Tree-sitter over naive splitting.** Line-based or fixed-size chunking cuts functions and impl blocks in half. AST queries keep semantic units intact so embeddings stay meaningful.

**Hard parser boundary.** rek0n's indexer handles embed and store. This crate answers one question only: what are the structural pieces? No hidden side effects.

**Best-effort on broken source.** A syntax error mid-file should not discard valid chunks above it. RAG ingestion prefers partial coverage over all-or-nothing.

**Hybrid metadata attachment.** Attributes are real AST siblings; doc comments are not in the tree at all. A small lexer indexes doc ranges and skips string/char/lifetime literals so lifetimes like `'a` do not corrupt the scan.

**Thin dependency stack.** tree-sitter, thiserror, optional serde. No tokenizer framework, no language-server stack.

## Shortcomings

- Rust only today; other languages need their own extractor module.
- Doc comments are lexer-indexed, not tree-walked, since tree-sitter-rust puts them in `extras`.
- `has_error` covers `ERROR`/`MISSING` nodes inside a chunk, not every subtle recovery artifact.
- Inline `mod` blocks produce both a `Mod` chunk and separate chunks for items inside it.
- `union`, `use`, and bare `extern mod` declarations are not captured yet.

## Usage

```rust
use rek0n_parser::{parse_file, ChunkKind};

let chunks = parse_file(source, "rust")?;
for chunk in &chunks {
    if chunk.has_error {
        continue;
    }
    // embed chunk.text
}
```

See `examples/chunk_file.rs` for a full file-to-chunks walkthrough:

```sh
cargo run --example chunk_file -- path/to/lib.rs
```

## License

MIT
