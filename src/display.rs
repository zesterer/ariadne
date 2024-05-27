use std::fmt::{self, Display};

#[derive(Copy, Clone, Debug)]
pub(crate) struct Show<T>(pub Option<T>);

impl<T: Display> Display for Show<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.0 {
            Some(x) => write!(f, "{x}"),
            None => Ok(()),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct Rept<T>(pub T, pub usize);

impl<T: Display> Display for Rept<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for _ in 0..self.1 {
            write!(f, "{}", self.0)?;
        }
        Ok(())
    }
}
