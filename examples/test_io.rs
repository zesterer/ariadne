use ariadne::{CausedByFrame,  Report, ReportKind, Source};
use std::fs::File;
use std::ops::Range;

fn main() {
    let error = match File::open("file/that/must/not/exist.rs") {
        Ok(_) => panic!("Oops! This should not happen."),
        Err(e) => e,
    };
    Report::<Range<usize>>::build(ReportKind::Error, (), 34)
        .with_message("File not found")
        .with_help("Please check the path and try again")
        .push_backtrace(CausedByFrame::io_error(error, "not-exist.rs"))
        .push_backtrace(CausedByFrame::new("std/fs/file.rs").with_object("std::fs::File").with_position(123, 45))
        .finish()
        .print(Source::from(include_str!("sample.tao")))
        .unwrap();
}
