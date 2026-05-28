use crate::Dumpable;
use crate::{Item, color, opts::Format, safeprintln};
use line_span::LineSpans;
use owo_colors::OwoColorize;
use std::{collections::BTreeMap, ops::Range};

pub struct Mir;

impl Dumpable for Mir {
    type Line<'a> = &'a str;

    fn find_items(lines: &[&str]) -> BTreeMap<Item, Range<usize>> {
        let mut res = BTreeMap::new();
        let mut current_item = None::<Item>;
        let mut block_start = None;
        let mut non_blank_count = 0usize;

        for (ix, &line) in lines.iter().enumerate() {
            if line.starts_with("//") {
                if block_start.is_none() {
                    block_start = Some(ix);
                }
            } else if line == "}" {
                if let Some(mut cur) = current_item.take() {
                    let range = cur.len..ix + 1;
                    cur.len = range.len();
                    cur.non_blank_len = non_blank_count;
                    res.insert(cur, range);
                }
                non_blank_count = 0;
            } else if !(line.starts_with(' ') || line.is_empty()) && current_item.is_none() {
                let start = block_start.take().unwrap_or(ix);
                let mut name = line;
                non_blank_count = 1;
                'outer: loop {
                    for suffix in [" {", " =", " -> ()"] {
                        if let Some(rest) = name.strip_suffix(suffix) {
                            name = rest;
                            continue 'outer;
                        }
                    }
                    break;
                }
                let name = name.trim().to_owned();
                current_item = Some(Item {
                    mangled_name: name.clone(),
                    name: name.clone(),
                    hashed: name,
                    index: res.len(),
                    len: start,
                    non_blank_len: 0,
                });
            } else if current_item.is_some() && !line.trim().is_empty() {
                non_blank_count += 1;
            }
        }

        res
    }

    fn dump_range(&self, _fmt: &Format, strings: &[&str]) -> anyhow::Result<()> {
        for line in strings {
            if let Some(ix) = line.rfind("//") {
                safeprintln!("{}{}", &line[..ix], color!(&line[ix..], OwoColorize::cyan));
            } else {
                safeprintln!("{line}");
            }
        }
        Ok(())
    }

    fn split_lines(contents: &str) -> anyhow::Result<Vec<&str>> {
        Ok(contents
            .line_spans()
            .map(|s| s.as_str())
            .collect::<Vec<_>>())
    }
}
