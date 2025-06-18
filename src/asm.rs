#![allow(clippy::missing_errors_doc)]
use crate::asm::statements::Label;
use crate::cached_lines::CachedLines;
use crate::demangle::LabelKind;
use crate::{
    color, demangle, esafeprintln, get_context_for, safeprintln, Dumpable, Item, RawLines, URange,
};
// TODO, use https://sourceware.org/binutils/docs/as/index.html
use crate::opts::{Format, NameDisplay, RedundantLabels, SourcesFrom};

mod statements;

use nom::Parser as _;
use owo_colors::OwoColorize;
use statements::{parse_statement, Loc};
pub use statements::{Directive, GenericDirective, Instruction, Statement};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ops::Range;
use std::path::{Path, PathBuf};

type SourceFile = (PathBuf, Option<(Source, CachedLines)>);

pub fn parse_file(input: &str) -> anyhow::Result<Vec<Statement>> {
    // eat all statements until the eof, so we can report the proper errors on failed parse
    match nom::multi::many0(parse_statement).parse(input) {
        Ok(("", stmts)) => Ok(stmts),
        Ok((leftovers, _)) =>
        {
            #[allow(clippy::redundant_else)]
            if leftovers.len() < 1000 {
                anyhow::bail!("Didn't consume everything, leftovers: {leftovers:?}")
            } else {
                let head = &leftovers[..leftovers
                    .char_indices()
                    .nth(200)
                    .expect("Shouldn't have that much unicode here...")
                    .0];
                anyhow::bail!("Didn't consume everything, leftovers prefix: {head:?}");
            }
        }
        Err(err) => anyhow::bail!("Couldn't parse the .s file: {err}"),
    }
}

#[must_use]
pub fn find_items(lines: &[Statement]) -> BTreeMap<Item, Range<usize>> {
    let mut res = BTreeMap::new();

    let mut sec_start = 0;
    let mut item: Option<Item> = None;
    let mut names = BTreeMap::new();

    for (ix, line) in lines.iter().enumerate() {
        if line.is_section_start() {
            if item.is_none() {
                sec_start = ix;
            } else {
                // on Windows, when panic unwinding is enabled, the compiler can
                // produce multiple blocks of exception-handling code for a
                // function, annotated by .seh_* directives (which we ignore).
                // For some reason (maybe a bug? or maybe we're misunderstanding
                // something?), each of those blocks starts with a .section
                // directive identical to the one at the start of the function.
                // We have to ignore such duplicates here, otherwise we'd output
                // only the last exception-handling block instead of the whole
                // function.
                //
                // See https://github.com/pacak/cargo-show-asm/issues/110
            }
        } else if line.is_global() && sec_start + 3 < ix {
            // On Linux and Windows every global function gets its own section.
            // On Mac for some reason this is not the case, so we have to look for
            // symbols marked globl within the section.  So if we encounter a globl
            // deep enough within the current section treat it as a new section start.
            // This little hack allows to include full section on Windows/Linux but
            // still capture full function body on Mac.
            sec_start = ix;
        } else if line.is_end_of_fn() {
            let sec_end = ix;
            let range = sec_start..sec_end;
            if let Some(mut item) = item.take() {
                item.len = ix - item.len;
                item.non_blank_len = item.len;
                res.insert(item, range);
            }
        } else if let Statement::Label(label) = line {
            if let Some(dem) = demangle::demangled(label.id) {
                let hashed = format!("{dem:?}");
                let name = format!("{dem:#?}");
                let name_entry = names.entry(name.clone()).or_insert(0);
                item = Some(Item {
                    mangled_name: label.id.to_owned(),
                    name,
                    hashed,
                    index: *name_entry,
                    len: ix,
                    non_blank_len: 0,
                });
                *name_entry += 1;
            } else if matches!(label.kind, LabelKind::Unknown | LabelKind::Global) {
                if let Some(mut i) = handle_non_mangled_labels(lines, ix, label, sec_start) {
                    let name_entry = names.entry(i.name.clone()).or_insert(0);
                    i.index = *name_entry;
                    item = Some(i);
                    *name_entry += 1;
                }
            }
        }
    }

    // detect merged functions
    // we'll define merged function as something with a global label and a reference to a different
    // global label

    let globals = lines
        .iter()
        .enumerate()
        .filter_map(|(ix, line)| {
            if let Statement::Directive(Directive::Global(name)) = line {
                Some((name, ix))
            } else {
                None
            }
        })
        .collect::<HashMap<_, _>>();

    for (end, line) in lines.iter().enumerate() {
        let Statement::Directive(Directive::SetValue(name, _)) = line else {
            continue;
        };
        let Some(start) = globals.get(name).copied() else {
            continue;
        };

        // Merged function is different on different system, lol.
        //
        // Linux: a sequence of 3 items
        //
        // .globl  _ZN13sample_merged3two17h0afab563317f9d7bE
        // .type   _ZN13sample_merged3two17h0afab563317f9d7bE,@function
        // .set _ZN13sample_merged3two17h0afab563317f9d7bE, _ZN13sample_merged12one_plus_one17h408b56cb936d6f10E
        //
        // MacOS: a sequence of 2 items
        //
        // .globl  _ZN13sample_merged3two17h0afab563317f9d7bE
        // .set _ZN13sample_merged3two17h0afab563317f9d7bE, _ZN13sample_merged12one_plus_one17h408b56cb936d6f10E
        //
        // Windows: a sequence of 6-ish items, different on CI machine LOL
        //
        //  .globl  _ZN13sample_merged7two_num17h2372a6fab541fa02E
        //  .def    _ZN13sample_merged7two_num17h2372a6fab541fa02E;
        //  .scl    2;
        //  .type   32;
        //  .endef
        // .set _ZN13sample_merged7two_num17h2372a6fab541fa02E, _ZN13sample_merged12one_plus_one17h96e22123e4e22951E

        let range = start..end + 1;
        if range.len() > 10 {
            // merged function body should contain just a few lines, use
            // this as a sanity check
            continue;
        }
        let sym = name;
        if let Some(dem) = demangle::demangled(sym) {
            let hashed = format!("{dem:?}");
            let name = format!("{dem:#?}");
            let name_entry = names.entry(name.clone()).or_insert(0);
            res.insert(
                Item {
                    mangled_name: sym.to_string(),
                    name,
                    hashed,
                    index: *name_entry,
                    len: range.len(),
                    non_blank_len: range.len(),
                },
                range,
            );
            *name_entry += 1;
        }
    }

    res
}

/// Handles the non-mangled labels found in the given lines of ASM statements.
///
/// Returns item if the label is a valid function item, otherwise returns None.
/// NOTE: Does not set `item.index`.
fn handle_non_mangled_labels(
    lines: &[Statement],
    ix: usize,
    label: &Label,
    sec_start: usize,
) -> Option<Item> {
    match lines.get(sec_start) {
        Some(Statement::Directive(Directive::SectionStart(ss))) => {
            // The first macOS symbol is found in this section.
            // Symbols after this are resolved by matching globl Generic Directive below
            // because of the sec_start hack in `find_items`.
            const MACOS_TEXT_SECTION: &str = "__TEXT,__text,regular,pure_instructions";
            // Windows symbols each have their own section with this prefix.
            const WINDOWS_TEXT_SECTION_PREFIX: &str = ".text,\"xr\",one_only,";
            let is_mac = *ss == MACOS_TEXT_SECTION;
            let is_windows = ss.starts_with(WINDOWS_TEXT_SECTION_PREFIX);
            if is_windows || is_mac {
                // Search for .globl between sec_start and ix
                for line in &lines[sec_start..ix] {
                    if let Statement::Directive(Directive::Global(g)) = line {
                        // last bool is responsible for stripping leading underscore.
                        // Stripping is not needed on Linux and 64-bit Windows.
                        // Currently we want to strip underscore on MacOS
                        // TODO: on 32-bit Windows we ought to remove underscores
                        if let Some(item) = get_item_in_section(ix, label, g, is_mac) {
                            return Some(item);
                        }
                    }
                }
                None
            } else {
                // Linux symbols each have their own section, named with this prefix.
                get_item_in_section(ix, label, ss.strip_prefix(".text.")?, false)
            }
        }
        //        Some(Statement::Directive(Directive::Generic(GenericDirective(g)))) => {
        // macOS symbols after the first are matched here.
        //            get_item_in_section(PrefixKind::Global, ix, label, g, true)
        //        }
        Some(Statement::Directive(Directive::Global(g))) => get_item_in_section(ix, label, g, true),
        _ => None,
    }
}

/// Checks if the place (ss) starts with the `label`. Place can be either section or .global
/// Creates a new [`Item`], but sets `item.index` to 0.
fn get_item_in_section(ix: usize, label: &Label, ss: &str, strip_underscore: bool) -> Option<Item> {
    if !ss.starts_with(label.id) {
        return None;
    }
    let name = if strip_underscore && label.id.starts_with('_') {
        String::from(&label.id[1..])
    } else {
        String::from(label.id)
    };
    Some(Item {
        mangled_name: label.id.to_owned(),
        name: name.clone(),
        hashed: name,
        index: 0, // Written later in find_items
        len: ix,
        non_blank_len: 0,
    })
}

fn used_labels<'a>(stmts: &'_ [Statement<'a>]) -> BTreeSet<&'a str> {
    stmts
        .iter()
        .filter_map(|stmt| match stmt {
            Statement::Label(_) | Statement::Nothing => None,
            Statement::Directive(dir) => match dir {
                Directive::File(_)
                | Directive::Loc(_)
                | Directive::Global(_)
                | Directive::SubsectionsViaSym
                | Directive::SymIsFun(_) => None,
                Directive::Data(_, val) | Directive::SetValue(_, val) => Some(*val),
                Directive::Generic(g) => Some(g.0),
                Directive::SectionStart(ss) => Some(*ss),
            },
            Statement::Instruction(i) => i.args,
            Statement::Dunno(s) => Some(s),
        })
        .flat_map(demangle::local_labels)
        .collect::<BTreeSet<_>>()
}

/// Scans for referenced constants
fn scan_constant(
    name: &str,
    sections: &BTreeMap<&str, usize>,
    body: &[Statement],
) -> Option<URange> {
    let start = *sections.get(name)?;
    let end = start
        + body[start + 1..]
            .iter()
            .take_while(|s| matches!(s, Statement::Directive(Directive::Data(_, _))))
            .count()
        + 1;
    Some(URange { start, end })
}

fn dump_range(
    files: &BTreeMap<u64, SourceFile>,
    fmt: &Format,
    print_range: Range<usize>,
    body: &[Statement], // full body
) -> anyhow::Result<()> {
    let print_range = URange::from(print_range);
    let mut prev_loc = Loc::default();

    let stmts = &body[print_range];
    let used = if fmt.redundant_labels == RedundantLabels::Keep {
        BTreeSet::new()
    } else {
        used_labels(stmts)
    };

    let mut empty_line = false;
    for (ix, line) in stmts.iter().enumerate() {
        if fmt.verbosity > 3 {
            safeprintln!("{line:?}");
        }
        if let Statement::Directive(Directive::File(_)) = &line {
            // do nothing, this directive was used previously to initialize rust sources
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
            match files.get(&loc.file) {
                Some((fname, Some((source, file)))) => {
                    if source.show_for(fmt.sources_from) {
                        let pos = format!("\t\t// {}:{}", fname.display(), loc.line);
                        safeprintln!("{}", color!(pos, OwoColorize::cyan));
                        if let Some(rust_line) = &file.get(loc.line as usize - 1) {
                            safeprintln!(
                                "\t\t{}",
                                color!(rust_line.trim_start(), OwoColorize::bright_red)
                            );
                        } else {
                            safeprintln!("\t\t{}",
                                color!("Corrupted rust-src installation? Try re-adding rust-src component.", OwoColorize::red)
                            );
                        }
                    }
                }
                Some((fname, None)) => {
                    if fmt.verbosity > 1 {
                        safeprintln!(
                            "\t\t{} {}",
                            color!("//", OwoColorize::cyan),
                            color!(
                                "Can't locate the file, please open a ticket with cargo-show-asm",
                                OwoColorize::red
                            ),
                        );
                    }
                    let pos = format!("\t\t// {}:{}", fname.display(), loc.line);
                    safeprintln!("{}", color!(pos, OwoColorize::cyan));
                }
                None => {
                    panic!("DWARF file refers to an undefined location {loc:?}");
                }
            }
            empty_line = false;
        } else if let Statement::Label(Label {
            kind: kind @ (LabelKind::Local | LabelKind::Temp),
            id,
        }) = line
        {
            match fmt.redundant_labels {
                // We always include used labels and labels at the very
                // beginning of the fragment - those are used for data declarations
                _ if ix == 0 || used.contains(id) => {
                    safeprintln!("{line}");
                }
                RedundantLabels::Keep => {
                    safeprintln!("{line}");
                }
                RedundantLabels::Blanks => {
                    if !empty_line && *kind != LabelKind::Temp {
                        safeprintln!();
                        empty_line = true;
                    }
                }
                RedundantLabels::Strip => {}
            }
        } else {
            if fmt.simplify && line.boring() {
                continue;
            }

            empty_line = false;
            match fmt.name_display {
                NameDisplay::Full => safeprintln!("{line:#}"),
                NameDisplay::Short => safeprintln!("{line}"),
                NameDisplay::Mangled => safeprintln!("{line:-}"),
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub enum Source {
    Crate,
    External,
    Stdlib,
    Rustc,
}

impl Source {
    fn show_for(&self, from: SourcesFrom) -> bool {
        match self {
            Source::Crate => true,
            Source::External => match from {
                SourcesFrom::ThisWorkspace => false,
                SourcesFrom::AllCrates | SourcesFrom::AllSources => true,
            },
            Source::Rustc | Source::Stdlib => match from {
                SourcesFrom::ThisWorkspace | SourcesFrom::AllCrates => false,
                SourcesFrom::AllSources => true,
            },
        }
    }
}

// DWARF information contains references to source files
// It can point to 3 different items:
// 1. a real file, cargo-show-asm can just read it
// 2. a file from rustlib, sources are under $sysroot/lib/rustlib/src/rust/$suffix
//    Some examples:
//        /rustc/a55dd71d5fb0ec5a6a3a9e8c27b2127ba491ce52/library/core/src/iter/range.rs
//        /private/tmp/rust-20230325-7327-rbrpyq/rustc-1.68.1-src/library/core/src/option.rs
//        /rustc/cc66ad468955717ab92600c770da8c1601a4ff33\\library\\core\\src\\convert\\mod.rs
// 3. a file from prebuilt (?) hashbrown, sources are probably available under
//    cargo registry, most likely under ~/.cargo/registry/$suffix
//    Some examples:
//        /cargo/registry/src/github.com-1ecc6299db9ec823/hashbrown-0.12.3/src/raw/bitmask.rs
//        /Users/runner/.cargo/registry/src/github.com-1ecc6299db9ec823/hashbrown-0.12.3/src/map.rs
// 4. rustc sources:
//    /rustc/89e2160c4ca5808657ed55392620ed1dbbce78d1/compiler/rustc_span/src/span_encoding.rs
//    $sysroot/lib/rustlib/rust-src/rust/compiler/rustc_span/src/span_encoding.rs
fn locate_sources(sysroot: &Path, workspace: &Path, path: &Path) -> Option<(Source, PathBuf)> {
    let mut path = Cow::Borrowed(path);
    // a real file that simply exists
    if path.exists() {
        let source = if path.starts_with(workspace) {
            Source::Crate
        } else {
            Source::External
        };

        return Some((source, path.into()));
    }

    let no_rust_src = || {
        esafeprintln!(
            "You need to install rustc sources to be able to see the rust annotations, try\n\
                                       \trustup component add rust-src"
        );
        std::process::exit(1);
    };

    // then during crosscompilation we can get this cursed mix of path names
    //
    // /rustc/cc66ad468955717ab92600c770da8c1601a4ff33\\library\\core\\src\\convert\\mod.rs
    //
    // where one bit comes from the host platform and second bit comes from the target platform
    // This feels like a problem in upstream, but supporting that is not _that_ hard.
    //
    // I think this should take care of Linux and MacOS support
    if (path.starts_with("/rustc/") || path.starts_with("/private/tmp"))
        && path
            .as_os_str()
            .to_str()
            .is_some_and(|s| s.contains("\\") && s.contains("/"))
    {
        let cursed_path = path
            .as_os_str()
            .to_str()
            .expect("They are coming from a text file");
        path = Cow::Owned(PathBuf::from(cursed_path.replace("\\", "/")));
    }

    // /rustc/89e2160c4ca5808657ed55392620ed1dbbce78d1/compiler/rustc_span/src/span_encoding.rs
    if path.starts_with("/rustc") && path.iter().any(|c| c == "compiler") {
        let mut source = sysroot.join("lib/rustlib/rustc-src/rust");
        for part in path.components().skip(3) {
            source.push(part);
        }

        if source.exists() {
            return Some((Source::Rustc, source));
        } else {
            no_rust_src();
        }
    }

    // rust sources, Linux style
    if path.starts_with("/rustc/") {
        let mut source = sysroot.join("lib/rustlib/src/rust");
        for part in path.components().skip(3) {
            source.push(part);
        }
        if source.exists() {
            return Some((Source::Stdlib, source));
        } else {
            no_rust_src();
        }
    }

    // rust sources, MacOS style
    if path.starts_with("/private/tmp") && path.components().any(|c| c.as_os_str() == "library") {
        let mut source = sysroot.join("lib/rustlib/src/rust");
        for part in path.components().skip(5) {
            source.push(part);
        }
        if source.exists() {
            return Some((Source::Stdlib, source));
        } else {
            no_rust_src();
        }
    }

    // cargo registry, Linux and macOS look for cargo/registry and .cargo/registry
    if let Some(ix) = path
        .components()
        .position(|c| c.as_os_str() == "cargo" || c.as_os_str() == ".cargo")
        .and_then(|ix| path.components().nth(ix).zip(Some(ix)))
        .and_then(|(c, ix)| (c.as_os_str() == "registry").then_some(ix))
    {
        // It does what I want as far as *nix is concerned, might not work for Windows...
        #[allow(deprecated)]
        let mut source = std::env::home_dir().expect("No home dir?");

        source.push(".cargo");
        for part in path.components().skip(ix) {
            source.push(part);
        }
        if source.exists() {
            return Some((Source::External, source));
        } else {
            panic!(
                "{path:?} looks like it can be a cargo registry reference but we failed to get it"
            );
        }
    }

    None
}

fn load_rust_sources(
    sysroot: &Path,
    workspace: &Path,
    statements: &[Statement],
    fmt: &Format,
    files: &mut BTreeMap<u64, SourceFile>,
) {
    for line in statements {
        if let Statement::Directive(Directive::File(f)) = line {
            files.entry(f.index).or_insert_with(|| {
                let path = f.path.as_full_path().into_owned();
                if fmt.verbosity > 2 {
                    safeprintln!("Reading file #{} {}", f.index, path.display());
                }

                if let Some((source, filepath)) = locate_sources(sysroot, workspace, &path) {
                    if fmt.verbosity > 3 {
                        safeprintln!("Resolved name is {filepath:?}");
                    }
                    let sources = std::fs::read_to_string(&filepath).expect("Can't read a file");
                    if sources.is_empty() {
                        if fmt.verbosity > 0 {
                            safeprintln!("Ignoring empty file {filepath:?}!");
                        }
                        (path, None)
                    } else {
                        if fmt.verbosity > 3 {
                            safeprintln!("Got {} bytes", sources.len());
                        }
                        let lines = CachedLines::without_ending(sources);
                        (path, Some((source, lines)))
                    }
                } else {
                    if fmt.verbosity > 1 {
                        safeprintln!("File not found {}", path.display());
                    }
                    (path, None)
                }
            });
        }
    }
}

impl RawLines for Statement<'_> {
    fn lines(&self) -> Option<&str> {
        match self {
            Statement::Instruction(i) => i.args,
            Statement::Directive(Directive::SetValue(_, i)) => Some(i),
            _ => None,
        }
    }
}

pub struct Asm<'a> {
    workspace: &'a Path,
    sysroot: &'a Path,
    sources: RefCell<BTreeMap<u64, SourceFile>>,
}

impl<'a> Asm<'a> {
    pub fn new(workspace: &'a Path, sysroot: &'a Path) -> Self {
        Self {
            workspace,
            sysroot,
            sources: Default::default(),
        }
    }
}

impl Dumpable for Asm<'_> {
    type Line<'l> = Statement<'l>;

    fn split_lines(contents: &str) -> anyhow::Result<Vec<Self::Line<'_>>> {
        parse_file(contents)
    }

    fn find_items(lines: &[Self::Line<'_>]) -> BTreeMap<Item, Range<usize>> {
        find_items(lines)
    }

    fn dump_range(&self, fmt: &Format, lines: &[Self::Line<'_>]) -> anyhow::Result<()> {
        dump_range(&self.sources.borrow(), fmt, 0..lines.len(), lines)
    }

    fn extra_context(
        &self,
        fmt: &Format,
        lines: &[Self::Line<'_>],
        range: Range<usize>,
        items: &BTreeMap<Item, Range<usize>>,
    ) -> Vec<Range<usize>> {
        let mut res = get_context_for(fmt.context, lines, range.clone(), items);
        if fmt.rust {
            load_rust_sources(
                self.sysroot,
                self.workspace,
                lines,
                fmt,
                &mut self.sources.borrow_mut(),
            );
        }

        if fmt.include_constants {
            let print_range = URange::from(range.clone());
            // scan for referenced constants such as strings, scan needs to be done recursively
            let mut pending = vec![print_range];
            let mut seen: BTreeSet<URange> = BTreeSet::new();

            // Let's define a constant as a label followed by one or more data declarations
            let constants = lines
                .iter()
                .enumerate()
                .filter_map(|(ix, stmt)| {
                    let Statement::Label(Label { id, .. }) = stmt else {
                        return None;
                    };
                    matches!(
                        lines.get(ix + 1),
                        Some(Statement::Directive(Directive::Data(_, _)))
                    )
                    .then_some((*id, ix))
                })
                .collect::<BTreeMap<_, _>>();
            while let Some(subset) = pending.pop() {
                seen.insert(subset);
                for s in &lines[subset] {
                    if let Statement::Instruction(Instruction {
                        args: Some(arg), ..
                    })
                    | Statement::Directive(Directive::Generic(GenericDirective(arg))) = s
                    {
                        for label in crate::demangle::local_labels(arg) {
                            if let Some(constant_range) = scan_constant(label, &constants, lines) {
                                if !seen.contains(&constant_range)
                                    && !print_range.fully_contains(constant_range)
                                {
                                    pending.push(constant_range);
                                }
                            }
                        }
                    }
                }
            }
            seen.remove(&print_range);
            for range in &seen {
                res.push(range.start..range.end);
            }
        }

        if fmt.simplify {
            res.retain(|range| {
                lines[range.start..range.end]
                    .iter()
                    .any(|s| !(s.boring() || matches!(s, Statement::Nothing | Statement::Label(_))))
            });
        }

        res
    }
}
