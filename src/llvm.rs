#![allow(clippy::missing_errors_doc)]
// https://llvm.org/docs/LangRef.html
use owo_colors::OwoColorize;
use regex::Regex;

pub use crate::asm::Item;
use crate::{
    color,
    demangle::{self, contents},
    opts::Format,
};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufRead, BufReader},
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

/// try to print `goal` from `path`, collect available items otherwise
///
///
pub fn dump_function(
    goal: (&str, usize),
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

                    if let Some(hashed) = regex
                        .captures(&line)
                        .and_then(|c| c.get(1))
                        .map(|c| c.as_str())
                        .and_then(demangle::demangled)
                    {
                        let hashed = format!("{hashed:?}");
                        let name_entry = names.entry(name.clone()).or_insert(0);
                        seen = (name.as_ref(), *name_entry) == goal || hashed == goal.0;

                        current_item = Some(Item {
                            name: name.clone(),
                            hashed,
                            index: *name_entry,
                            len: ix,
                        });
                        *name_entry += 1;

                        if seen {
                            println!("{}", color!(name, OwoColorize::cyan));
                            println!("{}", color!(attrs, OwoColorize::cyan));
                            println!("{}", contents(&line, fmt.full_name));
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
                    println!("{}", contents(&line, fmt.full_name));
                }
                if line == "}" {
                    if let Some(mut cur) = current_item.take() {
                        cur.len = ix - cur.len;
                        if goal.0.is_empty() || cur.name.contains(goal.0) {
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
