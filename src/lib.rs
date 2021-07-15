mod source;
mod display;
mod draw;
mod write;

pub use crate::{
    source::{Line, Source, Cache, FileCache, FnCache, sources},
    draw::Fmt,
};
pub use yansi::Color;

use crate::display::*;
use std::{
    ops::Range,
    io::{self, Write},
    hash::Hash,
    cmp::{PartialEq, Eq},
    fmt,
};

pub trait Span {
    type SourceId: fmt::Debug + Hash + PartialEq + Eq;

    fn source(&self) -> &Self::SourceId;
    fn start(&self) -> usize;
    fn end(&self) -> usize;
    fn len(&self) -> usize { self.end().saturating_sub(self.start()) }
    fn contains(&self, offset: usize) -> bool { (self.start()..self.end()).contains(&offset) }
}

impl Span for Range<usize> {
    type SourceId = ();

    fn source(&self) -> &Self::SourceId { &() }
    fn start(&self) -> usize { self.start }
    fn end(&self) -> usize { self.end }
}

impl<Id: fmt::Debug + Hash + PartialEq + Eq> Span for (Id, Range<usize>) {
    type SourceId = Id;

    fn source(&self) -> &Self::SourceId { &self.0 }
    fn start(&self) -> usize { self.1.start }
    fn end(&self) -> usize { self.1.end }
}

pub struct Label<S = Range<usize>> {
    span: S,
    note: Option<String>,
    color: Option<Color>,
}

impl<S> Label<S> {
    pub fn new(span: S) -> Self {
        Self { span, note: None, color: None }
    }

    pub fn with_note<N: ToString>(mut self, note: N) -> Self {
        self.note = Some(note.to_string());
        self
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

pub struct Report<S: Span = Range<usize>> {
    kind: ReportKind,
    code: Option<u32>,
    msg: Option<String>,
    location: (S::SourceId, usize),
    labels: Vec<Label<S>>,
    config: Config,
}

impl<S: Span> Report<S> {
    pub fn build(kind: ReportKind, src_id: S::SourceId, offset: usize) -> ReportBuilder<S> {
        ReportBuilder {
            kind,
            code: None,
            msg: None,
            location: (src_id, offset),
            labels: Vec::new(),
            config: Config::default(),
        }
    }

    pub fn print<C: Cache<S::SourceId>>(&self, cache: C) -> io::Result<()> {
        self.write(cache, io::stdout())
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

pub struct ReportBuilder<S: Span> {
    kind: ReportKind,
    code: Option<u32>,
    msg: Option<String>,
    location: (S::SourceId, usize),
    labels: Vec<Label<S>>,
    config: Config,
}

impl<S: Span> ReportBuilder<S> {
    pub fn with_code(mut self, code: u32) -> Self {
        self.code = Some(code);
        self
    }

    pub fn with_message<M: ToString>(mut self, msg: M) -> Self {
        self.msg = Some(msg.to_string());
        self
    }

    pub fn with_label(mut self, label: Label<S>) -> Self {
        self.labels.push(label);
        self
    }

    pub fn with_config(mut self, config: Config) -> Self {
        self.config = config;
        self
    }

    pub fn finish(self) -> Report<S> {
        Report {
            kind: self.kind,
            code: self.code,
            msg: self.msg,
            location: self.location,
            labels: self.labels,
            config: self.config,
        }
    }
}

pub enum LabelPoint {
    Start,
    Mid,
    End,
}

pub struct Config {
    cross_gap: bool,
    label_point: LabelPoint,
    compact: bool,
    underlines: bool,
    color: bool,
}

impl Config {
    /// When label lines cross one-another, should there be a gap?
    pub fn with_cross_gap(mut self, cross_gap: bool) -> Self { self.cross_gap = cross_gap; self }
    /// Where should inline labels attach to their spans?
    pub fn with_label_point(mut self, label_point: LabelPoint) -> Self { self.label_point = label_point; self }
    /// Whether to minimise gaps between parts of the report.
    pub fn with_compact(mut self, compact: bool) -> Self { self.compact = compact; self }
    /// Whether underlines should be used for label span where possible.
    pub fn with_underlines(mut self, underlines: bool) -> Self { self.underlines = underlines; self }
    /// Whether colored output should be enabled.
    pub fn with_color(mut self, color: bool) -> Self { self.color = color; self }

    fn err_color(&self) -> Option<Color> {
        Some(Color::Red).filter(|_| self.color)
    }

    fn margin_color(&self) -> Option<Color> {
        Some(Color::Fixed(246)).filter(|_| self.color)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cross_gap: true,
            label_point: LabelPoint::Mid,
            compact: false,
            underlines: true,
            color: true,
        }
    }
}
