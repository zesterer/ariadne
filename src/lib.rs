#![doc = include_str!("../README.md")]
#![deny(missing_docs)]

mod source;
mod display;
mod draw;
mod write;

pub use crate::{
    source::{Line, Source, Cache, FileCache, FnCache, sources},
    draw::{Fmt, ColorGenerator},
};
pub use yansi::Color;

#[cfg(any(feature = "concolor", doc))]
pub use crate::draw::StdoutFmt;

use crate::display::*;
use std::{
    ops::Range,
    io::{self, Write},
    hash::Hash,
    cmp::{PartialEq, Eq},
    fmt,
};
use unicode_width::UnicodeWidthChar;

/// A trait implemented by spans within a character-based source.
pub trait Span {
    /// The identifier used to uniquely refer to a source. In most cases, this is the fully-qualified path of the file.
    type SourceId: PartialEq + ToOwned + ?Sized;

    /// Get the identifier of the source that this span refers to.
    fn source(&self) -> &Self::SourceId;

    /// Get the start offset of this span.
    ///
    /// Offsets are zero-indexed character offsets from the beginning of the source.
    fn start(&self) -> usize;

    /// Get the (exclusive) end offset of this span.
    ///
    /// The end offset should *always* be greater than or equal to the start offset as given by [`Span::start`].
    ///
    /// Offsets are zero-indexed character offsets from the beginning of the source.
    fn end(&self) -> usize;

    /// Get the length of this span (difference between the start of the span and the end of the span).
    fn len(&self) -> usize { self.end().saturating_sub(self.start()) }

    /// Determine whether the span contains the given offset.
    fn contains(&self, offset: usize) -> bool { (self.start()..self.end()).contains(&offset) }
}

impl Span for Range<usize> {
    type SourceId = ();

    fn source(&self) -> &Self::SourceId { &() }
    fn start(&self) -> usize { self.start }
    fn end(&self) -> usize { self.end }
}

impl<Id: fmt::Debug + Hash + PartialEq + Eq + ToOwned> Span for (Id, Range<usize>) {
    type SourceId = Id;

    fn source(&self) -> &Self::SourceId { &self.0 }
    fn start(&self) -> usize { self.1.start }
    fn end(&self) -> usize { self.1.end }
}

/// A type that represents a labelled section of source code.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Label<S = Range<usize>> {
    span: S,
    msg: Option<String>,
    color: Option<Color>,
    order: i32,
    priority: i32,
}

impl<S> Label<S> {
    /// Create a new [`Label`].
    pub fn new(span: S) -> Self {
        Self {
            span,
            msg: None,
            color: None,
            order: 0,
            priority: 0,
        }
    }

    /// Give this label a message.
    pub fn with_message<M: ToString>(mut self, msg: M) -> Self {
        self.msg = Some(msg.to_string());
        self
    }

    /// Give this label a highlight colour.
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Specify the order of this label relative to other labels.
    ///
    /// Lower values correspond to this label having an earlier order.
    ///
    /// If unspecified, labels default to an order of `0`.
    ///
    /// When labels are displayed after a line the crate needs to decide which labels should be displayed first. By
    /// Default, the orders labels based on where their associated line meets the text (see [`LabelAttach`]).
    /// Additionally, multi-line labels are ordered before inline labels. You can use this function to override this
    /// behaviour.
    pub fn with_order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    /// Specify the priority of this label relative to other labels.
    ///
    /// Higher values correspond to this label having a higher priority.
    ///
    /// If unspecified, labels default to a priority of `0`.
    ///
    /// Label spans can overlap. When this happens, the crate needs to decide which labels to prioritise for various
    /// purposes such as highlighting. By default, spans with a smaller length get a higher priority. You can use this
    /// function to override this behaviour.
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}

/// A type representing a diagnostic that is ready to be written to output.
pub struct Report<'a, S: Span = Range<usize>> {
    kind: ReportKind<'a>,
    code: Option<String>,
    msg: Option<String>,
    note: Option<String>,
    help: Option<String>,
    location: (<S::SourceId as ToOwned>::Owned, usize),
    labels: Vec<Label<S>>,
    config: Config,
}

impl<S: Span> Report<'_, S> {
    /// Begin building a new [`Report`].
    pub fn build<Id: Into<<S::SourceId as ToOwned>::Owned>>(kind: ReportKind, src_id: Id, offset: usize) -> ReportBuilder<S> {
        ReportBuilder {
            kind,
            code: None,
            msg: None,
            note: None,
            help: None,
            location: (src_id.into(), offset),
            labels: Vec::new(),
            config: Config::default(),
        }
    }

    /// Write this diagnostic out to `stderr`.
    pub fn eprint<C: Cache<S::SourceId>>(&self, cache: C) -> io::Result<()> {
        self.write(cache, io::stderr())
    }

    /// Write this diagnostic out to `stdout`.
    ///
    /// In most cases, [`Report::eprint`] is the
    /// ['more correct'](https://en.wikipedia.org/wiki/Standard_streams#Standard_error_(stderr)) function to use.
    pub fn print<C: Cache<S::SourceId>>(&self, cache: C) -> io::Result<()> {
        self.write_for_stdout(cache, io::stdout())
    }
}

impl<'a, S: Span> fmt::Debug for Report<'a, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Report")
            .field("kind", &self.kind)
            .field("code", &self.code)
            .field("msg", &self.msg)
            .field("note", &self.note)
            .field("help", &self.help)
            .field("config", &self.config)
            .finish()
    }
}
/// A type that defines the kind of report being produced.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReportKind<'a> {
    /// The report is an error and indicates a critical problem that prevents the program performing the requested
    /// action.
    Error,
    /// The report is a warning and indicates a likely problem, but not to the extent that the requested action cannot
    /// be performed.
    Warning,
    /// The report is advice to the user about a potential anti-pattern of other benign issues.
    Advice,
    /// The report is of a kind not built into Ariadne.
    Custom(&'a str, Color),
}

impl fmt::Display for ReportKind<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ReportKind::Error => write!(f, "Error"),
            ReportKind::Warning => write!(f, "Warning"),
            ReportKind::Advice => write!(f, "Advice"),
            ReportKind::Custom(s, _) => write!(f, "{}", s),
        }
    }
}

/// A type used to build a [`Report`].
pub struct ReportBuilder<'a, S: Span> {
    kind: ReportKind<'a>,
    code: Option<String>,
    msg: Option<String>,
    note: Option<String>,
    help: Option<String>,
    location: (<S::SourceId as ToOwned>::Owned, usize),
    labels: Vec<Label<S>>,
    config: Config,
}

impl<'a, S: Span> ReportBuilder<'a, S> {
    /// Give this report a numerical code that may be used to more precisely look up the error in documentation.
    pub fn with_code<C: fmt::Display>(mut self, code: C) -> Self {
        self.code = Some(format!("{:02}", code));
        self
    }

    /// Set the message of this report.
    pub fn set_message<M: ToString>(&mut self, msg: M) {
        self.msg = Some(msg.to_string());
    }

    /// Add a message to this report.
    pub fn with_message<M: ToString>(mut self, msg: M) -> Self {
        self.msg = Some(msg.to_string());
        self
    }

    /// Set the note of this report.
    pub fn set_note<N: ToString>(&mut self, note: N) {
        self.note = Some(note.to_string());
    }

    /// Set the note of this report.
    pub fn with_note<N: ToString>(mut self, note: N) -> Self {
        self.set_note(note);
        self
    }

    /// Set the help message of this report.
    pub fn set_help<N: ToString>(&mut self, note: N) {
        self.help = Some(note.to_string());
    }

    /// Set the help message of this report.
    pub fn with_help<N: ToString>(mut self, note: N) -> Self {
        self.set_help(note);
        self
    }

    /// Add a label to the report.
    pub fn add_label(&mut self, label: Label<S>) {
        self.add_labels(std::iter::once(label));
    }

    /// Add multiple labels to the report.
    pub fn add_labels<L: IntoIterator<Item = Label<S>>>(&mut self, labels: L) {
        let config = &self.config; // This would not be necessary in Rust 2021 edition
        self.labels.extend(labels.into_iter().map(|mut label| { label.color = config.filter_color(label.color); label }));
    }

    /// Add a label to the report.
    pub fn with_label(mut self, label: Label<S>) -> Self {
        self.add_label(label);
        self
    }

    /// Add multiple labels to the report.
    pub fn with_labels<L: IntoIterator<Item = Label<S>>>(mut self, labels: L) -> Self {
        self.add_labels(labels);
        self
    }

    /// Use the given [`Config`] to determine diagnostic attributes.
    pub fn with_config(mut self, config: Config) -> Self {
        self.config = config;
        self
    }

    /// Finish building the [`Report`].
    pub fn finish(self) -> Report<'a, S> {
        Report {
            kind: self.kind,
            code: self.code,
            msg: self.msg,
            note: self.note,
            help: self.help,
            location: self.location,
            labels: self.labels,
            config: self.config,
        }
    }
}

impl<'a, S: Span> fmt::Debug for ReportBuilder<'a, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReportBuilder")
            .field("kind", &self.kind)
            .field("code", &self.code)
            .field("msg", &self.msg)
            .field("note", &self.note)
            .field("help", &self.help)
            .field("config", &self.config)
            .finish()
    }
}

/// The attachment point of inline label arrows
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LabelAttach {
    /// Arrows should attach to the start of the label span.
    Start,
    /// Arrows should attach to the middle of the label span (or as close to the middle as we can get).
    Middle,
    /// Arrows should attach to the end of the label span.
    End,
}

/// Possible character sets to use when rendering diagnostics.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CharSet {
    /// Unicode characters (an attempt is made to use only commonly-supported characters).
    Unicode,
    /// ASCII-only characters.
    Ascii,
}

/// A type used to configure a report
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Config {
    cross_gap: bool,
    label_attach: LabelAttach,
    compact: bool,
    underlines: bool,
    multiline_arrows: bool,
    color: bool,
    tab_width: usize,
    char_set: CharSet,
}

impl Config {
    /// When label lines cross one-another, should there be a gap?
    ///
    /// The alternative to this is to insert crossing characters. However, these interact poorly with label colours.
    ///
    /// If unspecified, this defaults to [`false`].
    pub fn with_cross_gap(mut self, cross_gap: bool) -> Self { self.cross_gap = cross_gap; self }
    /// Where should inline labels attach to their spans?
    ///
    /// If unspecified, this defaults to [`LabelAttach::Middle`].
    pub fn with_label_attach(mut self, label_attach: LabelAttach) -> Self { self.label_attach = label_attach; self }
    /// Should the report remove gaps to minimise used space?
    ///
    /// If unspecified, this defaults to [`false`].
    pub fn with_compact(mut self, compact: bool) -> Self { self.compact = compact; self }
    /// Should underlines be used for label span where possible?
    ///
    /// If unspecified, this defaults to [`true`].
    pub fn with_underlines(mut self, underlines: bool) -> Self { self.underlines = underlines; self }
    /// Should arrows be used to point to the bounds of multi-line spans?
    ///
    /// If unspecified, this defaults to [`true`].
    pub fn with_multiline_arrows(mut self, multiline_arrows: bool) -> Self { self.multiline_arrows = multiline_arrows; self }
    /// Should colored output should be enabled?
    ///
    /// If unspecified, this defaults to [`true`].
    pub fn with_color(mut self, color: bool) -> Self { self.color = color; self }
    /// How many characters width should tab characters be?
    ///
    /// If unspecified, this defaults to `4`.
    pub fn with_tab_width(mut self, tab_width: usize) -> Self { self.tab_width = tab_width; self }
    /// What character set should be used to display dynamic elements such as boxes and arrows?
    ///
    /// If unspecified, this defaults to [`CharSet::Unicode`].
    pub fn with_char_set(mut self, char_set: CharSet) -> Self { self.char_set = char_set; self }

    fn error_color(&self) -> Option<Color> { Some(Color::Red).filter(|_| self.color) }
    fn warning_color(&self) -> Option<Color> { Some(Color::Yellow).filter(|_| self.color) }
    fn advice_color(&self) -> Option<Color> { Some(Color::Fixed(147)).filter(|_| self.color) }
    fn margin_color(&self) -> Option<Color> { Some(Color::Fixed(246)).filter(|_| self.color) }
    fn skipped_margin_color(&self) -> Option<Color> { Some(Color::Fixed(240)).filter(|_| self.color) }
    fn unimportant_color(&self) -> Option<Color> { Some(Color::Fixed(249)).filter(|_| self.color) }
    fn note_color(&self) -> Option<Color> { Some(Color::Fixed(115)).filter(|_| self.color) }
    fn filter_color(&self, color: Option<Color>) -> Option<Color> { color.filter(|_| self.color) }

    // Find the character that should be drawn and the number of times it should be drawn for each char
    fn char_width(&self, c: char, col: usize) -> (char, usize) {
        match c {
            '\t' => {
                // Find the column that the tab should end at
                let tab_end = (col / self.tab_width + 1) * self.tab_width;
                (' ',  tab_end - col)
            },
            c if c.is_whitespace() => (' ', 1),
            _ => (c, c.width().unwrap_or(1)),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cross_gap: true,
            label_attach: LabelAttach::Middle,
            compact: false,
            underlines: true,
            multiline_arrows: true,
            color: true,
            tab_width: 4,
            char_set: CharSet::Unicode,
        }
    }
}
