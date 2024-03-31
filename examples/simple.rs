use ariadne::{Diagnostic, Label, ByteSpan};

const CODE: &str = r#"
match x {
    0 => false,
    1 => "true",
}
"#;

fn main() {
    Diagnostic::error()
        .with_message("Type mismatch between `bool` and `&str`")
        .with_label(Label::at(ByteSpan::from(20..25)))
        .with_label(Label::at(ByteSpan::from(36..42)))
        .eprint(CODE);
}
