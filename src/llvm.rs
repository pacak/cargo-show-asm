#![allow(clippy::missing_errors_doc)]
use line_span::LineSpans;
// https://llvm.org/docs/LangRef.html
use owo_colors::OwoColorize;
use regex::Regex;

use crate::Dumpable;
use crate::{
    color,
    demangle::{self, contents},
    opts::Format,
    safeprintln, Item,
};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufRead, BufReader},
    ops::Range,
    path::Path,
};

#[derive(Debug)]
enum State {
    Skipping,
    Seeking,
    Name,
    Type,
    Define,
}

pub struct Llvm;

impl Dumpable for Llvm {
    type Line<'a> = &'a str;
    fn split_lines(contents: &str) -> Vec<Self::Line<'_>> {
        contents
            .line_spans()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
    }
    fn find_items(lines: &[&str]) -> BTreeMap<Item, Range<usize>> {
        struct ItemParseState {
            item: Item,
            start: usize,
        }
        let mut res = BTreeMap::new();
        let mut current_item = None::<ItemParseState>;
        let regex = Regex::new("@\"?(_?_[a-zA-Z0-9_$.]+)\"?\\(").expect("regexp should be valid");

        for (ix, &line) in lines.iter().enumerate() {
            if line.starts_with("; Module") {
                #[allow(clippy::needless_continue)] // silly clippy, readability suffers otherwise
                continue;
            } else if let (true, Some(name)) = (current_item.is_none(), line.strip_prefix("; ")) {
                current_item = Some(ItemParseState {
                    item: Item {
                        mangled_name: name.to_owned(),
                        name: name.to_owned(),
                        hashed: String::new(),
                        index: res.len(),
                        len: 0,
                        non_blank_len: 0,
                    },
                    start: ix,
                });
            } else if line.starts_with("define ") {
                if let (Some(cur), Some((mangled_name, hashed))) = (
                    &mut current_item,
                    regex
                        .captures(line)
                        .and_then(|c| c.get(1))
                        .map(|c| c.as_str())
                        .and_then(|c| Some((c.to_owned(), demangle::demangled(c)?))),
                ) {
                    cur.item.mangled_name = mangled_name;
                    cur.item.hashed = format!("{hashed:?}");
                }
            } else if !line_is_blank(line) {
                if let Some(cur) = &mut current_item {
                    cur.item.non_blank_len += 1;
                }
            } else if line == "}" {
                if let Some(mut cur) = current_item.take() {
                    // go home clippy, you're drunk
                    #[allow(clippy::range_plus_one)]
                    let range = cur.start..ix + 1;
                    cur.item.len = range.len();
                    res.insert(cur.item, range);
                }
            }
        }
        res
    }

    fn dump_range(&self, fmt: &Format, strings: &[&str]) {
        for line in strings {
            if line.starts_with("; ") {
                safeprintln!("{}", color!(line, OwoColorize::bright_cyan));
            } else {
                let line = contents(line, fmt.name_display);
                safeprintln!("{line}");
            }
        }
    }
}

/// Returns true if the line should not be counted as meaningful for the function definition.
///
/// LLVM functions can contain whitespace-only lines or lines with labels/comments that are not codegened,
/// thus counted towards function size.
/// llvm-lines uses similar heuristic to drop lines from its counts.
fn line_is_blank(line: &str) -> bool {
    // Valid instructions have exactly two spaces as formatting.
    // Notable exceptions include comments (lines starting with ';') and
    // labels (lines starting with alphanumeric characters).
    let is_comment_or_label = !line.starts_with("  ");
    // That's not the end of story though. A line can have more than two spaces at the start of line,
    // but in that case it is an extension of the instruction started at previous line.
    // For example:
    //  invoke void @_ZN4bpaf7literal17hd39eb03fefd4e354E(ptr sret(%"bpaf::params::ParseAny<()>") align 8 %_5, ptr align 1 %cmd.0, i64 %cmd.1)
    //        to label %bb1 unwind label %cleanup, !dbg !4048
    //
    // First line is an instruction, so it should be counted towards function size.
    // The second one is a part of the instruction started on the previous line, so we should not
    // count that towards the function size.
    let is_multiline_instruction_extension = line.starts_with("   ");
    is_comment_or_label || is_multiline_instruction_extension
}

/// try to print `goal` from `path`, collect available items otherwise
pub fn collect_or_dump(
    goal: Option<(&str, usize)>,
    path: &Path,
    fmt: &Format,
    items: &mut Vec<Item>,
) -> anyhow::Result<bool> {
    let mut seen = false;

    let reader = BufReader::new(File::open(path)?);

    let regex = Regex::new("@\"?(_?_[a-zA-Z0-9_$.]+)\"?\\(")?;
    let mut state = State::Seeking;
    let mut name = String::new();
    let mut attrs = String::new();
    let mut current_item = None::<Item>;
    let mut names = BTreeMap::new();
    for (ix, line) in reader.lines().enumerate() {
        let line = line?;

        // glorious state machine
        match state {
            State::Skipping => {
                current_item = None;
                if line.is_empty() {
                    state = State::Seeking;
                }
            }
            State::Seeking => {
                if let Some(name_str) = line.strip_prefix("; ") {
                    state = State::Name;
                    name = name_str.to_string();
                } else {
                    state = State::Skipping;
                }
            }
            State::Name => {
                if line.starts_with("; Function Attrs: ") {
                    state = State::Type;
                    attrs = line;
                } else {
                    state = State::Skipping;
                }
            }
            State::Type => {
                if line.starts_with("define ") {
                    state = State::Define;

                    if let Some((mangled_name, hashed)) = regex
                        .captures(&line)
                        .and_then(|c| c.get(1))
                        .map(|c| c.as_str())
                        .and_then(|c| Some((c.to_owned(), demangle::demangled(c)?)))
                    {
                        let hashed = format!("{hashed:?}");
                        let name_entry = names.entry(name.clone()).or_insert(0);
                        seen = goal.map_or(true, |goal| {
                            (name.as_ref(), *name_entry) == goal || hashed == goal.0
                        });

                        current_item = Some(Item {
                            mangled_name,
                            name: name.clone(),
                            hashed,
                            index: *name_entry,
                            len: ix,
                            non_blank_len: 0,
                        });
                        *name_entry += 1;

                        if seen {
                            safeprintln!("{}", color!(name, OwoColorize::cyan));
                            safeprintln!("{}", color!(attrs, OwoColorize::cyan));
                            safeprintln!("{}", contents(&line, fmt.name_display));
                        }
                    } else {
                        state = State::Skipping;
                    }
                } else {
                    state = State::Skipping;
                }
            }
            State::Define => {
                if seen {
                    safeprintln!("{}", contents(&line, fmt.name_display));
                }
                if line == "}" {
                    if let Some(mut cur) = current_item.take() {
                        cur.len = ix - cur.len;
                        cur.non_blank_len = cur.len;
                        if goal.map_or(true, |goal| goal.0.is_empty() || cur.name.contains(goal.0))
                        {
                            items.push(cur);
                        }
                    }
                    if seen {
                        return Ok(true);
                    }
                    state = State::Skipping;
                }
            }
        }
    }

    Ok(seen)
}
