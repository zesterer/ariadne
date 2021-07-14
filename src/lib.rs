pub mod source;
pub mod display;
pub mod draw;

pub use crate::{
    source::{Line, Source, Cache},
};

use crate::display::*;
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
    fn contains(&self, offset: usize) -> bool { (self.start()..self.end()).contains(&offset) }
}

impl Span for Range<usize> {
    type SourceId = ();

    fn source(&self) -> &Self::SourceId { &() }
    fn start(&self) -> usize { self.start }
    fn end(&self) -> usize { self.end }
}

pub struct Label<S = Range<usize>> {
    span: S,
    note: Option<String>,
}

impl<S> Label<S> {
    pub fn new(span: S) -> Self {
        Self { span, note: None }
    }

    pub fn with_note<N: ToString>(mut self, note: N) -> Self {
        self.note = Some(note.to_string());
        self
    }
}

pub struct Report<S = Range<usize>> {
    kind: ReportKind,
    code: Option<u32>,
    msg: Option<String>,
    primary: Label<S>,
    secondary: Vec<Label<S>>,
}

impl<S: Span> Report<S> {
    pub fn build(kind: ReportKind, primary: Label<S>) -> ReportBuilder<S> {
        ReportBuilder {
            kind,
            code: None,
            msg: None,
            primary,
            secondary: Vec::new(),
        }
    }

    pub fn print<C: Cache<S::SourceId>>(&self, cache: C) -> io::Result<()> {
        self.write(cache, io::stdout())
    }

    fn get_source_groups(&self) -> Vec<(&S::SourceId, Range<usize>, Vec<&Label<S>>)> {
        let mut groups = Vec::new();
        for label in std::iter::once(&self.primary).chain(self.secondary.iter()) {
            if let Some((_, span, group)) = groups
                .iter_mut()
                .find(|(src, _, _): &&mut (&S::SourceId, Range<usize>, Vec<&Label<S>>)| *src == label.span.source())
            {
                span.start = span.start.min(label.span.start());
                span.end = span.end.max(label.span.end());
                group.push(label);
            } else {
                groups.push((label.span.source(), label.span.start()..label.span.end(), vec![label]));
            }
        }
        groups
    }

    pub fn write<C: Cache<S::SourceId>, W: Write>(&self, mut cache: C, mut w: W) -> io::Result<()> {
        let draw = draw::Characters::unicode();

        // --- Header ---

        let code = self.code.map(|c| format!("[{}{:02}] ", self.kind.letter(), c));
        writeln!(w, "{}{}: {}", Show(code), self.kind, Show(self.msg.as_ref()))?;

        // --- Source sections ---

        let groups = self.get_source_groups();
        let groups_len = groups.len();
        for (i, (src_id, span, labels)) in groups.into_iter().enumerate() {
            let src_name = cache
                .display(src_id)
                .map(|d| d.to_string())
                .unwrap_or_else(|| "<unknown>".to_string());

            let src = match cache.fetch(src_id) {
                Ok(src) => src,
                Err(e) => {
                    eprintln!("<unable to fetch source {}>", src_name);
                    continue;
                },
            };

            // File name
            let line_ref = if src_id == self.primary.span.source() {
                let (line_no, col_no) = src
                    .get_offset_line(self.primary.span.start())
                    .map(|(_, idx, col)| (format!("{}", idx + 1), format!("{}", col + 1)))
                    .unwrap_or_else(|| ('?'.to_string(), '?'.to_string()));
                Some(format!(":{}:{}", line_no, col_no))
            } else {
                None
            };
            writeln!(w, "    {}{}{}{}{}{}", draw.ltop, draw.hbar, if i == 0 { draw.lbox } else { draw.lcross }, src_name, Show(line_ref), draw.rbox)?;
            writeln!(w, "    {}", draw.vbar)?;

            let line_range = src.get_line_range(&span);

            for idx in line_range {
                let line = src.line(idx).unwrap();

                // for l in &labels {
                //     println!("{}..{}", l.span.start(), l.span.end());
                // }

                if !labels
                    .iter()
                    .any(|l| l.span.start() >= line.span().start && l.span.end() <= line.span().end)
                {
                    continue;
                }

                // Margin
                let line_no = format!("{:>3}", idx + 1);
                write!(w, "{} {} ", line_no, draw.vbar)?;

                // Line
                for c in line.chars() {
                    write!(w, "{}", c)?;
                }
                write!(w, "\n")?;

                // Underline margin
                write!(w, "    {} ", draw.vbar_break)?;

                // Underline
                for (i, c) in line.chars().enumerate() {
                    let underline = labels.iter().any(|l| (l.span.start()..l.span.end()).contains(&(line.offset() + i)));
                    write!(w, "{}", if underline { '^' } else { ' ' })?;
                }
                // if let Some(note) = &label.note {
                    write!(w, "{}{}{} {}", draw.hbar, draw.hbar, draw.hbar, /*note*/ "Foo");
                // }
                write!(w, "\n")?;
            }

            if i + 1 == groups_len {
                writeln!(w, "{}{}{}{}{}", draw.hbar, draw.hbar, draw.hbar, draw.hbar, draw.rbot)?;
            } else {
                writeln!(w, "    {}", draw.vbar)?;
            }
        }
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReportKind {
    Error,
    Warning,
}

impl ReportKind {
    fn letter(&self) -> char {
        match self {
            Self::Error => 'E',
            Self::Warning => 'W',
        }
    }
}

impl fmt::Display for ReportKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub struct ReportBuilder<S> {
    kind: ReportKind,
    code: Option<u32>,
    msg: Option<String>,
    primary: Label<S>,
    secondary: Vec<Label<S>>,
}

impl<S> ReportBuilder<S> {
    pub fn with_code(mut self, code: u32) -> Self {
        self.code = Some(code);
        self
    }

    pub fn with_message<M: ToString>(mut self, msg: M) -> Self {
        self.msg = Some(msg.to_string());
        self
    }

    pub fn with_label(mut self, label: Label<S>) -> Self {
        self.secondary.push(label);
        self
    }

    pub fn finish(self) -> Report<S> {
        Report {
            kind: self.kind,
            code: self.code,
            msg: self.msg,
            primary: self.primary,
            secondary: self.secondary,
        }
    }
}
