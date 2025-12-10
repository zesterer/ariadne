#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
// Silly diagnostic anyway
#![allow(clippy::unnecessary_map_or)]

mod display;
mod draw;
mod source;
mod write;

pub use crate::{
    draw::{ColorGenerator, Fmt},
    source::{sources, Cache, FileCache, FnCache, Line, Source},
};
pub use yansi::Color;

#[cfg(any(feature = "concolor", doc))]
pub use crate::draw::StdoutFmt;

use crate::display::*;
use std::{
    cmp::{Eq, PartialEq},
    fmt::{self, Debug, Display},
    hash::Hash,
    io::{self, Write},
    ops::Range,
    ops::RangeInclusive,
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
    fn len(&self) -> usize {
        self.end().saturating_sub(self.start())
    }

    /// Returns `true` if this span has length zero.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Determine whether the span contains the given offset.
    fn contains(&self, offset: usize) -> bool {
        (self.start()..self.end()).contains(&offset)
    }
}

impl Span for Range<usize> {
    type SourceId = ();

    fn source(&self) -> &Self::SourceId {
        &()
    }
    fn start(&self) -> usize {
        self.start
    }
    fn end(&self) -> usize {
        self.end
    }
}

impl<Id: fmt::Debug + Hash + PartialEq + Eq + ToOwned> Span for (Id, Range<usize>) {
    type SourceId = Id;

    fn source(&self) -> &Self::SourceId {
        &self.0
    }
    fn start(&self) -> usize {
        self.1.start
    }
    fn end(&self) -> usize {
        self.1.end
    }
}

impl Span for RangeInclusive<usize> {
    type SourceId = ();

    fn source(&self) -> &Self::SourceId {
        &()
    }
    fn start(&self) -> usize {
        *self.start()
    }
    fn end(&self) -> usize {
        *self.end() + 1
    }
}

impl<Id: fmt::Debug + Hash + PartialEq + Eq + ToOwned> Span for (Id, RangeInclusive<usize>) {
    type SourceId = Id;

    fn source(&self) -> &Self::SourceId {
        &self.0
    }
    fn start(&self) -> usize {
        *self.1.start()
    }
    fn end(&self) -> usize {
        *self.1.end() + 1
    }
}

/// A type that represents the way a label should be displayed.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct LabelDisplay {
    msg: Option<String>,
    raw_color: Option<Color>,
    order: i32,
    priority: i32,
}

impl LabelDisplay {
    fn color(&self, config: &Config) -> Option<Color> {
        self.raw_color.filter(|_| config.color)
    }
}

/// A type that represents a labelled section of source code.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Label<S = Range<usize>> {
    span: S,
    display_info: LabelDisplay,
}

impl<S: Span> Label<S> {
    /// Create a new [`Label`].
    /// If the span is specified as a `Range<usize>` the numbers have to be zero-indexed character offsets.
    ///
    /// # Panics
    ///
    /// Panics if the given span is backwards.
    pub fn new(span: S) -> Self {
        assert!(span.start() <= span.end(), "Label start is after its end");

        Self {
            span,
            display_info: LabelDisplay {
                msg: None,
                raw_color: None,
                order: 0,
                priority: 0,
            },
        }
    }

    /// Give this label a message.
    pub fn with_message<M: ToString>(mut self, msg: M) -> Self {
        self.display_info.msg = Some(msg.to_string());
        self
    }

    /// Give this label a highlight colour.
    pub fn with_color(mut self, color: Color) -> Self {
        self.display_info.raw_color = Some(color);
        self
    }

    /// Specify the order of this label relative to other labels.
    ///
    /// Lower values correspond to this label having an earlier order. Labels with the same order will be arranged
    /// in whatever order best suits their spans and layout constraints.
    ///
    /// If unspecified, labels default to an order of `0`.
    ///
    /// Label order is respected across files. If labels that appear earlier in a file have an order that requires
    /// them to appear later, the resulting diagnostic may result in multiple instances of the same file being
    /// displayed.
    pub fn with_order(mut self, order: i32) -> Self {
        self.display_info.order = order;
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
        self.display_info.priority = priority;
        self
    }
}

/// A type representing a diagnostic that is ready to be written to output.
pub struct Report<S: Span = Range<usize>, K: ReportStyle = ReportKind> {
    kind: K,
    code: Option<String>,
    msg: Option<String>,
    notes: Vec<String>,
    help: Vec<String>,
    span: S,
    labels: Vec<Label<S>>,
    config: Config,
}

impl<S: Span, K: ReportStyle> Report<S, K> {
    /// Begin building a new [`Report`].
    ///
    /// The span is the primary location at which the error should be reported.
    pub fn build(kind: K, span: S) -> ReportBuilder<S, K> {
        ReportBuilder {
            kind,
            code: None,
            msg: None,
            notes: vec![],
            help: vec![],
            span,
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

impl<S: Span, K: ReportStyle> fmt::Debug for Report<S, K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Report")
            .field("kind", &self.kind)
            .field("code", &self.code)
            .field("msg", &self.msg)
            .field("notes", &self.notes)
            .field("help", &self.help)
            .field("config", &self.config)
            .finish()
    }
}

/// A triat for coloring messages, requires Display for naming the Report error/warning/note etc
pub trait ReportStyle: Display + Debug {
    /// return the color (if any) to use for the Report
    fn get_color(&self, _config: &Config) -> Option<Color> {
        None
    }
}

impl ReportStyle for String {
    fn get_color(&self, _: &Config) -> Option<Color> {
        None
    }
}

impl ReportStyle for &str {
    fn get_color(&self, _: &Config) -> Option<Color> {
        None
    }
}

/// an implementation of `ReportStyle` intended for genral use
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BasicStyle<Str: Display + Debug = String> {
    /// the name to display in labels
    pub name: Str,
    /// color to use
    pub color: Color,
}

impl<Str: Display + Debug> Display for BasicStyle<Str> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl<Str: Display + Debug> ReportStyle for BasicStyle<Str> {
    fn get_color(&self, config: &Config) -> Option<Color> {
        Some(self.color).filter(|_| config.color)
    }
}

/**
 * A Type for basic error handeling in all common cases.
 */
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReportKind {
    /// The report is an error and indicates a critical problem that prevents the program performing the requested
    /// action.
    Error,
    /// The report is a warning and indicates a likely problem, but not to the extent that the requested action cannot
    /// be performed.
    Warning,
    /// The report is advice to the user about a potential anti-pattern of other benign issues.
    Advice,

    /// The report is of a kind not built into Ariadne.
    Custom(&'static str, Color),
}

impl fmt::Display for ReportKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[allow(deprecated)]
        match self {
            ReportKind::Error => write!(f, "Error"),
            ReportKind::Warning => write!(f, "Warning"),
            ReportKind::Advice => write!(f, "Advice"),
            ReportKind::Custom(s, _) => write!(f, "{s}"),
        }
    }
}

impl ReportStyle for ReportKind {
    fn get_color(&self, config: &Config) -> Option<Color> {
        #[allow(deprecated)]
        match self {
            ReportKind::Error => config.error_color(),
            ReportKind::Warning => config.warning_color(),
            ReportKind::Advice => config.advice_color(),
            ReportKind::Custom(_, color) => Some(*color).filter(|_| config.color),
        }
    }
}

/// A type used to build a [`Report`].
pub struct ReportBuilder<S: Span, K: ReportStyle> {
    kind: K,
    code: Option<String>,
    msg: Option<String>,
    notes: Vec<String>,
    help: Vec<String>,
    span: S,
    labels: Vec<Label<S>>,
    config: Config,
}

impl<S: Span, K: ReportStyle> ReportBuilder<S, K> {
    /// Give this report a numerical code that may be used to more precisely look up the error in documentation.
    pub fn with_code<C: fmt::Display>(mut self, code: C) -> Self {
        self.code = Some(format!("{code:02}"));
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
        self.notes = vec![note.to_string()];
    }

    /// Adds a note to this report.
    pub fn add_note<N: ToString>(&mut self, note: N) {
        self.notes.push(note.to_string());
    }

    /// Removes all notes in this report.
    pub fn with_notes<N: IntoIterator<Item = impl ToString>>(&mut self, notes: N) {
        for note in notes {
            self.add_note(note)
        }
    }

    /// Set the note of this report.
    pub fn with_note<N: ToString>(mut self, note: N) -> Self {
        self.add_note(note);
        self
    }

    /// Set the help message of this report.
    pub fn set_help<N: ToString>(&mut self, note: N) {
        self.help = vec![note.to_string()];
    }

    /// Add a help message to this report.
    pub fn add_help<N: ToString>(&mut self, note: N) {
        self.help.push(note.to_string());
    }

    /// Set the help messages of this report.
    pub fn with_helps<N: IntoIterator<Item = impl ToString>>(&mut self, helps: N) {
        for help in helps {
            self.add_help(help)
        }
    }

    /// Set the help message of this report.
    pub fn with_help<N: ToString>(mut self, note: N) -> Self {
        self.add_help(note);
        self
    }

    /// Add a label to the report.
    pub fn add_label(&mut self, label: Label<S>) {
        self.add_labels(std::iter::once(label));
    }

    /// Add multiple labels to the report.
    pub fn add_labels<L: IntoIterator<Item = Label<S>>>(&mut self, labels: L) {
        self.labels.extend(labels);
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
    pub fn finish(self) -> Report<S, K> {
        Report {
            kind: self.kind,
            code: self.code,
            msg: self.msg,
            notes: self.notes,
            help: self.help,
            span: self.span,
            labels: self.labels,
            config: self.config,
        }
    }
}

impl<S: Span, K: ReportStyle> fmt::Debug for ReportBuilder<S, K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReportBuilder")
            .field("kind", &self.kind)
            .field("code", &self.code)
            .field("msg", &self.msg)
            .field("notes", &self.notes)
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
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum CharSet {
    /// Unicode characters (an attempt is made to use only commonly-supported characters).
    #[default]
    Unicode,
    /// ASCII-only characters.
    Ascii,
}

/// Possible character sets to use when rendering diagnostics.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum IndexType {
    /// Byte spans. Always results in O(1) lookups
    Byte,
    /// Char based spans. May incur O(n) lookups
    Char,
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
    index_type: IndexType,
    minimise_crossings: bool,
    context_lines: usize,
}

impl Config {
    /// When label lines cross one-another, should there be a gap?
    ///
    /// The alternative to this is to insert crossing characters. However, these interact poorly with label colours.
    ///
    /// If unspecified, this defaults to [`false`].
    pub const fn with_cross_gap(mut self, cross_gap: bool) -> Self {
        self.cross_gap = cross_gap;
        self
    }
    /// Where should inline labels attach to their spans?
    ///
    /// If unspecified, this defaults to [`LabelAttach::Middle`].
    pub const fn with_label_attach(mut self, label_attach: LabelAttach) -> Self {
        self.label_attach = label_attach;
        self
    }
    /// Should the report remove gaps to minimise used space?
    ///
    /// If unspecified, this defaults to [`false`].
    pub const fn with_compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }
    /// Should underlines be used for label span where possible?
    ///
    /// If unspecified, this defaults to [`true`].
    pub const fn with_underlines(mut self, underlines: bool) -> Self {
        self.underlines = underlines;
        self
    }
    /// Should arrows be used to point to the bounds of multi-line spans?
    ///
    /// If unspecified, this defaults to [`true`].
    pub const fn with_multiline_arrows(mut self, multiline_arrows: bool) -> Self {
        self.multiline_arrows = multiline_arrows;
        self
    }
    /// Should colored output should be enabled?
    ///
    /// If unspecified, this defaults to [`true`].
    pub const fn with_color(mut self, color: bool) -> Self {
        self.color = color;
        self
    }
    /// How many characters width should tab characters be?
    ///
    /// If unspecified, this defaults to `4`.
    pub const fn with_tab_width(mut self, tab_width: usize) -> Self {
        self.tab_width = tab_width;
        self
    }
    /// What character set should be used to display dynamic elements such as boxes and arrows?
    ///
    /// If unspecified, this defaults to [`CharSet::Unicode`].
    pub const fn with_char_set(mut self, char_set: CharSet) -> Self {
        self.char_set = char_set;
        self
    }
    /// Should this report use byte spans instead of char spans?
    ///
    /// If unspecified, this defaults to [`false`].
    pub const fn with_index_type(mut self, index_type: IndexType) -> Self {
        self.index_type = index_type;
        self
    }
    /// Should label crossings be minimised rather than prioritising label ordering?
    ///
    /// Note that label order overridden via [`Label::with_order`] will still be respected.
    ///
    /// If unspecified, this defaults to [`false`].
    pub const fn with_minimise_crossings(mut self, minimise_crossings: bool) -> Self {
        self.minimise_crossings = minimise_crossings;
        self
    }
    /// How many lines of extra context should be displayed around the start and end of labels?
    ///
    /// If unspecified, this defaults to `0`.
    pub const fn with_context_lines(mut self, context_lines: usize) -> Self {
        self.context_lines = context_lines;
        self
    }

    fn error_color(&self) -> Option<Color> {
        Some(Color::Red).filter(|_| self.color)
    }
    fn warning_color(&self) -> Option<Color> {
        Some(Color::Yellow).filter(|_| self.color)
    }
    fn advice_color(&self) -> Option<Color> {
        Some(Color::Fixed(147)).filter(|_| self.color)
    }
    fn margin_color(&self) -> Option<Color> {
        Some(Color::Fixed(246)).filter(|_| self.color)
    }
    fn skipped_margin_color(&self) -> Option<Color> {
        Some(Color::Fixed(240)).filter(|_| self.color)
    }
    fn unimportant_color(&self) -> Option<Color> {
        Some(Color::Fixed(249)).filter(|_| self.color)
    }
    fn note_color(&self) -> Option<Color> {
        Some(Color::Fixed(115)).filter(|_| self.color)
    }

    // Find the character that should be drawn and the number of times it should be drawn for each char
    fn char_width(&self, c: char, col: usize) -> (char, usize) {
        match c {
            '\t' => {
                // Find the column that the tab should end at
                let tab_end = (col / self.tab_width + 1) * self.tab_width;
                (' ', tab_end - col)
            }
            c if c.is_whitespace() => (' ', 1),
            _ => (c, c.width().unwrap_or(1)),
        }
    }

    /// Create a new, default config.
    pub const fn new() -> Self {
        Self {
            cross_gap: true,
            label_attach: LabelAttach::Middle,
            compact: false,
            underlines: true,
            multiline_arrows: true,
            color: true,
            tab_width: 4,
            char_set: CharSet::Unicode,
            index_type: IndexType::Char,
            minimise_crossings: false,
            context_lines: 0,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

#[test]
#[should_panic]
#[allow(clippy::reversed_empty_ranges)]
fn backwards_label_should_panic() {
    Label::new(1..0);
}
