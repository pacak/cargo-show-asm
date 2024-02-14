use crate::Dumpable;
use crate::{cached_lines::CachedLines, color, opts::Format, safeprintln, Item};
use owo_colors::OwoColorize;
use std::{collections::BTreeMap, ops::Range};

pub struct Mir;

impl Dumpable for Mir {
    fn find_items(lines: &CachedLines) -> BTreeMap<Item, Range<usize>> {
        let mut res = BTreeMap::new();
        let mut current_item = None::<Item>;
        let mut block_start = None;

        for (ix, line) in lines.iter().enumerate() {
            if line.starts_with("//") {
                if block_start.is_none() {
                    block_start = Some(ix);
                }
            } else if line == "}" {
                if let Some(mut cur) = current_item.take() {
                    // go home clippy, you're drunk
                    #[allow(clippy::range_plus_one)]
                    let range = cur.len..ix + 1;
                    cur.len = range.len();
                    res.insert(cur, range);
                }
            } else if !(line.starts_with(' ') || line.is_empty()) && current_item.is_none() {
                let start = block_start.take().unwrap_or(ix);
                let mut name = line;
                'outer: loop {
                    for suffix in [" {", " =", " -> ()"] {
                        if let Some(rest) = name.strip_suffix(suffix) {
                            name = rest;
                            continue 'outer;
                        }
                    }
                    break;
                }
                current_item = Some(Item {
                    mangled_name: name.to_owned(),
                    name: name.to_owned(),
                    hashed: name.to_owned(),
                    index: res.len(),
                    len: start,
                    non_blank_len: 0,
                });
            }
        }

        res
    }

    fn dump_range(_fmt: &Format, strings: &[&str]) {
        for line in strings {
            if let Some(ix) = line.rfind("//") {
                safeprintln!("{}{}", &line[..ix], color!(&line[ix..], OwoColorize::cyan));
            } else {
                safeprintln!("{line}");
            }
        }
    }
}
