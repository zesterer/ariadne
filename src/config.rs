use unicode_width::UnicodeWidthChar;
use yansi::Color;

use crate::LabelAttach;

/// A type used to configure a report
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub(crate) cross_gap: bool,
    pub(crate) label_attach: LabelAttach,
    pub(crate) compact: bool,
    pub(crate) underlines: bool,
    pub(crate) multiline_arrows: bool,
    pub(crate) color: bool,
    pub(crate) tab_width: usize,
    pub(crate) char_set: CharSet,
    pub(crate) index_type: IndexType,
    pub(crate) minimise_crossings: bool,
    pub(crate) context_lines: usize,
    pub(crate) ansi_mode: AnsiMode,
    pub(crate) enumerate_notes: bool,
    pub(crate) enumerate_helps: bool,
}

impl Config {
    /// When label lines cross one-another, should there be a gap?
    ///
    /// The alternative to this is to insert crossing characters. However, these interact poorly with label colours.
    ///
    /// If unspecified, this defaults to [`true`].
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
    /// If unspecified, this defaults to [`IndexType::Char`].
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
    /// Should ANSI escape code styling be included in the diagnostic after writing?
    ///
    /// If unspecified, this defaults to `AnsiMode::On`.
    pub const fn with_ansi_mode(mut self, ansi_mode: AnsiMode) -> Self {
        self.ansi_mode = ansi_mode;
        self
    }

    /// Should separate notes be numbered?
    ///
    /// If unspecified, this defaults to [`true`]
    pub const fn with_enumerated_notes(mut self, enumerate_notes: bool) -> Self {
        self.enumerate_notes = enumerate_notes;
        self
    }

    /// Should separate helps be numbered?
    ///
    /// If unspecified, this defaults to [`true`]
    pub const fn with_enumerated_helps(mut self, enumerate_helps: bool) -> Self {
        self.enumerate_helps = enumerate_helps;
        self
    }

    pub(crate) fn error_color(&self) -> Option<Color> {
        Some(Color::Red).filter(|_| self.color)
    }
    pub(crate) fn warning_color(&self) -> Option<Color> {
        Some(Color::Yellow).filter(|_| self.color)
    }
    pub(crate) fn advice_color(&self) -> Option<Color> {
        Some(Color::Fixed(147)).filter(|_| self.color)
    }
    pub(crate) fn margin_color(&self) -> Option<Color> {
        Some(Color::Fixed(246)).filter(|_| self.color)
    }
    pub(crate) fn skipped_margin_color(&self) -> Option<Color> {
        Some(Color::Fixed(240)).filter(|_| self.color)
    }
    pub(crate) fn unimportant_color(&self) -> Option<Color> {
        Some(Color::Fixed(249)).filter(|_| self.color)
    }
    pub(crate) fn note_color(&self) -> Option<Color> {
        Some(Color::Fixed(115)).filter(|_| self.color)
    }
    pub(crate) fn filter_color(&self, color: Option<Color>) -> Option<Color> {
        color.filter(|_| self.color)
    }

    // Find the character that should be drawn and the number of times it should be drawn for each char
    pub(crate) fn char_width(&self, c: char, col: usize) -> (char, usize) {
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
            ansi_mode: AnsiMode::On,
            enumerate_notes: true,
            enumerate_helps: true,
        }
    }
}
impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
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

/// Whether rendering of ANSI styling, such as color and font weight, is enabled.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AnsiMode {
    /// ANSI styling is disabled, diagnostics will display without styling.
    Off,
    /// ANSI styling is disabled, diagnostics will have ANSI styling escape codes included.
    On,
}
