use super::*;
use core::fmt;

impl<K> Diagnostic<K> {
    pub fn display<'a, F>(&'a self, files: F) -> Display<'a, F, K>
    where
        F: Files<'a, K>,
    {
        Display { d: self, files }
    }
}

pub struct Display<'a, F, K = ()>
where
    F: Files<'a, K>,
{
    d: &'a Diagnostic<K>,
    // `RefCell` required because `fmt::Display::fmt` takes `&self`.
    files: F,
}

impl<'a, F, K> fmt::Display for Display<'a, F, K>
where
    F: Files<'a, K>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Self { d, files } = self;
        let mut file_cache = files.init_cache();

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

        let sorted_spans = SortedSpans::new(d.labels.iter().filter_map(|l| {
            match files.fetch_file(&mut file_cache, &l.file_id) {
                Ok(file) => Some((file.offsets_to_run(&l.offsets), l)),
                Err(_) => {
                    writeln!(f, "<cannot fetch file>").ok()?;
                    None
                }
            }
        }));

        for (run, label) in &sorted_spans.inline {
            match files.fetch_filename(&mut file_cache, &label.file_id) {
                Ok(None) => {}
                Ok(Some(fname)) => writeln!(f, "in {}:", fname)?,
                Err(_) => writeln!(f, "in <unknown>:")?,
            }

            match files.fetch_file(&mut file_cache, &label.file_id) {
                Ok(file) => file
                    .lines_of(*run)
                    .map(|(line, s)| {
                        write!(f, "{:>3} | ", line + 1)?;
                        write_line(f, s)?;
                        writeln!(f, "")?;
                        write!(f, "    | ")?;
                        write_span(f, line, s, *run)?;
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

fn write_line(f: &mut fmt::Formatter, s: &str) -> fmt::Result {
    for c in s.chars() {
        match canonicalize(c) {
            None => {}
            Some(Err(s)) => write!(f, "{s}")?,
            Some(Ok(c)) => write!(f, "{c}")?,
        }
    }
    Ok(())
}

fn write_span(f: &mut fmt::Formatter, line: usize, s: &str, run: Run) -> fmt::Result {
    for (offset, c) in s.char_indices() {
        let cols = match canonicalize(c) {
            None => 0,
            Some(Err(s)) => s.chars().count(),
            Some(Ok(_)) => 1,
        };

        let c = if (run.start..run.end).contains(&Point { line, offset }) {
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

struct SortedSpans<'a, K> {
    inline: Vec<(Run, &'a Label<K>)>,
    multiline: Vec<(Run, &'a Label<K>)>,
}

impl<'a, K> SortedSpans<'a, K> {
    fn new(labels: impl IntoIterator<Item = (Run, &'a Label<K>)>) -> Self {
        let mut inline = Vec::new();
        let mut multiline = Vec::new();

        for (run, label) in labels {
            if run.start.line == run.end.line {
                inline.push((run, label));
            } else {
                multiline.push((run, label));
            }
        }

        Self { inline, multiline }
    }
}
