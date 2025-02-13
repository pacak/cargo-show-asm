use crate::{asm::Statement, demangle, esafeprintln, opts::Format, safeprintln, Dumpable};
use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
};

pub struct Mca<'a> {
    /// mca specific arguments
    args: &'a [String],
    target_triple: Option<&'a str>,
    target_cpu: Option<&'a str>,
    intel_syntax: bool,
}
impl<'a> Mca<'a> {
    pub fn new(
        mca_args: &'a [String],
        target_triple: Option<&'a str>,
        target_cpu: Option<&'a str>,
    ) -> Self {
        Self {
            args: mca_args,
            target_triple,
            target_cpu,
            intel_syntax: false,
        }
    }
}

impl Dumpable for Mca<'_> {
    type Line<'a> = Statement<'a>;

    fn split_lines(contents: &str) -> anyhow::Result<Vec<Self::Line<'_>>> {
        crate::asm::parse_file(contents)
    }

    fn init(&mut self, lines: &[Self::Line<'_>]) {
        use crate::asm::{Directive, GenericDirective, Statement};
        for line in lines {
            let Statement::Directive(Directive::Generic(GenericDirective(dir))) = line else {
                return;
            };
            if dir.contains("intel_syntax") {
                self.intel_syntax = true;
            }
        }
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

        if fmt.verbosity >= 3 {
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

        if self.intel_syntax {
            // without that llvm-mca gets confused for some instructions
            writeln!(i, ".intel_syntax")?
        }

        for line in lines.iter() {
            match line {
                Statement::Label(l) => writeln!(i, "{}:", l.id)?,
                Statement::Directive(_) => {}
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
