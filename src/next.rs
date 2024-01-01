#![allow(missing_docs)]

use std::{io, fmt};

pub trait Format {
    type Error;

    fn write(&mut self, report: &Report) -> Result<(), Self::Error>;
}

struct Ansi<W>(pub W);

impl<W: io::Write> Format for Ansi<W> {
    type Error = io::Error;

    fn write(&mut self, report: &Report) -> Result<(), Self::Error> {
        for elem in &report.elements {
            match elem {
                Element::SourceView(source_view) => {
                    writeln!(self.0, "<source view>")?;
                },
            }
        }
        Ok(())
    }
}

// Report tree

enum Element {
    SourceView(SourceView),
}

pub struct Report {
    elements: Vec<Element>,
    style: Style,
}

impl Report {
    pub fn build() -> ReportBuilder {
        ReportBuilder {
            elements: Vec::new(),
            style: Style::default(),
        }
    }

    pub fn write<F: Format>(&self, mut fmt: F) -> Result<F, F::Error> {
        fmt.write(self)?;
        Ok(fmt)
    }

    pub fn eprint(&self) -> io::Result<()> {
        self.write(Ansi(io::stderr().lock())).map(std::mem::drop)
    }
}

pub struct ReportBuilder {
    elements: Vec<Element>,
    style: Style,
}

impl ReportBuilder {
    pub fn with_source_view(mut self, source_view: SourceView) -> Self {
        self.elements.push(Element::SourceView(source_view));
        self
    }

    pub fn finish(self) -> Report {
        Report {
            elements: self.elements,
            style: self.style,
        }
    }
}

impl Styled for ReportBuilder {
    fn style_mut(&mut self) -> &mut Style { &mut self.style }
}

pub struct SourceView {
    labels: Vec<Label>,
    style: Style,
}

impl SourceView {
    pub fn build() -> SourceViewBuilder {
        SourceViewBuilder {
            labels: Vec::new(),
            style: Style::default(),
        }
    }
}

pub struct SourceViewBuilder {
    labels: Vec<Label>,
    style: Style,
}

impl SourceViewBuilder {
    pub fn with_label(mut self, label: Label) -> Self {
        self.labels.push(label);
        self
    }

    pub fn finish(self) -> SourceView {
        SourceView {
            labels: self.labels,
            style: self.style,
        }
    }
}

impl Styled for SourceViewBuilder {
    fn style_mut(&mut self) -> &mut Style { &mut self.style }
}

pub struct Label {}

pub enum Color {
    Red,
    Blue,
}

// Style

#[derive(Default)]
pub struct Style {
    text_color: Option<Color>,
}

pub trait Styled {
    #[doc(hidden)]
    fn style_mut(&mut self) -> &mut Style;

    fn with_text_color(mut self, text_color: Color) -> Self where Self: Sized {
        self.style_mut().text_color = Some(text_color);
        self
    }
}
