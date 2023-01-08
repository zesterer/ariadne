use std::borrow::Borrow;
use std::io;
use std::ops::Range;

use super::draw::{self, StreamAwareFmt, StreamType};
use super::{Cache, CharSet, Label, LabelAttach, Report, ReportKind, Show, Span, Write};

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

struct LabelInfo<'a, S> {
    kind: LabelKind,
    label: &'a Label<S>,
}

struct SourceGroup<'a, S: Span> {
    src_id: &'a S::SourceId,
    span: Range<usize>,
    labels: Vec<LabelInfo<'a, S>>,
}

impl<S: Span> Report<'_, S> {
    fn get_source_groups(&self, cache: &mut impl Cache<S::SourceId>) -> Vec<SourceGroup<S>> {
        let mut groups = Vec::new();
        for label in self.labels.iter() {
            let src_display = cache.display(label.span.source());
            let src = match cache.fetch(label.span.source()) {
                Ok(src) => src,
                Err(e) => {
                    eprintln!("Unable to fetch source '{}': {:?}", Show(src_display), e);
                    continue;
                }
            };

            assert!(
                label.span.start() <= label.span.end(),
                "Label start is after its end"
            );

            let start_line = src.get_offset_line(label.span.start()).map(|(_, l, _)| l);
            let end_line = src
                .get_offset_line(label.span.end().saturating_sub(1).max(label.span.start()))
                .map(|(_, l, _)| l);

            let label_info = LabelInfo {
                kind: if start_line == end_line {
                    LabelKind::Inline
                } else {
                    LabelKind::Multiline
                },
                label,
            };

            if let Some(group) = groups
                .iter_mut()
                .find(|g: &&mut SourceGroup<S>| g.src_id == label.span.source())
            {
                group.span.start = group.span.start.min(label.span.start());
                group.span.end = group.span.end.max(label.span.end());
                group.labels.push(label_info);
            } else {
                groups.push(SourceGroup {
                    src_id: label.span.source(),
                    span: label.span.start()..label.span.end(),
                    labels: vec![label_info],
                });
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
        mut w: W,
        s: StreamType,
    ) -> io::Result<()> {
        let draw = match self.config.char_set {
            CharSet::Unicode => draw::Characters::unicode(),
            CharSet::Ascii => draw::Characters::ascii(),
        };

        // --- Header ---

        let code = self.code.as_ref().map(|c| format!("[{}] ", c));
        let id = format!("{}{}:", Show(code), self.kind);
        let kind_color = match self.kind {
            ReportKind::Error => self.config.error_color(),
            ReportKind::Warning => self.config.warning_color(),
            ReportKind::Advice => self.config.advice_color(),
            ReportKind::Custom(_, color) => Some(color),
        };
        writeln!(w, "{} {}", id.fg(kind_color, s), Show(self.msg.as_ref()))?;

        let groups = self.get_source_groups(&mut cache);

        // Line number maximum width
        let line_no_width = groups
            .iter()
            .filter_map(|SourceGroup { span, src_id, .. }| {
                let src_name = cache
                    .display(src_id)
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| "<unknown>".to_string());

                let src = match cache.fetch(src_id) {
                    Ok(src) => src,
                    Err(e) => {
                        eprintln!("Unable to fetch source {}: {:?}", src_name, e);
                        return None;
                    }
                };

                let line_range = src.get_line_range(span);
                Some(
                    (1..)
                        .map(|x| 10u32.pow(x))
                        .take_while(|x| line_range.end as u32 / x != 0)
                        .count()
                        + 1,
                )
            })
            .max()
            .unwrap_or(0);

        // --- Source sections ---
        let groups_len = groups.len();
        for (
            group_idx,
            SourceGroup {
                src_id,
                span,
                labels,
            },
        ) in groups.into_iter().enumerate()
        {
            let src_name = cache
                .display(src_id)
                .map(|d| d.to_string())
                .unwrap_or_else(|| "<unknown>".to_string());

            let src = match cache.fetch(src_id) {
                Ok(src) => src,
                Err(e) => {
                    eprintln!("Unable to fetch source {}: {:?}", src_name, e);
                    continue;
                }
            };

            let line_range = src.get_line_range(&span);

            // File name & reference
            let location = if src_id == self.location.0.borrow() {
                self.location.1
            } else {
                labels[0].label.span.start()
            };
            let (line_no, col_no) = src
                .get_offset_line(location)
                .map(|(_, idx, col)| (format!("{}", idx + 1), format!("{}", col + 1)))
                .unwrap_or_else(|| ('?'.to_string(), '?'.to_string()));
            let line_ref = format!(":{}:{}", line_no, col_no);
            writeln!(
                w,
                "{}{}{}{}{}{}{}",
                Show((' ', line_no_width + 2)),
                if group_idx == 0 {
                    draw.ltop
                } else {
                    draw.lcross
                }
                .fg(self.config.margin_color(), s),
                draw.hbar.fg(self.config.margin_color(), s),
                draw.lbox.fg(self.config.margin_color(), s),
                src_name,
                line_ref,
                draw.rbox.fg(self.config.margin_color(), s),
            )?;

            if !self.config.compact {
                writeln!(
                    w,
                    "{}{}",
                    Show((' ', line_no_width + 2)),
                    draw.vbar.fg(self.config.margin_color(), s)
                )?;
            }

            struct LineLabel<'a, S> {
                col: usize,
                label: &'a Label<S>,
                multi: bool,
                draw_msg: bool,
            }

            // Generate a list of multi-line labels
            let mut multi_labels = Vec::new();
            for label_info in &labels {
                if matches!(label_info.kind, LabelKind::Multiline) {
                    multi_labels.push(&label_info.label);
                }
            }

            // Sort multiline labels by length
            multi_labels.sort_by_key(|m| -(m.span.len() as isize));

            let write_margin = |w: &mut W,
                                idx: usize,
                                is_line: bool,
                                is_ellipsis: bool,
                                draw_labels: bool,
                                report_row: Option<(usize, bool)>,
                                line_labels: &[LineLabel<S>],
                                margin_label: &Option<LineLabel<S>>|
             -> std::io::Result<()> {
                let line_no_margin = if is_line && !is_ellipsis {
                    let line_no = format!("{}", idx + 1);
                    format!(
                        "{}{} {}",
                        Show((' ', line_no_width - line_no.chars().count())),
                        line_no,
                        draw.vbar,
                    )
                    .fg(self.config.margin_color(), s)
                } else {
                    format!(
                        "{}{}",
                        Show((' ', line_no_width + 1)),
                        if is_ellipsis {
                            draw.vbar_gap
                        } else {
                            draw.vbar
                        }
                    )
                    .fg(self.config.skipped_margin_color(), s)
                };

                write!(
                    w,
                    " {}{}",
                    line_no_margin,
                    Show(Some(' ').filter(|_| !self.config.compact)),
                )?;

                // Multi-line margins
                if draw_labels {
                    for col in 0..multi_labels.len() + (multi_labels.len() > 0) as usize {
                        let mut corner = None;
                        let mut hbar = None;
                        let mut vbar: Option<&&Label<S>> = None;
                        let mut margin_ptr = None;

                        let multi_label = multi_labels.get(col);
                        let line_span = src.line(idx).unwrap().span();

                        for (i, label) in multi_labels[0..(col + 1).min(multi_labels.len())]
                            .iter()
                            .enumerate()
                        {
                            let margin = margin_label
                                .as_ref()
                                .filter(|m| **label as *const _ == m.label as *const _);

                            if label.span.start() <= line_span.end
                                && label.span.end() > line_span.start
                            {
                                let is_parent = i != col;
                                let is_start = line_span.contains(&label.span.start());
                                let is_end = line_span.contains(&label.last_offset());

                                if let Some(margin) = margin.filter(|_| is_line) {
                                    margin_ptr = Some((margin, is_start));
                                } else if !is_start && (!is_end || is_line) {
                                    vbar = vbar.or(Some(*label).filter(|_| !is_parent));
                                } else if let Some((report_row, is_arrow)) = report_row {
                                    let label_row = line_labels
                                        .iter()
                                        .enumerate()
                                        .find(|(_, l)| **label as *const _ == l.label as *const _)
                                        .map_or(0, |(r, _)| r);
                                    if report_row == label_row {
                                        if let Some(margin) = margin {
                                            vbar = Some(&margin.label).filter(|_| col == i);
                                            if is_start {
                                                continue;
                                            }
                                        }

                                        if is_arrow {
                                            hbar = Some(**label);
                                            if !is_parent {
                                                corner = Some((label, is_start));
                                            }
                                        } else if !is_start {
                                            vbar = vbar.or(Some(*label).filter(|_| !is_parent));
                                        }
                                    } else {
                                        vbar = vbar.or(Some(*label).filter(|_| {
                                            !is_parent && (is_start ^ (report_row < label_row))
                                        }));
                                    }
                                }
                            }
                        }

                        if let (Some((margin, _is_start)), true) = (margin_ptr, is_line) {
                            let is_col = multi_label
                                .map_or(false, |ml| **ml as *const _ == margin.label as *const _);
                            let is_limit = col + 1 == multi_labels.len();
                            if !is_col && !is_limit {
                                hbar = hbar.or(Some(margin.label));
                            }
                        }

                        hbar = hbar.filter(|l| {
                            margin_label
                                .as_ref()
                                .map_or(true, |margin| margin.label as *const _ != *l as *const _)
                                || !is_line
                        });

                        let (a, b) = if let Some((label, is_start)) = corner {
                            (
                                if is_start { draw.ltop } else { draw.lbot }.fg(label.color, s),
                                draw.hbar.fg(label.color, s),
                            )
                        } else if let Some(label) =
                            hbar.filter(|_| vbar.is_some() && !self.config.cross_gap)
                        {
                            (draw.xbar.fg(label.color, s), draw.hbar.fg(label.color, s))
                        } else if let Some(label) = hbar {
                            (draw.hbar.fg(label.color, s), draw.hbar.fg(label.color, s))
                        } else if let Some(label) = vbar {
                            (
                                if is_ellipsis {
                                    draw.vbar_gap
                                } else {
                                    draw.vbar
                                }
                                .fg(label.color, s),
                                ' '.fg(None, s),
                            )
                        } else if let (Some((margin, is_start)), true) = (margin_ptr, is_line) {
                            let is_col = multi_label
                                .map_or(false, |ml| **ml as *const _ == margin.label as *const _);
                            let is_limit = col == multi_labels.len();
                            (
                                if is_limit {
                                    draw.rarrow
                                } else if is_col {
                                    if is_start {
                                        draw.ltop
                                    } else {
                                        draw.lcross
                                    }
                                } else {
                                    draw.hbar
                                }
                                .fg(margin.label.color, s),
                                if !is_limit { draw.hbar } else { ' ' }.fg(margin.label.color, s),
                            )
                        } else {
                            (' '.fg(None, s), ' '.fg(None, s))
                        };
                        write!(w, "{}", a)?;
                        if !self.config.compact {
                            write!(w, "{}", b)?;
                        }
                    }
                }

                Ok(())
            };

            let mut is_ellipsis = false;
            for idx in line_range {
                let line = if let Some(line) = src.line(idx) {
                    line
                } else {
                    continue;
                };

                let margin_label = multi_labels
                    .iter()
                    .enumerate()
                    .filter_map(|(_i, label)| {
                        let is_start = line.span().contains(&label.span.start());
                        let is_end = line.span().contains(&label.last_offset());
                        if is_start {
                            // TODO: Check to see whether multi is the first on the start line or first on the end line
                            Some(LineLabel {
                                col: label.span.start() - line.offset(),
                                label: **label,
                                multi: true,
                                draw_msg: false, // Multi-line spans don;t have their messages drawn at the start
                            })
                        } else if is_end {
                            Some(LineLabel {
                                col: label.last_offset() - line.offset(),
                                label: **label,
                                multi: true,
                                draw_msg: true, // Multi-line spans have their messages drawn at the end
                            })
                        } else {
                            None
                        }
                    })
                    .min_by_key(|ll| (ll.col, !ll.label.span.start()));

                // Generate a list of labels for this line, along with their label columns
                let mut line_labels = multi_labels
                    .iter()
                    .enumerate()
                    .filter_map(|(_i, label)| {
                        let is_start = line.span().contains(&label.span.start());
                        let is_end = line.span().contains(&label.last_offset());
                        if is_start
                            && margin_label
                                .as_ref()
                                .map_or(true, |m| **label as *const _ != m.label as *const _)
                        {
                            // TODO: Check to see whether multi is the first on the start line or first on the end line
                            Some(LineLabel {
                                col: label.span.start() - line.offset(),
                                label: **label,
                                multi: true,
                                draw_msg: false, // Multi-line spans don;t have their messages drawn at the start
                            })
                        } else if is_end {
                            Some(LineLabel {
                                col: label.last_offset() - line.offset(),
                                label: **label,
                                multi: true,
                                draw_msg: true, // Multi-line spans have their messages drawn at the end
                            })
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                for label_info in labels.iter().filter(|l| {
                    l.label.span.start() >= line.span().start
                        && l.label.span.end() <= line.span().end
                }) {
                    if matches!(label_info.kind, LabelKind::Inline) {
                        line_labels.push(LineLabel {
                            col: match &self.config.label_attach {
                                LabelAttach::Start => label_info.label.span.start(),
                                LabelAttach::Middle => {
                                    (label_info.label.span.start() + label_info.label.span.end())
                                        / 2
                                }
                                LabelAttach::End => label_info.label.last_offset(),
                            }
                            .max(label_info.label.span.start())
                                - line.offset(),
                            label: label_info.label,
                            multi: false,
                            draw_msg: true,
                        });
                    }
                }

                // Skip this line if we don't have labels for it
                if line_labels.len() == 0 && margin_label.is_none() {
                    let within_label = multi_labels
                        .iter()
                        .any(|label| label.span.contains(line.span().start()));
                    if !is_ellipsis && within_label {
                        is_ellipsis = true;
                    } else {
                        if !self.config.compact && !is_ellipsis {
                            write_margin(&mut w, idx, false, is_ellipsis, false, None, &[], &None)?;
                            write!(w, "\n")?;
                        }
                        is_ellipsis = true;
                        continue;
                    }
                } else {
                    is_ellipsis = false;
                }

                // Sort the labels by their columns
                line_labels.sort_by_key(|ll| (ll.label.order, ll.col, !ll.label.span.start()));

                // Determine label bounds so we know where to put error messages
                let arrow_end_space = if self.config.compact { 1 } else { 2 };
                let arrow_len = line_labels.iter().fold(0, |l, ll| {
                    if ll.multi {
                        line.len()
                    } else {
                        l.max(ll.label.span.end().saturating_sub(line.offset()))
                    }
                }) + arrow_end_space;

                // Should we draw a vertical bar as part of a label arrow on this line?
                let get_vbar = |col, row| {
                    line_labels
                        .iter()
                        // Only labels with notes get an arrow
                        .enumerate()
                        .filter(|(_, ll)| {
                            ll.label.msg.is_some()
                                && margin_label
                                    .as_ref()
                                    .map_or(true, |m| ll.label as *const _ != m.label as *const _)
                        })
                        .find(|(j, ll)| {
                            ll.col == col && ((row <= *j && !ll.multi) || (row <= *j && ll.multi))
                        })
                        .map(|(_, ll)| ll)
                };

                let get_highlight = |col| {
                    margin_label
                        .iter()
                        .map(|ll| ll.label)
                        .chain(multi_labels.iter().map(|l| **l))
                        .chain(line_labels.iter().map(|l| l.label))
                        .filter(|l| l.span.contains(line.offset() + col))
                        // Prioritise displaying smaller spans
                        .min_by_key(|l| (-l.priority, l.span.len()))
                };

                let get_underline = |col| {
                    line_labels
                        .iter()
                        .filter(|ll| {
                            self.config.underlines
                        // Underlines only occur for inline spans (highlighting can occur for all spans)
                        && !ll.multi
                        && ll.label.span.contains(line.offset() + col)
                        })
                        // Prioritise displaying smaller spans
                        .min_by_key(|ll| (-ll.label.priority, ll.label.span.len()))
                };

                // Margin
                write_margin(
                    &mut w,
                    idx,
                    true,
                    is_ellipsis,
                    true,
                    None,
                    &line_labels,
                    &margin_label,
                )?;

                // Line
                if !is_ellipsis {
                    for (col, c) in line.chars().enumerate() {
                        let color = if let Some(highlight) = get_highlight(col) {
                            highlight.color
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
                write!(w, "\n")?;

                // Arrows
                for row in 0..line_labels.len() {
                    let line_label = &line_labels[row];

                    if !self.config.compact {
                        // Margin alternate
                        write_margin(
                            &mut w,
                            idx,
                            false,
                            is_ellipsis,
                            true,
                            Some((row, false)),
                            &line_labels,
                            &margin_label,
                        )?;
                        // Lines alternate
                        let mut chars = line.chars();
                        for col in 0..arrow_len {
                            let width =
                                chars.next().map_or(1, |c| self.config.char_width(c, col).1);

                            let vbar = get_vbar(col, row);
                            let underline = get_underline(col).filter(|_| row == 0);
                            let [c, tail] = if let Some(vbar_ll) = vbar {
                                let [c, tail] = if underline.is_some() {
                                    // TODO: Is this good?
                                    if vbar_ll.label.span.len() <= 1 || true {
                                        [draw.underbar, draw.underline]
                                    } else if line.offset() + col == vbar_ll.label.span.start() {
                                        [draw.ltop, draw.underbar]
                                    } else if line.offset() + col == vbar_ll.label.last_offset() {
                                        [draw.rtop, draw.underbar]
                                    } else {
                                        [draw.underbar, draw.underline]
                                    }
                                } else if vbar_ll.multi && row == 0 && self.config.multiline_arrows
                                {
                                    [draw.uarrow, ' ']
                                } else {
                                    [draw.vbar, ' ']
                                };
                                [
                                    c.fg(vbar_ll.label.color, s),
                                    tail.fg(vbar_ll.label.color, s),
                                ]
                            } else if let Some(underline_ll) = underline {
                                [draw.underline.fg(underline_ll.label.color, s); 2]
                            } else {
                                [' '.fg(None, s); 2]
                            };

                            for i in 0..width {
                                write!(w, "{}", if i == 0 { c } else { tail })?;
                            }
                        }
                        write!(w, "\n")?;
                    }

                    // Margin
                    write_margin(
                        &mut w,
                        idx,
                        false,
                        is_ellipsis,
                        true,
                        Some((row, true)),
                        &line_labels,
                        &margin_label,
                    )?;
                    // Lines
                    let mut chars = line.chars();
                    for col in 0..arrow_len {
                        let width = chars.next().map_or(1, |c| self.config.char_width(c, col).1);

                        let is_hbar = (((col > line_label.col) ^ line_label.multi)
                            || (line_label.label.msg.is_some()
                                && line_label.draw_msg
                                && col > line_label.col))
                            && line_label.label.msg.is_some();
                        let [c, tail] = if col == line_label.col
                            && line_label.label.msg.is_some()
                            && margin_label.as_ref().map_or(true, |m| {
                                line_label.label as *const _ != m.label as *const _
                            }) {
                            [
                                if line_label.multi {
                                    if line_label.draw_msg {
                                        draw.mbot
                                    } else {
                                        draw.rbot
                                    }
                                } else {
                                    draw.lbot
                                }
                                .fg(line_label.label.color, s),
                                draw.hbar.fg(line_label.label.color, s),
                            ]
                        } else if let Some(vbar_ll) = get_vbar(col, row)
                            .filter(|_| (col != line_label.col || line_label.label.msg.is_some()))
                        {
                            if !self.config.cross_gap && is_hbar {
                                [
                                    draw.xbar.fg(line_label.label.color, s),
                                    ' '.fg(line_label.label.color, s),
                                ]
                            } else if is_hbar {
                                [draw.hbar.fg(line_label.label.color, s); 2]
                            } else {
                                [
                                    if vbar_ll.multi && row == 0 && self.config.compact {
                                        draw.uarrow
                                    } else {
                                        draw.vbar
                                    }
                                    .fg(vbar_ll.label.color, s),
                                    ' '.fg(line_label.label.color, s),
                                ]
                            }
                        } else if is_hbar {
                            [draw.hbar.fg(line_label.label.color, s); 2]
                        } else {
                            [' '.fg(None, s); 2]
                        };

                        if width > 0 {
                            write!(w, "{}", c)?;
                        }
                        for _ in 1..width {
                            write!(w, "{}", tail)?;
                        }
                    }
                    if line_label.draw_msg {
                        write!(w, " {}", Show(line_label.label.msg.as_ref()))?;
                    }
                    write!(w, "\n")?;
                }
            }

            let is_final_group = group_idx + 1 == groups_len;

            // Help
            if let (Some(note), true) = (&self.help, is_final_group) {
                if !self.config.compact {
                    write_margin(&mut w, 0, false, false, true, Some((0, false)), &[], &None)?;
                    write!(w, "\n")?;
                }
                write_margin(&mut w, 0, false, false, true, Some((0, false)), &[], &None)?;
                write!(w, "{}: {}\n", "Help".fg(self.config.note_color(), s), note)?;
            }

            // Note
            if let (Some(note), true) = (&self.note, is_final_group) {
                if !self.config.compact {
                    write_margin(&mut w, 0, false, false, true, Some((0, false)), &[], &None)?;
                    write!(w, "\n")?;
                }
                write_margin(&mut w, 0, false, false, true, Some((0, false)), &[], &None)?;
                write!(w, "{}: {}\n", "Note".fg(self.config.note_color(), s), note)?;
            }

            // Tail of report
            if !self.config.compact {
                if is_final_group {
                    let final_margin =
                        format!("{}{}", Show((draw.hbar, line_no_width + 2)), draw.rbot);
                    writeln!(w, "{}", final_margin.fg(self.config.margin_color(), s))?;
                } else {
                    writeln!(
                        w,
                        "{}{}",
                        Show((' ', line_no_width + 2)),
                        draw.vbar.fg(self.config.margin_color(), s)
                    )?;
                }
            }
        }
        Ok(())
    }
}

impl<S: Span> Label<S> {
    fn last_offset(&self) -> usize {
        self.span.end().saturating_sub(1).max(self.span.start())
    }
}
