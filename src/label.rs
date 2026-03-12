use std::ops::Range;

use yansi::Color;

use crate::Span;

/// A type that represents the way a label should be displayed.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(crate) struct LabelDisplay {
    pub msg: Option<String>,
    pub color: Option<Color>,
    pub order: i32,
    pub priority: i32,
}

/// A type that represents a labelled section of source code.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Label<S = Range<usize>> {
    pub(crate) span: S,
    pub(crate) display_info: LabelDisplay,
}

impl<S: Span> Label<S> {
    /// Create a new [`Label`].
    /// If the span is specified as a `Range<usize>` the numbers have to be zero-indexed character offsets.
    ///
    /// # Panics
    ///
    /// Panics if the given span is backwards.
    pub fn new(span: S) -> Self {
        assert!(span.start() <= span.end(), "Label start is after its end");

        Self {
            span,
            display_info: LabelDisplay {
                msg: None,
                color: None,
                order: 0,
                priority: 0,
            },
        }
    }

    /// Give this label a message.
    pub fn with_message<M: ToString>(mut self, msg: M) -> Self {
        self.display_info.msg = Some(msg.to_string());
        self
    }

    /// Give this label a highlight colour.
    pub fn with_color(mut self, color: Color) -> Self {
        self.display_info.color = Some(color);
        self
    }

    /// Specify the order of this label relative to other labels.
    ///
    /// Lower values correspond to this label having an earlier order. Labels with the same order will be arranged
    /// in whatever order best suits their spans and layout constraints.
    ///
    /// If unspecified, labels default to an order of `0`.
    ///
    /// Label order is respected across files. If labels that appear earlier in a file have an order that requires
    /// them to appear later, the resulting diagnostic may result in multiple instances of the same file being
    /// displayed.
    pub fn with_order(mut self, order: i32) -> Self {
        self.display_info.order = order;
        self
    }

    /// Specify the priority of this label relative to other labels.
    ///
    /// Higher values correspond to this label having a higher priority.
    ///
    /// If unspecified, labels default to a priority of `0`.
    ///
    /// Label spans can overlap. When this happens, the crate needs to decide which labels to prioritise for various
    /// purposes such as highlighting. By default, spans with a smaller length get a higher priority. You can use this
    /// function to override this behaviour.
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.display_info.priority = priority;
        self
    }
}

/// The attachment point of inline label arrows
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LabelAttach {
    /// Arrows should attach to the start of the label span.
    Start,
    /// Arrows should attach to the middle of the label span (or as close to the middle as we can get).
    Middle,
    /// Arrows should attach to the end of the label span.
    End,
}
#[test]
#[should_panic]
#[allow(clippy::reversed_empty_ranges)]
fn backwards_label_should_panic() {
    Label::new(1..0);
}
