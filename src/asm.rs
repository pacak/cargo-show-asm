#![allow(clippy::missing_errors_doc)]
use crate::asm::statements::{Instruction, Label};
use crate::cached_lines::CachedLines;
use crate::{color, demangle};
// TODO, use https://sourceware.org/binutils/docs/as/index.html
use crate::opts::Format;

mod statements;

use owo_colors::OwoColorize;
use statements::{parse_statement, Directive, Loc, Statement};
use std::collections::BTreeMap;
use std::ops::Range;
use std::path::Path;

pub fn parse_file(input: &str) -> anyhow::Result<Vec<Statement>> {
    // eat all statements until the eof, so we can report the proper errors on failed parse
    match nom::multi::many0(parse_statement)(input) {
        Ok(("", stmts)) => Ok(stmts),
        Ok((leftovers, _)) => {
            if leftovers.len() < 1000 {
                anyhow::bail!("Didn't consume everything, leftovers: {leftovers:?}")
            } else {
                let head = &leftovers[..leftovers.char_indices().nth(200).unwrap().0];
                anyhow::bail!("Didn't consume everything, leftovers prefix: {head:?}");
            }
        }
        Err(err) => anyhow::bail!("Couldn't parse the .s file: {err}"),
    }
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Item {
    /// demangled name
    pub name: String,
    /// demangled name with hash
    pub hashed: String,
    /// sequential number of demangled name
    pub index: usize,
    /// number of lines
    pub len: usize,
}

fn find_items(lines: &[Statement]) -> BTreeMap<Item, Range<usize>> {
    let mut res = BTreeMap::new();

    let mut sec_start = 0;
    let mut item: Option<Item> = None;
    let mut names = BTreeMap::new();

    for (ix, line) in lines.iter().enumerate() {
        if line.is_section_start() {
            sec_start = ix;
        } else if line.is_end_of_fn() {
            let sec_end = ix;
            let range = sec_start..sec_end;
            if let Some(mut item) = item.take() {
                item.len = ix - item.len;
                res.insert(item, range);
            }
        } else if let Statement::Label(label) = line {
            if let Some(dem) = demangle::demangled(label.id) {
                let hashed = format!("{dem:?}");
                let name = format!("{dem:#?}");
                let name_entry = names.entry(name.clone()).or_insert(0);
                item = Some(Item {
                    name,
                    hashed,
                    index: *name_entry,
                    len: ix,
                });
                *name_entry += 1;
            }
        }
    }
    res
}

pub fn used_labels<'a>(stmts: &'_ [Statement<'a>]) -> BTreeMap<&'a str, usize> {
    let labels = stmts
        .iter()
        .filter_map(|i| {
            if let Statement::Label(Label { local: true, id }) = i {
                Some(id)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let mut used_labels = BTreeMap::new();

    for (ix, item) in stmts.iter().enumerate() {
        if let Statement::Instruction(Instruction {
            args: Some(args), ..
        }) = item
        {
            for &label in &labels {
                if args.contains(label) {
                    used_labels.insert(*label, ix);
                }
            }
        }
    }
    used_labels
}

pub fn dump_range(sysroot: &Path, fmt: &Format, stmts: &[Statement]) -> anyhow::Result<()> {
    let mut files = BTreeMap::new();
    let mut prev_loc = Loc::default();

    let used = if fmt.keep_labels {
        BTreeMap::new()
    } else {
        used_labels(stmts)
    };

    let mut empty_line = false;
    for line in stmts.iter() {
        if let Statement::Directive(Directive::File(f)) = line {
            if !fmt.rust {
                continue;
            }
            files.entry(f.index).or_insert_with(|| {
                let path = f.path.as_full_path();
                if let Ok(payload) = std::fs::read_to_string(&path) {
                    return (path, CachedLines::without_ending(payload));
                } else if path.starts_with("/rustc/") {
                    let relative_path = {
                        let mut components = path.components();
                        // skip first three dirs in path
                        components.by_ref().take(3).for_each(|_| ());
                        components.as_path()
                    };
                    if relative_path.file_name().is_some() {
                        let src = sysroot.join("lib/rustlib/src/rust").join(relative_path);
                        if !src.exists() {
                            eprintln!("You need to install rustc sources to be able to see the rust annotations, try\n\
                                       \trustup component add rust-src");
                            std::process::exit(1);
                        }
                        if let Ok(payload) = std::fs::read_to_string(src) {
                            return (path, CachedLines::without_ending(payload));
                        }
                    }
                }
                // if file is not found - ust create a dummy
                (path, CachedLines::without_ending(String::new()))
            });
            continue;
        } else if let Statement::Directive(Directive::Loc(loc)) = &line {
            if !fmt.rust {
                continue;
            }
            if loc.line == 0 {
                continue;
            }
            if loc == &prev_loc {
                continue;
            }
            prev_loc = *loc;
            if let Some((fname, file)) = files.get(&loc.file) {
                let rust_line = &file[loc.line as usize - 1];
                let pos = format!("\t\t// {} : {}", fname.display(), loc.line);
                println!("{}", color!(pos, OwoColorize::cyan));
                println!(
                    "\t\t{}",
                    color!(rust_line.trim_start(), OwoColorize::bright_red)
                );
            }
        } else {
            if !fmt.keep_labels {
                if let Statement::Label(Label { local: true, id }) = line {
                    if used.contains_key(id) {
                        println!("{line}");
                    } else if !empty_line {
                        println!();
                        empty_line = true;
                    }
                    continue;
                }
            }
            empty_line = false;

            #[allow(clippy::collapsible_else_if)]
            if fmt.full_name {
                println!("{line:#}");
            } else {
                println!("{line}");
            }
        }
    }
    Ok(())
}

/// try to print `goal` from `path`, collect available items otherwise
pub fn dump_function(
    goal: (&str, usize),
    path: &Path,
    sysroot: &Path,
    fmt: &Format,
    items: &mut Vec<Item>,
) -> anyhow::Result<bool> {
    let contents = std::fs::read_to_string(path)?;
    let file = parse_file(&contents)?;
    let functions = find_items(&file);

    for (item, range) in &functions {
        if (item.name.as_ref(), item.index) == goal || item.hashed == goal.0 {
            dump_range(sysroot, fmt, &file[range.clone()])?;
            return Ok(true);
        }
    }

    *items = functions
        .keys()
        .filter(|i| i.name.contains(goal.0))
        .cloned()
        .collect::<Vec<_>>();

    Ok(false)
}
