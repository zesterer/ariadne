pub mod source;
pub mod draw;

pub use crate::{
    source::{Line, Source, Cache},
};
use std::{
    ops::Range,
    io::{self, Write},
    hash::Hash,
    cmp::{PartialEq, Eq},
    fmt,
};

pub trait Span {
    type SourceId: Hash + PartialEq + Eq;

    fn source(&self) -> &Self::SourceId;
    fn start(&self) -> usize;
    fn end(&self) -> usize;
}

impl Span for Range<usize> {
    type SourceId = ();

    fn source(&self) -> &Self::SourceId { &() }
    fn start(&self) -> usize { self.start }
    fn end(&self) -> usize { self.end }
}

pub struct Label<S = Range<usize>> {
    span: S,
}

impl<S> Label<S> {
    pub fn new(span: S) -> Self {
        Self { span }
    }
}

pub struct Report<S = Range<usize>> {
    primary: Label<S>,
}

impl<S: Span> Report<S> {
    pub fn build(primary: Label<S>) -> ReportBuilder<S> {
        ReportBuilder {
            primary,
        }
    }

    pub fn print<C: Cache<S::SourceId>>(&self, cache: C) -> io::Result<()> {
        self.write(cache, io::stdout())
    }

    pub fn write<C: Cache<S::SourceId>, W: Write>(&self, mut cache: C, mut w: W) -> io::Result<()> {
        let draw = draw::Characters::unicode();

        writeln!(w, "[E03] Error: Incompatible types")?;

        let src_name = cache
            .display(self.primary.span.source())
            .map(|d| d.to_string())
            .unwrap_or_else(|| "<unknown>".to_string());
        if let Ok(primary_src) = cache.fetch(self.primary.span.source()) {
            let (line_no, col_no) = primary_src
                .get_offset_line(self.primary.span.start())
                .map(|(_, idx, col)| (format!("{}", idx + 1), format!("{}", col + 1)))
                .unwrap_or_else(|| ('?'.to_string(), '?'.to_string()));
            writeln!(w, "    {}{}[{}:{}:{}]", draw.topl, draw.hbar, src_name, line_no, col_no)?;
            writeln!(w, "    {}", draw.vbar)?;

            let line_range = primary_src.get_line_range(&self.primary.span);
            for idx in line_range {
                let line = primary_src.line(idx).unwrap();
                let line_no = format!("{:>3}", idx + 1);

                write!(w, "{} {} ", line_no, draw.vbar)?;

                for c in line.chars() {
                    write!(w, "{}", c)?;
                }
                write!(w, "\n")?;
            }

            writeln!(w, "    {}", draw.vbar)?;
            writeln!(w, "{}{}{}{}{}", draw.hbar, draw.hbar, draw.hbar, draw.hbar, draw.botr)?;
        }
        Ok(())
    }
}

pub struct ReportBuilder<S> {
    primary: Label<S>,
}

impl<S> ReportBuilder<S> {
    pub fn finish(self) -> Report<S> {
        Report {
            primary: self.primary,
        }
    }
}
