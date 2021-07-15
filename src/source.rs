use super::*;

use std::{
    path::{Path, PathBuf},
    collections::{HashMap, hash_map::Entry},
    fs,
};

pub trait Cache<Id: ?Sized> {
    // TODO: Don't box
    fn fetch(&mut self, id: &Id) -> Result<&Source, Box<dyn fmt::Debug>>;

    /// Display the given ID.
    // TODO: Don't box
    fn display<'a>(&self, id: &'a Id) -> Option<Box<dyn fmt::Display + 'a>>;
}

pub struct Line {
    offset: usize,
    len: usize,
    chars: String,
}

impl Line {
    pub fn offset(&self) -> usize { self.offset }
    pub fn len(&self) -> usize { self.len }
    pub fn span(&self) -> Range<usize> { self.offset..self.offset + self.len }

    /// The chars of the line, excluding trailing whitespace.
    pub fn chars(&self) -> impl Iterator<Item = char> + '_ { self.chars.chars() }
}

pub struct Source {
    lines: Vec<Line>,
    len: usize,
}

impl<S: AsRef<str>> From<S> for Source {
    fn from(s: S) -> Self {
        let mut offset = 0;
        Self {
            lines: s
                .as_ref()
                .lines()
                .map(|line| {
                    let l = Line {
                        offset,
                        len: line.chars().count() + 1, // TODO: Don't assume all newlines are a single character!
                        chars: line.trim_end().to_owned(),
                    };
                    offset += l.len;
                    l
                })
                .collect(),
            len: offset,
        }
    }
}

impl Source {
    pub fn len(&self) -> usize { self.len }

    pub fn chars(&self) -> impl Iterator<Item = char> + '_ {
        self.lines.iter().map(|l| l.chars()).flatten()
    }

    pub fn line(&self, idx: usize) -> Option<&Line> { self.lines.get(idx) }

    pub fn lines(&self) -> impl ExactSizeIterator<Item = &Line> + '_ { self.lines.iter() }

    /// Get the line that the given offset appears on, and the line/column numbers of the offset.
    ///
    /// Note that the line/column numbers are zero-indexed.
    pub fn get_offset_line(&self, offset: usize) -> Option<(&Line, usize, usize)> {
        if offset <= self.len {
            let idx = self.lines
                .binary_search_by_key(&offset, |line| line.offset)
                .unwrap_or_else(|idx| idx.saturating_sub(1));
            let line = &self.lines[idx];
            assert!(offset >= line.offset, "offset = {}, line.offset = {}", offset, line.offset);
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
        let end = self.get_offset_line(span.end().saturating_sub(1).max(span.start())).map_or(self.lines.len(), |(_, l, _)| l + 1);
        start..end
    }
}

impl Cache<()> for Source {
    fn fetch(&mut self, _: &()) -> Result<&Source, Box<dyn fmt::Debug>> { Ok(self) }
    fn display(&self, _: &()) -> Option<Box<dyn fmt::Display>> { None }
}

pub struct FileCache {
    files: HashMap<PathBuf, Source>,
}

impl Default for FileCache {
    fn default() -> Self {
        Self { files: HashMap::default() }
    }
}

impl Cache<Path> for FileCache {
    fn fetch(&mut self, path: &Path) -> Result<&Source, Box<dyn fmt::Debug>> {
        Ok(match self.files.entry(path.to_path_buf()) { // TODO: Don't allocate here
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(Source::from(&fs::read_to_string(path).map_err(|e| Box::new(e) as _)?)),
        })
    }
    fn display<'a>(&self, path: &'a Path) -> Option<Box<dyn fmt::Display + 'a>> { Some(Box::new(path.display())) }
}

pub struct FnCache<Id, F> {
    sources: HashMap<Id, Source>,
    get: F,
}

impl<Id, F> FnCache<Id, F> {
    pub fn new(get: F) -> Self {
        Self {
            sources: HashMap::default(),
            get,
        }
    }

    pub fn with_sources(mut self, sources: HashMap<Id, Source>) -> Self {
        self.sources = sources;
        self
    }
}

impl<Id: fmt::Display + Hash + PartialEq + Eq + Clone, F> Cache<Id> for FnCache<Id, F>
    where F: for<'a> FnMut(&'a Id) -> Result<String, Box<dyn fmt::Debug>>
{
    fn fetch(&mut self, id: &Id) -> Result<&Source, Box<dyn fmt::Debug>> {
        Ok(match self.sources.entry(id.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(Source::from((self.get)(id)?)),
        })
    }
    fn display<'a>(&self, id: &'a Id) -> Option<Box<dyn fmt::Display + 'a>> { Some(Box::new(id)) }
}

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

