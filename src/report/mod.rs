use std::ops::Range;

use crate::{
    report::{builder::ReportBuilder, style::ReportStyle},
    *,
};
pub(crate) mod builder;
pub(crate) mod style;
#[cfg(test)]
mod tests;
pub(crate) mod write;
/// A type representing a diagnostic that is ready to be written to output.
#[must_use = "call `.print()` or `.eprint()` to print the report"]
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
            .finish_non_exhaustive()
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
    /// The report is advice to the user about a potential anti-pattern or other benign issues.
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
