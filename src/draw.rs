use super::*;
use yansi::Paint;

pub struct Characters {
    pub hbar: char,
    pub vbar: char,
    pub xbar: char,
    pub vbar_break: char,

    pub ltop: char,
    pub mtop: char,
    pub rtop: char,
    pub lbot: char,
    pub rbot: char,
    pub mbot: char,

    pub lbox: char,
    pub rbox: char,

    pub lcross: char,
    pub rcross: char,

    pub underbar: char,
    pub underline: char,
}

impl Characters {
    pub fn unicode() -> Self {
        Self {
            hbar: '─',
            vbar: '│',
            xbar: '┼',
            vbar_break: '·',
            ltop: '╭',
            mtop: '┬',
            rtop: '╮',
            lbot: '╰',
            mbot: '┴',
            rbot: '╯',
            lbox: '[',
            rbox: ']',
            lcross: '├',
            rcross: '┤',
            underbar: '┬',
            underline: '─',
        }
    }

    pub fn ascii() -> Self {
        Self {
            hbar: '-',
            vbar: '|',
            xbar: '+',
            vbar_break: ':',
            ltop: ',',
            mtop: 'v',
            rtop: '.',
            lbot: '`',
            mbot: '^',
            rbot: '\'',
            lbox: '[',
            rbox: ']',
            lcross: '|',
            rcross: '|',
            underbar: '|',
            underline: '^',
        }
    }

    pub fn extended_ascii() -> Self {
        Self {
            hbar: '─',
            vbar: '│',
            xbar: '┼',
            vbar_break: '·',
            ltop: '┌',
            mtop: 'v',
            rtop: '┐',
            lbot: '└',
            mbot: '^',
            rbot: '┘',
            lbox: '[',
            rbox: ']',
            lcross: '├',
            rcross: '┤',
            underbar: '^',
            underline: '^',
        }
    }
}

pub trait Fmt: Sized {
    fn fg(self, color: Option<Color>) -> Colored<Self> {
        Colored(self, color)
    }
}

impl<T: fmt::Display> Fmt for T {}

pub struct Colored<T>(T, Option<Color>);

impl<T: fmt::Display> fmt::Display for Colored<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(col) = self.1 {
            write!(f, "{}", Paint::new(&self.0).fg(col))
        } else {
            write!(f, "{}", self.0)
        }
    }
}
