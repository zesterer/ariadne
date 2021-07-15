mod source;
mod display;
mod draw;
mod write;

pub use crate::{
    source::{Line, Source, Cache, FileCache, FnCache, sources},
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
    type SourceId: fmt::Debug + Hash + PartialEq + Eq;

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

impl<Id: fmt::Debug + Hash + PartialEq + Eq> Span for (Id, Range<usize>) {
    type SourceId = Id;

    fn source(&self) -> &Self::SourceId { &self.0 }
    fn start(&self) -> usize { self.1.start }
    fn end(&self) -> usize { self.1.end }
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

pub struct Config {
    /// When label lines cross one-another, should there be a gap?
    pub cross_gap: bool,
    /// Whether to minimise gaps between parts of the report.
    pub compact: bool,
    /// Whether arrow heads should be preferred for label lines.
    pub arrows: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cross_gap: true,
            compact: false,
            arrows: true,
        }
    }
}
