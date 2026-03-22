//! These tests use [insta](https://insta.rs/). If you do `cargo install cargo-insta` you can
//! automatically update the snapshots with `cargo insta review` or `cargo insta accept`.
//!
//! When adding new tests you can leave the string in the `assert_snapshot!` macro call empty:
//!
//!     assert_snapshot!(msg, @"");
//!
//! and insta will fill it in.

use insta::assert_snapshot;

use crate::{
    Cache, Config, FnCache, IndexType, Label, Report, ReportKind, ReportStyle, Source, Span,
};

impl<S: Span, K: ReportStyle> Report<S, K> {
    fn write_to_string<C: Cache<S::SourceId>>(&self, cache: C) -> String {
        let mut vec = Vec::new();
        self.write(cache, &mut vec).unwrap();
        String::from_utf8(vec).unwrap()
    }
}

fn no_color() -> Config {
    Config::default().with_color(false)
}

fn remove_trailing(s: String) -> String {
    s.lines().flat_map(|l| [l.trim_end(), "\n"]).collect()
}

fn multi_sources<'srcs, const NB_SOURCES: usize>(
    sources: &'srcs [&'static str; NB_SOURCES],
) -> impl Cache<usize> + 'srcs {
    FnCache::new(move |id: &_| Ok::<_, std::convert::Infallible>(sources[*id]))
}

#[test]
fn one_message() {
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_message("can't compare apples with oranges")
            .finish()
            .write_to_string(Source::from("")),
    );
    assert_snapshot!(msg, @r###"Error: can't compare apples with oranges"###)
}

#[test]
fn two_labels_without_messages() {
    let source = "apple == orange;";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_message("can't compare apples with oranges")
            .with_label(Label::new(0..5))
            .with_label(Label::new(9..15))
            .finish()
            .write_to_string(Source::from(source)),
    );
    assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple == orange;
           │ ─────    ──────
        ───╯
        ");
}

#[test]
fn two_labels_without_messages_on_different_lines() {
    let source = "apple\n== orange;";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_message("can't compare apples with oranges")
            .with_label(Label::new(0..5))
            .with_label(Label::new(9..15))
            .finish()
            .write_to_string(Source::from(source)),
    );
    assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple
           │ ─────
         2 │ == orange;
           │    ──────
        ───╯
        ");
}

#[test]
fn two_labels_with_messages() {
    let source = "apple == orange;";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_message("can't compare apples with oranges")
            .with_label(Label::new(0..5).with_message("This is an apple"))
            .with_label(Label::new(9..15).with_message("This is an orange"))
            .finish()
            .write_to_string(Source::from(source)),
    );
    // TODO: it would be nice if these lines didn't cross
    assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple == orange;
           │ ──┬──    ───┬──
           │   ╰─────────│──── This is an apple
           │             │
           │             ╰──── This is an orange
        ───╯
        ");
}

#[test]
fn two_labels_with_messages_on_different_lines() {
    let source = "apple ==\norange;";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_message("can't compare apples with oranges")
            .with_label(Label::new(0..5).with_message("This is an apple"))
            .with_label(Label::new(9..15).with_message("This is an orange"))
            .finish()
            .write_to_string(Source::from(source)),
    );
    assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple ==
           │ ──┬──
           │   ╰──── This is an apple
         2 │ orange;
           │ ───┬──
           │    ╰──── This is an orange
        ───╯
        ");
}
#[test]
fn duplicate_label() {
    let source = "apple == orange;";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_message("can't compare apples with oranges")
            .with_label(Label::new(0..5).with_message("This is an apple"))
            .with_label(Label::new(0..5).with_message("This is an apple"))
            .finish()
            .write_to_string(Source::from(source)),
    );
    assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple == orange;
           │ ──┬──
           │   ╰──── This is an apple
           │   │
           │   ╰──── This is an apple
        ───╯
        ");
}

#[test]
fn multi_byte_chars() {
    let source = "äpplë == örängë;";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color().with_index_type(IndexType::Char))
            .with_message("can't compare äpplës with örängës")
            .with_label(Label::new(0..5).with_message("This is an äpplë"))
            .with_label(Label::new(9..15).with_message("This is an örängë"))
            .finish()
            .write_to_string(Source::from(source)),
    );
    // TODO: it would be nice if these lines didn't cross
    assert_snapshot!(msg, @"
        Error: can't compare äpplës with örängës
           ╭─┤ <unknown>:1:1 │
           │
         1 │ äpplë == örängë;
           │ ──┬──    ───┬──
           │   ╰─────────│──── This is an äpplë
           │             │
           │             ╰──── This is an örängë
        ───╯
        ");
}

#[test]
fn byte_label() {
    let source = "äpplë == örängë;";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color().with_index_type(IndexType::Byte))
            .with_message("can't compare äpplës with örängës")
            .with_label(Label::new(0..7).with_message("This is an äpplë"))
            .with_label(Label::new(11..20).with_message("This is an örängë"))
            .finish()
            .write_to_string(Source::from(source)),
    );
    // TODO: it would be nice if these lines didn't cross
    assert_snapshot!(msg, @"
        Error: can't compare äpplës with örängës
           ╭─┤ <unknown>:1:1 │
           │
         1 │ äpplë == örängë;
           │ ──┬──    ───┬──
           │   ╰─────────│──── This is an äpplë
           │             │
           │             ╰──── This is an örängë
        ───╯
        ");
}

#[test]
fn byte_column() {
    let source = "äpplë == örängë;";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 11..11)
            .with_config(no_color().with_index_type(IndexType::Byte))
            .with_message("can't compare äpplës with örängës")
            .with_label(Label::new(0..7).with_message("This is an äpplë"))
            .with_label(Label::new(11..20).with_message("This is an örängë"))
            .finish()
            .write_to_string(Source::from(source)),
    );
    // TODO: it would be nice if these lines didn't cross
    assert_snapshot!(msg, @"
        Error: can't compare äpplës with örängës
           ╭─┤ <unknown>:1:10 │
           │
         1 │ äpplë == örängë;
           │ ──┬──    ───┬──
           │   ╰─────────│──── This is an äpplë
           │             │
           │             ╰──── This is an örängë
        ───╯
        ");
}

#[test]
fn crossing_lines() {
    let source = "äpplë == örängë;";
    let msg = Report::build(ReportKind::Error, 11..11)
        .with_config(no_color().with_cross_gap(false))
        .with_message("can't compare äpplës with örängës")
        .with_label(Label::new(0..5).with_message("This is an äpplë"))
        .with_label(Label::new(9..15).with_message("This is an örängë"))
        .finish()
        .write_to_string(Source::from(source));
    // TODO: it would be nice if these lines didn't cross
    assert_snapshot!(msg, @"
        Error: can't compare äpplës with örängës
           ╭─┤ <unknown>:1:12 │
           │
         1 │ äpplë == örängë;
           │ ──┬──    ───┬──  
           │   ╰─────────┼──── This is an äpplë
           │             │    
           │             ╰──── This is an örängë
        ───╯
        ");
}

#[test]
fn label_at_end_of_long_line() {
    let source = format!("{}orange", "apple == ".repeat(100));
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_message("can't compare apples with oranges")
            .with_label(
                Label::new(source.len() - 6..source.len()).with_message("This is an orange"),
            )
            .finish()
            .write_to_string(Source::from(source)),
    );
    // TODO: it would be nice if the start of long lines would be omitted (like rustc does)
    assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == orange
           │                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     ───┬──
           │                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        ╰──── This is an orange
        ───╯
        ");
}

#[test]
fn label_of_width_zero_at_end_of_line() {
    let source = "apple ==\n";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color().with_index_type(IndexType::Byte))
            .with_message("unexpected end of file")
            .with_label(Label::new(9..9).with_message("Unexpected end of file"))
            .finish()
            .write_to_string(Source::from(source)),
    );

    assert_snapshot!(msg, @"
        Error: unexpected end of file
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple ==
           │          │
           │          ╰─ Unexpected end of file
        ───╯
        ");
}

#[test]
fn empty_input() {
    let source = "";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_message("unexpected end of file")
            .with_label(Label::new(0..0).with_message("No more fruit!"))
            .finish()
            .write_to_string(Source::from(source)),
    );

    assert_snapshot!(msg, @"
        Error: unexpected end of file
           ╭─┤ <unknown>:1:1 │
           │
         1 │
           │ │
           │ ╰─ No more fruit!
        ───╯
        ");
}

#[test]
fn empty_input_help() {
    let source = "";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_message("unexpected end of file")
            .with_label(Label::new(0..0).with_message("No more fruit!"))
            .with_help("have you tried going to the farmer's market?")
            .finish()
            .write_to_string(Source::from(source)),
    );

    assert_snapshot!(msg, @"
        Error: unexpected end of file
           ╭─┤ <unknown>:1:1 │
           │
         1 │
           │ │
           │ ╰─ No more fruit!
           │
           │ Help: have you tried going to the farmer's market?
        ───╯
        ");
}

#[test]
fn empty_input_note() {
    let source = "";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_message("unexpected end of file")
            .with_label(Label::new(0..0).with_message("No more fruit!"))
            .with_note("eat your greens!")
            .finish()
            .write_to_string(Source::from(source)),
    );

    assert_snapshot!(msg, @"
        Error: unexpected end of file
           ╭─┤ <unknown>:1:1 │
           │
         1 │
           │ │
           │ ╰─ No more fruit!
           │
           │ Note: eat your greens!
        ───╯
        ");
}

#[test]
fn empty_input_help_note() {
    let source = "";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_message("unexpected end of file")
            .with_label(Label::new(0..0).with_message("No more fruit!"))
            .with_note("eat your greens!")
            .with_help("have you tried going to the farmer's market?")
            .finish()
            .write_to_string(Source::from(source)),
    );

    assert_snapshot!(msg, @"
        Error: unexpected end of file
           ╭─┤ <unknown>:1:1 │
           │
         1 │
           │ │
           │ ╰─ No more fruit!
           │
           │ Help: have you tried going to the farmer's market?
           │
           │ Note: eat your greens!
        ───╯
        ");
}

#[test]
fn byte_spans_never_crash() {
    let source = "apple\np\n\nempty\n";

    for i in 0..=source.len() {
        for j in i..=source.len() {
            let _ = remove_trailing(
                Report::build(ReportKind::Error, 0..0)
                    .with_config(no_color().with_index_type(IndexType::Byte))
                    .with_message("Label")
                    .with_label(Label::new(i..j).with_message("Label"))
                    .finish()
                    .write_to_string(Source::from(source)),
            );
        }
    }
}

#[test]
fn multiline_label() {
    let source = "apple\n==\norange";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_label(Label::new(0..source.len()).with_message("illegal comparison"))
            .finish()
            .write_to_string(Source::from(source)),
    );
    // TODO: it would be nice if the 2nd line wasn't omitted
    assert_snapshot!(msg, @"
        Error:
           ╭─┤ <unknown>:1:1 │
           │
         1 │ ╭─▶ apple
           ┆ ┆
         3 │ ├─▶ orange
           │ │
           │ ╰─────────── illegal comparison
        ───╯
        ");
}

#[test]
fn multiple_multilines_same_span() {
    let source = "apple\n==\norange";
    let msg = Report::build(ReportKind::Error, 0..0)
        .with_config(no_color())
        .with_label(Label::new(0..source.len()).with_message("illegal comparison"))
        .with_label(Label::new(0..source.len()).with_message("do not do this"))
        .with_label(Label::new(0..source.len()).with_message("please reconsider"))
        .finish()
        .write_to_string(Source::from(source));
    // TODO: it would be nice if the 2nd line wasn't omitted
    // TODO: it would be nice if the lines didn't cross, or at least less so
    assert_snapshot!(msg, @"
    Error: 
       ╭─┤ <unknown>:1:1 │
       │
     1 │ ╭─────▶ apple
       │ │       ▲       
       │ │ ╭─────╯       
       │ │ │     │       
       │ │ │ ╭───╯       
       ┆ ┆ ┆ ┆   
     3 │ ├─│─│─▶ orange
       │ │ │ │        ▲  
       │ ╰─│─│────────│── illegal comparison
       │   │ │        │  
       │   ╰─│────────┴── do not do this
       │     │        │  
       │     ╰────────┴── please reconsider
    ───╯
    ");
}

#[test]
fn multiline_context_label() {
    let source = "apple\nbanana\ncarrot\ndragonfruit\negg\nfruit\ngrapes";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color().with_context_lines(1))
            .with_label(Label::new(13..35).with_message("illegal comparison"))
            .finish()
            .write_to_string(Source::from(source)),
    );
    // TODO: it would be nice if the 2nd line wasn't omitted
    assert_snapshot!(msg, @"
        Error:
           ╭─┤ <unknown>:1:1 │
           │
         2 │     banana
         3 │ ╭─▶ carrot
         4 │ │   dragonfruit
         5 │ ├─▶ egg
           │ │
           │ ╰───────── illegal comparison
         6 │     fruit
        ───╯
        ");
}

#[test]
fn partially_overlapping_labels() {
    let source = "https://example.com/";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_label(Label::new(0..source.len()).with_message("URL"))
            .with_label(Label::new(0..source.find(':').unwrap()).with_message("scheme"))
            .finish()
            .write_to_string(Source::from(source)),
    );
    // TODO: it would be nice if you could tell where the spans start and end.
    assert_snapshot!(msg, @"
        Error:
           ╭─┤ <unknown>:1:1 │
           │
         1 │ https://example.com/
           │ ──┬───────┬─────────
           │   ╰───────│─────────── scheme
           │           │
           │           ╰─────────── URL
        ───╯
        ");
}

#[test]
fn multiple_labels_same_span() {
    let source = "apple == orange;";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_message("can't compare apples with oranges")
            .with_label(Label::new(0..5).with_message("This is an apple"))
            .with_label(Label::new(0..5).with_message("Have I mentioned that this is an apple?"))
            .with_label(Label::new(0..5).with_message("No really, have I mentioned that?"))
            .with_label(Label::new(9..15).with_message("This is an orange"))
            .with_label(Label::new(9..15).with_message("Have I mentioned that this is an orange?"))
            .with_label(Label::new(9..15).with_message("No really, have I mentioned that?"))
            .finish()
            .write_to_string(Source::from(source)),
    );
    assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple == orange;
           │ ──┬──    ───┬──
           │   ╰─────────│──── This is an apple
           │   │         │
           │   ╰─────────│──── Have I mentioned that this is an apple?
           │   │         │
           │   ╰─────────│──── No really, have I mentioned that?
           │             │
           │             ╰──── This is an orange
           │             │
           │             ╰──── Have I mentioned that this is an orange?
           │             │
           │             ╰──── No really, have I mentioned that?
        ───╯
        ")
}

#[test]
fn note() {
    let source = "apple == orange;";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_message("can't compare apples with oranges")
            .with_label(Label::new(0..5).with_message("This is an apple"))
            .with_label(Label::new(9..15).with_message("This is an orange"))
            .with_note("stop trying ... this is a fruitless endeavor")
            .finish()
            .write_to_string(Source::from(source)),
    );
    assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple == orange;
           │ ──┬──    ───┬──
           │   ╰─────────│──── This is an apple
           │             │
           │             ╰──── This is an orange
           │
           │ Note: stop trying ... this is a fruitless endeavor
        ───╯
        ")
}

#[test]
fn help() {
    let source = "apple == orange;";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_message("can't compare apples with oranges")
            .with_label(Label::new(0..5).with_message("This is an apple"))
            .with_label(Label::new(9..15).with_message("This is an orange"))
            .with_help("have you tried peeling the orange?")
            .finish()
            .write_to_string(Source::from(source)),
    );
    assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple == orange;
           │ ──┬──    ───┬──
           │   ╰─────────│──── This is an apple
           │             │
           │             ╰──── This is an orange
           │
           │ Help: have you tried peeling the orange?
        ───╯
        ")
}

#[test]
fn help_and_note() {
    let source = "apple == orange;";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_message("can't compare apples with oranges")
            .with_label(Label::new(0..5).with_message("This is an apple"))
            .with_label(Label::new(9..15).with_message("This is an orange"))
            .with_help("have you tried peeling the orange?")
            .with_note("stop trying ... this is a fruitless endeavor")
            .finish()
            .write_to_string(Source::from(source)),
    );
    assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple == orange;
           │ ──┬──    ───┬──
           │   ╰─────────│──── This is an apple
           │             │
           │             ╰──── This is an orange
           │
           │ Help: have you tried peeling the orange?
           │
           │ Note: stop trying ... this is a fruitless endeavor
        ───╯
        ")
}

#[test]
fn single_note_single_line() {
    let source = "apple == orange;";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_message("can't compare apples with oranges")
            .with_label(Label::new(0..15).with_message("This is a strange comparison"))
            .with_note("No need to try, they can't be compared.")
            .finish()
            .write_to_string(Source::from(source)),
    );
    assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple == orange;
           │ ───────┬───────
           │        ╰───────── This is a strange comparison
           │
           │ Note: No need to try, they can't be compared.
        ───╯
        ")
}

#[test]
fn multi_notes_single_lines() {
    let source = "apple == orange;";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_message("can't compare apples with oranges")
            .with_label(Label::new(0..15).with_message("This is a strange comparison"))
            .with_note("No need to try, they can't be compared.")
            .with_note("Yeah, really, please stop.")
            .finish()
            .write_to_string(Source::from(source)),
    );
    assert_snapshot!(msg, @"
    Error: can't compare apples with oranges
       ╭─┤ <unknown>:1:1 │
       │
     1 │ apple == orange;
       │ ───────┬───────
       │        ╰───────── This is a strange comparison
       │
       │ Note 1: No need to try, they can't be compared.
       │ Note 2: Yeah, really, please stop.
    ───╯
    ")
}

#[test]
fn multi_notes_multi_lines() {
    let source = "apple == orange;";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_message("can't compare apples with oranges")
            .with_label(Label::new(0..15).with_message("This is a strange comparison"))
            .with_note("No need to try, they can't be compared.")
            .with_note("Yeah, really, please stop.\nIt has no resemblance.")
            .finish()
            .write_to_string(Source::from(source)),
    );
    assert_snapshot!(msg, @"
    Error: can't compare apples with oranges
       ╭─┤ <unknown>:1:1 │
       │
     1 │ apple == orange;
       │ ───────┬───────
       │        ╰───────── This is a strange comparison
       │
       │ Note 1: No need to try, they can't be compared.
       │ Note 2: Yeah, really, please stop.
       │         It has no resemblance.
    ───╯
    ")
}

#[test]
fn multi_helps_multi_lines() {
    let source = "apple == orange;";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_message("can't compare apples with oranges")
            .with_label(Label::new(0..15).with_message("This is a strange comparison"))
            .with_help("No need to try, they can't be compared.")
            .with_help("Yeah, really, please stop.\nIt has no resemblance.")
            .finish()
            .write_to_string(Source::from(source)),
    );
    assert_snapshot!(msg, @"
    Error: can't compare apples with oranges
       ╭─┤ <unknown>:1:1 │
       │
     1 │ apple == orange;
       │ ───────┬───────
       │        ╰───────── This is a strange comparison
       │
       │ Help 1: No need to try, they can't be compared.
       │ Help 2: Yeah, really, please stop.
       │         It has no resemblance.
    ───╯
    ")
}

#[test]
fn ordered_labels() {
    let msg = remove_trailing(
        Report::build(ReportKind::Error, ("", 0..0))
            .with_config(no_color())
            .with_label(Label::new(("b", 13..18)).with_order(1).with_message("1"))
            .with_label(Label::new(("a", 0..6)).with_order(2).with_message("2"))
            .with_label(Label::new(("a", 7..12)).with_order(3).with_message("3"))
            .with_label(Label::new(("b", 0..6)).with_order(4).with_message("4"))
            .with_label(Label::new(("b", 7..12)).with_order(5).with_message("5"))
            .finish()
            .write_to_string(crate::sources([
                ("a", "second\nthird"),
                ("b", "fourth\nfifth\nfirst"),
            ])),
    );
    assert_snapshot!(msg, @"
        Error:
           ╭─┤ b:3:1 │
           │
         3 │ first
           │ ──┬──
           │   ╰──── 1
           │
           ├─┤ a:1:1 │
           │
         1 │ second
           │ ───┬──
           │    ╰──── 2
         2 │ third
           │ ──┬──
           │   ╰──── 3
           │
           ├─┤ b:1:1 │
           │
         1 │ fourth
           │ ───┬──
           │    ╰──── 4
         2 │ fifth
           │ ──┬──
           │   ╰──── 5
        ───╯
        ")
}

#[test]
fn minimise_crossings() {
    let source = "begin\napple == orange;\nend";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color().with_minimise_crossings(true))
            .with_message("can't compare apples with oranges")
            .with_label(Label::new(6..11).with_message("This is an apple"))
            .with_label(Label::new(15..21).with_message("This is an orange"))
            .with_label(Label::new(3..25).with_message("multi 1"))
            .with_label(Label::new(25..26).with_message("single"))
            .finish()
            .write_to_string(Source::from(source)),
    );
    assert_snapshot!(msg, @"
    Error: can't compare apples with oranges
       ╭─┤ <unknown>:1:1 │
       │
     1 │ ╭─▶ begin
     2 │ │   apple == orange;
       │ │   ──┬──    ───┬──
       │ │     │         ╰──── This is an orange
       │ │     │
       │ │     ╰────────────── This is an apple
     3 │ ├─▶ end
       │ │     ▲
       │ │     ╰── single
       │ │
       │ ╰──────── multi 1
    ───╯
    ")
}

#[test]
fn only_help_and_note() {
    let source = "this should not be printed";
    let msg = remove_trailing(
        Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_message("Programming language \"Rest\" not found")
            .with_help("a language with a similar name exists: Rust")
            .with_note("perhaps you'd like some sleep?")
            .finish()
            .write_to_string(Source::from(source)),
    );
    assert_snapshot!(msg, @r###"
        Error: Programming language "Rest" not found
        
        Help: a language with a similar name exists: Rust
        
        Note: perhaps you'd like some sleep?
        "###)
}

#[test]
fn multi_source() {
    let msg = remove_trailing(
        Report::build(ReportKind::Error, (0, 0..0))
            .with_config(no_color())
            .with_message("can't compare apples with oranges or pears")
            .with_label(Label::new((0, 0..5)).with_message("This is an apple"))
            .with_label(Label::new((0, 9..15)).with_message("This is an orange"))
            .with_label(Label::new((1, 0..5)).with_message("This is an apple"))
            .with_label(Label::new((1, 9..13)).with_message("This is a pear"))
            .finish()
            .write_to_string(multi_sources(&["apple == orange;", "apple == pear;"])),
    );
    assert_snapshot!(msg, @"
        Error: can't compare apples with oranges or pears
           ╭─┤ 0:1:1 │
           │
         1 │ apple == orange;
           │ ──┬──    ───┬──
           │   ╰─────────│──── This is an apple
           │             │
           │             ╰──── This is an orange
           │
           ├─┤ 1:1:1 │
           │
         1 │ apple == pear;
           │ ──┬──    ──┬─
           │   ╰────────│─── This is an apple
           │            │
           │            ╰─── This is a pear
        ───╯
        ")
}

#[test]
fn help_and_note_multi() {
    let msg = remove_trailing(
        Report::build(ReportKind::Error, (0, 0..0))
            .with_config(no_color())
            .with_message("can't compare apples with oranges or pears")
            .with_label(Label::new((0, 0..5)).with_message("This is an apple"))
            .with_label(Label::new((0, 9..15)).with_message("This is an orange"))
            .with_label(Label::new((1, 0..5)).with_message("This is an apple"))
            .with_label(Label::new((1, 9..13)).with_message("This is a pear"))
            .with_help("have you tried peeling the orange?")
            .with_note("stop trying ... this is a fruitless endeavor")
            .finish()
            .write_to_string(multi_sources(&["apple == orange;", "apple == pear;"])),
    );
    assert_snapshot!(msg, @"
        Error: can't compare apples with oranges or pears
           ╭─┤ 0:1:1 │
           │
         1 │ apple == orange;
           │ ──┬──    ───┬──
           │   ╰─────────│──── This is an apple
           │             │
           │             ╰──── This is an orange
           │
           ├─┤ 1:1:1 │
           │
         1 │ apple == pear;
           │ ──┬──    ──┬─
           │   ╰────────│─── This is an apple
           │            │
           │            ╰─── This is a pear
           │
           │ Help: have you tried peeling the orange?
           │
           │ Note: stop trying ... this is a fruitless endeavor
        ───╯
        ")
}

#[test]
fn no_labels() {
    let msg = remove_trailing(
        Report::build(ReportKind::Error, (0, 0..0))
            .with_config(no_color())
            .with_message("no code")
            .with_help("have you tried adding code?")
            .with_note("code needs to exist")
            .finish()
            .write_to_string(multi_sources(&[])),
    );
    assert_snapshot!(msg, @"
        Error: no code
        
        Help: have you tried adding code?
        
        Note: code needs to exist
        ")
}
