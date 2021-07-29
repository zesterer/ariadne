use super::*;
use yansi::Paint;

pub struct Characters {
    pub hbar: char,
    pub vbar: char,
    pub xbar: char,
    pub vbar_break: char,

    pub uarrow: char,
    pub rarrow: char,

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
            uarrow: '🭯',
            rarrow: '▶',
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
            uarrow: '^',
            rarrow: '>',
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
}

/// A trait used to add formatting attributes to displayable items.
///
/// Attributes specified through this trait are not composable (i.e: the behaviour of two nested attributes each with a
/// conflicting attribute is left unspecified).
pub trait Fmt: Sized {
    /// Give this value the specified foreground colour
    fn fg<C: Into<Option<Color>>>(self, color: C) -> Foreground<Self> {
        Foreground(self, color.into())
    }

    /// Give this value the specified background colour
    fn bg<C: Into<Option<Color>>>(self, color: C) -> Background<Self> {
        Background(self, color.into())
    }
}
impl<T: fmt::Display> Fmt for T {}

#[derive(Copy, Clone, Debug)]
pub struct Foreground<T>(T, Option<Color>);
impl<T: fmt::Display> fmt::Display for Foreground<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(col) = self.1 {
            write!(f, "{}", Paint::new(&self.0).fg(col))
        } else {
            write!(f, "{}", self.0)
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Background<T>(T, Option<Color>);
impl<T: fmt::Display> fmt::Display for Background<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(col) = self.1 {
            write!(f, "{}", Paint::new(&self.0).fg(col))
        } else {
            write!(f, "{}", self.0)
        }
    }
}

/// A type that can generate distinct 8-bit colors.
pub struct ColorGenerator {
    state: [u16; 3],
    min_brightness: f32,
}

impl Default for ColorGenerator {
    fn default() -> Self { Self::from_state([30000, 15000, 35000], 0.5) }
}

impl ColorGenerator {
    /// Create a new [`ColorGenerator`] with the given pre-chosen state.
    ///
    /// The minimum brightness can be used to control the colour brightness (0.0 - 1.0). The default is 0.5.
    pub fn from_state(state: [u16; 3], min_brightness: f32) -> Self {
        Self { state, min_brightness: min_brightness.max(0.0).min(1.0) }
    }

    /// Create a new [`ColorGenerator`] with the default state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate the next colour in the sequence.
    pub fn next(&mut self) -> Color {
        for i in 0..3 {
            // magic constant, one of only two that have this property!
            self.state[i] = (self.state[i] as usize).wrapping_add(40503 * (i * 4 + 1130)) as u16;
        }
        Color::Fixed(16
            + ((self.state[2] as f32 / 65535.0 * (1.0 - self.min_brightness) + self.min_brightness) * 5.0
            + (self.state[1] as f32 / 65535.0 * (1.0 - self.min_brightness) + self.min_brightness) * 30.0
            + (self.state[0] as f32 / 65535.0 * (1.0 - self.min_brightness) + self.min_brightness) * 180.0) as u8)
    }
}
