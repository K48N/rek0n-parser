use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use rek0n_parser::{parse_file_with_options, ParseOptions, ParserError};

fn large_rust_source(function_count: usize) -> String {
    (0..function_count)
        .map(|index| format!("fn workload_{index}() {{ let value = {index}; }}\n"))
        .collect()
}

#[test]
fn parse_timed_out_returns_parse_timed_out() {
    let err = parse_file_with_options(
        &large_rust_source(20_000),
        "rust",
        ParseOptions {
            timeout: Duration::from_micros(1),
            cancellation: None,
        },
    )
    .expect_err("parse with a 1 µs timeout should fail");

    assert!(matches!(err, ParserError::ParseTimedOut));
}

#[test]
fn cancelled_parse_returns_parse_cancelled() {
    let flag = AtomicUsize::new(1);
    let err = parse_file_with_options(
        "fn main() {}",
        "rust",
        ParseOptions {
            timeout: Duration::from_secs(5),
            cancellation: Some(&flag),
        },
    )
    .expect_err("pre-cancelled parse should fail");

    assert!(matches!(err, ParserError::ParseCancelled));
}

#[test]
fn default_parse_options_use_five_second_timeout() {
    assert_eq!(
        ParseOptions::default().timeout,
        rek0n_parser::DEFAULT_PARSE_TIMEOUT
    );
}
