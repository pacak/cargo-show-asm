#![allow(clippy::missing_errors_doc)]
use crate::asm::statements::Label;
use crate::cached_lines::CachedLines;
use crate::{color, demangle};
// TODO, use https://sourceware.org/binutils/docs/as/index.html
use crate::opts::Format;

mod statements;

use owo_colors::OwoColorize;
use statements::{parse_statement, Directive, Loc, Statement};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::path::Path;

fn parse_file(input: &str) -> anyhow::Result<Vec<Statement>> {
    // eat all statements until the eof, so we can report the proper errors on failed parse
    match nom::multi::many0(parse_statement)(input) {
        Ok(("", stmts)) => Ok(stmts),
        Ok((leftovers, _)) =>
        {
            #[allow(clippy::redundant_else)]
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
        #[allow(clippy::if_same_then_else)]
        if line.is_section_start() {
            sec_start = ix;
        } else if line.is_global() && sec_start + 3 < ix {
            // on Linux and Windows every global function gets it's own section
            // on Mac for some reason this is not the case so we have to look for
            // global variables. This little hack allows to include full section
            // on Windows/Linux but still capture full function body on Mac
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

fn used_labels<'a>(stmts: &'_ [Statement<'a>]) -> BTreeSet<&'a str> {
    stmts
        .iter()
        .filter_map(|stmt| match stmt {
            Statement::Label(_) | Statement::Nothing => None,
            Statement::Directive(dir) => match dir {
                Directive::File(_)
                | Directive::Loc(_)
                | Directive::SubsectionsViaSym
                | Directive::Set(_) => None,
                Directive::Generic(g) => Some(g.0),
                Directive::SectionStart(ss) => Some(*ss),
            },
            Statement::Instruction(i) => i.args,
            Statement::Dunno(s) => Some(s),
        })
        .flat_map(crate::demangle::local_labels)
        .map(|m| m.as_str())
        .collect::<BTreeSet<_>>()
}

pub fn dump_range(
    files: &BTreeMap<u64, (std::borrow::Cow<Path>, CachedLines)>,
    fmt: &Format,
    stmts: &[Statement],
) -> anyhow::Result<()> {
    let mut prev_loc = Loc::default();

    let used = if fmt.keep_labels {
        BTreeSet::new()
    } else {
        used_labels(stmts)
    };

    let mut empty_line = false;
    for line in stmts.iter() {
        if fmt.verbosity > 2 {
            println!("{line:?}");
        }
        if let Statement::Directive(Directive::File(_)) = &line {
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
            empty_line = false;
        } else if let Statement::Label(Label { local: true, id }) = line {
            if fmt.keep_labels || used.contains(id) {
                println!("{line}");
            } else if !empty_line && !demangle::is_temp_label(id) {
                println!();
                empty_line = true;
            }
        } else {
            if fmt.simplify && matches!(line, Statement::Directive(_) | Statement::Dunno(_)) {
                continue;
            }

            empty_line = false;
            #[allow(clippy::match_bool)]
            match fmt.full_name {
                true => println!("{line:#}"),
                false => println!("{line}"),
            }
        }
    }
    Ok(())
}

/// try to print `goal` from `path`, collect available items otherwise
pub fn dump_function(
    goal: Option<(&str, usize)>,
    path: &Path,
    sysroot: &Path,
    fmt: &Format,
    items: &mut Vec<Item>,
) -> anyhow::Result<bool> {
    if fmt.verbosity > 2 {
        println!("goal: {:?}", goal);
    }

    let contents = std::fs::read_to_string(path)?;
    let file = parse_file(&contents)?;
    let functions = find_items(&file);

    if fmt.verbosity > 2 {
        println!("{:?}", functions);
    }

    let mut files = BTreeMap::new();
    if fmt.rust {
        for line in &file {
            if let Statement::Directive(Directive::File(f)) = line {
                files.entry(f.index).or_insert_with(|| {
                let path = f.path.as_full_path();
                if fmt.verbosity > 1 {
                    println!("Reading file #{} {:?}", f.index, path);
                }
                if let Ok(payload) = std::fs::read_to_string(&path) {
                    return (path, CachedLines::without_ending(payload));
                } else if path.starts_with("/rustc/") {
                    // file looks like this and is located in rustlib sysroot
                    // /rustc/a55dd71d5fb0ec5a6a3a9e8c27b2127ba491ce52/library/core/src/iter/range.rs

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
                } else if path.starts_with("/cargo/registry/") {
                    // file looks like this and located ~/.cargo/registry/ ...
                    // /cargo/registry/src/github.com-1ecc6299db9ec823/hashbrown-0.12.3/src/raw/bitmask.rs

                    // It does what I want as far as *nix is concerned, might not work for Windows...
                    #[allow(deprecated)]
                    let mut homedir = std::env::home_dir().expect("No home dir?");

                    let mut components = path.components();
                    // drop `/cargo` part
                        components.by_ref().take(2).for_each(|_| ());
                    homedir.push(".cargo");
                    let src = homedir.join(components.as_path());

                    if let Ok(payload) = std::fs::read_to_string(src) {
                       return (path, CachedLines::without_ending(payload));
                    }
                } else if fmt.verbosity > 0 {
                    println!("File not found {:?}", path);
                }
                // if file is not found - ust create a dummy
                (path, CachedLines::without_ending(String::new()))
            });
            }
        }
    }

    if let Some(goal) = goal {
        for (item, range) in &functions {
            if (item.name.as_ref(), item.index) == goal || item.hashed == goal.0 {
                if fmt.verbosity > 1 {
                    println!("dumping range: {:?} of 0..{}", range, file.len());
                }

                dump_range(&files, fmt, &file[range.clone()])?;
                return Ok(true);
            }
        }

        *items = functions
            .keys()
            .filter(|i| i.name.contains(goal.0))
            .cloned()
            .collect::<Vec<_>>();

        Ok(false)
    } else {
        if fmt.verbosity > 0 {
            println!("Going to print the whole file");
        }
        dump_range(&files, fmt, &file)?;
        Ok(true)
    }
}
