use report::{Report, Label, Source};

fn main() {
    let primary = 34..35;
    let secondary = 48..51;
    Report::build(Label::new(primary.start..secondary.end))
        .finish()
        .print(Source::from(include_str!("sample.tao")))
        .unwrap();
}
