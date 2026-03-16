use core::fmt;

use crate::{
    report::{Report, ReportStyle},
    Config, Label, Span,
};

/// A type used to build a [`Report`].
#[must_use = "call `.finish()` to obtain a `Report`"]
pub struct ReportBuilder<S: Span, K: ReportStyle> {
    pub(crate) kind: K,
    pub(crate) code: Option<String>,
    pub(crate) msg: Option<String>,
    pub(crate) notes: Vec<String>,
    pub(crate) help: Vec<String>,
    pub(crate) span: S,
    pub(crate) labels: Vec<Label<S>>,
    pub(crate) config: Config,
}

impl<S: Span, K: ReportStyle> ReportBuilder<S, K> {
    /// Give this report a numerical code that may be used to more precisely look up the error in documentation.
    pub fn with_code<C: fmt::Display>(mut self, code: C) -> Self {
        self.code = Some(format!("{code:02}"));
        self
    }

    /// Set the message of this report.
    pub fn set_message<M: ToString>(&mut self, msg: M) {
        self.msg = Some(msg.to_string());
    }

    /// Add a message to this report.
    pub fn with_message<M: ToString>(mut self, msg: M) -> Self {
        self.msg = Some(msg.to_string());
        self
    }

    /// Set the note of this report.
    pub fn set_note<N: ToString>(&mut self, note: N) {
        self.notes = vec![note.to_string()];
    }

    /// Adds a note to this report.
    pub fn add_note<N: ToString>(&mut self, note: N) {
        self.notes.push(note.to_string());
    }

    /// Removes all notes in this report.
    pub fn with_notes<N: IntoIterator<Item = impl ToString>>(&mut self, notes: N) {
        for note in notes {
            self.add_note(note)
        }
    }

    /// Set the note of this report.
    pub fn with_note<N: ToString>(mut self, note: N) -> Self {
        self.add_note(note);
        self
    }

    /// Set the help message of this report.
    pub fn set_help<N: ToString>(&mut self, note: N) {
        self.help = vec![note.to_string()];
    }

    /// Add a help message to this report.
    pub fn add_help<N: ToString>(&mut self, note: N) {
        self.help.push(note.to_string());
    }

    /// Set the help messages of this report.
    pub fn with_helps<N: IntoIterator<Item = impl ToString>>(&mut self, helps: N) {
        for help in helps {
            self.add_help(help)
        }
    }

    /// Set the help message of this report.
    pub fn with_help<N: ToString>(mut self, note: N) -> Self {
        self.add_help(note);
        self
    }

    /// Add a label to the report.
    pub fn add_label(&mut self, label: Label<S>) {
        self.add_labels(std::iter::once(label));
    }

    /// Add multiple labels to the report.
    pub fn add_labels<L: IntoIterator<Item = Label<S>>>(&mut self, labels: L) {
        let config = &self.config; // This would not be necessary in Rust 2021 edition
        self.labels.extend(labels.into_iter().map(|mut label| {
            label.display_info.color = config.filter_color(label.display_info.color);
            label
        }));
    }

    /// Add a label to the report.
    pub fn with_label(mut self, label: Label<S>) -> Self {
        self.add_label(label);
        self
    }

    /// Add multiple labels to the report.
    pub fn with_labels<L: IntoIterator<Item = Label<S>>>(mut self, labels: L) -> Self {
        self.add_labels(labels);
        self
    }

    /// Use the given [`Config`] to determine diagnostic attributes.
    pub fn with_config(mut self, config: Config) -> Self {
        self.config = config;
        self
    }

    /// Finish building the [`Report`].
    pub fn finish(self) -> Report<S, K> {
        Report {
            kind: self.kind,
            code: self.code,
            msg: self.msg,
            notes: self.notes,
            help: self.help,
            span: self.span,
            labels: self.labels,
            config: self.config,
        }
    }
}

impl<S: Span, K: ReportStyle> fmt::Debug for ReportBuilder<S, K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReportBuilder")
            .field("kind", &self.kind)
            .field("code", &self.code)
            .field("msg", &self.msg)
            .field("notes", &self.notes)
            .field("help", &self.help)
            .field("config", &self.config)
            .finish()
    }
}
