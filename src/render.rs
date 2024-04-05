use super::*;
use core::fmt;

pub(crate) trait Target {
    type Output;

    fn render<'a, K, F>(&mut self, d: &Diagnostic<K>, files: F) -> Self::Output
    where
        K: FileId,
        F: Files<'a, K>;
}

#[cfg(feature = "std")]
pub struct IoWriter<W> {
    writer: W,
    cfg: TextConfig,
}

#[cfg(feature = "std")]
impl<W> IoWriter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            cfg: TextConfig::default(),
        }
    }

    pub fn into_inner(self) -> W {
        self.writer
    }
}

#[cfg(feature = "std")]
impl<W> Target for IoWriter<W>
where
    W: std::io::Write,
{
    type Output = std::io::Result<()>;

    fn render<'a, K, F>(&mut self, d: &Diagnostic<K>, files: F) -> Self::Output
    where
        K: FileId,
        F: Files<'a, K>,
    {
        write!(
            self.writer,
            "{}",
            Display {
                d,
                files,
                cfg: &self.cfg
            }
        )
    }
}

pub struct FmtWriter<W> {
    writer: W,
    cfg: TextConfig,
}

impl<W> FmtWriter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            cfg: TextConfig::default(),
        }
    }

    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W> Target for FmtWriter<W>
where
    W: fmt::Write,
{
    type Output = fmt::Result;

    fn render<'a, K, F>(&mut self, d: &Diagnostic<K>, files: F) -> Self::Output
    where
        K: FileId,
        F: Files<'a, K>,
    {
        write!(
            self.writer,
            "{}",
            Display {
                d,
                files,
                cfg: &self.cfg
            }
        )
    }
}

pub struct TextConfig {
    pub(crate) chars: CharacterSet,
}

impl Default for TextConfig {
    fn default() -> Self {
        Self {
            chars: CharacterSet::unicode(),
        }
    }
}

#[non_exhaustive]
pub struct CharacterSet {
    pub margin_bar: char,
    pub margin_bar_skip: char,
    pub margin_h: char,
    pub margin_top_left: char,
    pub margin_top_right: char,
    pub margin_bottom_right: char,

    pub label_v: char,
    pub label_h: char,
    pub label_top_left: char,
    pub label_bottom_left: char,
    pub arrow_underline: char,
    pub arrow_underconnect: char,
}

impl CharacterSet {
    pub fn ascii() -> Self {
        Self {
            margin_bar: '|',
            margin_bar_skip: '|',
            margin_h: '-',
            margin_top_left: ',',
            margin_top_right: '.',
            margin_bottom_right: '\'',
            label_v: '|',
            label_h: '-',
            label_top_left: ',',
            label_bottom_left: '\'',
            arrow_underline: '-',
            arrow_underconnect: '-',
        }
    }

    pub fn unicode() -> Self {
        Self {
            margin_bar: '│',
            margin_bar_skip: '┆',
            margin_h: '─',
            margin_top_left: '╭',
            margin_top_right: '╮',
            margin_bottom_right: '╯',
            label_v: '│',
            label_h: '─',
            label_top_left: '╭',
            label_bottom_left: '╰',
            arrow_underline: '─',
            arrow_underconnect: '┬',
        }
    }
}
