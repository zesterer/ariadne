use super::*;
use core::{fmt, cell::RefCell};

impl<S> Diagnostic<S> {
    pub fn display<F>(&self, files: F) -> Display<F, S>
    where
        S: Span,
        F: Files<S::FileId>,
    {
        Display { d: self, files: RefCell::new(files) }
    }
}

pub struct Display<'a, F, S = ByteSpan> {
    d: &'a Diagnostic<S>,
    // `RefCell` required because `fmt::Display::fmt` takes `&self`.
    files: RefCell<F>,
}

impl<'a, F, S> fmt::Display for Display<'a, F, S>
where
    S: Span,
    F: Files<S::FileId>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Self { d, files } = self;
        let mut files = self.files.borrow_mut();

        // Header

        match d.kind {
            DiagnosticKind::Error => write!(f, "[E] Error")?,
            DiagnosticKind::Warning => write!(f, "[W] Warning")?,
            DiagnosticKind::Info => write!(f, "[I] Info")?,
        }

        if let Some(msg) = &d.msg {
            writeln!(f, ": {msg}")?;
        } else {
            writeln!(f, "")?;
        }

        for label in &d.labels {
            match files.fetch_filename(label.span.file_id()) {
                Ok(None) => {},
                Ok(Some(fname)) => writeln!(f, "in {}:", fname.borrow())?,
                Err(_) => writeln!(f, "in <unknown>:")?,
            }

            match files.fetch_file(label.span.file_id()) {
                Ok(file) => file
                    .borrow()
                    .lines_of(&label.span)
                    .map(|(i, l)| {
                        write!(f, "{:>3} | ", i + 1);
                        write_line(f, l)?;
                        writeln!(f, "")?;
                        write!(f, "    | ")?;
                        write_span(f, (file.borrow().lines[i].0.start, l), label.span.byte_range(file.borrow()))?;
                        writeln!(f, "")?;
                        Ok(())
                    })
                    .collect::<Result<_, _>>()?,
                Err(_) => writeln!(f, "<cannot fetch file>")?,
            }
        }

        Ok(())
    }
}

fn canonicalize(c: char) -> Option<Result<char, &'static str>> {
    match c {
        '\r' | '\n' => None,
        '\t' => Some(Err("    ")),
        c => Some(Ok(c)),
    }
}

fn write_line(f: &mut fmt::Formatter, l: &str) -> fmt::Result {
    for c in l.chars() {
        match canonicalize(c) {
            None => {},
            Some(Err(s)) => write!(f, "{s}")?,
            Some(Ok(c)) => write!(f, "{c}")?,
        }
    }
    Ok(())
}

fn write_span(f: &mut fmt::Formatter, (l_start, l): (usize, &str), span: Range<usize>) -> fmt::Result {
    for (byte_offset, c) in l.char_indices() {
        let cols = match canonicalize(c) {
            None => 0,
            Some(Err(s)) => s.chars().count(),
            Some(Ok(_)) => 1,
        };

        let c = if span.contains(&(l_start + byte_offset)) {
            '^'
        } else {
            ' '
        };

        for _ in 0..cols {
            write!(f, "{c}")?;
        }
    }
    Ok(())
}
