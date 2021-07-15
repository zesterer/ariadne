use ariadne::{Report, ReportKind, Label, Source, Config, Color, sources};

fn main() {
    Report::build(ReportKind::Error, "b.tao", 10)
        .with_code(3)
        .with_message(format!("Cannot add types Nat and Str"))
        .with_label(Label::new(("b.tao", 10..14))
            .with_note("This is of type Nat")
            .with_color(Color::Fixed(166)))
        .with_label(Label::new(("b.tao", 17..20))
            .with_note("This is of type Str")
            .with_color(Color::Fixed(164)))
        .with_label(Label::new(("a.tao", 4..8))
            .with_note("Original definition of 'five' is here")
            .with_color(Color::Cyan))
        .with_config(Config::default()
            .with_compact(false)
            .with_color(true))
        .finish()
        .print(sources(vec![
            ("a.tao", include_str!("a.tao")),
            ("b.tao", include_str!("b.tao")),
        ]))
        .unwrap();
}
