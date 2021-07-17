use ariadne::{Report, ReportKind, Label, ColorGenerator, Fmt, sources};

fn main() {
    let mut colors = ColorGenerator::new();

    // Generate some colours for each of our elements
    let a = colors.next();
    let b = colors.next();
    let c = colors.next();

    Report::build(ReportKind::Error, "b.tao", 10)
        .with_code(3)
        .with_message(format!("Cannot add types Nat and Str"))
        .with_label(Label::new(("b.tao", 10..14))
            .with_message(format!("This is of type {}", "Nat".fg(a)))
            .with_color(a))
        .with_label(Label::new(("b.tao", 17..20))
            .with_message(format!("This is of type {}", "Str".fg(b)))
            .with_color(b))
        .with_label(Label::new(("b.tao", 15..16))
            .with_message(format!(" {} and {} undergo addition here", "Nat".fg(a), "Str".fg(b)))
            .with_color(c)
            .with_order(10))
        .with_label(Label::new(("a.tao", 4..8))
            .with_message(format!("Original definition of {} is here", "five".fg(a)))
            .with_color(a))
        .with_note(format!("{} is a number and can only be added to other numbers", "Nat".fg(a)))
        .finish()
        .print(sources(vec![
            ("a.tao", include_str!("a.tao")),
            ("b.tao", include_str!("b.tao")),
        ]))
        .unwrap();
}
