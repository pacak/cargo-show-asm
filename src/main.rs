use anyhow::Context;
use cargo_metadata::{Artifact, Message, MetadataCommand};
use cargo_show_asm::{
    asm::{self, Item},
    color, llvm, mir,
    opts::{self, ToDump},
};
use std::io::BufReader;
use std::path::PathBuf;
use std::process::Stdio;
use std::{collections::BTreeMap, path::Path};

/// This should be called before calling any cli method or printing any output.
fn reset_signal_pipe_handler() -> anyhow::Result<()> {
    #[cfg(target_family = "unix")]
    {
        use nix::sys::signal;
        // Safety: previous handler returned by signal can be invalid and trigger UB if used, we are not
        // keeping it around so it's safe
        unsafe {
            signal::signal(signal::Signal::SIGPIPE, signal::SigHandler::SigDfl)?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn main() -> anyhow::Result<()> {
    reset_signal_pipe_handler()?;

    let opts = opts::options().run();
    owo_colors::set_override(opts.format.color);

    let cargo_path = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let rustc_path = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());

    let sysroot = {
        let output = std::process::Command::new(&rustc_path)
            .arg("--print=sysroot")
            .stdin(Stdio::null())
            .stderr(Stdio::inherit())
            .stdout(Stdio::piped())
            .output()?;
        if !output.status.success() {
            anyhow::bail!(
                "Failed to get sysroot. '{} --print=sysroot' exited with {}",
                rustc_path,
                output.status,
            );
        }
        // `rustc` prints a trailing newline.
        PathBuf::from(std::str::from_utf8(&output.stdout)?.trim_end())
    };
    if opts.format.verbosity > 0 {
        eprintln!("Found sysroot: {}", sysroot.display());
    }

    let metadata = MetadataCommand::new()
        .cargo_path(&cargo_path)
        .manifest_path(&opts.manifest_path)
        .no_deps()
        .exec()?;

    let focus_package = match opts.package {
        Some(name) => metadata
            .packages
            .iter()
            .find(|p| p.name == name)
            .with_context(|| format!("Package '{}' is not found", name))?,
        None if metadata.packages.len() == 1 => &metadata.packages[0],
        None => {
            eprintln!(
                "{:?} refers to multiple packages, you need to specify which one to use",
                opts.manifest_path
            );
            for package in &metadata.packages {
                eprintln!("\t-p {}", package.name);
            }
            anyhow::bail!("Multiple packages found")
        }
    };

    let focus_artifact = match opts.focus {
        Some(focus) => focus,
        None => match focus_package.targets.len() {
            0 => anyhow::bail!("No targets found"),
            1 => opts::Focus::try_from(&focus_package.targets[0])?,
            _ => {
                eprintln!(
                    "{} defines multiple targets, you need to specify which one to use:",
                    focus_package.name
                );
                for target in &focus_package.targets {
                    if let Ok(focus) = opts::Focus::try_from(target) {
                        eprintln!("\t{}", focus.as_cargo_args().collect::<Vec<_>>().join(" "));
                    }
                }
                anyhow::bail!("Multiple targets found")
            }
        },
    };

    let mut cargo_child = {
        use std::ffi::OsStr;

        let mut cmd = std::process::Command::new(&cargo_path);

        // Cargo flags.
        cmd.arg("rustc")
            // General.
            .args([
                "--message-format=json",
                "--color",
                if opts.format.color { "always" } else { "never" },
            ])
            .args(std::iter::repeat("-v").take(opts.format.verbosity))
            // Workspace location.
            .arg("--manifest-path")
            .arg(opts.manifest_path)
            // Artifact selectors.
            .args(["--package", &focus_package.name])
            .args(focus_artifact.as_cargo_args())
            // Compile options.
            .args(opts.dry.then_some("--dry"))
            .args(opts.frozen.then_some("--frozen"))
            .args(opts.locked.then_some("--locked"))
            .args(opts.offline.then_some("--offline"))
            .args(opts.target.iter().flat_map(|t| ["--target", t]))
            .args(
                opts.target_dir
                    .iter()
                    .flat_map(|t| [OsStr::new("--target-dir"), t.as_ref()]),
            )
            .args(
                opts.cli_features
                    .no_default_features
                    .then_some("--no-default-features"),
            )
            .args(opts.cli_features.all_features.then_some("--all-features"))
            .args(
                opts.cli_features
                    .features
                    .iter()
                    .flat_map(|feat| ["--features", feat]),
            );
        match opts.compile_mode {
            opts::CompileMode::Dev => {}
            opts::CompileMode::Release => {
                cmd.arg("--release");
            }
            opts::CompileMode::Custom(profile) => {
                cmd.args(["--profile", &profile]);
            }
        }

        // Cargo flags terminator.
        cmd.arg("--");

        // Rustc flags.
        // We care about asm.
        cmd.args(["--emit", opts.syntax.emit()])
            // So only one file gets created.
            .arg("-Ccodegen-units=1")
            // Debug info is needed to map to rust source.
            .arg("-Cdebuginfo=2")
            .args(opts.syntax.format().iter().flat_map(|s| ["-C", s]))
            .args(
                opts.target_cpu
                    .iter()
                    .map(|cpu| format!("-Ctarget-cpu={}", cpu)),
            );

        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?
    };

    let mut result_artifact = None;
    let mut success = false;
    for msg in Message::parse_stream(BufReader::new(cargo_child.stdout.take().unwrap())) {
        match msg? {
            Message::CompilerArtifact(artifact) if focus_artifact.matches_artifact(&artifact) => {
                result_artifact = Some(artifact);
            }
            Message::BuildFinished(fin) => {
                success = fin.success;
                break;
            }
            Message::CompilerMessage(msg) => {
                eprintln!("{}", msg);
            }
            _ => {}
        }
    }
    if !success {
        let status = cargo_child.wait()?;
        eprintln!("Cargo failed with {}", status);
        std::process::exit(101);
    }
    let artifact = result_artifact.context("No artifact found")?;

    if opts.format.verbosity > 0 {
        eprintln!("Artifact files: {:?}", artifact.filenames);
    }

    let asm_path = locate_asm_path_via_artifact(&artifact, opts.syntax.ext())?;
    if opts.format.verbosity > 0 {
        eprintln!("Asm file: {}", asm_path.display());
    }

    let mut target_function = match &opts.to_dump {
        ToDump::Everything => None,
        ToDump::Function { function, nth } => Some((function.as_deref().unwrap_or(""), *nth)),
    };

    // this variable exists to deal with the case where there's only
    // one matching function - we might as well show it to the user directly
    let mut single_target;
    let mut existing = Vec::new();
    let mut seen;

    loop {
        seen = match opts.syntax {
            opts::Syntax::Intel | opts::Syntax::Att => asm::dump_function(
                target_function,
                &asm_path,
                &sysroot,
                &opts.format,
                &mut existing,
            ),
            opts::Syntax::Llvm => {
                llvm::dump_function(target_function, &asm_path, &opts.format, &mut existing)
            }
            opts::Syntax::Mir => {
                mir::dump_function(target_function, &asm_path, &opts.format, &mut existing)
            }
        }?;
        if seen {
            return Ok(());
        } else if existing.len() == 1 {
            single_target = existing[0].name.clone();
            target_function = Some((&single_target, 0));
        } else {
            break;
        }
    }

    if let (false, ToDump::Function { function, .. }) = (seen, &opts.to_dump) {
        suggest_name(
            function.as_deref().unwrap_or(""),
            opts.format.full_name,
            &existing,
        )?;
    }

    Ok(())
}

fn locate_asm_path_via_artifact(artifact: &Artifact, expect_ext: &str) -> anyhow::Result<PathBuf> {
    // For lib, test, bench, lib-type example, `filenames` hint the file stem of the asm file.
    // We could locate asm files precisely.
    //
    // `filenames`:
    // [..]/target/debug/deps/libfoo-01234567.rmeta         # lib by-product
    // [..]/target/debug/deps/foo-01234567                  # test & bench
    // [..]/target/debug/deps/example/libfoo-01234567.rmeta # lib-type example by-product
    // Asm files:
    // [..]/target/debug/deps/foo-01234567.s
    // [..]/target/debug/deps/example/foo-01234567.s
    if let Some(path) = artifact
        .filenames
        .iter()
        .filter(|path| {
            matches!(
                path.parent().unwrap().file_name(),
                Some("deps" | "examples")
            )
        })
        .find_map(|path| {
            let path = path.with_extension(expect_ext);
            if path.exists() {
                return Some(path);
            }
            let path = path.with_file_name(path.file_name()?.strip_prefix("lib")?);
            if path.exists() {
                return Some(path);
            }
            None
        })
    {
        return Ok(path.into_std_path_buf());
    }

    // For bin or bin-type example artifacts, `filenames` provide hard-linked paths
    // without extra-filename.
    // We scans all possible original artifacts by checking hard links,
    // in order to retrieve the correct extra-filename, and then locate asm files.
    //
    // `filenames`, also `executable`:
    // [..]/target/debug/foobin                    <+
    // [..]/target/debug/examples/fooexample        | <+ Hard linked.
    // Origins:                                     |  |
    // [..]/target/debug/deps/foobin-01234567      <+  |
    // [..]/target/debug/examples/fooexample-01234567 <+
    // Asm files:
    // [..]/target/debug/deps/foobin-01234567.s
    // [..]/target/debug/examples/fooexample-01234567.s
    if let Some(exe_path) = &artifact.executable {
        let parent = exe_path.parent().unwrap();
        let deps_dir = if parent.file_name() == Some("examples") {
            parent.to_owned()
        } else {
            exe_path.with_file_name("deps")
        };

        for entry in deps_dir.read_dir()? {
            let maybe_origin = entry?.path();
            if same_contents(&exe_path, &maybe_origin)? {
                let asm_file = maybe_origin.with_extension(expect_ext);
                if asm_file.exists() {
                    return Ok(asm_file);
                }
            }
        }
    }

    anyhow::bail!("Cannot locate the path to the asm file");
}

fn same_contents<A: AsRef<Path>, B: AsRef<Path>>(a: &A, b: &B) -> anyhow::Result<bool> {
    Ok(same_file::is_same_file(a, b)?
        || (std::fs::metadata(a)?.len() == std::fs::metadata(b)?.len()
            && std::fs::read(a)? == std::fs::read(b)?))
}

fn suggest_name(search: &str, full: bool, items: &[Item]) -> anyhow::Result<()> {
    let names = items.iter().fold(BTreeMap::new(), |mut m, item| {
        m.entry(if full { &item.hashed } else { &item.name })
            .or_insert_with(Vec::new)
            .push(item.len);
        m
    });

    if names.is_empty() {
        #[allow(clippy::redundant_else)]
        if search.is_empty() {
            anyhow::bail!("This target defines no functions")
        } else {
            anyhow::bail!("No matching functions, try relaxing your search request")
        }
    }
    println!("Try one of those");
    for (name, lens) in &names {
        println!(
            "{:?} {:?}",
            color!(name, owo_colors::OwoColorize::green),
            color!(lens, owo_colors::OwoColorize::cyan)
        );
    }

    std::process::exit(1);
}
