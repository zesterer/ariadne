use std::path::{Path, PathBuf};

pub struct BacktraceConfig {
    /// Show the backtrace with number labels
    pub show_label_numbers: bool,
    /// Number labels start at this index, `index0` vs `index1`
    pub label_index_start: usize,
}

pub struct BackTrace {
    frames: Vec<BackTraceFrame>,
}

pub struct BackTraceFrame {
    /// File path buf
    pub file: String,
    /// Function name, or module path, e.g. `std::io::read_line`
    pub object: Option<String>,
    /// Line Column
    pub position: Option<(usize, usize)>,
}

impl Default for BacktraceConfig {
    fn default() -> Self {
        Self {
            show_label_numbers: true,
            label_index_start: 1,
        }
    }
}

impl Default for BackTrace {
    fn default() -> Self {
        Self { frames: Vec::new() }
    }
}

impl BackTrace {
    /// Add a frame to the backtrace
    pub fn with_frame(&mut self, frame: BackTraceFrame) {
        self.frames.push(frame);
    }
    /// Add a io error to the backtrace
    pub fn with_io_error(&mut self, error: std::io::Error, path: impl AsRef<Path>) {
        let path = path.as_ref();
        let path = path.to_str().unwrap_or_else(|| path.to_string_lossy().as_ref());
        self.frames.push(BackTraceFrame::new(path).with_object(error.to_string()));
    }
}

impl BackTraceFrame {
    /// Create a new backtrace frame
    pub fn new(file: impl Into<String>) -> Self {
        let path = file.into();
        debug_assert!(path.lines().count() == 1, "File path must be in a single line");
        Self {
            file: path,
            object: None,
            position: None,
        }
    }
    /// Name to display for the object
    pub fn with_object(mut self, object: impl Into<String>) -> Self {
        let name = object.into();
        debug_assert!(name.lines().count() == 1, "Object name must be in a single line");
        self.object = Some(name);
        self
    }
    /// Where the object is located
    pub fn with_position(mut self, position: (usize, usize)) -> Self {
        self.position = Some(position);
        self
    }
}