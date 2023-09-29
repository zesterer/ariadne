use super::*;

use std::{
    path::{Path, PathBuf},
    collections::{HashMap, hash_map::Entry},
    fs, mem::replace,
};

/// A trait implemented by [`Source`] caches.
pub trait Cache<Id: ?Sized> {
    /// Fetch the [`Source`] identified by the given ID, if possible.
    // TODO: Don't box
    fn fetch(&mut self, id: &Id) -> Result<&Source, Box<dyn fmt::Debug + '_>>;

    /// Display the given ID. as a single inline value.
    ///
    /// This function may make use of attributes from the [`Fmt`] trait.
    // TODO: Don't box
    fn display<'a>(&self, id: &'a Id) -> Option<Box<dyn fmt::Display + 'a>>;
}

impl<'b, C: Cache<Id>, Id: ?Sized> Cache<Id> for &'b mut C {
    fn fetch(&mut self, id: &Id) -> Result<&Source, Box<dyn fmt::Debug + '_>> { C::fetch(self, id) }
    fn display<'a>(&self, id: &'a Id) -> Option<Box<dyn fmt::Display + 'a>> { C::display(self, id) }
}

impl<C: Cache<Id>, Id: ?Sized> Cache<Id> for Box<C> {
    fn fetch(&mut self, id: &Id) -> Result<&Source, Box<dyn fmt::Debug + '_>> { C::fetch(self, id) }
    fn display<'a>(&self, id: &'a Id) -> Option<Box<dyn fmt::Display + 'a>> { C::display(self, id) }
}

/// A type representing a single line of a [`Source`].
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct Line {
    offset: usize,
    char_len: usize,
    byte_offset: usize,
    byte_len: usize,
}

impl Line {
    /// Get the offset of this line in the original [`Source`] (i.e: the number of characters that precede it).
    pub fn offset(&self) -> usize { self.offset }

    /// Get the character length of this line.
    pub fn len(&self) -> usize { self.char_len }

    /// Get the offset span of this line in the original [`Source`].
    pub fn span(&self) -> Range<usize> { self.offset..self.offset + self.char_len }

    /// Get the byte offset span of this line in the original [`Source`]. This can be used to
    /// directly slice into its source text.
    fn byte_span(&self) -> Range<usize> { self.byte_offset..self.byte_offset + self.byte_len }
}

/// A type representing a single source that may be referred to by [`Span`]s.
///
/// In most cases, a source is a single input file.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Source {
    text: String,
    lines: Vec<Line>,
    len: usize,
}

impl<S: AsRef<str>> From<S> for Source {
    /// Generate a [`Source`] from the given [`str`].
    ///
    /// Note that this function can be expensive for long strings. Use an implementor of [`Cache`] where possible.
    fn from(s: S) -> Self {
        let mut char_offset = 0;
        let mut byte_offset = 0;
        // (Last line, last line ends with CR)
        let mut last_line: Option<(Line, bool)> = None;
        let mut lines: Vec<Line> = s
            .as_ref()
            .split_inclusive([
                '\r', // Carriage return
                '\n', // Line feed
                '\x0B', // Vertical tab
                '\x0C', // Form feed
                '\u{0085}', // Next line
                '\u{2028}', // Line separator
                '\u{2029}' // Paragraph separator
            ])
            .flat_map(|line| {
                // Returns last line and set `last_line` to current `line`
                // A hack that makes `flat_map` deals with consecutive lines

                if let Some((last, ends_with_cr)) = last_line.as_mut() {
                    if *ends_with_cr && line == "\n" {
                        last.char_len += 1;
                        last.byte_len += 1;
                        char_offset += 1;
                        byte_offset += 1;
                        return replace(&mut last_line, None).map(|(l, _)| l);
                    }
                }

                let char_len = line.chars().count();
                let ends_with_cr = line.ends_with('\r');
                let line = Line {
                    offset: char_offset,
                    char_len,
                    byte_offset,
                    byte_len: line.len(),
                };
                char_offset += char_len;
                byte_offset += line.byte_len;
                replace(&mut last_line, Some((line, ends_with_cr))).map(|(l, _)| l)
            })
            .collect();

        if let Some((l, _)) = last_line {
            lines.push(l);
        }

        Self {
            text: s.as_ref().to_string(),
            lines,
            len: char_offset,
        }
    }
}

impl Source {
    /// Get the length of the total number of characters in the source.
    pub fn len(&self) -> usize { self.len }

    /// Return an iterator over the characters in the source.
    pub fn chars(&self) -> impl Iterator<Item = char> + '_ { self.text.chars() }

    /// Get access to a specific, zero-indexed [`Line`].
    pub fn line(&self, idx: usize) -> Option<Line> { self.lines.get(idx).copied() }

    /// Return an iterator over the [`Line`]s in this source.
    pub fn lines(&self) -> impl ExactSizeIterator<Item = Line> + '_ { self.lines.iter().copied() }

    /// Get the line that the given offset appears on, and the line/column numbers of the offset.
    ///
    /// Note that the line/column numbers are zero-indexed.
    pub fn get_offset_line(&self, offset: usize) -> Option<(Line, usize, usize)> {
        if offset <= self.len {
            let idx = self
                .lines
                .binary_search_by_key(&offset, |line| line.offset)
                .unwrap_or_else(|idx| idx.saturating_sub(1));
            let line = self.lines[idx];
            assert!(
                offset >= line.offset,
                "offset = {}, line.offset = {}",
                offset,
                line.offset
            );
            Some((line, idx, offset - line.offset))
        } else {
            None
        }
    }

    /// Get the range of lines that this span runs across.
    ///
    /// The resulting range is guaranteed to contain valid line indices (i.e: those that can be used for
    /// [`Source::line`]).
    pub fn get_line_range<S: Span>(&self, span: &S) -> Range<usize> {
        let start = self.get_offset_line(span.start()).map_or(0, |(_, l, _)| l);
        let end = self
            .get_offset_line(span.end().saturating_sub(1).max(span.start()))
            .map_or(self.lines.len(), |(_, l, _)| l + 1);
        start..end
    }

    /// Get the source text for a line, excluding trailing whitespace.
    pub fn get_line_text(&self, line: Line) -> Option<&'_ str> {
        self.text.get(line.byte_span()).map(|text| text.trim_end())
    }
}

impl Cache<()> for Source {
    fn fetch(&mut self, _: &()) -> Result<&Source, Box<dyn fmt::Debug + '_>> { Ok(self) }
    fn display(&self, _: &()) -> Option<Box<dyn fmt::Display>> { None }
}

impl<Id: fmt::Display + Eq> Cache<Id> for (Id, Source) {
    fn fetch(&mut self, id: &Id) -> Result<&Source, Box<dyn fmt::Debug + '_>> {
        if id == &self.0 { Ok(&self.1) } else { Err(Box::new(format!("Failed to fetch source '{}'", id))) }
    }
    fn display<'a>(&self, id: &'a Id) -> Option<Box<dyn fmt::Display + 'a>> { Some(Box::new(id)) }
}

/// A [`Cache`] that fetches [`Source`]s from the filesystem.
#[derive(Default, Debug, Clone)]
pub struct FileCache {
    files: HashMap<PathBuf, Source>,
}

impl Cache<Path> for FileCache {
    fn fetch(&mut self, path: &Path) -> Result<&Source, Box<dyn fmt::Debug + '_>> {
        Ok(match self.files.entry(path.to_path_buf()) { // TODO: Don't allocate here
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(Source::from(&fs::read_to_string(path).map_err(|e| Box::new(e) as _)?)),
        })
    }
    fn display<'a>(&self, path: &'a Path) -> Option<Box<dyn fmt::Display + 'a>> { Some(Box::new(path.display())) }
}

/// A [`Cache`] that fetches [`Source`]s using the provided function.
#[derive(Debug, Clone)]
pub struct FnCache<Id, F> {
    sources: HashMap<Id, Source>,
    get: F,
}

impl<Id, F> FnCache<Id, F> {
    /// Create a new [`FnCache`] with the given fetch function.
    pub fn new(get: F) -> Self {
        Self {
            sources: HashMap::default(),
            get,
        }
    }

    /// Pre-insert a selection of [`Source`]s into this cache.
    pub fn with_sources(mut self, sources: HashMap<Id, Source>) -> Self
        where Id: Eq + Hash
    {
        self.sources.reserve(sources.len());
        for (id, src) in sources {
            self.sources.insert(id, src);
        }
        self
    }
}

impl<Id: fmt::Display + Hash + PartialEq + Eq + Clone, F> Cache<Id> for FnCache<Id, F>
    where F: for<'a> FnMut(&'a Id) -> Result<String, Box<dyn fmt::Debug>>
{
    fn fetch(&mut self, id: &Id) -> Result<&Source, Box<dyn fmt::Debug + '_>> {
        Ok(match self.sources.entry(id.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(Source::from((self.get)(id)?)),
        })
    }
    fn display<'a>(&self, id: &'a Id) -> Option<Box<dyn fmt::Display + 'a>> { Some(Box::new(id)) }
}

/// Create a [`Cache`] from a collection of ID/strings, where each corresponds to a [`Source`].
pub fn sources<Id, S, I>(iter: I) -> impl Cache<Id>
where
    Id: fmt::Display + Hash + PartialEq + Eq + Clone + 'static,
    I: IntoIterator<Item = (Id, S)>,
    S: AsRef<str>,
{
    FnCache::new((move |id| Err(Box::new(format!("Failed to fetch source '{}'", id)) as _)) as fn(&_) -> _)
        .with_sources(iter
            .into_iter()
            .map(|(id, s)| (id, Source::from(s.as_ref())))
            .collect())
}

#[cfg(test)]
mod tests {
    use std::iter::zip;

    use super::Source;

    fn test_from(lines: Vec<&str>) {
        let source: String = lines.iter().map(|s| *s).collect();
        let source = Source::from(source);

        assert_eq!(source.lines.len(), lines.len());

        let mut offset = 0;
        for (source_line, raw_line) in zip(source.lines.iter().copied(), lines.into_iter()) {
            // dbg!(source_line, &raw_line[source_line.byte_span()]);
            assert_eq!(source_line.offset, offset);
            assert_eq!(source_line.char_len, raw_line.chars().count());
            assert_eq!(
                source.get_line_text(source_line).unwrap(),
                raw_line.trim_end()
            );
            offset += source_line.char_len;
        }

        assert_eq!(source.len, offset);
    }

    #[test]
    fn source_from_empty() {
        test_from(vec![]); // Empty string
    }

    #[test]
    fn source_from_single() {
        test_from(vec!["Single line"]);
        test_from(vec!["Single line with LF\n"]);
        test_from(vec!["Single line with CRLF\r\n"]);
    }

    #[test]
    fn source_from_multi() {
        test_from(vec!["Two\r\n", "lines\n"]);
        test_from(vec!["Some\n", "more\r\n", "lines"]);
        test_from(vec!["\n", "\r\n", "\n", "Empty Lines"]);
    }

    #[test]
    fn source_from_trims_trailing_spaces() {
        test_from(vec!["Trailing spaces  \n", "are trimmed\t"]);
    }

    #[test]
    fn source_from_alternate_line_endings() {
        // Line endings other than LF or CRLF
        test_from(vec![
            "CR\r",
            "VT\x0B",
            "FF\x0C",
            "NEL\u{0085}",
            "LS\u{2028}",
            "PS\u{2029}",
        ]);
    }
}
