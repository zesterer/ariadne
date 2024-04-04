use super::*;

pub(crate) struct MultilineLayout<'a, K> {
    // An ordered identity of the multiline span with respect to all multiline spans in the file
    pub(crate) file_idx: usize,
    // An ordered identity of the multiline span, guaranteed to not conflict with multiline spans covering the same line
    pub(crate) line_idx: Option<usize>,
    pub(crate) run: Run,
    pub(crate) label: &'a Label<K>,
}

pub(crate) struct LineLayout<'a, K> {
    pub(crate) idx: usize,
    pub(crate) inline: Vec<(Run, &'a Label<K>)>,
    // Index to be looked up in `FileLayout.multilines`
    pub(crate) multiline: Vec<usize>,
}

pub(crate) struct FileLayout<'a, K> {
    pub(crate) lines: Vec<LineLayout<'a, K>>,
    pub(crate) multilines: Vec<MultilineLayout<'a, K>>,
    pub(crate) max_multiline_nesting: usize,
}

impl<'a, K> FileLayout<'a, K> {
    pub(crate) fn new(labels: impl IntoIterator<Item = (Run, &'a Label<K>)>) -> Self {
        let mut inline = Vec::new();
        let mut multiline = Vec::new();

        for (run, label) in labels {
            if run.start.line == run.end.line {
                inline.push((run, label));
            } else {
                multiline.push((run, label));
            }
        }

        // Multiline spans have a canonical ordering according to number of lines they cover
        // TODO: Is there an ordering that makes more sense and results in less line-crossing?
        multiline.sort_by_key(|(r, _)| !0 - (r.end.line - r.start.line));

        let mut slots = BTreeMap::<_, usize>::new();

        let mut multilines = multiline
            .iter()
            .enumerate()
            .map(|(file_idx, (r, l))| MultilineLayout {
                file_idx,
                line_idx: None,
                run: *r,
                label: *l,
            })
            .collect::<Vec<_>>();

        // Find the set of lines that have an inline or the ends of a multiline span on them
        // TODO: Integrate some notion of padding/context space so that additional lines can be shown
        let mut lines = inline
            .iter()
            .map(|(r, _)| r.start.line)
            .chain(
                multiline
                    .iter()
                    .flat_map(|(r, _)| [r.start.line, r.end.line]),
            )
            .map(|idx| LineLayout {
                idx,
                inline: inline
                    .iter()
                    .filter(|(r, _)| r.start.line == idx)
                    .copied()
                    .collect(),
                multiline: multilines
                    .iter_mut()
                    .filter(|ml| (ml.run.start.line..=ml.run.end.line).contains(&idx))
                    .map(|ml| {
                        // Find an idx that is consistent across the span, but that reuses the indices of non-intersecting multiline spans
                        // TODO: Don't do this per-line, choose this value once
                        if ml.line_idx.is_none() {
                            ml.line_idx = Some(if let Some(line_idx) = slots.get(&ml.file_idx) {
                                *line_idx
                            } else {
                                // Find a free idx or override a stale idx
                                let mut i = 0;
                                loop {
                                    if let Some(ml) = slots.get(&i) {
                                        if multiline[*ml].0.end.line < idx {
                                            slots.remove(&i);
                                        }
                                    }

                                    if !slots.contains_key(&i) {
                                        slots.insert(i, ml.file_idx);
                                        break i;
                                    }

                                    i += 1;
                                }
                            });
                        }

                        ml.file_idx
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        // Ensure that every line appears at most once, and in-order
        lines.sort_by_key(|l| l.idx);
        lines.dedup_by_key(|l| l.idx);

        // Find maximum number of multiline spans that intersect with any one line
        let max_multiline_nesting = lines.iter().map(|l| l.multiline.len()).max().unwrap_or(0);

        Self {
            lines,
            multilines,
            max_multiline_nesting,
        }
    }
}