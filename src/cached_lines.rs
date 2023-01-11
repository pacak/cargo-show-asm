use line_span::LineSpans;
use std::ops::{Index, Range};

pub struct CachedLines {
    pub content: String,
    pub splits: Vec<Range<usize>>,
}

impl CachedLines {
    #[must_use]
    pub fn without_ending(content: String) -> Self {
        let splits = content.line_spans().map(|s| s.range()).collect::<Vec<_>>();
        Self { content, splits }
    }

    pub fn iter(&self) -> LineIter {
        LineIter {
            payload: self,
            current: 0,
        }
    }
    pub fn get(&self, index: usize) -> Option<&str> {
        let range = self.splits.get(index)?.clone();
        Some(&self.content[range])
    }
}

impl Index<usize> for CachedLines {
    type Output = str;

    fn index(&self, index: usize) -> &Self::Output {
        &self.content[self.splits[index].clone()]
    }
}

pub struct LineIter<'a> {
    payload: &'a CachedLines,
    current: usize,
}

impl<'a> IntoIterator for &'a CachedLines {
    type Item = &'a str;

    type IntoIter = LineIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> Iterator for LineIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        self.current += 1;
        self.payload.get(self.current - 1)
    }
}
