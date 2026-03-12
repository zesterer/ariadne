use std::fmt::{self, Debug, Display};

use yansi::Color;

use crate::Config;

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
