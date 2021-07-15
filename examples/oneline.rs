use report::{Report, ReportKind, Label, Source, Config};

fn main() {
    Report::build(
        ReportKind::Error,
        Label::new(13..16)
            .with_note("This is of type Str")
    )
        .with_code(3)
        .with_message(format!("Incompatible types"))
        .with_label(Label::new(18..19).with_note("This is of type Nat"))
        .with_label(Label::new(35..38).with_note("This is of type Str"))
        .with_label(Label::new(40..41).with_note("This is of type Nat"))
        .with_label(Label::new(43..47).with_note("This is of type Bool"))
        .with_label(Label::new(49..51).with_note("This is of type ()"))
        .with_label(Label::new(53..55).with_note("This is of type [_]"))
        .with_label(Label::new(75..78).with_note("This is of type Str"))
        .with_label(Label::new(81..128).with_note("This is of type Nat"))
        .with_label(Label::new(100..126).with_note("This is an inner multi-line"))
        .with_label(Label::new(106..120).with_note("This is another inner multi-line"))
        .with_label(Label::new(108..122).with_note("This is *really* nested multi-line"))
        .with_label(Label::new(110..111).with_note("This is an inline within the nesting!"))
        .with_label(Label::new(111..112).with_note("And another!"))
        .with_label(Label::new(103..123).with_note("This is *really* nested multi-line"))
        .with_label(Label::new(105..125).with_note("This is *really* nested multi-line"))
        .with_label(Label::new(107..116).with_note("This is *really* nested multi-line"))
        .with_label(Label::new(83..117).with_note("Hahaha!"))
        .with_label(Label::new(85..110).with_note("Oh god, no more 1"))
        .with_label(Label::new(84..114).with_note("Oh god, no more 2"))
        .with_label(Label::new(89..113).with_note("Oh god, no more 3"))
        .with_config(Config {
            cross_gap: false,
            compact: false,
            ..Default::default()
        })
        .finish()
        .print(Source::from(include_str!("oneline.tao")))
        .unwrap();
}
