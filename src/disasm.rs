use crate::{
    Item, color,
    demangle::{self, demangled},
    esafeprintln,
    opts::{Format, NameDisplay, OutputStyle, ToDump},
    pick_dump_item, safeprintln,
    sources::{File, SourceFileIndex},
};
use addr2line::Location;
use anyhow::Context as _;
use ar::Archive;
use capstone::{Capstone, Insn};
use object::{
    Architecture, Object, ObjectSection, ObjectSymbol, Relocation, RelocationTarget, SectionIndex,
    SymbolKind,
};
use owo_colors::OwoColorize;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fmt::Write as _,
    path::Path,
};

/// Reference to some other symbol
#[derive(Copy, Clone)]
struct Reference<'a> {
    name: &'a str,
    name_display: NameDisplay,
}

impl std::fmt::Display for Reference<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", demangle::contents(self.name, self.name_display))
    }
}

struct HexDump<'a> {
    max_width: usize,
    bytes: &'a [u8],
}

impl std::fmt::Display for HexDump<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.bytes.is_empty() {
            return Ok(());
        }
        for byte in self.bytes {
            write!(f, "{byte:02x} ")?;
        }
        for _ in 0..(1 + self.max_width - self.bytes.len()) {
            f.write_str("   ")?;
        }
        Ok(())
    }
}

#[derive(Copy, Clone)]
struct PickedItem {
    file_idx: usize,
    section_index: SectionIndex,
    addr: usize,
    len: usize,
}

/// disassemble rlib or exe, one file at a time
///
/// `source_files` provides the workspace/sysroot roots used to resolve and read Rust sources.
pub fn dump_disasm(
    goal: ToDump,
    file: &Path,
    fmt: &Format,
    syntax: OutputStyle,
    source_files: SourceFileIndex,
) -> anyhow::Result<()> {
    if file.extension().is_some_and(|e| e == "rlib") {
        let mut slices = Vec::new();
        let mut archive = Archive::new(std::fs::File::open(file)?);

        while let Some(entry) = archive.next_entry() {
            let mut entry = entry?;
            let name = std::str::from_utf8(entry.header().identifier())?;
            if !name.ends_with(".o") {
                continue;
            }
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut bytes)?;
            slices.push(bytes);
        }
        dump_slices(goal, slices.as_slice(), fmt, syntax, source_files, None)
    } else {
        let binary_data = std::fs::read(file)?;
        dump_slices(
            goal,
            &[binary_data][..],
            fmt,
            syntax,
            source_files,
            Some(file),
        )
    }
}

fn pick_item(goal: ToDump, files: &[object::File], fmt: &Format) -> anyhow::Result<PickedItem> {
    let mut items = BTreeMap::new();

    for (file_idx, file) in files.iter().enumerate() {
        let mut addresses: Vec<_> = file
            .symbols()
            .filter(|s| s.is_definition() && s.kind() == SymbolKind::Text)
            .map(|s| s.address() as usize)
            .collect();
        addresses.sort_unstable();

        for (index, symbol) in file
            .symbols()
            .filter(|s| s.is_definition() && s.kind() == SymbolKind::Text)
            .enumerate()
        {
            let raw_name = symbol.name()?;
            let (name, hashed) = match demangled(raw_name) {
                Some(dem) => (format!("{dem:#?}"), format!("{dem:?}")),
                None => (raw_name.to_owned(), raw_name.to_owned()),
            };

            let Some(section_index) = symbol.section_index() else {
                // external symbol?
                continue;
            };

            let addr = symbol.address() as usize;
            let mut len = symbol.size() as usize; // sorry 32bit platforms, you are not real
            if len == 0 {
                // Most symbols do not have a size.
                // Guess size from the address of the next symbol after it.
                let (Ok(idx) | Err(idx)) = addresses.binary_search(&addr);
                let next_address = match addresses[idx..].iter().copied().find(|&a| a > addr) {
                    Some(addr) => addr,
                    None => {
                        let section = file.section_by_index(section_index)?;
                        (section.address() + section.size()) as usize
                    }
                };
                len = next_address - addr;
            }
            let item = Item {
                name,
                hashed,
                index,
                len,
                non_blank_len: len,
                mangled_name: raw_name.to_owned(),
                depth: None,
            };
            items.insert(
                item,
                PickedItem {
                    file_idx,
                    section_index,
                    addr,
                    len,
                },
            );
        }
    }

    // there are things that can be supported and there are things that I consider useful to
    // support. --everything with --disasm is not one of them for now
    pick_dump_item(goal, fmt, &items)
        .ok_or_else(|| anyhow::anyhow!("no can do --everything with --disasm"))
}

/// Get printable name from relocation info
fn reloc_info<'a>(
    file: &'a object::File,
    reloc_map: &'a BTreeMap<u64, Relocation>,
    insn: &Insn,
    fmt: &Format,
) -> Option<Reference<'a>> {
    let addr = insn.address();
    let range = addr..addr + insn.len() as u64;
    let (_range, relocation) = reloc_map.range(range).next()?;
    let name = match relocation.target() {
        RelocationTarget::Symbol(sym) => file.symbol_by_index(sym).ok()?.name().ok(),
        RelocationTarget::Section(sec) => file.section_by_index(sec).ok()?.name().ok(),
        RelocationTarget::Absolute => None,
        _ => None,
    }?;
    Some(Reference {
        name,
        name_display: fmt.name_display,
    })
}

fn dwarf_from_object(obj: &object::File) -> anyhow::Result<Addr2LineCtx> {
    let endian = if obj.is_little_endian() {
        gimli::RunTimeEndian::Little
    } else {
        gimli::RunTimeEndian::Big
    };

    let dwarf = gimli::Dwarf::load(|id: gimli::SectionId| -> Result<_, gimli::Error> {
        let data: std::rc::Rc<[u8]> = obj
            .section_by_name(id.name())
            .and_then(|section| section.uncompressed_data().ok())
            .map_or_else(
                || std::rc::Rc::from(&[][..]),
                |cow| std::rc::Rc::from(&*cow),
            );
        Ok(gimli::EndianRcSlice::new(data, endian))
    })
    .context("failed to load DWARF sections")?;

    addr2line::Context::from_dwarf(dwarf).context("failed to build an addr2line context")
}

/// On macOS debug info lives in a separate `.dSYM` bundle rather than in the
/// binary itself. The DWARF file inside the bundle is named after the build
/// artifact (e.g. `cargo_asm-2d5399dd0f42d340` vs `cargo-asm`) due to Cargo's
/// hashing, enumerate the directory instead of guessing the filename.
///
/// `Ok(None)` means there is no bundle here and the caller should fall back to
/// the object's own debug sections; `Err` means a bundle exists but is unusable.
#[cfg(target_os = "macos")]
fn dsym_dwarf(binary_path: Option<&Path>) -> anyhow::Result<Option<Addr2LineCtx>> {
    let Some(binary) = binary_path else {
        return Ok(None);
    };
    let (Some(name), Some(parent)) = (binary.file_name(), binary.parent()) else {
        return Ok(None);
    };

    let bundle = parent.join(format!("{}.dSYM", name.to_string_lossy()));
    if !bundle.is_dir() {
        return Ok(None);
    }

    let dwarf_dir = bundle.join("Contents/Resources/DWARF");
    let dwarf_path = std::fs::read_dir(&dwarf_dir)
        .with_context(|| format!("can't read the DWARF directory {}", dwarf_dir.display()))?
        .filter_map(Result::ok)
        .find_map(|entry| {
            let path = entry.path();
            path.is_file().then_some(path)
        })
        .with_context(|| format!("no DWARF file inside {}", dwarf_dir.display()))?;

    let data = std::fs::read(&dwarf_path)
        .with_context(|| format!("can't read {}", dwarf_path.display()))?;
    let obj = object::File::parse(&*data)
        .with_context(|| format!("can't parse {}", dwarf_path.display()))?;

    dwarf_from_object(&obj)
        .map(Some)
        .with_context(|| format!("in the dSYM bundle {}", bundle.display()))
}

#[cfg(not(target_os = "macos"))]
fn dsym_dwarf(_binary_path: Option<&Path>) -> anyhow::Result<Option<Addr2LineCtx>> {
    Ok(None)
}

fn make_addr2line_context(
    object_data: &[u8],
    binary_path: Option<&Path>,
) -> anyhow::Result<Addr2LineCtx> {
    if let Some(ctx) = dsym_dwarf(binary_path)? {
        return Ok(ctx);
    }

    let obj = object::File::parse(object_data).context("can't parse the object for debug info")?;
    dwarf_from_object(&obj)
}

/// A DWARF lookup context that owns its section data, see [`dwarf_from_object`].
type Addr2LineCtx = addr2line::Context<gimli::EndianRcSlice<gimli::RunTimeEndian>>;

/// Tracks source location state for inline annotations in disasm output.
struct SourceLookup<'a> {
    ctx: Addr2LineCtx,
    display: SourceDisplay<'a>,
    failed_lookups: usize,
    successful_lookups: usize,
}

struct SourceDisplay<'a> {
    path_to_index_file: HashMap<String, File<'static>>,
    source_file_index: SourceFileIndex<'a>,
    fmt: &'a Format,
    /// Last annotated (file index, line), to avoid re-printing the same location.
    prev_loc: Option<(u64, u64)>,
}

impl<'a> SourceLookup<'a> {
    fn new(ctx: Addr2LineCtx, source_file_index: SourceFileIndex<'a>, fmt: &'a Format) -> Self {
        Self {
            ctx,
            failed_lookups: 0,
            successful_lookups: 0,
            display: SourceDisplay {
                path_to_index_file: HashMap::new(),
                source_file_index,
                fmt,
                prev_loc: None,
            },
        }
    }

    /// Show a source annotation for the instruction at `addr`, if available.
    fn show(&mut self, addr: u64) {
        // split dwarf (.dwo) isn't supported due to skip_all_loads()
        let mut frames = match self.ctx.find_frames(addr).skip_all_loads() {
            Ok(frames) => frames,
            Err(e) => {
                if self.failed_lookups == 0 {
                    safeprintln!("Warning: addr2line lookup failed at {addr:#x}: {e}");
                }
                self.failed_lookups += 1;
                return;
            }
        };

        while let Ok(Some(frame)) = frames.next() {
            let Some(Location {
                file: Some(file),
                line: Some(line),
                ..
            }) = frame.location
            else {
                continue;
            };
            if line == 0 {
                continue;
            }
            if self.display.show_location(file, line.into()) {
                self.successful_lookups += 1;
                return;
            }
        }
        self.failed_lookups += 1;
    }

    fn report(&self) {
        if self.successful_lookups == 0 && self.failed_lookups > 0 {
            safeprintln!(
                "Warning: --rust: no source locations found for any of the {} instructions",
                self.failed_lookups
            );
        }
    }
}

impl<'a> SourceDisplay<'a> {
    fn show_location(&mut self, file_path: &str, line: u64) -> bool {
        if !self.path_to_index_file.contains_key(file_path) {
            let index = self.path_to_index_file.len() as u64;
            let file = File {
                index,
                path: crate::sources::FilePath::FullPath(file_path.into()),
                md5: None,
            };
            self.source_file_index.load(&file, self.fmt);
            self.path_to_index_file.insert(file_path.to_string(), file);
        }

        let file = &self.path_to_index_file[file_path];

        if self.prev_loc == Some((file.index, line)) {
            return true;
        }

        let Some((display_path, content)) = self.source_file_index.get(file.index) else {
            return false;
        };

        if content
            .as_ref()
            .is_some_and(|(s, _)| !s.show_for(self.fmt.sources_from))
        {
            return false;
        }

        self.prev_loc = Some((file.index, line));

        let pos = format!("\t\t// {display_path}:{line}");
        safeprintln!("{}", color!(pos, OwoColorize::cyan));
        if let Some((_, cached)) = content {
            if let Some(src_line) = cached.get(line as usize - 1) {
                safeprintln!(
                    "\t\t{}",
                    color!(src_line.trim_start(), OwoColorize::bright_red)
                );
            }
        }
        true
    }
}

fn dump_slices(
    goal: ToDump,
    binary_data: &[Vec<u8>],
    fmt: &Format,
    syntax: OutputStyle,
    source_files: SourceFileIndex,
    binary_path: Option<&Path>,
) -> anyhow::Result<()> {
    let files = binary_data
        .iter()
        .map(|data| object::File::parse(data.as_slice()))
        .collect::<Result<Vec<_>, _>>()?;
    let PickedItem {
        file_idx,
        section_index,
        addr,
        len,
    } = pick_item(goal, &files, fmt)?;
    let file = &files[file_idx];
    let file_data = &binary_data[file_idx];
    let mut opcode_cache = BTreeMap::new();

    let section = file.section_by_index(section_index)?;
    let reloc_map = section.relocations().collect::<BTreeMap<_, _>>();

    // if relocation map is present - addresses are going to be base 0 = useless
    //
    // For executable files there will be just one section...
    let symbol_names = if reloc_map.is_empty() {
        files
            .iter()
            .flat_map(|f| f.symbols())
            .map(|s| {
                let name = s.name().unwrap();
                let name = name.split_once('$').map_or(name, |(p, _)| p);
                let reloc = Reference {
                    name,
                    name_display: fmt.name_display,
                };
                (s.address(), reloc)
            })
            .collect::<BTreeMap<_, _>>()
    } else {
        BTreeMap::new()
    };

    // In ARM ELF files, bit zero of the symbol address indicates its encoding.
    // It is one for Thumb-v2 (aka "t32") instructions and zero for ARM (aka
    // "a32") instructions. See ARM Arch ABI, 2024Q3, ELF, section 5.5.3.
    let is_thumb = addr & 1 == 1;
    let addr = addr & !1;
    let start = addr - section.address() as usize;
    let cs = make_capstone(file, syntax, is_thumb)?;
    let code = &section.data()?[start..start + len];

    if fmt.verbosity >= 2 {
        if reloc_map.is_empty() {
            safeprintln!("There is no relocation table");
        } else {
            safeprintln!("reloc_map {:#?}", reloc_map);
        }
    }

    let insns = cs.disasm_all(code, addr as u64)?;
    if insns.is_empty() {
        if fmt.verbosity > 0 {
            safeprintln!("No instructions - empty code block?");
        }
        return Ok(());
    }

    let max_width = insns.iter().map(|i| i.len()).max().unwrap_or(1);

    // branch target addresses for local labels
    let addrs = insns
        .iter()
        .map(|insn| {
            if *opcode_cache.entry(insn.op_str()).or_insert_with(|| {
                cs.insn_detail(insn)
                    .expect("Can't get instruction info")
                    .groups()
                    .iter()
                    .any(|g| matches!(cs.group_name(*g).as_deref(), Some("call" | "jump")))
            }) {
                get_branch_target(&cs, insn)
                    .filter(|addr| *addr != insn.address() + insn.len() as u64)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let local_range = insns[0].address()..insns.last().unwrap().address();

    let local_labels = addrs
        .iter()
        .copied()
        .flatten()
        .filter(|addr| local_range.contains(addr))
        .collect::<BTreeSet<_>>();
    let local_labels = local_labels
        .into_iter()
        .enumerate()
        .map(|n| (n.1, n.0))
        .collect::<BTreeMap<_, _>>();

    // Build source display context for --rust annotations via DWARF debug info.
    // Uses the same .o file that contains the selected function (important for rlibs).

    let mut source_display = if fmt.rust {
        match make_addr2line_context(file_data, binary_path) {
            Ok(ctx) => Some(SourceLookup::new(ctx, source_files, fmt)),
            Err(err) => {
                esafeprintln!("--rust: no source annotations available: {err:#}");
                None
            }
        }
    } else {
        drop(source_files);
        None
    };

    let mut buf = String::new();
    for (insn, &maddr) in insns.iter().zip(addrs.iter()) {
        let hex = HexDump {
            max_width,
            bytes: if fmt.simplify { &[] } else { insn.bytes() },
        };

        let addr = insn.address();

        if let Some(ref mut sd) = source_display {
            sd.show(addr);
        }

        // binary code will have pending relocations if we are dealing with disassembling a library
        // code or with relocations already applied if we are working with a binary
        let mut refn = reloc_info(file, &reloc_map, insn, fmt)
            .or_else(|| maddr.and_then(|addr| symbol_names.get(&addr).copied()));

        if let Some(id) = local_labels.get(&addr) {
            use owo_colors::OwoColorize;
            safeprintln!(
                "{}{}:",
                crate::color!(".L", OwoColorize::bright_yellow),
                crate::color!(id, OwoColorize::bright_yellow),
            );
        }

        let i = crate::asm::Instruction {
            op: insn.mnemonic().unwrap_or("???"),
            args: insn.op_str(),
        };

        if let Some(id) = maddr.and_then(|a| local_labels.get(&a)) {
            buf.clear();
            write!(
                buf,
                "{}{}",
                color!(".L", OwoColorize::bright_yellow),
                color!(id, OwoColorize::bright_yellow)
            )
            .unwrap();
            refn = Some(Reference {
                name: buf.as_str(),
                name_display: fmt.name_display,
            });
        }

        if let Some(reloc) = refn {
            safeprintln!("{addr:8x}:    {hex}{i} # {reloc}");
        } else {
            safeprintln!("{addr:8x}:    {hex}{i}");
        }
    }

    if let Some(sd) = source_display {
        sd.report();
    }

    Ok(())
}

/// Extract the target address from a call/jump instruction with an immediate operand.
///
/// Returns `None` for indirect branches (through memory or registers) since their
/// targets can't be resolved at disassembly time. GOT calls like `jmp [rip]` are
/// resolved via relocation instead.
fn get_branch_target(cs: &Capstone, insn: &Insn) -> Option<u64> {
    use capstone::arch::{
        ArchDetail, DetailsArchInsn, arm64::Arm64OperandType, x86::X86OperandType,
    };
    let details = cs.insn_detail(insn).unwrap();
    match details.arch_detail() {
        ArchDetail::X86Detail(x86) => match x86.operands().next()?.op_type {
            X86OperandType::Imm(rel) => Some(rel.try_into().unwrap()),
            _ => None,
        },

        ArchDetail::Arm64Detail(arm) => match arm.operands().next()?.op_type {
            Arm64OperandType::Imm(rel) => Some(rel.try_into().unwrap()),
            _ => None,
        },

        _ => None,
    }
}

impl From<OutputStyle> for capstone::arch::x86::ArchSyntax {
    fn from(value: OutputStyle) -> Self {
        match value {
            OutputStyle::Intel => Self::Intel,
            OutputStyle::Att => Self::Att,
        }
    }
}

fn make_capstone(
    file: &object::File,
    syntax: OutputStyle,
    is_thumb: bool,
) -> anyhow::Result<Capstone> {
    use capstone::{
        Endian,
        arch::{self, BuildsCapstone, BuildsCapstoneExtraMode, BuildsCapstoneSyntax},
    };

    let endianness = match file.endianness() {
        object::Endianness::Little => Endian::Little,
        object::Endianness::Big => Endian::Big,
    };

    let mut capstone = match file.architecture() {
        Architecture::Arm if is_thumb => Capstone::new()
            .arm()
            .mode(arch::arm::ArchMode::Thumb)
            .build()?,
        Architecture::Arm => Capstone::new()
            .arm()
            .mode(arch::arm::ArchMode::Arm)
            .build()?,
        Architecture::Aarch64 => Capstone::new()
            .arm64()
            .mode(arch::arm64::ArchMode::Arm)
            .build()?,

        Architecture::I386 => Capstone::new()
            .x86()
            .mode(arch::x86::ArchMode::Mode32)
            .syntax(syntax.into())
            .build()?,
        Architecture::X86_64_X32 | Architecture::X86_64 => Capstone::new()
            .x86()
            .mode(arch::x86::ArchMode::Mode64)
            .syntax(syntax.into())
            .build()?,

        // Capstone obliges us to choose a CPU "mode" even though m68k CPUs only have one: m68k.
        // (Compare with x86 which has 16-, 32- and 64-bit modes.) The mode options it offers are
        // actually between different CPU models. I've picked the 68040 as being the superset of the
        // others. (Pedantically, the 68040 lacks the 68881/68882 trancendental functions, and
        // supervisor mode varies slightly, but LLVM doesn't emit those instructions.)
        Architecture::M68k => Capstone::new()
            .m68k()
            .mode(arch::m68k::ArchMode::M68k040)
            .build()?,

        Architecture::Riscv32 => Capstone::new()
            .riscv()
            .mode(arch::riscv::ArchMode::RiscV32)
            .extra_mode([arch::riscv::ArchExtraMode::RiscVC].into_iter())
            .build()?,
        Architecture::Riscv64 => Capstone::new()
            .riscv()
            .mode(arch::riscv::ArchMode::RiscV64)
            .extra_mode([arch::riscv::ArchExtraMode::RiscVC].into_iter())
            .build()?,

        unknown => anyhow::bail!("Dunno how to decompile {unknown:?}"),
    };
    capstone.set_detail(true)?;
    capstone.set_endian(endianness)?;
    Ok(capstone)
}
