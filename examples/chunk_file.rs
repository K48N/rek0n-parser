use rek0n_parser::{parse_file, SemanticChunk};
use std::env;
use std::fs;
use std::path::Path;
use std::process;

fn main() {
    let path = match env::args().nth(1) {
        Some(path) => path,
        None => {
            eprintln!("usage: cargo run --example chunk_file -- <source-file>");
            process::exit(1);
        }
    };

    let language = language_for_path(&path);
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("failed to read {path}: {err}");
            process::exit(1);
        }
    };

    match parse_file(&source, language) {
        Ok(chunks) => print_chunks(&path, &chunks),
        Err(err) => {
            eprintln!("parse failed: {err}");
            process::exit(1);
        }
    }
}

fn language_for_path(path: &str) -> &str {
    match Path::new(path).extension().and_then(|ext| ext.to_str()) {
        Some("rs") => "rust",
        _ => "rust",
    }
}

fn print_chunks(path: &str, chunks: &[SemanticChunk]) {
    println!("file: {path}");
    println!("chunks: {}", chunks.len());

    for (index, chunk) in chunks.iter().enumerate() {
        let name = chunk.name.as_deref().unwrap_or("<anonymous>");
        let flag = if chunk.has_error { " (has_error)" } else { "" };
        println!(
            "[{index}] {:?} {name}  L{}-{}{flag}",
            chunk.kind, chunk.start_line, chunk.end_line
        );
        print_preview(&chunk.text);
    }
}

fn print_preview(text: &str) {
    let line_count = text.lines().count();
    for line in text.lines().take(3) {
        println!("  {line}");
    }
    if line_count > 3 {
        println!("  ... ({line_count} lines total)");
    }
}
