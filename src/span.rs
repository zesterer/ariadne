use super::*;

#[derive(Copy, Clone, Debug)]
#[non_exhaustive]
pub enum Offset {
    Byte(usize),
    Char(usize),
    // TODO: What is a column? A byte offset? A char offset? A visual thing?
    //LineColumn(usize, usize),
}

pub trait Span {
    type FileId;

    /// Return the identifier of the file that this span refers to.
    fn file_id(&self) -> &Self::FileId;
    /// Return the offset range that this span corresponds to.
    fn range(&self) -> Range<Offset>;
    /// Turn this span into its file identifier and
    fn into_parts(self) -> (Self::FileId, Range<Offset>);
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

impl From<Range<usize>> for ByteSpan {
    fn from(byte_range: Range<usize>) -> Self {
        Self::new(byte_range, ())
    }
}

impl<K> Span for ByteSpan<K> {
    type FileId = K;

    fn file_id(&self) -> &Self::FileId {
        &self.file_id
    }
    fn range(&self) -> Range<Offset> {
        Offset::Byte(self.start)..Offset::Byte(self.end)
    }
    fn into_parts(self) -> (Self::FileId, Range<Offset>) {
        let range = self.range();
        (self.file_id, range)
    }
}

pub struct CharSpan<K = ()> {
    pub start: usize,
    pub end: usize,
    pub file_id: K,
}

impl<K> CharSpan<K> {
    pub fn new(char_range: Range<usize>, file_id: K) -> Self {
        Self {
            start: char_range.start,
            end: char_range.end,
            file_id,
        }
    }
}

impl From<Range<usize>> for CharSpan {
    fn from(char_range: Range<usize>) -> Self {
        Self::new(char_range, ())
    }
}

impl<K> Span for CharSpan<K> {
    type FileId = K;

    fn file_id(&self) -> &Self::FileId {
        &self.file_id
    }
    fn range(&self) -> Range<Offset> {
        Offset::Char(self.start)..Offset::Char(self.end)
    }
    fn into_parts(self) -> (Self::FileId, Range<Offset>) {
        let range = self.range();
        (self.file_id, range)
    }
}
