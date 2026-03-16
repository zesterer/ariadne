#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
// Silly diagnostic anyway
#![allow(clippy::unnecessary_map_or)]

mod config;
mod display;
mod draw;
mod label;
mod report;
mod source;
mod span;
pub use crate::label::*;
pub use crate::report::builder::*;
pub use crate::report::style::*;
pub use crate::report::{Report, ReportKind};
pub use crate::span::*;
pub use crate::{
    draw::{ColorGenerator, Fmt},
    source::{sources, Cache, FileCache, FnCache, Line, Source},
};
pub use config::*;

pub use yansi::Color;

#[cfg(any(feature = "concolor", doc))]
pub use crate::draw::StdoutFmt;

use crate::display::*;
use std::{
    cmp::{Eq, PartialEq},
    fmt::{self, Debug},
    hash::Hash,
    io::{self, Write},
    ops::Range,
};
