#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod display;
mod file;
mod span;

pub use crate::{
    file::{files, File, Files},
    span::{ByteSpan, CharSpan, Offset, Span},
};

use crate::file::{Point, Run};
use alloc::{
    borrow::Cow,
    string::{String, ToString},
    vec::Vec,
};
use core::{borrow::Borrow, ops::Range};

#[derive(Copy, Clone)]
pub enum DiagnosticKind {
    Error,
    Warning,
    Info,
}

pub struct Diagnostic<K = ()> {
    kind: DiagnosticKind,
    msg: Option<String>, // TODO: <Sch as Schema>::Text
    labels: Vec<Label<K>>,
}

impl<K> Diagnostic<K> {
    pub fn new(kind: DiagnosticKind) -> Self {
        Self {
            kind,
            msg: None,
            labels: Vec::new(),
        }
    }

    pub fn error() -> Self {
        Self::new(DiagnosticKind::Error)
    }
    pub fn warning() -> Self {
        Self::new(DiagnosticKind::Warning)
    }
    pub fn info() -> Self {
        Self::new(DiagnosticKind::Info)
    }

    pub fn with_message<M>(mut self, message: M) -> Self
    where
        M: ToString,
    {
        self.msg = Some(message.to_string());
        self
    }

    pub fn with_label(mut self, label: Label<K>) -> Self {
        self.labels.push(label);
        self
    }

    #[cfg(feature = "std")]
    pub fn eprint<'a, F>(&'a self, files: F)
    where
        F: Files<'a, K>,
    {
        eprint!("{}", self.display(files));
    }
}

pub struct Label<K = ()> {
    file_id: K,
    offsets: Range<Offset>,
}

impl<K> Label<K> {
    pub fn at<S: Span<FileId = K>>(span: S) -> Self {
        let (file_id, offsets) = span.into_parts();
        Self { file_id, offsets }
    }
}
