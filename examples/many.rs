use ariadne::{Report, ReportKind, Label, Color, Fmt, sources};

fn main() {
    Report::build(ReportKind::Error, "b.tao", 10)
        .with_code(3)
        .with_message(format!("Cannot add types Nat and Str"))
        .with_label(Label::new(("b.tao", 10..14))
            .with_message(format!("This is of type {}", "Nat".fg(Color::Fixed(166))))
            .with_color(Color::Fixed(166)))
        .with_label(Label::new(("b.tao", 17..20))
            .with_message(format!("This is of type {}", "Str".fg(Color::Fixed(164))))
            .with_color(Color::Fixed(164)))
        .with_label(Label::new(("b.tao", 15..16))
            .with_message(format!(" {} and {} undergo addition here", "Nat".fg(Color::Fixed(166)), "Str".fg(Color::Fixed(164))))
            .with_color(Color::Green)
            .with_order(10))
        .with_label(Label::new(("a.tao", 4..8))
            .with_message(format!("Original definition of {} is here", "five".fg(Color::Cyan)))
            .with_color(Color::Cyan))
        .with_note(format!("{} is a number and can only be added to other numbers", "Nat".fg(Color::Fixed(166))))
        .finish()
        .print(sources(vec![
            ("a.tao", include_str!("a.tao")),
            ("b.tao", include_str!("b.tao")),
        ]))
        .unwrap();
}
