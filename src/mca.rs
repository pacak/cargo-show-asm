use std::{
    io::{BufRead, BufReader},
    path::Path,
    process::{Command, Stdio},
};

use crate::{
    demangle, esafeprintln, get_dump_range,
    opts::{Format, ToDump},
    safeprintln,
};

/// dump mca analysis
///
/// # Errors
/// Clippy, why do you care?
pub fn dump_function(
    goal: ToDump,
    path: &Path,
    fmt: &Format,
    mca_intel: bool,
    triple: &Option<String>,
    target_cpu: &Option<String>,
) -> anyhow::Result<()> {
    use std::io::Write;

    let contents = std::fs::read_to_string(path)?;
    let statements = crate::asm::parse_file(&contents)?;
    let functions = crate::asm::find_items(&statements);

    let lines = contents.lines().collect::<Vec<_>>();

    let lines = if let Some(range) = get_dump_range(goal, fmt, functions) {
        &lines[range]
    } else {
        if fmt.verbosity > 0 {
            safeprintln!("Going to use the whole file");
        }
        &lines
    };

    let mut mca = Command::new("llvm-mca");
    mca.args(&fmt.mca_arg)
        .args(triple.iter().flat_map(|t| ["--mtriple", t]))
        .args(target_cpu.iter().flat_map(|t| ["--mcpu", t]))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if fmt.verbosity >= 2 {
        safeprintln!("running {:?}", mca);
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

    if mca_intel {
        writeln!(i, ".intel_syntax")?;
    }

    'outer: for line in lines {
        let line = line.trim();
        for skip in [".loc", ".file"] {
            if line.starts_with(skip) {
                continue 'outer;
            }
        }

        writeln!(i, "{line}")?;
    }
    writeln!(i, ".cfi_endproc")?;
    drop(i);

    for line in BufRead::lines(BufReader::new(o)) {
        let line = line?;
        let line = demangle::contents(&line, fmt.full_name);
        safeprintln!("{line}");
    }

    for line in BufRead::lines(BufReader::new(e)) {
        let line = line?;
        esafeprintln!("{line}");
    }

    Ok(())
}
