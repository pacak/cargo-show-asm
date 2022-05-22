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
}

impl Index<usize> for CachedLines {
    type Output = str;

    fn index(&self, index: usize) -> &Self::Output {
        &self.content[self.splits[index].clone()]
    }
}
