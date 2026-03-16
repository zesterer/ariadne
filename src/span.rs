use std::hash::Hash;
use std::ops::RangeInclusive;
use std::{fmt, ops::Range};
/// A trait implemented by spans within a character-based source.
pub trait Span {
    /// The identifier used to uniquely refer to a source. In most cases, this is the fully-qualified path of the file.
    type SourceId: PartialEq + ToOwned + ?Sized;

    /// Get the identifier of the source that this span refers to.
    fn source(&self) -> &Self::SourceId;

    /// Get the start offset of this span.
    ///
    /// Offsets are zero-indexed character offsets from the beginning of the source.
    fn start(&self) -> usize;

    /// Get the (exclusive) end offset of this span.
    ///
    /// The end offset should *always* be greater than or equal to the start offset as given by [`Span::start`].
    ///
    /// Offsets are zero-indexed character offsets from the beginning of the source.
    fn end(&self) -> usize;

    /// Get the length of this span (difference between the start of the span and the end of the span).
    fn len(&self) -> usize {
        self.end().saturating_sub(self.start())
    }

    /// Returns `true` if this span has length zero.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Determine whether the span contains the given offset.
    fn contains(&self, offset: usize) -> bool {
        (self.start()..self.end()).contains(&offset)
    }
}

impl Span for Range<usize> {
    type SourceId = ();

    fn source(&self) -> &Self::SourceId {
        &()
    }
    fn start(&self) -> usize {
        self.start
    }
    fn end(&self) -> usize {
        self.end
    }
}

impl<Id: fmt::Debug + Hash + PartialEq + Eq + ToOwned> Span for (Id, Range<usize>) {
    type SourceId = Id;

    fn source(&self) -> &Self::SourceId {
        &self.0
    }
    fn start(&self) -> usize {
        self.1.start
    }
    fn end(&self) -> usize {
        self.1.end
    }
}

impl Span for RangeInclusive<usize> {
    type SourceId = ();

    fn source(&self) -> &Self::SourceId {
        &()
    }
    fn start(&self) -> usize {
        *self.start()
    }
    fn end(&self) -> usize {
        *self.end() + 1
    }
}

impl<Id: fmt::Debug + Hash + PartialEq + Eq + ToOwned> Span for (Id, RangeInclusive<usize>) {
    type SourceId = Id;

    fn source(&self) -> &Self::SourceId {
        &self.0
    }
    fn start(&self) -> usize {
        *self.1.start()
    }
    fn end(&self) -> usize {
        *self.1.end() + 1
    }
}
