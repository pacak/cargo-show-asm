use std::{
    io::{BufRead, BufReader},
    path::Path,
    process::{Command, Stdio},
};

use crate::{
    demangle, get_dump_range,
    opts::{Format, ToDump},
};

pub fn dump_function(
    goal: ToDump,
    path: &Path,
    fmt: &Format,
    mca_intel: bool,
    mca_args: &[String],
) -> anyhow::Result<()> {
    let contents = std::fs::read_to_string(path)?;
    let statements = crate::asm::parse_file(&contents)?;
    let functions = crate::asm::find_items(&statements);

    let lines = contents.lines().collect::<Vec<_>>();

    let lines = if let Some(range) = get_dump_range(goal, *fmt, functions) {
        &lines[range]
    } else {
        if fmt.verbosity > 0 {
            println!("Going to use the whole file");
        }
        &lines
    };

    let mut mca = Command::new("llvm-mca")
        .args(mca_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut i = mca.stdin.take().unwrap();
    let o = mca.stdout.take().unwrap();
    let e = mca.stderr.take().unwrap();

    use std::io::Write;
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
        println!("{line}");
    }

    for line in BufRead::lines(BufReader::new(e)) {
        let line = line?;
        eprintln!("{line}");
    }

    Ok(())
}

