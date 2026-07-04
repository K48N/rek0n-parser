#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ByteRange {
    pub start: usize,
    pub end: usize,
}

pub(super) fn scan_doc_comments(source: &str) -> Vec<ByteRange> {
    let bytes = source.as_bytes();
    let mut ranges = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        if let Some(end) = skip_byte_string_literal(bytes, index) {
            index = end;
            continue;
        }
        if let Some(end) = skip_raw_string_literal(bytes, index) {
            index = end;
            continue;
        }
        if let Some(end) = skip_quoted_literal(bytes, index, b'"') {
            index = end;
            continue;
        }
        if let Some(end) = skip_apostrophe(bytes, index) {
            index = end;
            continue;
        }

        if bytes[index] == b'/' && index + 1 < bytes.len() {
            match bytes[index + 1] {
                b'/' => {
                    let is_doc = index + 2 < bytes.len()
                        && (bytes[index + 2] == b'/' || bytes[index + 2] == b'!');
                    let start = index;
                    index += 2;
                    while index < bytes.len() && bytes[index] != b'\n' {
                        index += 1;
                    }
                    if is_doc {
                        ranges.push(ByteRange { start, end: index });
                    }
                    continue;
                }
                b'*' => {
                    let is_doc = index + 2 < bytes.len()
                        && (bytes[index + 2] == b'*' || bytes[index + 2] == b'!');
                    let start = index;
                    index += 2;
                    let mut depth = 1usize;
                    while index < bytes.len() && depth > 0 {
                        if index + 1 < bytes.len()
                            && bytes[index] == b'/'
                            && bytes[index + 1] == b'*'
                        {
                            depth += 1;
                            index += 2;
                        } else if index + 1 < bytes.len()
                            && bytes[index] == b'*'
                            && bytes[index + 1] == b'/'
                        {
                            depth -= 1;
                            index += 2;
                        } else {
                            index += 1;
                        }
                    }
                    if is_doc {
                        ranges.push(ByteRange { start, end: index });
                    }
                    continue;
                }
                _ => {}
            }
        }

        index += 1;
    }

    ranges
}

pub(super) fn leading_doc_start(
    doc_comments: &[ByteRange],
    source: &str,
    mut start: usize,
) -> usize {
    while let Some(new_start) = doc_comments.iter().rev().find_map(|range| {
        if range.end > start {
            return None;
        }
        let gap = &source[range.end..start];
        gap.chars().all(char::is_whitespace).then_some(range.start)
    }) {
        if new_start == start {
            break;
        }
        start = new_start;
    }

    start
}

fn skip_byte_string_literal(bytes: &[u8], index: usize) -> Option<usize> {
    if bytes.get(index) == Some(&b'b') {
        skip_quoted_literal(bytes, index + 1, b'"')
    } else {
        None
    }
}

fn skip_raw_string_literal(bytes: &[u8], index: usize) -> Option<usize> {
    if bytes.get(index) != Some(&b'r') {
        return None;
    }

    let mut pos = index + 1;
    let mut hashes = 0usize;
    while bytes.get(pos) == Some(&b'#') {
        hashes += 1;
        pos += 1;
    }

    if bytes.get(pos) != Some(&b'"') {
        return None;
    }
    pos += 1;

    while pos < bytes.len() {
        if bytes[pos] == b'"' {
            let mut matched = 0usize;
            while matched < hashes && bytes.get(pos + 1 + matched) == Some(&b'#') {
                matched += 1;
            }
            if matched == hashes {
                return Some(pos + 1 + hashes);
            }
        }
        pos += 1;
    }

    Some(index + 1)
}

fn skip_quoted_literal(bytes: &[u8], index: usize, quote: u8) -> Option<usize> {
    if bytes.get(index) != Some(&quote) {
        return None;
    }

    let mut pos = index + 1;
    while pos < bytes.len() {
        if bytes[pos] == b'\\' {
            pos = (pos + 2).min(bytes.len());
            continue;
        }
        if bytes[pos] == quote {
            return Some(pos + 1);
        }
        pos += 1;
    }

    Some(index + 1)
}

fn skip_apostrophe(bytes: &[u8], index: usize) -> Option<usize> {
    if bytes.get(index) != Some(&b'\'') {
        return None;
    }

    let pos = index + 1;
    if pos >= bytes.len() {
        return Some(pos);
    }

    if is_lifetime_static(bytes, pos) {
        return Some(pos + b"static".len());
    }

    if bytes[pos] == b'\\' {
        return skip_char_literal(bytes, index);
    }

    if bytes.get(pos + 1) == Some(&b'\'') {
        return Some(pos + 2);
    }

    if bytes[pos].is_ascii_alphabetic() || bytes[pos] == b'_' {
        return Some(lifetime_end(bytes, pos));
    }

    skip_char_literal(bytes, index)
}

fn is_lifetime_static(bytes: &[u8], pos: usize) -> bool {
    bytes.get(pos..pos + b"static".len()) == Some(b"static")
        && !bytes
            .get(pos + b"static".len())
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
}

fn lifetime_end(bytes: &[u8], pos: usize) -> usize {
    let mut end = pos;
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
        end += 1;
    }
    end
}

fn skip_char_literal(bytes: &[u8], index: usize) -> Option<usize> {
    let mut pos = index + 1;
    if pos >= bytes.len() {
        return Some(pos);
    }

    if bytes[pos] == b'\\' {
        pos += 1;
        if pos >= bytes.len() {
            return Some(pos);
        }
        match bytes[pos] {
            b'x' => pos = (pos + 3).min(bytes.len()),
            b'u' if bytes.get(pos + 1) == Some(&b'{') => {
                pos += 2;
                while pos < bytes.len() && bytes[pos] != b'}' {
                    pos += 1;
                }
                if pos < bytes.len() {
                    pos += 1;
                }
            }
            _ => pos += 1,
        }
    } else {
        pos += utf8_char_len(bytes[pos]);
    }

    if bytes.get(pos) == Some(&b'\'') {
        Some(pos + 1)
    } else {
        Some(index + 1)
    }
}

fn utf8_char_len(byte: u8) -> usize {
    if byte & 0x80 == 0 {
        1
    } else if byte & 0xE0 == 0xC0 {
        2
    } else if byte & 0xF0 == 0xE0 {
        3
    } else if byte & 0xF8 == 0xF0 {
        4
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_doc_like_text_inside_raw_string() {
        let source = r##"r#" /** not a doc comment "# struct Widget;"##;
        let ranges = scan_doc_comments(source);
        assert!(ranges.is_empty());
    }

    #[test]
    fn records_contiguous_line_docs() {
        let source = "/// one\n/// two\nstruct Widget;\n";
        let ranges = scan_doc_comments(source);
        assert_eq!(ranges.len(), 2);
        assert_eq!(&source[ranges[0].start..ranges[0].end], "/// one");
        assert_eq!(&source[ranges[1].start..ranges[1].end], "/// two");
    }

    #[test]
    fn records_inner_line_doc_comments() {
        let source = "//! crate docs\nfn main() {}\n";
        let ranges = scan_doc_comments(source);
        assert_eq!(ranges.len(), 1);
        assert_eq!(&source[ranges[0].start..ranges[0].end], "//! crate docs");
    }

    #[test]
    fn leading_doc_start_walks_back_through_whitespace() {
        let source = "/// one\n/// two\n\nstruct Widget;\n";
        let ranges = scan_doc_comments(source);
        let item_start = source.find("struct Widget").expect("struct should exist");
        let start = leading_doc_start(&ranges, source, item_start);
        assert_eq!(source[start..item_start].trim(), "/// one\n/// two");
    }

    #[test]
    fn lifetimes_do_not_swallow_following_doc_comments() {
        let source = r#"
fn foo<'a>(x: &'a str) -> &'a str {
    x
}

/// Docs for another.
struct Other;
"#;
        let ranges = scan_doc_comments(source);
        assert_eq!(ranges.len(), 1);
        assert_eq!(
            &source[ranges[0].start..ranges[0].end],
            "/// Docs for another."
        );
    }

    #[test]
    fn char_literals_are_still_skipped() {
        let source = "const C: char = 'a'; /// real doc\nstruct Widget;\n";
        let ranges = scan_doc_comments(source);
        assert_eq!(ranges.len(), 1);
        assert_eq!(&source[ranges[0].start..ranges[0].end], "/// real doc");
    }

    #[test]
    fn static_lifetime_is_not_treated_as_char_literal() {
        let source = "fn foo(x: &'static str) {}\n/// doc\nstruct Widget;\n";
        let ranges = scan_doc_comments(source);
        assert_eq!(ranges.len(), 1);
        assert_eq!(&source[ranges[0].start..ranges[0].end], "/// doc");
    }
}
