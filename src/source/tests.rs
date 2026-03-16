use std::iter::zip;
use std::sync::Arc;

use super::Source;

fn test_with_lines(lines: Vec<&str>) {
    let source: String = lines.iter().copied().collect();
    let source = Source::from(source);

    assert_eq!(source.lines.len(), lines.len());

    let mut offset = 0;
    for (source_line, raw_line) in zip(source.lines.iter().copied(), lines.into_iter()) {
        assert_eq!(source_line.offset, offset);
        assert_eq!(source_line.char_len, raw_line.chars().count());
        assert_eq!(source.get_line_text(source_line).unwrap(), raw_line);
        offset += source_line.char_len;
    }

    assert_eq!(source.len, offset);
}

#[test]
fn source_from_empty() {
    test_with_lines(vec![""]); // Empty string
}

#[test]
fn source_from_single() {
    test_with_lines(vec!["Single line"]);
    test_with_lines(vec!["Single line with LF\n"]);
    test_with_lines(vec!["Single line with CRLF\r\n"]);
}

#[test]
fn source_from_multi() {
    test_with_lines(vec!["Two\r\n", "lines\n"]);
    test_with_lines(vec!["Some\n", "more\r\n", "lines"]);
    test_with_lines(vec!["\n", "\r\n", "\n", "Empty Lines"]);
}

#[test]
fn source_from_trims_trailing_spaces() {
    test_with_lines(vec!["Trailing spaces  \n", "are trimmed\t"]);
}

#[test]
fn source_from_alternate_line_endings() {
    // Line endings other than LF or CRLF
    test_with_lines(vec![
        "CR\r",
        "VT\x0B",
        "FF\x0C",
        "NEL\u{0085}",
        "LS\u{2028}",
        "PS\u{2029}",
    ]);
}

#[test]
fn source_from_other_string_types() {
    let raw = r#"A raw string
            with multiple
            lines behind
            an Arc"#;
    let arc = Arc::from(raw);
    let source = Source::from(arc);

    assert_eq!(source.lines.len(), 4);

    let mut offset = 0;
    for (source_line, raw_line) in zip(source.lines.iter().copied(), raw.split_inclusive('\n')) {
        assert_eq!(source_line.offset, offset);
        assert_eq!(source_line.char_len, raw_line.chars().count());
        assert_eq!(source.get_line_text(source_line).unwrap(), raw_line);
        offset += source_line.char_len;
    }

    assert_eq!(source.len, offset);
}

#[test]
fn source_from_reference() {
    let raw = r#"A raw string
            with multiple
            lines"#;

    fn non_owning_source(input: &str) -> Source<&str> {
        Source::from(input)
    }

    let source = non_owning_source(raw);
    assert_eq!(source.lines.len(), 3);
}
