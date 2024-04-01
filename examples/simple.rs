use ariadne::{files, ByteSpan, CharSpan, Diagnostic, Label};

const CODE: &str = r#"
match x {
    0 => false,
    _ => "true",
}
"#;

fn main() {
    Diagnostic::error()
        .with_message("Type mismatch between `bool` and `&str`")
        .with_label(Label::at(ByteSpan::from(20..25)))
        .with_label(Label::at(ByteSpan::from(36..42)))
        .eprint(CODE);

    Diagnostic::error()
        .with_message("Type mismatch between `bool` and `&str`")
        .with_label(Label::at(ByteSpan::new(20..25, "a")))
        .with_label(Label::at(ByteSpan::new(36..42, "b")))
        .eprint(files([("a", "a.rs", CODE), ("b", "b.rs", CODE)]));
}
