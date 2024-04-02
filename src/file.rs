use super::*;

pub trait Files<'a, K> {
    type Cache;
    type Error;

    fn init_cache(&self) -> Self::Cache;
    fn fetch_filename<'b>(
        &'b self,
        cache: &'b mut Self::Cache,
        file_id: &K,
    ) -> Result<Option<&'b str>, Self::Error>;
    fn fetch_file<'b>(
        &'b self,
        cache: &'b mut Self::Cache,
        file_id: &K,
    ) -> Result<Cow<'b, File<'a>>, Self::Error>;
}

impl<'a, F: Files<'a, K>, K> Files<'a, K> for &'a mut F {
    type Cache = F::Cache;
    type Error = F::Error;

    fn init_cache(&self) -> Self::Cache {
        (**self).init_cache()
    }
    fn fetch_filename<'b>(
        &'b self,
        cache: &'b mut Self::Cache,
        file_id: &K,
    ) -> Result<Option<&'b str>, Self::Error> {
        (**self).fetch_filename(cache, file_id)
    }
    fn fetch_file<'b>(
        &'b self,
        cache: &'b mut Self::Cache,
        file_id: &K,
    ) -> Result<Cow<'b, File<'a>>, Self::Error> {
        (**self).fetch_file(cache, file_id)
    }
}

impl<'a> Files<'a, ()> for &'a str {
    type Cache = File<'a>;
    type Error = core::convert::Infallible;

    fn init_cache(&self) -> Self::Cache {
        File::new(*self)
    }
    fn fetch_filename<'b>(
        &'b self,
        _: &'b mut Self::Cache,
        _: &(),
    ) -> Result<Option<&'b str>, Self::Error> {
        Ok(None)
    }
    fn fetch_file<'b>(
        &'b self,
        cache: &'b mut Self::Cache,
        _: &(),
    ) -> Result<Cow<'b, File<'a>>, Self::Error> {
        Ok(Cow::Borrowed(cache))
    }
}

struct FileFetcher<I, F, G> {
    init_cache: I,
    fetch_filename: F,
    fetch_file: G,
}

impl<'a, K, I, F, G, E, S> Files<'a, K> for FileFetcher<I, F, G>
where
    I: Fn() -> S,
    F: for<'b> Fn(&'b mut S, &K) -> Result<&'b str, E>,
    G: for<'b> Fn(&'b mut S, &K) -> Result<Cow<'b, File<'a>>, E>,
{
    type Cache = S;
    type Error = E;

    fn init_cache(&self) -> Self::Cache {
        (self.init_cache)()
    }
    fn fetch_filename<'b>(
        &'b self,
        cache: &'b mut Self::Cache,
        file_id: &K,
    ) -> Result<Option<&'b str>, Self::Error> {
        (self.fetch_filename)(cache, file_id).map(Some)
    }
    fn fetch_file<'b>(
        &'b self,
        cache: &'b mut Self::Cache,
        file_id: &K,
    ) -> Result<Cow<'b, File<'a>>, Self::Error> {
        (self.fetch_file)(cache, file_id)
    }
}

pub fn files<'a, K, N, V>(files: impl IntoIterator<Item = (K, N, V)>) -> impl Files<'a, K>
where
    K: Eq + core::hash::Hash + Ord,
    N: Borrow<str> + Clone,
    V: Into<Cow<'a, str>>,
{
    use alloc::{collections::BTreeMap, rc::Rc};

    let files = Rc::new(
        files
            .into_iter()
            .map(|(k, fname, content)| (k, (fname, File::new(content))))
            .collect::<BTreeMap<_, _>>(),
    );

    fn fetch_filename<'a, 'b, K: Eq + core::hash::Hash + Ord, N: Borrow<str>>(
        files: &'b mut Rc<BTreeMap<K, (N, File<'a>)>>,
        key: &K,
    ) -> Result<&'b str, &'static str> {
        files
            .get(key)
            .map(|(name, _)| name.borrow())
            .ok_or("no such file")
    }

    fn fetch_file<'a, 'b, K: Eq + core::hash::Hash + Ord, N: Borrow<str>>(
        files: &'b mut Rc<BTreeMap<K, (N, File<'a>)>>,
        key: &K,
    ) -> Result<Cow<'b, File<'a>>, &'static str> {
        files
            .get(key)
            .map(|(_, file)| Cow::Owned(file.clone()))
            .ok_or("no such file")
    }

    FileFetcher {
        init_cache: move || files.clone(),
        fetch_filename,
        fetch_file,
    }
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub(crate) struct Point {
    pub(crate) line: usize,   // Zero-indexed line in a file
    pub(crate) offset: usize, // Byte offset within the given line
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct Run {
    pub(crate) start: Point,
    pub(crate) end: Point,
}

#[derive(Clone)]
pub struct File<'a> {
    content: Cow<'a, str>,
    // (bytes, chars)
    lines: Vec<(Range<usize>, Range<usize>)>,
}

impl<'a> File<'a> {
    pub fn new<C>(content: C) -> Self
    where
        C: Into<Cow<'a, str>>,
    {
        let content = content.into();

        let mut lines = Vec::new();
        let mut line_start = (0, 0);
        let mut char_offset = 0;
        let mut was_newline = false;
        for (byte_offset, c) in content.char_indices() {
            // If the last-seen character was a newline, push a new line
            if was_newline {
                lines.push((line_start.0..byte_offset, line_start.0..char_offset));
                line_start = (byte_offset, char_offset);
            }

            // Note: We can ignore `\r` here because it always ends up being coupled with `\n` to count as a newline anyway.
            was_newline = c == '\n';
            char_offset += 1;
        }

        // Push whatever is left as the final line
        lines.push((line_start.0..content.len(), line_start.0..char_offset));

        Self { lines, content }
    }

    pub fn line(&self, idx: usize) -> Option<&str> {
        self.lines
            .get(idx)
            .map(|(bytes, _)| &self.content[bytes.clone()])
    }

    fn line_of_byte(&self, byte_offset: usize) -> Option<(usize, &str)> {
        let idx = self
            .lines
            .binary_search_by_key(&byte_offset, |(bytes, _)| bytes.end)
            .unwrap_or_else(core::convert::identity);
        self.line(idx).map(|s| (idx, s))
    }

    fn line_of_char(&self, char_offset: usize) -> Option<(usize, &str)> {
        let idx = self
            .lines
            .binary_search_by_key(&char_offset, |(_, chars)| chars.end)
            .unwrap_or_else(core::convert::identity);
        self.line(idx).map(|s| (idx, s))
    }

    fn offset_to_point(&self, offset: Offset, is_start: bool) -> Point {
        match offset {
            Offset::Byte(byte_offset) => self
                .line_of_byte(byte_offset + is_start as usize)
                .map(|(line, s)| Point {
                    line,
                    offset: {
                        let offset = byte_offset - self.lines[line].0.start;
                        assert!(s.is_char_boundary(offset), "byte offset {byte_offset} does not fall on a UTF-8 character boundary");
                        offset
                    },
                })
                .unwrap_or_else(|| panic!("byte offset {byte_offset} is greater than the length of the file, {}", self.content.len())),
            Offset::Char(char_offset) => self
                .line_of_char(char_offset + is_start as usize)
                .and_then(|(line, s)| Some(Point {
                    line,
                    offset: s
                        .char_indices()
                        // We allow offsets to be one past the last character on each line
                        .chain(core::iter::once((s.len(), '\0')))
                        .skip(char_offset - self.lines[line].1.start)
                        .next()?.0,
                }))
                .unwrap_or_else(|| panic!("char offset {char_offset} is greater than the number of characters in the file, {}", self.lines.last().unwrap().1.end)),
        }
    }

    pub(crate) fn offsets_to_run(&self, offsets: &Range<Offset>) -> Run {
        let run = Run {
            start: self.offset_to_point(offsets.start, true),
            end: self.offset_to_point(offsets.end, false),
        };
        assert!(
            run.end >= run.start,
            "end offset {:?} should be greater than or equal to {:?}, but isn't",
            offsets.end,
            offsets.start
        );
        run
    }

    pub(crate) fn lines_of(&self, run: Run) -> impl ExactSizeIterator<Item = (usize, &str)> {
        (run.start.line..(run.end.line + 1)).map(|i| (i, self.line(i).unwrap()))
    }
}

pub trait FileId: Ord + Eq + core::hash::Hash {}
impl<T: Ord + Eq + core::hash::Hash> FileId for T {}
