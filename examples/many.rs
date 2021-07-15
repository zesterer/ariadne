use report::{Report, ReportKind, Label, Source, Config, sources};

fn main() {
    Report::build(ReportKind::Error, "b.tao", 10)
        .with_code(3)
        .with_message(format!("Cannot add types Nat and Str"))
        .with_label(Label::new(("b.tao", 10..14)).with_note("This is of type Nat"))
        .with_label(Label::new(("b.tao", 17..20)).with_note("This is of type Str"))
        .with_label(Label::new(("a.tao", 4..8)).with_note("Original definition of 'five' is here"))
        .with_config(Config {
            cross_gap: false,
            compact: false,
            ..Default::default()
        })
        .finish()
        .print(sources(vec![
            ("a.tao", include_str!("a.tao")),
            ("b.tao", include_str!("b.tao")),
        ]))
        .unwrap();
}
