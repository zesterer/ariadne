#[cfg(test)]
mod tests;

use std::fmt::Display;
use std::io;
use std::ops::Range;

use crate::source::Location;
use crate::{Config, IndexType, LabelDisplay, Source};

use super::draw::{self, StreamAwareFmt, StreamType, WrappedWriter};
use super::{Cache, CharSet, LabelAttach, Report, ReportStyle, Rept, Show, Span, Write};

// A WARNING, FOR ALL YE WHO VENTURE IN HERE
//
// - This code is complex and has a lot of implicit invariants
// - Yes, it has some bugs
// - Yes, it needs rewriting
// - No, you are not expected to understand it. I will probably not understand it either in a month, but that will only
//   give me a reason to rewrite it

enum LabelKind {
    Inline,
    Multiline,
}

struct LabelInfo<'a> {
    kind: LabelKind,
    char_span: Range<usize>,
    display_info: &'a LabelDisplay,
    #[allow(dead_code)]
    start_line: usize,
    end_line: usize,
}

impl LabelInfo<'_> {
    fn last_offset(&self) -> usize {
        self.char_span
            .end
            .saturating_sub(1)
            .max(self.char_span.start)
    }

    fn display_range(&self, config: &Config) -> Range<usize> {
        self.start_line.saturating_sub(config.context_lines)
            ..self.end_line + config.context_lines + 1
    }
}

struct SourceGroup<'a, S: Span> {
    src_id: &'a S::SourceId,
    char_span: Range<usize>,
    display_range: Range<usize>,
    labels: Vec<LabelInfo<'a>>,
}

impl<S: Span, K: ReportStyle> Report<S, K> {
    fn get_source_groups(&self, cache: &mut impl Cache<S::SourceId>) -> Vec<SourceGroup<'_, S>> {
        let mut labels = Vec::new();
        for label in self.labels.iter() {
            let label_source = label.span.source();

            let Some((src, _src_name)) = fetch_source(cache, label_source) else {
                continue;
            };

            let given_label_span = label.span.start()..label.span.end();

            let (label_char_span, start_line, end_line) = match self.config.index_type {
                IndexType::Char => {
                    let Some(start_line) = src.get_offset_line(given_label_span.start) else {
                        continue;
                    };
                    let end_line = if given_label_span.start >= given_label_span.end {
                        start_line.line_idx
                    } else {
                        let Some(end_line) = src.get_offset_line(given_label_span.end - 1) else {
                            continue;
                        };
                        end_line.line_idx
                    };
                    (given_label_span, start_line.line_idx, end_line)
                }
                IndexType::Byte => {
                    let Some(start_location) = src.get_byte_line(given_label_span.start) else {
                        continue;
                    };
                    let line_text = src.get_line_text(start_location.line).unwrap();

                    let num_chars_before_start = line_text
                        [..start_location.col_idx.min(line_text.len())]
                        .chars()
                        .count();
                    let start_char_offset = start_location.line.offset() + num_chars_before_start;

                    if given_label_span.start >= given_label_span.end {
                        (
                            start_char_offset..start_char_offset,
                            start_location.line_idx,
                            start_location.line_idx,
                        )
                    } else {
                        // We can subtract 1 from end, because get_byte_line doesn't actually index into the text.
                        let end_pos = given_label_span.end - 1;
                        let Some(end_location) = src.get_byte_line(end_pos) else {
                            continue;
                        };
                        let end_line_text = src.get_line_text(end_location.line).unwrap();
                        // Have to add 1 back now, so we don't cut a char in two.
                        let num_chars_before_end =
                            end_line_text[..end_location.col_idx + 1].chars().count();
                        let end_char_offset = end_location.line.offset() + num_chars_before_end;

                        (
                            start_char_offset..end_char_offset,
                            start_location.line_idx,
                            end_location.line_idx,
                        )
                    }
                }
            };

            let label_info = LabelInfo {
                kind: if start_line == end_line {
                    LabelKind::Inline
                } else {
                    LabelKind::Multiline
                },
                char_span: label_char_span,
                display_info: &label.display_info,
                start_line,
                end_line,
            };

            labels.push((label_info, label_source));
        }
        labels.sort_by_key(|(l, _)| (l.display_info.order, l.end_line, l.start_line));
        let mut groups = Vec::<SourceGroup<_>>::new();
        for (label, src_id) in labels {
            match groups.last_mut() {
                Some(group)
                    if group.src_id == src_id
                        && group
                            .labels
                            .last()
                            .map_or(true, |last| last.end_line <= label.end_line) =>
                {
                    group.char_span.start = group.char_span.start.min(label.char_span.start);
                    group.char_span.end = group.char_span.end.max(label.char_span.end);
                    let display_range = label.display_range(&self.config);
                    group.display_range.start = group.display_range.start.min(display_range.start);
                    group.display_range.end = group.display_range.end.max(display_range.end);
                    group.labels.push(label);
                }
                _ => {
                    groups.push(SourceGroup {
                        src_id,
                        char_span: label.char_span.clone(),
                        display_range: label.display_range(&self.config),
                        labels: vec![label],
                    });
                }
            }
        }
        groups
    }

    /// Write this diagnostic to an implementor of [`Write`].
    ///
    /// If using the `concolor` feature, this method assumes that the output is ultimately going to be printed to
    /// `stderr`.  If you are printing to `stdout`, use the [`write_for_stdout`](Self::write_for_stdout) method instead.
    ///
    /// If you wish to write to `stderr` or `stdout`, you can do so via [`Report::eprint`] or [`Report::print`] respectively.
    pub fn write<C: Cache<S::SourceId>, W: Write>(&self, cache: C, w: W) -> io::Result<()> {
        self.write_for_stream(cache, w, StreamType::Stderr)
    }

    /// Write this diagnostic to an implementor of [`Write`], assuming that the output is ultimately going to be printed
    /// to `stdout`.
    pub fn write_for_stdout<C: Cache<S::SourceId>, W: Write>(
        &self,
        cache: C,
        w: W,
    ) -> io::Result<()> {
        self.write_for_stream(cache, w, StreamType::Stdout)
    }

    /// Write this diagnostic to an implementor of [`Write`], assuming that the output is ultimately going to be printed
    /// to the given output stream (`stdout` or `stderr`).
    fn write_for_stream<C: Cache<S::SourceId>, W: Write>(
        &self,
        mut cache: C,
        w: W,
        s: StreamType,
    ) -> io::Result<()> {
        let mut w = WrappedWriter::new(w, &self.config);
        let draw = match self.config.char_set {
            CharSet::Unicode => draw::Characters::unicode(),
            CharSet::Ascii => draw::Characters::ascii(),
        };

        // --- Header ---

        let code = self.code.as_ref().map(|c| format!("[{c}] "));
        let kind_color = self.kind.get_color(&self.config);
        writeln!(
            w,
            "{}: {}",
            format_args!("{}{}", Show(code), self.kind).fg(kind_color, s),
            Show(self.msg.as_ref())
        )?;

        let groups = self.get_source_groups(&mut cache);

        // Line number maximum width
        let line_num_width = max_line_num(&groups).map_or(0, nb_digits);

        let margin_char = |c: char| c.fg(self.config.margin_color(), s);

        let write_margin = |w: &mut WrappedWriter<W>, idx, is_src_line, is_ellipsis: bool| {
            let line_num_margin = if is_src_line && !is_ellipsis {
                format!("{:line_num_width$} {}", idx + 1, draw.vbar)
                    .fg(self.config.margin_color(), s)
            } else {
                format!(
                    "{}{}",
                    Rept(' ', line_num_width + 1),
                    draw.vbar(is_ellipsis)
                )
                .fg(self.config.skipped_margin_color(), s)
            };

            write!(
                w,
                " {line_num_margin}{}",
                Show((!self.config.compact).then_some(' ')),
            )
        };
        let write_spacer_line = |w: &mut WrappedWriter<W>| {
            if !self.config.compact {
                writeln!(
                    w,
                    "{}{}",
                    Rept(' ', line_num_width + 2),
                    margin_char(draw.vbar)
                )
            } else {
                Ok(())
            }
        };

        // --- Source sections ---
        for (
            group_idx,
            SourceGroup {
                src_id,
                display_range,
                labels,
                ..
            },
        ) in groups.iter().enumerate()
        {
            let Some((src, src_name)) = fetch_source(&mut cache, src_id) else {
                // `fetch_source` should have reported the error.
                continue;
            };

            // File name & reference
            let (location, index_type) = if *src_id == self.span.source() {
                (self.span.start(), self.config.index_type)
            } else {
                // This has already been converted from bytes to chars, if applicable.
                (labels[0].char_span.start, IndexType::Char)
            };
            let location = Loc(
                src,
                src_name,
                match index_type {
                    IndexType::Char => src.get_offset_line(location),
                    IndexType::Byte => src.get_byte_line(location).map(|location| {
                        let line_text = src.get_line_text(location.line).unwrap();

                        let col = line_text[..location.col_idx.min(line_text.len())]
                            .chars()
                            .count();

                        Location {
                            line: location.line,
                            line_idx: location.line_idx,
                            col_idx: col,
                        }
                    }),
                },
            );
            let corner_char = if group_idx == 0 {
                draw.ltop
            } else {
                write_spacer_line(&mut w)?;
                draw.lcross
            };
            writeln!(
                w,
                "{}{}{}{} {location} {}",
                Rept(' ', line_num_width + 2).fg(self.config.margin_color(), s),
                margin_char(corner_char),
                margin_char(draw.hbar),
                margin_char(draw.lbox),
                margin_char(draw.rbox),
            )?;

            if !self.config.compact {
                write_spacer_line(&mut w)?;
            }

            // Generate a list of multi-line labels
            let mut multi_labels: Vec<_> = labels
                .iter()
                .filter(|label_info| matches!(label_info.kind, LabelKind::Multiline))
                .collect();
            // Sort them by length; this also ensures that the next array is sorted.
            multi_labels.sort_unstable_by_key(|label_info| !Span::len(&label_info.char_span));

            let mut multi_labels_with_message: Vec<_> = multi_labels
                .iter()
                .copied()
                .filter(|label_info| label_info.display_info.msg.is_some())
                .collect();
            // Since we're filtering a sorted array, this one is also sorted.
            // However, we may want to re-sort it:
            if self.config.minimise_crossings {
                // There is no total ordering to labels, so just spin around a bunch rearranging labels making tiny improvements
                // Crap bubble sort, basically
                for i in (0..multi_labels_with_message.len().saturating_sub(1))
                    .cycle()
                    .take(multi_labels_with_message.len().pow(2) * 2)
                {
                    let a = &multi_labels_with_message[i];
                    let b = &multi_labels_with_message[i + 1];
                    let pro_a = (a.char_span.start < b.char_span.start) as i32
                        + (a.char_span.end > b.char_span.end) as i32;
                    let pro_b = (b.char_span.start < a.char_span.start) as i32
                        + (b.char_span.end > a.char_span.end) as i32;
                    if pro_a < pro_b {
                        multi_labels_with_message.swap(i, i + 1);
                    }
                }
            }

            let write_margin_and_arrows = |w: &mut WrappedWriter<W>,
                                           idx: usize,
                                           is_src_line: bool,
                                           is_ellipsis: bool,
                                           report_row: Option<(usize, bool)>,
                                           line_labels: &[LineLabel],
                                           margin_label: &Option<LineLabel>|
             -> std::io::Result<()> {
                write_margin(w, idx, is_src_line, is_ellipsis)?;

                // Multi-line margins
                for col in 0..multi_labels_with_message.len()
                    + (!multi_labels_with_message.is_empty()) as usize
                {
                    let mut corner = None;
                    let mut hbar: Option<&LabelInfo> = None;
                    let mut vbar: Option<&LabelInfo> = None;
                    let mut margin_ptr = None;

                    let multi_label = multi_labels_with_message.get(col);
                    let line_span = src.line(idx).unwrap().span();

                    for (i, label) in multi_labels_with_message
                        [0..(col + 1).min(multi_labels_with_message.len())]
                        .iter()
                        .enumerate()
                    {
                        let margin = margin_label.as_ref().filter(|m| m.is_referencing(label));

                        if label.char_span.start < line_span.end
                            && label.char_span.end > line_span.start
                        {
                            let is_parent = i != col;
                            let is_start = line_span.contains(&label.char_span.start);
                            let is_end = line_span.contains(&label.last_offset());

                            if let (Some(margin), true) = (margin, is_src_line) {
                                margin_ptr = Some((margin, is_start));
                            } else if !is_start && (!is_end || is_src_line) {
                                vbar = vbar.or((!is_parent).then_some(*label));
                            } else if let Some((report_row, is_arrow)) = report_row {
                                let label_row = line_labels
                                    .iter()
                                    .enumerate()
                                    .find(|(_, l)| l.is_referencing(label))
                                    .map_or(0, |(r, _)| r);
                                if report_row == label_row {
                                    if let Some(margin) = margin {
                                        vbar = (col == i).then_some(margin.label);
                                        if is_start {
                                            continue;
                                        }
                                    }

                                    if is_arrow {
                                        hbar = Some(*label);
                                        if !is_parent {
                                            corner = Some((label, is_start));
                                        }
                                    } else if !is_start {
                                        vbar = vbar.or((!is_parent).then_some(*label));
                                    }
                                } else {
                                    vbar = vbar
                                        .or((!is_parent && (is_start ^ (report_row < label_row)))
                                            .then_some(*label));
                                }
                            }

                            if let Some(margin) = margin_label.as_ref().filter(|m| {
                                is_end && is_src_line && std::ptr::eq(*label, m.label) && col > i
                            }) {
                                hbar = Some(margin.label);
                            }
                        }
                    }

                    if let (Some((margin, _is_start)), true) = (margin_ptr, is_src_line) {
                        let is_col = multi_label.map_or(false, |ml| margin.is_referencing(ml));
                        let is_limit = col + 1 == multi_labels_with_message.len();
                        if !is_col && !is_limit {
                            hbar = hbar.or(Some(margin.label));
                        }
                    }

                    // hbar = hbar.filter(|l| {
                    //     margin_label
                    //         .as_ref()
                    //         .map_or(false, |margin| !margin.is_referencing(l))
                    //         || !is_src_line
                    // });

                    let (a, b) = if let Some((label, is_start)) = corner {
                        (
                            Some((draw.arrow_bend(is_start), *label)),
                            Some((draw.hbar, *label)),
                        )
                    } else if let Some((v_label, h_label)) = vbar.zip(hbar) {
                        (
                            if self.config.cross_gap {
                                Some((draw.vbar, v_label))
                            } else {
                                Some((draw.xbar, v_label))
                            },
                            Some((draw.hbar, h_label)),
                        )
                    } else if let (Some((margin, is_start)), true) = (margin_ptr, is_src_line) {
                        let is_col = multi_label.map_or(false, |ml| margin.is_referencing(ml));
                        let is_limit = col == multi_labels_with_message.len();
                        (
                            Some((
                                if is_limit {
                                    if self.config.multiline_arrows {
                                        draw.rarrow
                                    } else {
                                        draw.hbar
                                    }
                                } else if is_col {
                                    if is_start {
                                        draw.ltop
                                    } else {
                                        draw.lcross
                                    }
                                } else {
                                    draw.hbar
                                },
                                margin.label,
                            )),
                            Some((if is_limit { ' ' } else { draw.hbar }, margin.label)),
                        )
                    } else if let Some(label) = hbar {
                        (Some((draw.hbar, label)), Some((draw.hbar, label)))
                    } else if let Some(label) = vbar {
                        (Some((draw.vbar(is_ellipsis), label)), None)
                    } else {
                        (None, None)
                    };

                    let arrow_char = |opt: Option<(char, &LabelInfo<'_>)>| match opt {
                        Some((c, label)) => c.fg(label.display_info.color, s),
                        None => ' '.fg(None, s),
                    };
                    write!(w, "{}", arrow_char(a))?;
                    if !self.config.compact {
                        write!(w, "{}", arrow_char(b))?;
                    }
                }

                Ok(())
            };

            let mut is_ellipsis = false;
            for idx in display_range.clone() {
                let Some(line) = src.line(idx) else {
                    continue;
                };

                // The (optional) label whose arrows are drawn in the margin (horizontal),
                // instead of normally (vertical).
                let margin_label = multi_labels_with_message
                    .iter()
                    .enumerate()
                    .filter_map(|(i, label)| {
                        let is_start = line.span().contains(&label.char_span.start);
                        let is_end = line.span().contains(&label.last_offset());
                        if is_start {
                            // TODO: Check to see whether multi is the first on the start line or first on the end line
                            Some(LineLabel {
                                col: label.char_span.start - line.offset(),
                                label,
                                multi: Some(i),
                                draw_msg: false, // Multi-line spans don;t have their messages drawn at the start
                            })
                        } else if is_end {
                            Some(LineLabel {
                                col: label.last_offset() - line.offset(),
                                label,
                                multi: Some(i),
                                draw_msg: true, // Multi-line spans have their messages drawn at the end
                            })
                        } else {
                            None
                        }
                    })
                    .min_by_key(|ll| (ll.col, !ll.label.char_span.start));
                let is_margin_label = |label| {
                    margin_label
                        .as_ref()
                        .map_or(false, |m_label| m_label.is_referencing(label))
                };

                // Generate a list of labels for this line, along with their label columns
                let mut line_labels = multi_labels_with_message
                    .iter()
                    .enumerate()
                    .filter_map(|(i, label)| {
                        let is_start = line.span().contains(&label.char_span.start);
                        let is_end = line.span().contains(&label.last_offset());
                        if is_start && !is_margin_label(label) {
                            // TODO: Check to see whether multi is the first on the start line or first on the end line
                            Some(LineLabel {
                                col: label.char_span.start - line.offset(),
                                label,
                                multi: Some(i),
                                draw_msg: false, // Multi-line spans don't have their messages drawn at the start
                            })
                        } else if is_end {
                            Some(LineLabel {
                                col: label.last_offset() - line.offset(),
                                label,
                                multi: Some(i),
                                draw_msg: true, // Multi-line spans have their messages drawn at the end
                            })
                        } else {
                            None
                        }
                    })
                    .chain(
                        labels
                            .iter()
                            .filter(|label_info| {
                                matches!(label_info.kind, LabelKind::Inline)
                                    && label_info.char_span.start >= line.span().start
                                    && label_info.char_span.end <= line.span().end
                            })
                            .map(|label_info| LineLabel {
                                col: match &self.config.label_attach {
                                    LabelAttach::Start => label_info.char_span.start,
                                    LabelAttach::Middle => {
                                        (label_info.char_span.start + label_info.char_span.end) / 2
                                    }
                                    LabelAttach::End => label_info.last_offset(),
                                }
                                .max(label_info.char_span.start)
                                    - line.offset(),
                                label: label_info,
                                multi: None,
                                draw_msg: true,
                            }),
                    )
                    .collect::<Vec<_>>();

                // Skip this line if we don't have labels for it...
                if line_labels.is_empty()
                    && margin_label.is_none()
                    // ...and it does not intersect the display area of any labels
                    && labels.iter().all(|l| {
                        (l.start_line as isize - idx as isize).abs()
                            .min((l.end_line as isize - idx as isize).abs()) > self.config.context_lines as isize
                    })
                {
                    let within_label = multi_labels
                        .iter()
                        .any(|label| label.char_span.contains(&line.span().start()));
                    if !is_ellipsis && within_label {
                        is_ellipsis = true;
                    } else {
                        if !self.config.compact && !is_ellipsis {
                            write_margin(&mut w, idx, false, is_ellipsis)?;
                            writeln!(w)?;
                        }
                        is_ellipsis = true;
                        continue;
                    }
                } else {
                    is_ellipsis = false;
                }

                // Sort the labels by their columns
                line_labels.sort_by_key(|ll| {
                    (
                        ll.label.display_info.order,
                        // `draw_msg = true` means that this is the end of the label
                        if self.config.minimise_crossings {
                            ll.multi.map(|i| if ll.draw_msg { !i } else { i })
                        } else {
                            None
                        },
                        if self.config.minimise_crossings ^ ll.draw_msg {
                            ll.col
                        } else {
                            !ll.col
                        },
                        !ll.label.char_span.start,
                    )
                });

                // Determine label bounds so we know where to put error messages
                let arrow_end_space = if self.config.compact { 1 } else { 2 };
                let arrow_len = line_labels.iter().fold(0, |l, ll| {
                    if ll.multi.is_some() {
                        line.len()
                    } else {
                        l.max(ll.label.char_span.end().saturating_sub(line.offset()))
                    }
                }) + arrow_end_space;

                // Should we draw a vertical bar as part of a label arrow on this line?
                let get_vbar = |col, row| {
                    line_labels
                        .iter()
                        // Only labels with notes get an arrow
                        .enumerate()
                        .filter(|(_, ll)| {
                            ll.label.display_info.msg.is_some() && !is_margin_label(ll.label)
                        })
                        .find(|(j, ll)| ll.col == col && row <= *j)
                        .map(|(_, ll)| ll)
                };

                let get_highlight = |col| {
                    margin_label
                        .iter()
                        .map(|ll| &ll.label)
                        .chain(multi_labels.iter())
                        .chain(line_labels.iter().map(|l| &l.label))
                        .filter(|l| l.char_span.contains(&(line.offset() + col)))
                        // Prioritise displaying smaller spans
                        .min_by_key(|l| {
                            (
                                -l.display_info.priority,
                                ExactSizeIterator::len(&l.char_span),
                            )
                        })
                };

                let get_underline = |col| {
                    line_labels
                        .iter()
                        .filter(|ll| {
                            self.config.underlines
                        // Underlines only occur for inline spans (highlighting can occur for all spans)
                        && ll.multi.is_none()
                        && ll.label.char_span.contains(&(line.offset() + col))
                        })
                        // Prioritise displaying smaller spans
                        .min_by_key(|ll| {
                            (
                                -ll.label.display_info.priority,
                                ExactSizeIterator::len(&ll.label.char_span),
                            )
                        })
                };

                // Margin
                write_margin_and_arrows(
                    &mut w,
                    idx,
                    true,
                    is_ellipsis,
                    None,
                    &line_labels,
                    &margin_label,
                )?;

                // Line
                if !is_ellipsis {
                    for (col, c) in src
                        .get_line_text(line)
                        .unwrap()
                        .trim_end()
                        .chars()
                        .enumerate()
                    {
                        let color = if let Some(highlight) = get_highlight(col) {
                            highlight.display_info.color
                        } else {
                            self.config.unimportant_color()
                        };
                        let (c, width) = self.config.char_width(c, col);
                        if c.is_whitespace() {
                            for _ in 0..width {
                                write!(w, "{}", c.fg(color, s))?;
                            }
                        } else {
                            write!(w, "{}", c.fg(color, s))?;
                        };
                    }
                }
                writeln!(w)?;

                // Arrows
                for row in 0..line_labels.len() {
                    let line_label = &line_labels[row];
                    if row == 0
                        || (line_label.label.display_info.msg.is_some() && !self.config.compact)
                    {
                        // Margin alternate
                        write_margin_and_arrows(
                            &mut w,
                            idx,
                            false,
                            is_ellipsis,
                            Some((row, false)),
                            &line_labels,
                            &margin_label,
                        )?;
                        // Lines alternate
                        let mut chars = src.get_line_text(line).unwrap().trim_end().chars();
                        for col in 0..arrow_len {
                            let width =
                                chars.next().map_or(1, |c| self.config.char_width(c, col).1);

                            let vbar = get_vbar(col, row);
                            let underline = get_underline(col).filter(|_| row == 0);
                            let [c, tail] = if let Some(vbar_ll) = vbar {
                                let [c, tail] = if underline.is_some() {
                                    if ExactSizeIterator::len(&vbar_ll.label.char_span) <= 1 {
                                        [draw.underbar_single, draw.underline]
                                    } else if line.offset() + col == vbar_ll.label.char_span.start {
                                        [draw.lunderbar, draw.munderbar]
                                    } else if line.offset() + col == vbar_ll.label.last_offset() {
                                        [draw.runderbar, draw.munderbar]
                                    } else {
                                        [draw.munderbar, draw.underline]
                                    }
                                } else if vbar_ll.multi.is_some()
                                    && row == 0
                                    && self.config.multiline_arrows
                                {
                                    [draw.uarrow, ' ']
                                } else {
                                    [draw.vbar, ' ']
                                };
                                [
                                    c.fg(vbar_ll.label.display_info.color, s),
                                    tail.fg(vbar_ll.label.display_info.color, s),
                                ]
                            } else if let Some(underline_ll) = underline {
                                [draw.underline.fg(underline_ll.label.display_info.color, s); 2]
                            } else {
                                [' '.fg(None, s); 2]
                            };

                            for i in 0..width {
                                write!(w, "{}", if i == 0 { c } else { tail })?;
                            }
                        }
                        writeln!(w)?;
                    }

                    // No message to draw thus no arrow to draw
                    if line_label.label.display_info.msg.is_none() {
                        continue;
                    }

                    // Margin
                    write_margin_and_arrows(
                        &mut w,
                        idx,
                        false,
                        is_ellipsis,
                        Some((row, true)),
                        &line_labels,
                        &margin_label,
                    )?;
                    // Lines
                    let mut chars = src.get_line_text(line).unwrap().trim_end().chars();
                    for col in 0..arrow_len {
                        let width = chars.next().map_or(1, |c| self.config.char_width(c, col).1);

                        let is_hbar = (((col > line_label.col) ^ line_label.multi.is_some())
                            || (line_label.label.display_info.msg.is_some()
                                && line_label.draw_msg
                                && col > line_label.col))
                            && line_label.label.display_info.msg.is_some();
                        let [c, tail] = if col == line_label.col
                            && line_label.label.display_info.msg.is_some()
                            && !is_margin_label(line_label.label)
                        {
                            [
                                if line_label.multi.is_some() {
                                    if line_label.draw_msg {
                                        draw.mbot
                                    } else {
                                        draw.rbot
                                    }
                                } else {
                                    draw.lbot
                                }
                                .fg(line_label.label.display_info.color, s),
                                draw.hbar.fg(line_label.label.display_info.color, s),
                            ]
                        } else if let Some(vbar_ll) = get_vbar(col, row).filter(|_| {
                            col != line_label.col || line_label.label.display_info.msg.is_some()
                        }) {
                            if !self.config.cross_gap && is_hbar {
                                [
                                    draw.xbar.fg(vbar_ll.label.display_info.color, s),
                                    ' '.fg(line_label.label.display_info.color, s),
                                ]
                            } else {
                                [
                                    draw.vbar.fg(vbar_ll.label.display_info.color, s),
                                    ' '.fg(line_label.label.display_info.color, s),
                                ]
                            }
                        } else if is_hbar {
                            [draw.hbar.fg(line_label.label.display_info.color, s); 2]
                        } else {
                            [' '.fg(None, s); 2]
                        };

                        if width > 0 {
                            write!(w, "{c}")?;
                        }
                        for _ in 1..width {
                            write!(w, "{tail}")?;
                        }
                    }
                    if line_label.draw_msg {
                        write!(w, " {}", Show(line_label.label.display_info.msg.as_ref()))?;
                    }
                    writeln!(w)?;
                }
            }
        }

        // Help
        for (i, help) in self.help.iter().enumerate() {
            if !self.config.compact && (i == 0) {
                write_margin(&mut w, 0, false, false)?;
                writeln!(w)?;
            }
            let help_prefix = format!("{} {}", "Help", i + 1);
            let help_prefix_len = if self.help.len() > 1 && self.config.enumerate_helps {
                help_prefix.len()
            } else {
                "Help".len()
            };
            let mut lines = help.split('\n');
            if let Some(line) = lines.next() {
                write_margin(&mut w, 0, false, false)?;
                if self.help.len() > 1 && self.config.enumerate_helps {
                    writeln!(w, "{}: {line}", help_prefix.fg(self.config.note_color(), s))?;
                } else {
                    writeln!(w, "{}: {line}", "Help".fg(self.config.note_color(), s))?;
                }
            }
            for line in lines {
                write_margin(&mut w, 0, false, false)?;
                writeln!(w, "{:>pad$}{line}", "", pad = help_prefix_len + 2)?;
            }
        }

        // Notes
        for (i, note) in self.notes.iter().enumerate() {
            if !self.config.compact && i == 0 {
                write_margin(&mut w, 0, false, false)?;
                writeln!(w)?;
            }
            let note_prefix = format!("{} {}", "Note", i + 1);
            let note_prefix_len = if self.notes.len() > 1 && self.config.enumerate_notes {
                note_prefix.len()
            } else {
                "Note".len()
            };
            let mut lines = note.split('\n');
            if let Some(line) = lines.next() {
                write_margin(&mut w, 0, false, false)?;
                if self.notes.len() > 1 && self.config.enumerate_notes {
                    writeln!(w, "{}: {line}", note_prefix.fg(self.config.note_color(), s),)?;
                } else {
                    writeln!(w, "{}: {line}", "Note".fg(self.config.note_color(), s))?;
                }
            }
            for line in lines {
                write_margin(&mut w, 0, false, false)?;
                writeln!(w, "{:>pad$}{line}", "", pad = note_prefix_len + 2)?;
            }
        }

        // Tail of report.
        // Not to be emitted in compact mode, or if nothing has had the margin printed.
        if !(self.config.compact
            || groups.is_empty() && self.help.is_empty() && self.notes.is_empty())
        {
            writeln!(
                w,
                "{}",
                format_args!("{}{}", Rept(draw.hbar, line_num_width + 2), draw.rbot)
                    .fg(self.config.margin_color(), s)
            )?;
        }

        Ok(())
    }
}

struct LineLabel<'a> {
    col: usize,
    label: &'a LabelInfo<'a>,
    multi: Option<usize>,
    draw_msg: bool,
}

impl LineLabel<'_> {
    fn is_referencing(&self, label: &LabelInfo<'_>) -> bool {
        // Do they point to the same label?
        // Note that we want this, and not to compare the labels themselves, so as to support
        // printing the same label twice if we were given that.
        std::ptr::eq(self.label, label)
    }
}

fn fetch_source<'a, Id: ?Sized, C: Cache<Id>>(
    cache: &'a mut C,
    src_id: &Id,
) -> Option<(&'a Source<C::Storage>, String)> {
    let src_name = display_name(cache, src_id);
    match cache.fetch(src_id) {
        Ok(src) => Some((src, src_name)),
        Err(err) => {
            eprintln!("Unable to fetch source {src_name}: {err:?}");
            None
        }
    }
}

fn display_name<Id: ?Sized, C: Cache<Id>>(cache: &C, src_id: &Id) -> String {
    cache
        .display(src_id)
        .map(|d| d.to_string())
        .unwrap_or_else(|| "<unknown>".to_string())
}

fn max_line_num<S: Span>(groups: &[SourceGroup<'_, S>]) -> Option<usize> {
    groups
        .iter()
        .map(|group| nb_digits(group.display_range.end))
        .max()
}

/// Returns how many digits it takes to print `value`.
fn nb_digits(value: usize) -> usize {
    value.checked_ilog10().unwrap_or(0) as usize + 1
}

#[derive(Debug, Clone)]
struct Loc<'src, I: AsRef<str>>(&'src Source<I>, String, Option<Location>);

impl<I: AsRef<str>> Display for Loc<'_, I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.2.as_ref() {
            Some(location) => write!(
                f,
                "{}:{}:{}",
                self.1,
                location.line_idx + 1 + self.0.display_line_offset(),
                location.col_idx + 1,
            ),
            None => write!(f, ":?:?"),
        }
    }
}
