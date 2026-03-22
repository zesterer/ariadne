use ariadne::{Color, Config, Label, Report, ReportKind, Source};
use ariadne::{Fmt, ColorGenerator};

fn main() {
    Report::build(ReportKind::Error, 34..34)
        .with_message("Incompatible types")
        .with_label(Label::new(32..33).with_message("This is of type Nat"))
        .with_label(Label::new(52..55).with_message("This is of type Str"))
        .finish()
        .print(Source::from(include_str!("sample.tao")))
        .unwrap();
}
