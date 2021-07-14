use report::{Report, ReportKind, Label, Source};

fn main() {
    let primary = 34..35;
    let secondary = 46..49;
    Report::build(
        ReportKind::Error,
        Label::new(primary)
            .with_note("This is of type Nat")
    )
        .with_code(3)
        .with_message(format!("Incompatible types"))
        .with_label(Label::new(secondary).with_note("This is of type Str"))
        .finish()
        .print(Source::from(include_str!("sample.tao")))
        .unwrap();
}
