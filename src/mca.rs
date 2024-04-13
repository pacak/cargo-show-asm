use crate::{
    asm::{Directive, Statement},
    demangle, esafeprintln,
    opts::Format,
    safeprintln, Dumpable,
};
use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
};

pub struct Mca<'a> {
    /// mca specific arguments
    args: &'a [String],
    /// Use intel syntax?
    use_intel_syntax: bool,
    target_triple: Option<&'a str>,
    target_cpu: Option<&'a str>,
}
impl<'a> Mca<'a> {
    pub fn new(
        mca_args: &'a [String],
        use_intel_syntax: bool,
        target_triple: Option<&'a str>,
        target_cpu: Option<&'a str>,
    ) -> Self {
        Self {
            args: mca_args,
            use_intel_syntax,
            target_triple,
            target_cpu,
        }
    }
}

impl Dumpable for Mca<'_> {
    type Line<'a> = Statement<'a>;

    fn split_lines(contents: &str) -> anyhow::Result<Vec<Self::Line<'_>>> {
        crate::asm::parse_file(contents)
    }

    fn find_items(
        lines: &[Self::Line<'_>],
    ) -> std::collections::BTreeMap<crate::Item, std::ops::Range<usize>> {
        crate::asm::find_items(lines)
    }

    fn dump_range(&self, fmt: &Format, lines: &[Self::Line<'_>]) -> anyhow::Result<()> {
        use std::io::Write;

        let mut mca = Command::new("llvm-mca");
        mca.args(self.args)
            .args(self.target_triple.iter().flat_map(|t| ["--mtriple", t]))
            .args(self.target_cpu.iter().flat_map(|t| ["--mcpu", t]))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if fmt.verbosity >= 2 {
            safeprintln!("running {mca:?}");
        }
        let mca = mca.spawn();
        let mut mca = match mca {
            Ok(mca) => mca,
            Err(err) => {
                esafeprintln!("Failed to start llvm-mca, do you have it installed? The error was");
                esafeprintln!("{err}");
                std::process::exit(1);
            }
        };

        let mut i = mca.stdin.take().expect("Stdin should be piped");
        let o = mca.stdout.take().expect("Stdout should be piped");
        let e = mca.stderr.take().expect("Stderr should be piped");

        if self.use_intel_syntax {
            writeln!(i, ".intel_syntax")?;
        }

        for line in lines.iter() {
            match line {
                Statement::Label(l) => writeln!(i, "{}:", l.id)?,
                Statement::Directive(dir) => match dir {
                    Directive::File(_) | Directive::Loc(_) | Directive::SubsectionsViaSym => {}
                    Directive::SectionStart(ss) => writeln!(i, ".section {ss}")?,
                    Directive::Generic(gen) => writeln!(i, ".{gen}")?,
                    Directive::Set(set) => writeln!(i, ".set {set}")?,
                },
                Statement::Instruction(instr) => match instr.args {
                    Some(args) => writeln!(i, "{} {}", instr.op, args)?,
                    None => writeln!(i, "{}", instr.op)?,
                },
                Statement::Nothing => {}
                // we couldn't parse it, maybe mca can?
                Statement::Dunno(unk) => writeln!(i, "{unk}")?,
            }
        }
        drop(i);

        for line in BufRead::lines(BufReader::new(o)) {
            let line = line?;
            let line = demangle::contents(&line, fmt.name_display);
            safeprintln!("{line}");
        }

        for line in BufRead::lines(BufReader::new(e)) {
            let line = line?;
            esafeprintln!("{line}");
        }

        Ok(())
    }
}
