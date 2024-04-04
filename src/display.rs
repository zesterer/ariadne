use super::*;
use core::fmt;

pub(crate) struct Display<'a, F, K = ()> {
    pub(crate) d: &'a Diagnostic<K>,
    pub(crate) files: F,
    pub(crate) cfg: &'a render::TextConfig,
}

impl<'a, 'b, F, K> fmt::Display for Display<'b, F, K>
where
    K: FileId,
    F: Files<'a, K>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Self { d, files, cfg } = self;
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

        // Work out which files we need to render
        let files_to_render = d.labels.iter().map(|l| &l.file_id).collect::<BTreeSet<_>>();

        for file_id in files_to_render {
            let fname = match files.fetch_filename(&mut file_cache, file_id) {
                Ok(fname) => fname.map(str::to_string),
                Err(_) => {
                    writeln!(f, "<cannot fetch filename>")?;
                    continue;
                }
            };

            let file = match files.fetch_file(&mut file_cache, file_id) {
                Ok(file) => file,
                Err(_) => {
                    writeln!(f, "<cannot fetch file>")?;
                    continue;
                }
            };

            // Compute the abstract layout for this file
            let layout = FileLayout::new(
                d.labels
                    .iter()
                    .filter(|l| &l.file_id == file_id)
                    .map(|l| (file.offsets_to_run(&l.offsets), l)),
            );

            // The space required to render line numbers
            let max_line_no_width = layout.lines.last().map_or(0, |l| l.idx / 10 + 1);
            let draw_margin = |line_no: Option<usize>, pad| {
                draw(move |f| {
                    write!(f, "{}", pad)?;
                    match line_no {
                        Some(n) => {
                            write!(f, "{}", repeat(max_line_no_width - (n / 10 + 1), pad))?;
                            write!(f, "{}", n)?;
                        }
                        None => write!(f, "{}", repeat(max_line_no_width, pad))?,
                    }
                    write!(f, "{}", pad)
                })
            };

            // Header
            match fname {
                None => writeln!(
                    f,
                    "{}{}",
                    draw_margin(None, cfg.chars.margin_h),
                    cfg.chars.margin_top_right
                )?,
                Some(fname) => writeln!(
                    f,
                    "{}{} in {fname}:",
                    draw_margin(None, ' '),
                    cfg.chars.margin_top_left
                )?,
            }

            for line in &layout.lines {
                let s = file.line(line.idx).expect("tried to render invalid line");

                // Draw the edge of multiline labels
                let draw_multiline_edge = |is_src| {
                    let multilines = &layout.multilines;
                    draw(move |f| {
                        for col in 0..layout.max_multiline_nesting {
                            if let Some(ml) = line
                                .multiline
                                .iter()
                                .map(|ml| &multilines[*ml])
                                .find(|ml| ml.line_idx == Some(col))
                            {
                                let c = if is_src && ml.run.start.line == line.idx {
                                    cfg.chars.label_top_left
                                } else if is_src && ml.run.end.line == line.idx {
                                    cfg.chars.label_bottom_left
                                } else if is_src || line.idx < ml.run.end.line {
                                    cfg.chars.label_v
                                } else {
                                    ' '
                                };
                                write!(f, "{c}")?;
                            } else {
                                write!(f, " ")?;
                            }
                        }
                        Ok(())
                    })
                };

                // Source code
                write!(
                    f,
                    "{}{}{} ",
                    draw_margin(Some(line.idx + 1), ' '),
                    cfg.chars.margin_bar,
                    draw_multiline_edge(true),
                )?;
                write_line(f, s)?;
                writeln!(f, "")?;

                // Underline
                if !line.inline.is_empty() {
                    write!(
                        f,
                        "{}{}{} ",
                        draw_margin(None, ' '),
                        cfg.chars.margin_bar_skip,
                        draw_multiline_edge(false)
                    )?;
                    write_underlines(f, s, |offset| {
                        line.inline.iter().any(|(r, _)| {
                            (r.start..r.end).contains(&Point {
                                line: line.idx,
                                offset,
                            })
                        })
                    })?;
                    writeln!(f, "")?;
                }
            }

            // Footer
            writeln!(
                f,
                "{}{}",
                draw_margin(None, cfg.chars.margin_h),
                cfg.chars.margin_bottom_right
            )?;
        }

        Ok(())
    }
}

fn canonicalize(c: char) -> Option<Result<char, &'static str>> {
    match c {
        '\r' | '\n' => Some(Ok(' ')),
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

fn write_underlines(
    f: &mut fmt::Formatter,
    s: &str,
    should_underline: impl Fn(usize) -> bool,
) -> fmt::Result {
    for (offset, c) in s.char_indices() {
        let cols = match canonicalize(c) {
            None => 0,
            Some(Err(s)) => s.chars().count(),
            Some(Ok(_)) => 1,
        };

        let c = if should_underline(offset) { '^' } else { ' ' };

        for _ in 0..cols {
            write!(f, "{c}")?;
        }
    }
    Ok(())
}

struct Draw<F>(F);
impl<F: Fn(&mut fmt::Formatter) -> fmt::Result> fmt::Display for Draw<F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (self.0)(f)
    }
}

fn draw<F: Fn(&mut fmt::Formatter) -> fmt::Result>(f: F) -> Draw<F> {
    Draw(f)
}

fn repeat<T: fmt::Display>(n: usize, x: T) -> impl fmt::Display {
    draw(move |f| {
        for _ in 0..n {
            write!(f, "{}", x)?
        }
        Ok(())
    })
}
