use crate::{
    demangle::{self, demangled},
    opts::{Format, NameDisplay, OutputStyle, ToDump},
    pick_dump_item, safeprintln, Item,
};
use ar::Archive;
use capstone::{Capstone, Insn};
use object::{
    Architecture, Object, ObjectSection, ObjectSymbol, Relocation, RelocationTarget, SectionIndex,
    SymbolKind,
};
use std::{collections::BTreeMap, path::Path};

struct Reloc<'a> {
    name: &'a str,
    name_display: NameDisplay,
}

impl std::fmt::Display for Reloc<'_> {
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
        for byte in self.bytes.iter() {
            write!(f, "{:02x} ", byte)?;
        }
        for _ in 0..(1 + self.max_width - self.bytes.len()) {
            f.write_str("   ")?;
        }
        Ok(())
    }
}

/// disassemble rlib or exe, one file at a time
pub fn dump_disasm(
    goal: ToDump,
    file: &Path,
    fmt: &Format,
    syntax: OutputStyle,
) -> anyhow::Result<()> {
    if file.extension().map_or(false, |e| e == "rlib") {
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
        dump_slices(goal, slices.as_slice(), fmt, syntax)
    } else {
        let binary_data = std::fs::read(file)?;
        dump_slices(goal, &[binary_data][..], fmt, syntax)
    }
}

fn pick_item<'a>(
    goal: ToDump,
    files: &'a [object::File],
    fmt: &Format,
) -> anyhow::Result<(&'a object::File<'a>, SectionIndex, usize, usize)> {
    let mut items = BTreeMap::new();

    for file in files {
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

            let len = symbol.size() as usize; // sorry 32bit platforms, you are not real
            if len == 0 {
                continue;
            }
            let addr = symbol.address() as usize;
            let item = Item {
                name,
                hashed,
                index,
                len,
                non_blank_len: len,
                mangled_name: raw_name.to_owned(),
            };
            items.insert(item, (file, section_index, addr, len));
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
) -> Option<Reloc<'a>> {
    let addr = insn.address();
    let range = addr..addr + insn.len() as u64;
    let (_range, relocation) = reloc_map.range(range).next()?;
    let name = match relocation.target() {
        RelocationTarget::Symbol(sym) => file.symbol_by_index(sym).ok()?.name().ok(),
        RelocationTarget::Section(sec) => file.section_by_index(sec).ok()?.name().ok(),
        RelocationTarget::Absolute => None,
        _ => None,
    }?;
    Some(Reloc {
        name,
        name_display: fmt.name_display,
    })
}

fn dump_slices(
    goal: ToDump,
    binary_data: &[Vec<u8>],
    fmt: &Format,
    syntax: OutputStyle,
) -> anyhow::Result<()> {
    let files = binary_data
        .iter()
        .map(|data| object::File::parse(data.as_slice()))
        .collect::<Result<Vec<_>, _>>()?;
    let (file, section_index, addr, len) = pick_item(goal, &files, fmt)?;
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
                let reloc = Reloc {
                    name,
                    name_display: fmt.name_display,
                };
                (s.address(), reloc)
            })
            .collect::<BTreeMap<_, _>>()
    } else {
        BTreeMap::new()
    };

    let start = addr - section.address() as usize;
    let cs = make_capstone(file, syntax)?;
    let code = &section.data()?[start..start + len];

    if fmt.verbosity >= 2 {
        if reloc_map.is_empty() {
            safeprintln!("There is no relocation table");
        } else {
            safeprintln!("{:?}", reloc_map);
        }
    }

    let insns = cs.disasm_all(code, addr as u64)?;

    let max_width = insns.iter().map(|i| i.len()).max().unwrap_or(1);

    for insn in insns.iter() {
        let hex = HexDump {
            max_width,
            bytes: if fmt.simplify { &[] } else { insn.bytes() },
        };

        let interesting_addr = if !reloc_map.is_empty() {
            // binary contains a relocation map - use that
            false
        } else {
            // otherwise check if instruction is dealing with control flow
            //
            // group ids/names are not stable so look for their string representation.
            // this allocates but at most once per instruction
            *opcode_cache.entry(insn.op_str()).or_insert_with(|| {
                cs.insn_detail(insn)
                    .expect("Can't get instruction info")
                    .groups()
                    .iter()
                    .any(|g| matches!(cs.group_name(*g).as_deref(), Some("call" | "jump")))
            })
        };

        if let Some(op) = insn.mnemonic() {
            let i = crate::asm::Instruction {
                op,
                args: insn.op_str(),
            };
            if let Some(reloc) = reloc_info(file, &reloc_map, insn, fmt) {
                safeprintln!("{:8x}:    {hex}{i} # {reloc}", insn.address());
            } else if let Some(reloc) = interesting_addr
                .then(|| get_reference(&cs, insn))
                .flatten()
                .and_then(|addr| symbol_names.get(&addr))
            {
                safeprintln!("{:8x}:    {hex}{i} # {reloc}", insn.address());
            } else {
                safeprintln!("{:8x}:    {hex}{i}", insn.address())
            }
        }
    }

    Ok(())
}

fn get_reference(cs: &Capstone, insn: &Insn) -> Option<u64> {
    use capstone::arch::{
        arm64::Arm64OperandType, x86::X86OperandType, ArchDetail, DetailsArchInsn,
    };
    let details = cs.insn_detail(insn).unwrap();
    match details.arch_detail() {
        ArchDetail::X86Detail(x86) => match x86.operands().next()?.op_type {
            X86OperandType::Imm(rel) => Some(rel.try_into().unwrap()),
            X86OperandType::Mem(mem) => {
                assert_eq!(mem.scale(), 1);
                (insn.address() + insn.len() as u64).checked_add_signed(mem.disp())
            }
            _ => None, // ¯\_ (ツ)_/¯
        },

        // I have no idea what I'm doing here :)
        ArchDetail::Arm64Detail(arm) => match arm.operands().next()?.op_type {
            Arm64OperandType::Imm(rel) => Some(rel.try_into().unwrap()),
            Arm64OperandType::Mem(mem) => {
                (insn.address() + insn.len() as u64).checked_add_signed(mem.disp() as i64)
            }
            _ => None, // ¯\_ (ツ)_/¯
        },

        _ => None,
    }
}

impl From<OutputStyle> for capstone::Syntax {
    fn from(value: OutputStyle) -> Self {
        match value {
            OutputStyle::Intel => Self::Intel,
            OutputStyle::Att => Self::Att,
        }
    }
}

fn make_capstone(file: &object::File, syntax: OutputStyle) -> anyhow::Result<Capstone> {
    use capstone::{
        arch::{self, BuildsCapstone},
        Endian,
    };

    let endiannes = match file.endianness() {
        object::Endianness::Little => Endian::Little,
        object::Endianness::Big => Endian::Big,
    };
    let x86_width = if file.is_64() {
        arch::x86::ArchMode::Mode64
    } else {
        arch::x86::ArchMode::Mode32
    };

    let mut capstone = match file.architecture() {
        Architecture::Aarch64 => Capstone::new().arm64().build()?,
        Architecture::X86_64 => Capstone::new().x86().mode(x86_width).build()?,
        unknown => anyhow::bail!("Dunno how to decompile {unknown:?}"),
    };
    capstone.set_syntax(syntax.into())?;
    capstone.set_detail(true)?;
    capstone.set_endian(endiannes)?;
    Ok(capstone)
}
