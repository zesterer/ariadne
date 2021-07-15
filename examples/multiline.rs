use ariadne::{Report, ReportKind, Label, Source, Color, Fmt};

fn main() {
    println!("");
    Report::build(ReportKind::Error, "sample.tao", 12)
        .with_code(3)
        .with_message(format!("Incompatible types"))
        .with_label(Label::new(("sample.tao", 32..33))
            .with_message(format!("This is of type {}", "Nat".fg(Color::Fixed(166))))
            .with_color(Color::Fixed(166)))
        .with_label(Label::new(("sample.tao", 42..45))
            .with_message(format!("This is of type {}", "Str".fg(Color::Fixed(164))))
            .with_color(Color::Fixed(164)))
        .with_label(Label::new(("sample.tao", 11..48))
            .with_message(format!(
                "The values are outputs of this {} expression",
                "match".fg(Color::Fixed(81)),
            ))
            .with_color(Color::Fixed(81)))
        .with_note(format!("Outputs of {} expressions must coerce to the same type", "match".fg(Color::Fixed(81))))
        .finish()
        .print(("sample.tao", Source::from(include_str!("sample.tao"))))
        .unwrap();

    println!("");
}
