#![allow(clippy::missing_errors_doc)]
use line_span::LineSpans;
// https://llvm.org/docs/LangRef.html
use owo_colors::OwoColorize;
use regex::Regex;

use crate::Dumpable;
use crate::{
    Item, color,
    demangle::{self, contents},
    opts::Format,
    safeprintln,
};
use std::{collections::BTreeMap, ops::Range, sync::LazyLock};

static LLVM_FUNC_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("@\"?(_?[a-zA-Z0-9_$.]+)\"?\\(").expect("regexp should be valid"));

pub struct Llvm;

impl Dumpable for Llvm {
    type Line<'a> = &'a str;
    fn split_lines(contents: &str) -> anyhow::Result<Vec<Self::Line<'_>>> {
        Ok(contents
            .line_spans()
            .map(|s| s.as_str())
            .collect::<Vec<_>>())
    }
    fn find_items(lines: &[&str]) -> BTreeMap<Item, Range<usize>> {
        struct ItemParseState {
            item: Item,
            start: usize,
        }
        let mut res = BTreeMap::new();
        let mut current_item = None::<ItemParseState>;

        for (ix, &line) in lines.iter().enumerate() {
            if line.starts_with("; Module") || line.starts_with("; Function Attrs: ") {
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
                if let Some(name) = LLVM_FUNC_RE.captures(line).and_then(|c| c.get(1)) {
                    let name = name.as_str();
                    let cur = current_item.get_or_insert_with(|| ItemParseState {
                        item: Item {
                            mangled_name: String::new(),
                            name: name.to_owned(),
                            hashed: String::new(),
                            index: res.len(),
                            len: 0,
                            non_blank_len: 0,
                        },
                        start: ix,
                    });
                    name.clone_into(&mut cur.item.mangled_name);
                    cur.item.hashed = demangle::demangled(name)
                        .map_or_else(|| name.to_owned(), |hashed| format!("{hashed:?}"));
                }
            } else if !line_is_blank(line) {
                if let Some(cur) = &mut current_item {
                    cur.item.non_blank_len += 1;
                }
            } else if line == "}" {
                if let Some(mut cur) = current_item.take() {
                    let range = cur.start..ix + 1;
                    cur.item.len = range.len();
                    res.insert(cur.item, range);
                }
            }
        }
        res
    }

    fn dump_range(&self, fmt: &Format, strings: &[&str]) -> anyhow::Result<()> {
        for line in strings {
            if line.starts_with("; ") {
                safeprintln!("{}", color!(line, OwoColorize::bright_cyan));
            } else {
                let line = contents(line, fmt.name_display);
                safeprintln!("{line}");
            }
        }
        Ok(())
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
