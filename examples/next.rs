use ariadne::next::*;

fn main() {
    Report::build()
        .with_source_view(SourceView::build()
            .with_text_color(Color::Red)
            .finish())
        .with_text_color(Color::Blue)
        .finish()
        .eprint()
        .unwrap();
}
