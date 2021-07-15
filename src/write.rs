use super::*;

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

impl<S: Span> Report<S> {
    fn get_source_groups(&self, cache: &mut impl Cache<S::SourceId>) -> Vec<SourceGroup<S>> {
        let mut groups = Vec::new();
        for label in self.labels.iter() {
            let src = match cache.fetch(label.span.source()) {
                Ok(src) => src,
                Err(e) => {
                    eprintln!("Unable to fetch source {:?}: {:?}", label.span.source(), e);
                    continue;
                },
            };

            let start_line = src.get_offset_line(label.span.start()).map(|(_, l, _)| l);
            let end_line = src.get_offset_line(label.span.end().saturating_sub(1).max(label.span.start())).map(|(_, l, _)| l);

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
    /// If you wish to write to `stderr` or `stdout`, you can do so via [`Report::eprint`] or [`Report::print`] respectively.
    pub fn write<C: Cache<S::SourceId>, W: Write>(&self, mut cache: C, mut w: W) -> io::Result<()> {
        let draw = match self.config.char_set {
            CharSet::Unicode => draw::Characters::unicode(),
            CharSet::Ascii => draw::Characters::ascii(),
            CharSet::ExtendedAscii => draw::Characters::extended_ascii(),
        };

        // --- Header ---

        let code = self.code.map(|c| format!("[{}{:02}] ", self.kind.letter(), c));
        let id = format!("{}{}:", Show(code), self.kind);
        let kind_color = match self.kind {
            ReportKind::Error => self.config.error_color(),
            ReportKind::Warning => self.config.warning_color(),
            ReportKind::Advice => self.config.advice_color(),
        };
        writeln!(w, "{} {}", id.fg(kind_color), Show(self.msg.as_ref()))?;

        // --- Source sections ---

        let groups = self.get_source_groups(&mut cache);
        let groups_len = groups.len();
        for (group_idx, SourceGroup { src_id, span, labels }) in groups.into_iter().enumerate() {
            let src_name = cache
                .display(src_id)
                .map(|d| d.to_string())
                .unwrap_or_else(|| "<unknown>".to_string());

            let src = match cache.fetch(src_id) {
                Ok(src) => src,
                Err(e) => {
                    eprintln!("Unable to fetch source {}: {:?}", src_name, e);
                    continue;
                },
            };

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
                "    {}{}{}{}{}{}",
                if group_idx == 0 { draw.ltop } else { draw.lcross }.fg(self.config.margin_color()),
                draw.hbar.fg(self.config.margin_color()),
                draw.lbox.fg(self.config.margin_color()),
                src_name,
                line_ref,
                draw.rbox.fg(self.config.margin_color()),
            )?;

            if !self.config.compact {
                writeln!(w, "    {}", draw.vbar.fg(self.config.margin_color()))?;
            }

            let line_range = src.get_line_range(&span);

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

            let write_margin = |w: &mut W, idx: usize, is_line: bool, report_row: Option<(usize, bool)>, line_labels: &[LineLabel<S>]| -> std::io::Result<()> {
                let line_no_margin = if is_line {
                    let line_no = format!("{:>3}", idx + 1);
                    format!("{} {} ", line_no, draw.vbar)
                } else {
                    format!("    {} ", draw.vbar_break)
                };

                write!(w, "{}", line_no_margin.fg(self.config.margin_color()))?;

                // Multi-line margins
                for col in 0..multi_labels.len() {
                    let mut corner = None;
                    let mut hbar = None;
                    let mut vbar = None;

                    let line_span = src.line(idx).unwrap().span();

                    for (i, label) in multi_labels[0..=col].iter().enumerate() {
                        if label.span.start() <= line_span.end && label.span.end() > line_span.start {
                            let is_parent = i != col;
                            let is_start = line_span.contains(&label.span.start());
                            let is_end = line_span.contains(&label.last_offset());

                            if !is_start && (!is_end || is_line) {
                                vbar = vbar.or(Some(label).filter(|_| !is_parent));
                            } else if let Some((report_row, is_arrow)) = report_row {
                                let label_row = line_labels
                                    .iter()
                                    .enumerate()
                                    .find(|(_, l)| **label as *const _ == l.label as *const _)
                                    .map_or(0, |(r, _)| r);
                                if report_row == label_row {
                                    if is_arrow {
                                        hbar = Some(label);
                                        if !is_parent {
                                            corner = Some((label, is_start));
                                        }
                                    } else if !is_start {
                                        vbar = vbar.or(Some(label).filter(|_| !is_parent));
                                    }
                                } else {
                                    vbar = vbar.or(Some(label).filter(|_| !is_parent && (is_start ^ (report_row < label_row))));
                                }
                            }
                        }
                    }

                    let (a, b) = if let Some((label, is_start)) = corner {
                        (if is_start { draw.ltop } else { draw.lbot }.fg(label.color), draw.hbar.fg(label.color))
                    } else if let Some(label) = hbar.filter(|_| vbar.is_some() && !self.config.cross_gap) {
                        (draw.xbar.fg(label.color), draw.hbar.fg(label.color))
                    } else if let Some(label) = hbar {
                        (draw.hbar.fg(label.color), draw.hbar.fg(label.color))
                    } else if let Some(label) = vbar {
                        (draw.vbar.fg(label.color), ' '.fg(None))
                    } else {
                        (' '.fg(None), ' '.fg(None))
                    };
                    write!(w, "{}", a)?;
                    if !self.config.compact {
                        write!(w, "{}", b)?;
                    }
                }

                Ok(())
            };

            for idx in line_range {
                let line = src.line(idx).unwrap();

                // Generate a list of labels for this line, along with their label columns
                let mut line_labels = multi_labels
                    .iter()
                    .filter_map(|label| {
                        let is_start = line.span().contains(&label.span.start());
                        let is_end = line.span().contains(&label.last_offset());
                        if is_start {
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
                for label_info in labels
                    .iter()
                    .filter(|l| l.label.span.start() >= line.span().start && l.label.span.end() <= line.span().end)
                {
                    if matches!(label_info.kind, LabelKind::Inline) {
                        line_labels.push(LineLabel {
                            col: match &self.config.label_attach {
                                LabelAttach::Start => label_info.label.span.start(),
                                LabelAttach::Middle => (label_info.label.span.start() + label_info.label.span.end()) / 2,
                                LabelAttach::End => label_info.label.last_offset(),
                            } - line.offset(),
                            label: label_info.label,
                            multi: false,
                            draw_msg: true,
                        });
                    }
                }

                // Skip this line if we don't have labels for it
                if line_labels.len() == 0 { continue; }

                // Sort the labels by their columns
                line_labels.sort_by_key(|ll| (ll.label.order, !ll.multi, ll.col));

                // Determine label bounds so we know where to put error messages
                let arrow_end_space = if self.config.compact { 1 } else { 2 };
                let arrow_len = line_labels
                    .iter()
                    .fold(0, |l, ll| if ll.multi {
                        line.len()
                    } else {
                        l.max(ll.label.span.end() - line.offset())
                    }) + arrow_end_space;

                // Should we draw a vertical bar as part of a label arrow on this line?
                let get_vbar = |col, row| line_labels
                    .iter()
                    // Only labels with notes get an arrow
                    .filter(|ll| ll.label.msg.is_some())
                    .enumerate()
                    .find(|(j, ll)| ll.col == col && ((row <= *j && !ll.multi) || (row <= *j && ll.multi)))
                    .map(|(_, ll)| ll);

                let get_highlight = |col| line_labels
                    .iter()
                    .filter(|ll| ll.label.span.contains(line.offset() + col))
                    // Prioritise displaying smaller spans
                    .min_by_key(|ll| (-ll.label.priority, ll.label.span.len()));

                let get_underline = |col| line_labels
                    .iter()
                    .filter(|ll| self.config.underlines
                        // Underlines only occur for inline spans (highlighting can occur for all spans)
                        && !ll.multi
                        && ll.label.span.contains(line.offset() + col))
                    // Prioritise displaying smaller spans
                    .min_by_key(|ll| (-ll.label.priority, ll.label.span.len()));

                // Margin
                write_margin(&mut w, idx, true, None, &line_labels)?;

                // Line
                for (col, c) in line.chars().enumerate() {
                    let color = if let Some(highlight) = get_highlight(col) {
                        highlight.label.color
                    } else {
                        self.config.unimportant_color()
                    };
                    let (c, width) = self.config.char_width(c, col);
                    for _ in 0..width {
                        write!(w, "{}", c.fg(color))?;
                    }
                }
                write!(w, "\n")?;

                // Arrows
                for row in 0..line_labels.len() {
                    let line_label = &line_labels[row];

                    if !self.config.compact {
                        // Margin alternate
                        write_margin(&mut w, idx, false, Some((row, false)), &line_labels)?;
                        // Lines alternate
                        let mut chars = line.chars();
                        for col in 0..arrow_len {
                            let width = chars.next().map_or(1, |c| self.config.char_width(c, col).1);

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
                                } else if vbar_ll.multi && row == 0 && self.config.multiline_arrows {
                                    [draw.uarrow, ' ']
                                } else {
                                    [draw.vbar, ' ']
                                };
                                [c.fg(vbar_ll.label.color), tail.fg(vbar_ll.label.color)]
                            } else if let Some(underline_ll) = underline {
                                [draw.underline.fg(underline_ll.label.color); 2]
                            } else {
                                [' '.fg(None); 2]
                            };

                            for i in 0..width {
                                write!(w, "{}", if i == 0 { c } else { tail })?;
                            }
                        }
                        write!(w, "\n")?;
                    }

                    // Margin
                    write_margin(&mut w, idx, false, Some((row, true)), &line_labels)?;
                    // Lines
                    let mut chars = line.chars();
                    for col in 0..arrow_len {
                        let width = chars.next().map_or(1, |c| self.config.char_width(c, col).1);

                        let is_hbar = ((col > line_label.col) ^ line_label.multi || (col > line_label.col && line_label.draw_msg))
                            && line_label.label.msg.is_some();
                        let c = if col == line_label.col && line_label.label.msg.is_some() {
                            if line_label.multi {
                                if line_label.draw_msg { draw.mbot } else { draw.rbot }
                            } else {
                                draw.lbot
                            }.fg(line_label.label.color)
                        } else if let Some(vbar_ll) = get_vbar(col, row).filter(|_| (col != line_label.col || line_label.label.msg.is_some())) {
                            if !self.config.cross_gap && is_hbar { draw.xbar } else { draw.vbar }.fg(vbar_ll.label.color)
                        } else if is_hbar {
                            draw.hbar.fg(line_label.label.color)
                        } else {
                            ' '.fg(None)
                        };

                        for _ in 0..width {
                            write!(w, "{}", c)?;
                        }
                    }
                    if line_label.draw_msg {
                        write!(w, " {}", Show(line_label.label.msg.as_ref()))?;
                    }
                    write!(w, "\n")?;
                }
            }

            let is_final_group = group_idx + 1 == groups_len;

            // Note
            if let (Some(note), true) = (&self.note, is_final_group) {
                if !self.config.compact {
                    write_margin(&mut w, 0, false, Some((0, false)), &[])?;
                    write!(w, "\n")?;
                }
                write_margin(&mut w, 0, false, Some((0, false)), &[])?;
                write!(w, "{}: {}\n", "Note".fg(self.config.note_color()), note)?;
            }

            // Tail of report
            if !self.config.compact {
                if is_final_group {
                    let final_margin = format!("{}{}{}{}{}", draw.hbar, draw.hbar, draw.hbar, draw.hbar, draw.rbot);
                    writeln!(w, "{}", final_margin.fg(self.config.margin_color()))?;
                } else {
                    writeln!(w, "    {}", draw.vbar.fg(self.config.margin_color()))?;
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
