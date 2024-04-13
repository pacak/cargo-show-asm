#![doc = include_str!("../README.md")]

use opts::{Format, NameDisplay, ToDump};
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
    path::{Path, PathBuf},
};

pub mod asm;
pub mod cached_lines;
pub mod demangle;
pub mod llvm;
pub mod mca;
pub mod mir;
pub mod opts;

#[macro_export]
macro_rules! color {
    ($item:expr, $color:expr) => {
        owo_colors::OwoColorize::if_supports_color(&$item, owo_colors::Stream::Stdout, $color)
    };
}

/// Safe version of `print[ln]!` macro
/// By default `print[ln]!` macro panics when print fails. Usually print fails when output
/// stream is disconnected, for purposes of this application disconnected stream means output
/// was piped somewhere and this something was terminated before printing completed.
///
/// At this point we might as well exit
#[macro_export]
macro_rules! safeprintln {
    ($($x:expr),* $(,)?) => {{
        use std::io::Write;
        if writeln!(std::io::stdout(), $($x),*).is_err() {
            std::process::exit(0);
        }
    }};
}

#[macro_export]
macro_rules! safeprint {
    ($($x:expr),* $(,)?) => {{
        use std::io::Write;
        if write!(std::io::stdout(), $($x),*).is_err() {
            std::process::exit(0);
        }
    }};
}

#[macro_export]
macro_rules! esafeprintln {
    ($($x:expr),* $(,)?) => {{
        use std::io::Write;
        if writeln!(std::io::stderr(), $($x),*).is_err() {
            std::process::exit(0);
        }
    }};
}

#[macro_export]
macro_rules! esafeprint {
    ($($x:expr),* $(,)?) => {{
        use std::io::Write;
        if write!(std::io::stderr(), $($x),*).is_err() {
            std::process::exit(0);
        }
    }};
}

/// read a set of source files to a set of strings
///
/// perform lossy conversion to utf8
pub fn read_sources(names: &[PathBuf]) -> anyhow::Result<Vec<String>> {
    names
        .iter()
        .map(|name| {
            let bytes = std::fs::read(name)?;
            // For some reason llvm/rustc can produce non utf8 files...
            // Also there's no (without unsafe) way to reuse allocation
            // from bytes in resulting String...
            Ok(String::from_utf8_lossy(&bytes).into_owned())
        })
        .collect()
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Item {
    // name and hashed MUST be first two fields - they are
    // used to produce correct Ord/PartialOrd
    /// demangled name
    pub name: String,
    /// demangled name with hash
    pub hashed: String,
    /// sequential number of demangled name
    pub index: usize,
    /// number of lines
    pub len: usize,
    /// number of non-blank lines
    pub non_blank_len: usize,
    /// mangled name
    pub mangled_name: String,
}

pub fn suggest_name<'a>(
    search: &str,
    name_display: &NameDisplay,
    items: impl IntoIterator<Item = &'a Item>,
) {
    let mut count = 0usize;
    let names: BTreeMap<&String, Vec<usize>> =
        items.into_iter().fold(BTreeMap::new(), |mut m, item| {
            count += 1;
            let entry = match name_display {
                NameDisplay::Full => &item.hashed,
                NameDisplay::Short => &item.name,
                NameDisplay::Mangled => &item.mangled_name,
            };
            m.entry(entry).or_default().push(item.non_blank_len);
            m
        });

    if names.is_empty() {
        if search.is_empty() {
            safeprintln!("This target defines no functions (or cargo-show-asm can't find them)");
        } else {
            safeprintln!("No matching functions, try relaxing your search request");
        }
        safeprintln!("You can pass --everything to see the demangled contents of a file");
    } else {
        safeprintln!("Try one of those by name or a sequence number");
    }

    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_precision_loss)]
    let width = (count as f64).log10().ceil() as usize;

    let mut ix = 0;
    for (name, lens) in &names {
        safeprintln!(
            "{ix:width$} {:?} {:?}",
            color!(name, owo_colors::OwoColorize::green),
            color!(lens, owo_colors::OwoColorize::cyan),
        );
        ix += lens.len();
    }

    std::process::exit(1);
}

/// Pick an item to dump based on a goal
///
/// Prints suggestions and exits if goal can't be reached or more info is needed
#[must_use]
pub fn get_dump_range(
    goal: ToDump,
    fmt: &Format,
    items: &BTreeMap<Item, Range<usize>>,
) -> Option<Range<usize>> {
    if items.len() == 1 {
        return Some(
            items
                .values()
                .next()
                .cloned()
                .expect("We just checked there's one item present"),
        );
    }
    match goal {
        // to dump everything just return an empty range
        ToDump::Everything => None,

        // By index without filtering
        ToDump::ByIndex { value } => {
            if let Some(range) = items.values().nth(value) {
                Some(range.clone())
            } else {
                let actual = items.len();
                safeprintln!("You asked to display item #{value} (zero based), but there's only {actual} items");
                std::process::exit(1);
            }
        }

        // By index with filtering
        ToDump::Function { function, nth } => {
            let filtered = items
                .iter()
                .filter(|(item, _range)| item.name.contains(&function))
                .collect::<Vec<_>>();

            let range = if nth.is_none() && filtered.len() == 1 {
                filtered
                    .first()
                    .expect("Must have one item as checked above")
                    .1
                    .clone()
            } else if let Some(range) = nth.and_then(|nth| filtered.get(nth)) {
                range.1.clone()
            } else if let Some(value) = nth {
                let filtered = filtered.len();
                safeprintln!("You asked to display item #{value} (zero based), but there's only {filtered} matching items");
                std::process::exit(1);
            } else {
                if filtered.is_empty() {
                    safeprintln!("Can't find any items matching {function:?}");
                } else {
                    suggest_name(&function, &fmt.name_display, filtered.iter().map(|x| x.0));
                }
                std::process::exit(1);
            };
            Some(range)
        }

        // Unspecified, so print suggestions and exit
        ToDump::Unspecified => {
            let items = items.keys();
            suggest_name("", &fmt.name_display, items);
            unreachable!("suggest_name exits");
        }
    }
}

trait RawLines {
    fn lines(&self) -> Option<&str>;
}

impl RawLines for &str {
    fn lines(&self) -> Option<&str> {
        Some(self)
    }
}

/// Recursively scan for references to global objects
fn get_context_for<R: RawLines>(
    depth: usize,
    all_stmts: &[R],
    self_range: Range<usize>,
    items: &BTreeMap<Item, Range<usize>>,
) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    if depth == 0 {
        return out;
    }
    let items = items
        .iter()
        .map(|(item, range)| (item.mangled_name.as_str(), range.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut pending = vec![(self_range.clone(), depth)];
    let mut processed = BTreeSet::new();
    while let Some((range, depth)) = pending.pop() {
        for raw in all_stmts[range]
            .iter()
            .filter_map(R::lines)
            .filter_map(demangle::global_reference)
        {
            if !processed.insert(raw) {
                continue;
            }
            if let Some(range) = items.get(raw) {
                if range == &self_range {
                    continue;
                }
                if depth > 0 {
                    pending.push((range.clone(), depth - 1));
                }
                out.push(range.clone());
            }
        }
    }
    out.sort_by_key(|r| r.start);
    out
}

pub trait Dumpable {
    type Line<'a>;
    /// Split source code into multiple lines, code can do some parsing here
    fn split_lines(contents: &str) -> anyhow::Result<Vec<Self::Line<'_>>>;

    /// Given a set of lines find all the interesting items
    fn find_items(lines: &[Self::Line<'_>]) -> BTreeMap<Item, Range<usize>>;

    /// print all the lines from this range, aplying the required formatting
    fn dump_range(&self, fmt: &Format, lines: &[Self::Line<'_>]) -> anyhow::Result<()>;

    /// starting at an initial range find more ranges to include
    fn extra_context(
        &self,
        fmt: &Format,
        lines: &[Self::Line<'_>],
        range: Range<usize>,
        items: &BTreeMap<Item, Range<usize>>,
    ) -> Vec<Range<usize>> {
        #![allow(unused_variables)]
        Vec::new()
    }
}

/// Parse a dumpable item from a file and dump it with all the extra context
pub fn dump_function<T: Dumpable>(
    dumpable: &T,
    goal: ToDump,
    path: &Path,
    fmt: &Format,
) -> anyhow::Result<()> {
    // first we need to read the data and do a lossy conversion to a string slice
    // (files generated by rustc/llvm can have non-utf8 characters in them
    let raw_bytes = std::fs::read(path)?;
    let contents = String::from_utf8_lossy(&raw_bytes[..]);

    let lines = T::split_lines(&contents)?;
    let items = T::find_items(&lines);

    match get_dump_range(goal, fmt, &items) {
        Some(range) => {
            let context = T::extra_context(dumpable, fmt, &lines, range.clone(), &items);
            dumpable.dump_range(fmt, &lines[range])?;

            if !context.is_empty() {
                safeprintln!(
                    "\n\n======================= Additional context ========================="
                );
                for range in context {
                    safeprintln!("\n");
                    dumpable.dump_range(fmt, &lines[range])?;
                }
            }
        }
        None => dumpable.dump_range(fmt, &lines)?,
    }
    Ok(())
}

/// Mostly the same as Range, but Copy and Ord
#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub struct URange {
    start: usize,
    end: usize,
}

impl From<Range<usize>> for URange {
    fn from(Range { start, end }: Range<usize>) -> Self {
        Self { start, end }
    }
}

impl<T> std::ops::Index<URange> for [T] {
    type Output = [T];
    fn index(&self, index: URange) -> &Self::Output {
        &self[index.start..index.end]
    }
}

impl URange {
    pub fn fully_contains(&self, other: Self) -> bool {
        self.start >= other.start && self.end <= other.end
    }
}
