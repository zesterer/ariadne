use super::*;
use core::fmt;

pub(crate) trait Target {
    type Output;

    fn render<K, F>(&mut self, d: Diagnostic<K>, files: F) -> Self::Output
    where
        K: FileId,
        F: for<'a> Files<'a, K>;
}

pub struct Plaintext<W> {
    writer: W,
    chars: CharacterSet,
}

impl<W> Plaintext<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            chars: CharacterSet::ascii(),
        }
    }
}

impl<W> Target for Plaintext<W>
where
    W: fmt::Write,
{
    type Output = fmt::Result;

    fn render<K, F>(&mut self, d: Diagnostic<K>, files: F) -> Self::Output
    where
        K: FileId,
        F: for<'a> Files<'a, K>,
    {
        write!(
            self.writer,
            "{}",
            Display {
                d: &d,
                files,
                chars: CharacterSet::ascii(),
            }
        )
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
        }
    }
}
