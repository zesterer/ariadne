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
            "{} {}",
            format_args!("{}{}:", Show(code), self.kind).fg(kind_color, s),
            Show(self.msg.as_ref())
        )?;

        let groups = self.get_source_groups(&mut cache);

        // Line number maximum width
        let line_num_width = max_line_num(&groups, &mut cache).map_or(0, nb_digits);

        let margin_char = |c: char| c.fg(self.config.margin_color(), s);

        let write_margin = |w: &mut W, idx, is_src_line, is_ellipsis: bool| {
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
        let write_spacer_line = |w: &mut W| {
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
        let groups_len = groups.len();
        for (
            group_idx,
            SourceGroup {
                src_id,
                display_range,
                labels,
                ..
            },
        ) in groups.into_iter().enumerate()
        {
            let Some((src, src_name)) = fetch_source(&mut cache, src_id) else {
                // `fetch_source` should have reported the error.
                continue;
            };

            // File name & reference
            let (location, index_type) = if src_id == self.span.source() {
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
            writeln!(
                w,
                "{}{}{}{} {location} {}",
                Rept(' ', line_num_width + 2).fg(self.config.margin_color(), s),
                margin_char(draw.group_connector(group_idx == 0)),
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
            for idx in display_range {
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

            let is_final_group = group_idx + 1 == groups_len;

            // Help
            if is_final_group {
                for (i, help) in self.help.iter().enumerate() {
                    if !self.config.compact {
                        write_margin(&mut w, 0, false, false)?;
                        writeln!(w)?;
                    }
                    let help_prefix = format!("{} {}", "Help", i + 1);
                    let help_prefix_len = if self.help.len() > 1 {
                        help_prefix.len()
                    } else {
                        4
                    };
                    let mut lines = help.lines();
                    if let Some(line) = lines.next() {
                        write_margin(&mut w, 0, false, false)?;
                        if self.help.len() > 1 {
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
            }

            // Note
            if is_final_group {
                for (i, note) in self.notes.iter().enumerate() {
                    if !self.config.compact {
                        write_margin(&mut w, 0, false, false)?;
                        writeln!(w)?;
                    }
                    let note_prefix = format!("{} {}", "Note", i + 1);
                    let note_prefix_len = if self.notes.len() > 1 {
                        note_prefix.len()
                    } else {
                        4
                    };
                    let mut lines = note.lines();
                    if let Some(line) = lines.next() {
                        write_margin(&mut w, 0, false, false)?;
                        if self.notes.len() > 1 {
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
            }

            // Tail of report
            if !self.config.compact {
                if is_final_group {
                    writeln!(
                        w,
                        "{}",
                        format_args!("{}{}", Rept(draw.hbar, line_num_width + 2), draw.rbot)
                            .fg(self.config.margin_color(), s)
                    )?;
                } else {
                    write_spacer_line(&mut w)?;
                }
            }
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

fn max_line_num<S: Span, C: Cache<S::SourceId>>(
    groups: &[SourceGroup<'_, S>],
    cache: &mut C,
) -> Option<usize> {
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

#[cfg(test)]
mod tests {
    //! These tests use [insta](https://insta.rs/). If you do `cargo install cargo-insta` you can
    //! automatically update the snapshots with `cargo insta review` or `cargo insta accept`.
    //!
    //! When adding new tests you can leave the string in the `assert_snapshot!` macro call empty:
    //!
    //!     assert_snapshot!(msg, @"");
    //!
    //! and insta will fill it in.

    use insta::assert_snapshot;

    use crate::{
        Cache, Config, FnCache, IndexType, Label, Report, ReportKind, ReportStyle, Source, Span,
    };

    impl<S: Span, K: ReportStyle> Report<S, K> {
        fn write_to_string<C: Cache<S::SourceId>>(&self, cache: C) -> String {
            let mut vec = Vec::new();
            self.write(cache, &mut vec).unwrap();
            String::from_utf8(vec).unwrap()
        }
    }

    fn no_color() -> Config {
        Config::default().with_color(false)
    }

    fn remove_trailing(s: String) -> String {
        s.lines().flat_map(|l| [l.trim_end(), "\n"]).collect()
    }

    fn multi_sources<'srcs, const NB_SOURCES: usize>(
        sources: &'srcs [&'static str; NB_SOURCES],
    ) -> impl Cache<usize> + 'srcs {
        FnCache::new(move |id: &_| Ok::<_, std::convert::Infallible>(sources[*id]))
    }

    #[test]
    fn one_message() {
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color())
                .with_message("can't compare apples with oranges")
                .finish()
                .write_to_string(Source::from("")),
        );
        assert_snapshot!(msg, @r###"Error: can't compare apples with oranges"###)
    }

    #[test]
    fn two_labels_without_messages() {
        let source = "apple == orange;";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color())
                .with_message("can't compare apples with oranges")
                .with_label(Label::new(0..5))
                .with_label(Label::new(9..15))
                .finish()
                .write_to_string(Source::from(source)),
        );
        assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple == orange;
           │ ─────    ──────
        ───╯
        ");
    }

    #[test]
    fn two_labels_without_messages_on_different_lines() {
        let source = "apple\n== orange;";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color())
                .with_message("can't compare apples with oranges")
                .with_label(Label::new(0..5))
                .with_label(Label::new(9..15))
                .finish()
                .write_to_string(Source::from(source)),
        );
        assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple
           │ ─────
         2 │ == orange;
           │    ──────
        ───╯
        ");
    }

    #[test]
    fn two_labels_with_messages() {
        let source = "apple == orange;";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color())
                .with_message("can't compare apples with oranges")
                .with_label(Label::new(0..5).with_message("This is an apple"))
                .with_label(Label::new(9..15).with_message("This is an orange"))
                .finish()
                .write_to_string(Source::from(source)),
        );
        // TODO: it would be nice if these lines didn't cross
        assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple == orange;
           │ ──┬──    ───┬──
           │   ╰─────────│──── This is an apple
           │             │
           │             ╰──── This is an orange
        ───╯
        ");
    }

    #[test]
    fn two_labels_with_messages_on_different_lines() {
        let source = "apple ==\norange;";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color())
                .with_message("can't compare apples with oranges")
                .with_label(Label::new(0..5).with_message("This is an apple"))
                .with_label(Label::new(9..15).with_message("This is an orange"))
                .finish()
                .write_to_string(Source::from(source)),
        );
        assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple ==
           │ ──┬──
           │   ╰──── This is an apple
         2 │ orange;
           │ ───┬──
           │    ╰──── This is an orange
        ───╯
        ");
    }
    #[test]
    fn duplicate_label() {
        let source = "apple == orange;";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color())
                .with_message("can't compare apples with oranges")
                .with_label(Label::new(0..5).with_message("This is an apple"))
                .with_label(Label::new(0..5).with_message("This is an apple"))
                .finish()
                .write_to_string(Source::from(source)),
        );
        assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple == orange;
           │ ──┬──
           │   ╰──── This is an apple
           │   │
           │   ╰──── This is an apple
        ───╯
        ");
    }

    #[test]
    fn multi_byte_chars() {
        let source = "äpplë == örängë;";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color().with_index_type(IndexType::Char))
                .with_message("can't compare äpplës with örängës")
                .with_label(Label::new(0..5).with_message("This is an äpplë"))
                .with_label(Label::new(9..15).with_message("This is an örängë"))
                .finish()
                .write_to_string(Source::from(source)),
        );
        // TODO: it would be nice if these lines didn't cross
        assert_snapshot!(msg, @"
        Error: can't compare äpplës with örängës
           ╭─┤ <unknown>:1:1 │
           │
         1 │ äpplë == örängë;
           │ ──┬──    ───┬──
           │   ╰─────────│──── This is an äpplë
           │             │
           │             ╰──── This is an örängë
        ───╯
        ");
    }

    #[test]
    fn byte_label() {
        let source = "äpplë == örängë;";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color().with_index_type(IndexType::Byte))
                .with_message("can't compare äpplës with örängës")
                .with_label(Label::new(0..7).with_message("This is an äpplë"))
                .with_label(Label::new(11..20).with_message("This is an örängë"))
                .finish()
                .write_to_string(Source::from(source)),
        );
        // TODO: it would be nice if these lines didn't cross
        assert_snapshot!(msg, @"
        Error: can't compare äpplës with örängës
           ╭─┤ <unknown>:1:1 │
           │
         1 │ äpplë == örängë;
           │ ──┬──    ───┬──
           │   ╰─────────│──── This is an äpplë
           │             │
           │             ╰──── This is an örängë
        ───╯
        ");
    }

    #[test]
    fn byte_column() {
        let source = "äpplë == örängë;";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 11..11)
                .with_config(no_color().with_index_type(IndexType::Byte))
                .with_message("can't compare äpplës with örängës")
                .with_label(Label::new(0..7).with_message("This is an äpplë"))
                .with_label(Label::new(11..20).with_message("This is an örängë"))
                .finish()
                .write_to_string(Source::from(source)),
        );
        // TODO: it would be nice if these lines didn't cross
        assert_snapshot!(msg, @"
        Error: can't compare äpplës with örängës
           ╭─┤ <unknown>:1:10 │
           │
         1 │ äpplë == örängë;
           │ ──┬──    ───┬──
           │   ╰─────────│──── This is an äpplë
           │             │
           │             ╰──── This is an örängë
        ───╯
        ");
    }

    #[test]
    fn crossing_lines() {
        let source = "äpplë == örängë;";
        let msg = Report::build(ReportKind::Error, 11..11)
            .with_config(no_color().with_cross_gap(false))
            .with_message("can't compare äpplës with örängës")
            .with_label(Label::new(0..5).with_message("This is an äpplë"))
            .with_label(Label::new(9..15).with_message("This is an örängë"))
            .finish()
            .write_to_string(Source::from(source));
        // TODO: it would be nice if these lines didn't cross
        assert_snapshot!(msg, @"
        Error: can't compare äpplës with örängës
           ╭─[ <unknown>:1:12 ]
           │
         1 │ äpplë == örängë;
           │ ──┬──    ───┬──  
           │   ╰─────────┼──── This is an äpplë
           │             │    
           │             ╰──── This is an örängë
        ───╯
        ");
    }

    #[test]
    fn label_at_end_of_long_line() {
        let source = format!("{}orange", "apple == ".repeat(100));
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color())
                .with_message("can't compare apples with oranges")
                .with_label(
                    Label::new(source.len() - 6..source.len()).with_message("This is an orange"),
                )
                .finish()
                .write_to_string(Source::from(source)),
        );
        // TODO: it would be nice if the start of long lines would be omitted (like rustc does)
        assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == apple == orange
           │                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     ───┬──
           │                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        ╰──── This is an orange
        ───╯
        ");
    }

    #[test]
    fn label_of_width_zero_at_end_of_line() {
        let source = "apple ==\n";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color().with_index_type(IndexType::Byte))
                .with_message("unexpected end of file")
                .with_label(Label::new(9..9).with_message("Unexpected end of file"))
                .finish()
                .write_to_string(Source::from(source)),
        );

        assert_snapshot!(msg, @"
        Error: unexpected end of file
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple ==
           │          │
           │          ╰─ Unexpected end of file
        ───╯
        ");
    }

    #[test]
    fn empty_input() {
        let source = "";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color())
                .with_message("unexpected end of file")
                .with_label(Label::new(0..0).with_message("No more fruit!"))
                .finish()
                .write_to_string(Source::from(source)),
        );

        assert_snapshot!(msg, @"
        Error: unexpected end of file
           ╭─┤ <unknown>:1:1 │
           │
         1 │
           │ │
           │ ╰─ No more fruit!
        ───╯
        ");
    }

    #[test]
    fn empty_input_help() {
        let source = "";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color())
                .with_message("unexpected end of file")
                .with_label(Label::new(0..0).with_message("No more fruit!"))
                .with_help("have you tried going to the farmer's market?")
                .finish()
                .write_to_string(Source::from(source)),
        );

        assert_snapshot!(msg, @"
        Error: unexpected end of file
           ╭─┤ <unknown>:1:1 │
           │
         1 │
           │ │
           │ ╰─ No more fruit!
           │
           │ Help: have you tried going to the farmer's market?
        ───╯
        ");
    }

    #[test]
    fn empty_input_note() {
        let source = "";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color())
                .with_message("unexpected end of file")
                .with_label(Label::new(0..0).with_message("No more fruit!"))
                .with_note("eat your greens!")
                .finish()
                .write_to_string(Source::from(source)),
        );

        assert_snapshot!(msg, @"
        Error: unexpected end of file
           ╭─┤ <unknown>:1:1 │
           │
         1 │
           │ │
           │ ╰─ No more fruit!
           │
           │ Note: eat your greens!
        ───╯
        ");
    }

    #[test]
    fn empty_input_help_note() {
        let source = "";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color())
                .with_message("unexpected end of file")
                .with_label(Label::new(0..0).with_message("No more fruit!"))
                .with_note("eat your greens!")
                .with_help("have you tried going to the farmer's market?")
                .finish()
                .write_to_string(Source::from(source)),
        );

        assert_snapshot!(msg, @"
        Error: unexpected end of file
           ╭─┤ <unknown>:1:1 │
           │
         1 │
           │ │
           │ ╰─ No more fruit!
           │
           │ Help: have you tried going to the farmer's market?
           │
           │ Note: eat your greens!
        ───╯
        ");
    }

    #[test]
    fn byte_spans_never_crash() {
        let source = "apple\np\n\nempty\n";

        for i in 0..=source.len() {
            for j in i..=source.len() {
                let _ = remove_trailing(
                    Report::build(ReportKind::Error, 0..0)
                        .with_config(no_color().with_index_type(IndexType::Byte))
                        .with_message("Label")
                        .with_label(Label::new(i..j).with_message("Label"))
                        .finish()
                        .write_to_string(Source::from(source)),
                );
            }
        }
    }

    #[test]
    fn multiline_label() {
        let source = "apple\n==\norange";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color())
                .with_label(Label::new(0..source.len()).with_message("illegal comparison"))
                .finish()
                .write_to_string(Source::from(source)),
        );
        // TODO: it would be nice if the 2nd line wasn't omitted
        assert_snapshot!(msg, @"
        Error:
           ╭─┤ <unknown>:1:1 │
           │
         1 │ ╭─▶ apple
           ┆ ┆
         3 │ ├─▶ orange
           │ │
           │ ╰─────────── illegal comparison
        ───╯
        ");
    }

    #[test]
    fn multiple_multilines_same_span() {
        let source = "apple\n==\norange";
        let msg = Report::build(ReportKind::Error, 0..0)
            .with_config(no_color())
            .with_label(Label::new(0..source.len()).with_message("illegal comparison"))
            .with_label(Label::new(0..source.len()).with_message("do not do this"))
            .with_label(Label::new(0..source.len()).with_message("please reconsider"))
            .finish()
            .write_to_string(Source::from(source));
        // TODO: it would be nice if the 2nd line wasn't omitted
        // TODO: it would be nice if the lines didn't cross, or at least less so
        assert_snapshot!(msg, @"
        Error: 
           ╭─[ <unknown>:1:1 ]
           │
         1 │ ╭─────▶ apple
           │ │       ▲       
           │ │ ╭─────╯       
           │ │ │     │       
           │ │ │ ╭───╯       
           ┆ ┆ ┆ ┆   
         3 │ ├─│─│─▶ orange
           │ │ │ │        ▲  
           │ ╰─│─│────────│── illegal comparison
           │   │ │        │  
           │   ╰─│────────┴── do not do this
           │     │        │  
           │     ╰────────┴── please reconsider
        ───╯
        ");
    }

    #[test]
    fn multiline_context_label() {
        let source = "apple\nbanana\ncarrot\ndragonfruit\negg\nfruit\ngrapes";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color().with_context_lines(1))
                .with_label(Label::new(13..35).with_message("illegal comparison"))
                .finish()
                .write_to_string(Source::from(source)),
        );
        // TODO: it would be nice if the 2nd line wasn't omitted
        assert_snapshot!(msg, @"
        Error:
           ╭─┤ <unknown>:1:1 │
           │
         2 │     banana
         3 │ ╭─▶ carrot
         4 │ │   dragonfruit
         5 │ ├─▶ egg
           │ │
           │ ╰───────── illegal comparison
         6 │     fruit
        ───╯
        ");
    }

    #[test]
    fn partially_overlapping_labels() {
        let source = "https://example.com/";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color())
                .with_label(Label::new(0..source.len()).with_message("URL"))
                .with_label(Label::new(0..source.find(':').unwrap()).with_message("scheme"))
                .finish()
                .write_to_string(Source::from(source)),
        );
        // TODO: it would be nice if you could tell where the spans start and end.
        assert_snapshot!(msg, @"
        Error:
           ╭─┤ <unknown>:1:1 │
           │
         1 │ https://example.com/
           │ ──┬───────┬─────────
           │   ╰───────│─────────── scheme
           │           │
           │           ╰─────────── URL
        ───╯
        ");
    }

    #[test]
    fn multiple_labels_same_span() {
        let source = "apple == orange;";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color())
                .with_message("can't compare apples with oranges")
                .with_label(Label::new(0..5).with_message("This is an apple"))
                .with_label(
                    Label::new(0..5).with_message("Have I mentioned that this is an apple?"),
                )
                .with_label(Label::new(0..5).with_message("No really, have I mentioned that?"))
                .with_label(Label::new(9..15).with_message("This is an orange"))
                .with_label(
                    Label::new(9..15).with_message("Have I mentioned that this is an orange?"),
                )
                .with_label(Label::new(9..15).with_message("No really, have I mentioned that?"))
                .finish()
                .write_to_string(Source::from(source)),
        );
        assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple == orange;
           │ ──┬──    ───┬──
           │   ╰─────────│──── This is an apple
           │   │         │
           │   ╰─────────│──── Have I mentioned that this is an apple?
           │   │         │
           │   ╰─────────│──── No really, have I mentioned that?
           │             │
           │             ╰──── This is an orange
           │             │
           │             ╰──── Have I mentioned that this is an orange?
           │             │
           │             ╰──── No really, have I mentioned that?
        ───╯
        ")
    }

    #[test]
    fn note() {
        let source = "apple == orange;";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color())
                .with_message("can't compare apples with oranges")
                .with_label(Label::new(0..5).with_message("This is an apple"))
                .with_label(Label::new(9..15).with_message("This is an orange"))
                .with_note("stop trying ... this is a fruitless endeavor")
                .finish()
                .write_to_string(Source::from(source)),
        );
        assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple == orange;
           │ ──┬──    ───┬──
           │   ╰─────────│──── This is an apple
           │             │
           │             ╰──── This is an orange
           │
           │ Note: stop trying ... this is a fruitless endeavor
        ───╯
        ")
    }

    #[test]
    fn help() {
        let source = "apple == orange;";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color())
                .with_message("can't compare apples with oranges")
                .with_label(Label::new(0..5).with_message("This is an apple"))
                .with_label(Label::new(9..15).with_message("This is an orange"))
                .with_help("have you tried peeling the orange?")
                .finish()
                .write_to_string(Source::from(source)),
        );
        assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple == orange;
           │ ──┬──    ───┬──
           │   ╰─────────│──── This is an apple
           │             │
           │             ╰──── This is an orange
           │
           │ Help: have you tried peeling the orange?
        ───╯
        ")
    }

    #[test]
    fn help_and_note() {
        let source = "apple == orange;";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color())
                .with_message("can't compare apples with oranges")
                .with_label(Label::new(0..5).with_message("This is an apple"))
                .with_label(Label::new(9..15).with_message("This is an orange"))
                .with_help("have you tried peeling the orange?")
                .with_note("stop trying ... this is a fruitless endeavor")
                .finish()
                .write_to_string(Source::from(source)),
        );
        assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple == orange;
           │ ──┬──    ───┬──
           │   ╰─────────│──── This is an apple
           │             │
           │             ╰──── This is an orange
           │
           │ Help: have you tried peeling the orange?
           │
           │ Note: stop trying ... this is a fruitless endeavor
        ───╯
        ")
    }

    #[test]
    fn single_note_single_line() {
        let source = "apple == orange;";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color())
                .with_message("can't compare apples with oranges")
                .with_label(Label::new(0..15).with_message("This is a strange comparison"))
                .with_note("No need to try, they can't be compared.")
                .finish()
                .write_to_string(Source::from(source)),
        );
        assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple == orange;
           │ ───────┬───────
           │        ╰───────── This is a strange comparison
           │
           │ Note: No need to try, they can't be compared.
        ───╯
        ")
    }

    #[test]
    fn multi_notes_single_lines() {
        let source = "apple == orange;";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color())
                .with_message("can't compare apples with oranges")
                .with_label(Label::new(0..15).with_message("This is a strange comparison"))
                .with_note("No need to try, they can't be compared.")
                .with_note("Yeah, really, please stop.")
                .finish()
                .write_to_string(Source::from(source)),
        );
        assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple == orange;
           │ ───────┬───────
           │        ╰───────── This is a strange comparison
           │
           │ Note 1: No need to try, they can't be compared.
           │
           │ Note 2: Yeah, really, please stop.
        ───╯
        ")
    }

    #[test]
    fn multi_notes_multi_lines() {
        let source = "apple == orange;";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color())
                .with_message("can't compare apples with oranges")
                .with_label(Label::new(0..15).with_message("This is a strange comparison"))
                .with_note("No need to try, they can't be compared.")
                .with_note("Yeah, really, please stop.\nIt has no resemblance.")
                .finish()
                .write_to_string(Source::from(source)),
        );
        assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple == orange;
           │ ───────┬───────
           │        ╰───────── This is a strange comparison
           │
           │ Note 1: No need to try, they can't be compared.
           │
           │ Note 2: Yeah, really, please stop.
           │         It has no resemblance.
        ───╯
        ")
    }

    #[test]
    fn multi_helps_multi_lines() {
        let source = "apple == orange;";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color())
                .with_message("can't compare apples with oranges")
                .with_label(Label::new(0..15).with_message("This is a strange comparison"))
                .with_help("No need to try, they can't be compared.")
                .with_help("Yeah, really, please stop.\nIt has no resemblance.")
                .finish()
                .write_to_string(Source::from(source)),
        );
        assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ apple == orange;
           │ ───────┬───────
           │        ╰───────── This is a strange comparison
           │
           │ Help 1: No need to try, they can't be compared.
           │
           │ Help 2: Yeah, really, please stop.
           │         It has no resemblance.
        ───╯
        ")
    }

    #[test]
    fn ordered_labels() {
        let msg = remove_trailing(
            Report::build(ReportKind::Error, ("", 0..0))
                .with_config(no_color())
                .with_label(Label::new(("b", 13..18)).with_order(1).with_message("1"))
                .with_label(Label::new(("a", 0..6)).with_order(2).with_message("2"))
                .with_label(Label::new(("a", 7..12)).with_order(3).with_message("3"))
                .with_label(Label::new(("b", 0..6)).with_order(4).with_message("4"))
                .with_label(Label::new(("b", 7..12)).with_order(5).with_message("5"))
                .finish()
                .write_to_string(crate::sources([
                    ("a", "second\nthird"),
                    ("b", "fourth\nfifth\nfirst"),
                ])),
        );
        assert_snapshot!(msg, @"
        Error:
           ╭─┤ b:3:1 │
           │
         3 │ first
           │ ──┬──
           │   ╰──── 1
           │
           ├─┤ a:1:1 │
           │
         1 │ second
           │ ───┬──
           │    ╰──── 2
         2 │ third
           │ ──┬──
           │   ╰──── 3
           │
           ├─┤ b:1:1 │
           │
         1 │ fourth
           │ ───┬──
           │    ╰──── 4
         2 │ fifth
           │ ──┬──
           │   ╰──── 5
        ───╯
        ")
    }

    #[test]
    fn minimise_crossings() {
        let source = "begin\napple == orange;\nend";
        let msg = remove_trailing(
            Report::build(ReportKind::Error, 0..0)
                .with_config(no_color().with_minimise_crossings(true))
                .with_message("can't compare apples with oranges")
                .with_label(Label::new(6..11).with_message("This is an apple"))
                .with_label(Label::new(15..21).with_message("This is an orange"))
                .with_label(Label::new(3..25).with_message("multi 1"))
                .with_label(Label::new(25..26).with_message("single"))
                .finish()
                .write_to_string(Source::from(source)),
        );
        assert_snapshot!(msg, @"
        Error: can't compare apples with oranges
           ╭─┤ <unknown>:1:1 │
           │
         1 │ ╭─▶ begin
         2 │ │   apple == orange;
           │ │   ──┬──    ───┬──
           │ │     │         ╰──── This is an orange
           │ │     │
           │ │     ╰────────────── This is an apple
         3 │ ├─▶ end
           │ │     🭯
           │ │     ╰── single
           │ │
           │ ╰──────── multi 1
        ───╯
        ")
    }

    #[test]
    fn multi_source() {
        let msg = remove_trailing(
            Report::build(ReportKind::Error, (0, 0..0))
                .with_config(no_color())
                .with_message("can't compare apples with oranges or pears")
                .with_label(Label::new((0, 0..5)).with_message("This is an apple"))
                .with_label(Label::new((0, 9..15)).with_message("This is an orange"))
                .with_label(Label::new((1, 0..5)).with_message("This is an apple"))
                .with_label(Label::new((1, 9..13)).with_message("This is a pear"))
                .finish()
                .write_to_string(multi_sources(&["apple == orange;", "apple == pear;"])),
        );
        assert_snapshot!(msg, @"
        Error: can't compare apples with oranges or pears
           ╭─[ 0:1:1 ]
           │
         1 │ apple == orange;
           │ ──┬──    ───┬──
           │   ╰─────────│──── This is an apple
           │             │
           │             ╰──── This is an orange
           │
           ├─[ 1:1:1 ]
           │
         1 │ apple == pear;
           │ ──┬──    ──┬─
           │   ╰────────│─── This is an apple
           │            │
           │            ╰─── This is a pear
        ───╯
        ")
    }

    #[test]
    fn help_and_note_multi() {
        let msg = remove_trailing(
            Report::build(ReportKind::Error, (0, 0..0))
                .with_config(no_color())
                .with_message("can't compare apples with oranges or pears")
                .with_label(Label::new((0, 0..5)).with_message("This is an apple"))
                .with_label(Label::new((0, 9..15)).with_message("This is an orange"))
                .with_label(Label::new((1, 0..5)).with_message("This is an apple"))
                .with_label(Label::new((1, 9..13)).with_message("This is a pear"))
                .with_help("have you tried peeling the orange?")
                .with_note("stop trying ... this is a fruitless endeavor")
                .finish()
                .write_to_string(multi_sources(&["apple == orange;", "apple == pear;"])),
        );
        assert_snapshot!(msg, @"
        Error: can't compare apples with oranges or pears
           ╭─[ 0:1:1 ]
           │
         1 │ apple == orange;
           │ ──┬──    ───┬──
           │   ╰─────────│──── This is an apple
           │             │
           │             ╰──── This is an orange
           │
           ├─[ 1:1:1 ]
           │
         1 │ apple == pear;
           │ ──┬──    ──┬─
           │   ╰────────│─── This is an apple
           │            │
           │            ╰─── This is a pear
           │
           │ Help: have you tried peeling the orange?
           │
           │ Note: stop trying ... this is a fruitless endeavor
        ───╯
        ")
    }
}
