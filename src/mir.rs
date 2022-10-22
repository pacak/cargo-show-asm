use crate::{color, llvm::Item, opts::Format};
use owo_colors::OwoColorize;
use regex::Regex;
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

#[derive(Debug)]
enum State {
    Skipping,
    Body,
}

/// try to print `goal` from `path`, collect available items overwise
pub fn dump_function(
    goal: Option<(&str, usize)>,
    path: &Path,
    _fmt: &Format,
    items: &mut Vec<Item>,
) -> anyhow::Result<bool> {
    let mut seen = false;
    let reader = BufReader::new(File::open(path)?);
    let mut current_item = None::<Item>;
    let mut names = BTreeMap::new();
    let mut state = State::Skipping;
    let regex = Regex::new("^fn ([^{]+) \\{$")?;

    let mut block_start = None;
    let mut prefix = Vec::new();

    for (ix, line) in reader.lines().enumerate() {
        let line = line?;
        match state {
            State::Skipping => {
                if line.starts_with("//") {
                    if block_start.is_none() {
                        block_start = Some(ix);
                        prefix.push(line);
                    }
                } else if let Some(name) = regex.captures(&line).and_then(|c| c.get(1)) {
                    state = State::Body;

                    let name = name.as_str().to_owned();
                    let name_entry = names.entry(name.clone()).or_insert(0);
                    let hashed = format!("{name}:{name_entry}");
                    seen = goal.map_or(true, |goal| {
                        (name.as_ref(), *name_entry) == goal || hashed == goal.0
                    });
                    current_item = Some(Item {
                        index: *name_entry,
                        len: block_start.take().unwrap_or(ix),
                        name,
                        hashed,
                    });
                    *name_entry += 1;
                    prefix.push(line);
                } else {
                    prefix.clear();
                }
            }
            State::Body => {
                if seen {
                    for p in prefix.drain(..) {
                        println!("{p}");
                    }
                    if let Some(ix) = line.rfind("//") {
                        println!("{}{}", &line[..ix], color!(&line[ix..], OwoColorize::cyan));
                    } else {
                        println!("{line}");
                    }
                }

                if line == "}" {
                    state = State::Skipping;
                    if let Some(mut cur) = current_item.take() {
                        cur.len = ix - cur.len;
                        if goal.map_or(true, |goal| goal.0.is_empty() || cur.name.contains(goal.0))
                        {
                            items.push(cur);
                        }
                    }
                    if seen {
                        return Ok(true);
                    }
                }
            }
        }
    }

    Ok(seen)
}
