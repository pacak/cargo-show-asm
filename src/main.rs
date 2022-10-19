use anyhow::Context;
use cargo_metadata::{Message, MetadataCommand};
use cargo_show_asm::{
    asm::{self, Item},
    color, llvm, mir, opts,
};
use std::collections::BTreeMap;
use std::io::BufReader;
use std::path::PathBuf;
use std::process::Stdio;

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
        None => anyhow::bail!("Multiple packages found"),
    };

    let focus_artifact = match opts.focus {
        Some(focus) => focus,
        None => match focus_package.targets.len() {
            0 => anyhow::bail!("No targets found"),
            1 => opts::Focus::try_from(&focus_package.targets[0])?,
            _ => anyhow::bail!("Multiple targets found"),
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
            .args(std::iter::repeat("-v").take(opts.format.verbosity as usize))
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
                    .feature
                    .iter()
                    .flat_map(|feat| ["--feature", feat]),
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

    let asm_path = artifact
        .filenames
        .iter()
        // For lib, test or bench artifacts, it provides paths under `deps` with extra-filename.
        // We could locate asm files precisely.
        // [..]/target/debug/deps/libfoo-01234567.rmeta # lib
        // [..]/target/debug/deps/foo-01234567          # test & bench
        // <->
        // [..]/target/debug/deps/foo-01234567.s
        .find_map(|path| {
            if path.parent()?.file_name()? != "deps" {
                return None;
            }
            let path = path.with_extension(opts.syntax.ext());
            if path.exists() {
                return Some(path);
            }
            let path_without_lib =
                path.with_file_name(path.file_name().unwrap().strip_prefix("lib")?);
            if path_without_lib.exists() {
                return Some(path_without_lib);
            }
            None
        })
        // For bin or example artifacts, the filenames are missing extra-filename (the hash part).
        // [..]/target/debug/foobin
        // [..]/target/debug/examples/fooexample
        // <->
        // [..]/target/debug/deps/foobin-01234567.s
        // [..]/target/debug/examples/fooexample-01234567.s
        .or_else(|| todo!())
        .context("Cannot find asm file")?
        .into_std_path_buf();

    if opts.format.verbosity > 0 {
        eprintln!("Asm file: {}", asm_path.display());
    }

    let target_name = opts.function.as_deref().unwrap_or("");
    let mut target_function = (target_name, opts.nth);

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
            target_function = (&single_target, 0);
        } else {
            break;
        }
    }

    if !seen {
        suggest_name(target_name, opts.format.full_name, &existing)?;
    }

    Ok(())
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
