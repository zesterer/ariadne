use ariadne::{Report, ReportKind, Label, Source, Config, Color};

fn main() {
    let mut src = String::new();
    for c in include_str!("oneline.tao").chars() {
        if c == '\t' {
            src += "    ";
        } else {
            src.push(c);
        }
    }

    println!("");

    Report::build(ReportKind::Error, (), 13)
        .with_code(3)
        .with_message(format!("Incompatible types"))
        .with_label(Label::new(18..19).with_note("This is of type Nat").with_color(Color::Fixed(166)))
        .with_label(Label::new(13..16).with_note("This is of type Str").with_color(Color::Fixed(23)))
        .with_label(Label::new(40..41).with_note("This is of type Nat").with_color(Color::Fixed(48)))
        .with_label(Label::new(43..47).with_note("This is of type Bool").with_color(Color::Fixed(86)))
        .with_label(Label::new(49..51).with_note("This is of type ()").with_color(Color::Fixed(99)))
        .with_label(Label::new(53..55).with_note("This is of type [_]").with_color(Color::Fixed(130)))
        .with_label(Label::new(75..78).with_note("This is of type Str").with_color(Color::Fixed(145)))
        .with_label(Label::new(81..134).with_note("This is of type Nat").with_color(Color::Fixed(178)))
        .with_label(Label::new(100..126).with_note("This is an inner multi-line").with_color(Color::Fixed(185)))
        .with_label(Label::new(106..120).with_note("This is another inner multi-line").with_color(Color::Fixed(197)))
        .with_label(Label::new(108..122).with_note("This is *really* nested multi-line").with_color(Color::Fixed(210)))
        .with_label(Label::new(110..111).with_note("This is an inline within the nesting!").with_color(Color::Fixed(43)))
        .with_label(Label::new(111..112).with_note("And another!").with_color(Color::Fixed(71)))
        .with_label(Label::new(103..123).with_note("This is *really* nested multi-line").with_color(Color::Fixed(101)))
        .with_label(Label::new(105..125).with_note("This is *really* nested multi-line").with_color(Color::Fixed(140)))
        .with_label(Label::new(112..116).with_note("This is *really* nested multi-line").with_color(Color::Fixed(120)))
        .with_label(Label::new(83..117).with_note("Hahaha!").with_color(Color::Fixed(75)))
        .with_label(Label::new(85..110).with_note("Oh god, no more 1").with_color(Color::Fixed(33)))
        .with_label(Label::new(84..114).with_note("Oh god, no more 2").with_color(Color::Fixed(52)))
        .with_label(Label::new(89..113).with_note("Oh god, no more 3").with_color(Color::Fixed(167)))
        .with_config(Config::default()
            .with_cross_gap(true)
            .with_compact(true)
            .with_underlines(true))
        .finish()
        .print(Source::from(src))
        .unwrap();

    println!("");
}
