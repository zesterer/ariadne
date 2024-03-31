mod display;

use core::{
    ops::Range,
    borrow::Borrow,
};

pub trait Span {
    type FileId;

    fn file_id(&self) -> &Self::FileId;

    fn byte_range<C>(&self, file: &File<C>) -> Range<usize>
    where
        C: Borrow<str>,
    {
        todo!("implement in terms of char_range")
    }

    fn char_range<C>(&self, file: &File<C>) -> Range<usize>
    where
        C: Borrow<str>,
    {
        todo!("implement in terms of byte_range")
    }
}

pub trait Files<K> {
    type Filename: Borrow<str>;

    type Content: Borrow<str>;
    type File: Borrow<File<Self::Content>>;

    type Error;

    fn fetch_filename(&mut self, file_id: &K) -> Result<Option<Self::Filename>, Self::Error>;
    fn fetch_file(&mut self, file_id: &K) -> Result<Self::File, Self::Error>;
}

impl<'a, F: Files<K>, K> Files<K> for &'a mut F {
    type Filename = F::Filename;
    type Content = F::Content;
    type File = F::File;
    type Error = F::Error;

    fn fetch_filename(&mut self, file_id: &K) -> Result<Option<Self::Filename>, Self::Error> { (*self).fetch_filename(file_id) }
    fn fetch_file(&mut self, file_id: &K) -> Result<Self::File, Self::Error> { (*self).fetch_file(file_id) }
}

impl<'a> Files<()> for &'a str {
    type Filename = &'static str;
    type Content = &'a str;
    type File = File<&'a str>;
    type Error = core::convert::Infallible;

    fn fetch_filename(&mut self, _: &()) -> Result<Option<Self::Filename>, Self::Error> { Ok(None) }
    fn fetch_file(&mut self, _: &()) -> Result<Self::File, Self::Error> { Ok(File::new(*self)) }
}

pub struct File<C> {
    content: C,
    // (bytes, chars)
    lines: Vec<(Range<usize>, Range<usize>)>,
}

impl<C: Borrow<str>> File<C> {
    pub fn new(content: C) -> Self {
        let mut lines = Vec::new();
        let mut line_start = (0, 0);
        let mut char_offset = 0;
        let mut was_newline = false;
        for (byte_offset, c) in content
            .borrow()
            .char_indices()
        {
            // If the last-seen character was a newline, push a new line
            if was_newline {
                lines.push((line_start.0..byte_offset, line_start.0..char_offset));
                line_start = (byte_offset, char_offset);
                was_newline = false;
            }

            // Note: We can ignore `\r` here because it always ends up being coupled with `\n` to count as a newline anyway.
            was_newline = c == '\n';
            char_offset += 1;
        }

        // Push whatever is left as the final line
        lines.push((line_start.0..content.borrow().len(), line_start.0..char_offset));

        Self {
            lines,
            content,
        }
    }

    fn line(&self, idx: usize) -> Option<&str> {
        self.lines.get(idx).map(|(bytes, _)| &self.content.borrow()[bytes.clone()])
    }

    fn line_of_byte(&self, byte_offset: usize) -> Option<(usize, &str)> {
        let idx = self.lines
            .binary_search_by_key(&byte_offset, |(bytes, _)| bytes.end)
            .unwrap_or_else(core::convert::identity);
        self.line(idx).map(|s| (idx, s))
    }

    fn lines_of<S: Span>(&self, span: &S) -> impl ExactSizeIterator<Item = (usize, &str)> {
        let bytes = span.byte_range(self);
        let range = if let Some((start, _)) = self.line_of_byte(bytes.start) {
            // Ranges are exclusive
            if let Some((end, _)) = self.line_of_byte(bytes.end.saturating_sub(1).max(bytes.start)) {
                start..end + 1
            } else {
                start..self.lines.len()
            }
        } else {
            0..0
        };

        range.map(|i| (i, self.line(i).unwrap()))
    }
}

pub struct ByteSpan<K = ()> {
    pub start: usize,
    pub end: usize,
    pub file_id: K,
}

impl<K> ByteSpan<K> {
    pub fn new(byte_range: Range<usize>, file_id: K) -> Self {
        Self {
            start: byte_range.start,
            end: byte_range.end,
            file_id,
        }
    }
}

impl<K: Default> From<Range<usize>> for ByteSpan<K> {
    fn from(byte_range: Range<usize>) -> Self {
        Self::new(byte_range, K::default())
    }
}

impl<K> Span for ByteSpan<K> {
    type FileId = K;

    fn file_id(&self) -> &Self::FileId { &self.file_id }
    fn byte_range<C>(&self, file: &File<C>) -> Range<usize>
    where
        C: Borrow<str>,
    {
        self.start..self.end
    }
}

#[derive(Copy, Clone)]
pub enum DiagnosticKind {
    Error,
    Warning,
    Info,
}

pub struct Diagnostic<S = ByteSpan> {
    kind: DiagnosticKind,
    msg: Option<String>, // TODO: <Sch as Schema>::Text
    labels: Vec<Label<S>>,
}

impl<S> Diagnostic<S> {
    pub fn new(kind: DiagnosticKind) -> Self {
        Self { kind, msg: None, labels: Vec::new() }
    }

    pub fn error() -> Self { Self::new(DiagnosticKind::Error) }
    pub fn warning() -> Self { Self::new(DiagnosticKind::Warning) }
    pub fn info() -> Self { Self::new(DiagnosticKind::Info) }

    pub fn with_message<M>(mut self, message: M) -> Self
    where
        M: ToString,
    {
        self.msg = Some(message.to_string());
        self
    }

    pub fn with_label(mut self, label: Label<S>) -> Self {
        self.labels.push(label);
        self
    }

    pub fn eprint<F>(&self, files: F)
    where
        S: Span,
        F: Files<S::FileId>,
    {
        eprint!("{}", self.display(files));
    }
}

pub struct Label<S = ByteSpan> {
    span: S,
}

impl<S> Label<S> {
    pub fn at(span: S) -> Self {
        Self { span }
    }
}
