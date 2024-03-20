use std::path::{Path};

/// Configuration for display backtraces
pub struct CausedByConfig {
    /// Show the backtrace with number labels
    pub show_label_numbers: bool,
    /// Number labels start at this index, `index0` vs `index1`
    pub label_index_start: usize,
}

/// A backtrace collection
pub struct CausedBy {
    pub(crate) frames: Vec<CausedByFrame>,
}

/// A backtrace frame
#[derive(Debug)]
pub struct CausedByFrame {
    /// File path buf
    pub file: String,
    /// Function name, or module path, e.g. `std::io::read_line`
    pub object: Option<String>,
    /// Line Column
    pub position: Option<(usize, usize)>,
}

impl Default for CausedByConfig {
    fn default() -> Self {
        Self {
            show_label_numbers: true,
            label_index_start: 1,
        }
    }
}

impl Default for CausedBy {
    fn default() -> Self {
        Self { frames: Vec::new() }
    }
}

impl CausedBy {
    /// Add a frame to the backtrace
    pub fn push_frame(&mut self, frame: CausedByFrame) {
        self.frames.push(frame);
    }
    /// Clear all of the frames
    pub fn clear(&mut self) {
        self.frames.clear();
    }
}

impl CausedByFrame {
    /// Create a new backtrace frame
    pub fn new(file: impl Into<String>) -> Self {
        let path = file.into();
        debug_assert!(
            path.lines().count() == 1,
            "File path must be in a single line"
        );
        Self {
            file: path,
            object: None,
            position: None,
        }
    }
    /// Add an io error to the backtrace
    pub fn io_error(error: std::io::Error, path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        let path = path.to_string_lossy().to_string();
        Self::new(path).with_object(error.to_string())
    }
    /// Name to display for the object
    pub fn with_object(mut self, object: impl Into<String>) -> Self {
        let name = object.into();
        debug_assert!(
            name.lines().count() == 1,
            "Object name must be in a single line"
        );
        self.object = Some(name);
        self
    }
    /// Where the object is located
    pub fn with_position(mut self, line: usize, column: usize) -> Self {
        self.position = Some((line, column));
        self
    }
}
