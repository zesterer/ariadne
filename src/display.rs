use super::*;

use std::fmt::{self, Debug, Display};

pub struct Show<T>(pub T);

impl<T: Display> Display for Show<Option<T>> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.0 {
            Some(x) => write!(f, "{}", x),
            None => Ok(()),
        }
    }
}
