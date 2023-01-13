use anyhow::Context;
use cargo_metadata::{Artifact, Message, MetadataCommand, Package};
use cargo_show_asm::{asm, llvm, mca, mir, opts};
use once_cell::sync::Lazy;
use std::{
    io::BufReader,
    path::{Path, PathBuf},
    process::Stdio,
};

static CARGO_PATH: Lazy<PathBuf> =
    Lazy::new(|| std::env::var_os("CARGO").map_or_else(|| "cargo".into(), PathBuf::from));
static RUSTC_PATH: Lazy<PathBuf> =
    Lazy::new(|| std::env::var_os("RUSTC").map_or_else(|| "rustc".into(), PathBuf::from));

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

fn spawn_cargo(
    cargo: &opts::Cargo,
    format: &opts::Format,
    syntax: opts::Syntax,
    target_cpu: Option<&str>,
    focus_package: &Package,
    focus_artifact: &opts::Focus,
) -> std::io::Result<std::process::Child> {
    use std::ffi::OsStr;

    let mut cmd = std::process::Command::new(&*CARGO_PATH);

    // Cargo flags.
    cmd.arg("rustc")
        // General.
        .args([
            "--message-format=json-render-diagnostics",
            "--color",
            if format.color { "always" } else { "never" },
        ])
        .args(std::iter::repeat("-v").take(format.verbosity))
        // Workspace location.
        .arg("--manifest-path")
        .arg(&cargo.manifest_path)
        // Artifact selectors.
        .args(["--package", &focus_package.name])
        .args(focus_artifact.as_cargo_args())
        // Compile options.
        .args(cargo.dry.then_some("--dry"))
        .args(cargo.frozen.then_some("--frozen"))
        .args(cargo.locked.then_some("--locked"))
        .args(cargo.offline.then_some("--offline"))
        .args(cargo.target.iter().flat_map(|t| ["--target", t]))
        .args(cargo.unstable.iter().flat_map(|z| ["-Z", z]))
        .args((syntax == opts::Syntax::Wasm).then_some("--target=wasm32-unknown-unknown"))
        .args(
            cargo
                .target_dir
                .iter()
                .flat_map(|t| [OsStr::new("--target-dir"), t.as_ref()]),
        )
        .args(
            cargo
                .cli_features
                .no_default_features
                .then_some("--no-default-features"),
        )
        .args(cargo.cli_features.all_features.then_some("--all-features"))
        .args(
            cargo
                .cli_features
                .features
                .iter()
                .flat_map(|feat| ["--features", feat]),
        );
    match &cargo.compile_mode {
        opts::CompileMode::Dev => {}
        opts::CompileMode::Release => {
            cmd.arg("--release");
        }
        opts::CompileMode::Custom(profile) => {
            cmd.args(["--profile", profile]);
        }
    }

    // Cargo flags terminator.
    cmd.arg("--");

    // Rustc flags.
    // We care about asm.
    cmd.args(["--emit", syntax.emit()])
        // So only one file gets created.
        .arg("-Ccodegen-units=1")
        // Debug info is needed to map to rust source.
        .arg("-Cdebuginfo=2")
        .args(syntax.format().iter().flat_map(|s| ["-C", s]))
        .args(target_cpu.iter().map(|cpu| format!("-Ctarget-cpu={cpu}")));

    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
}

fn sysroot() -> anyhow::Result<PathBuf> {
    let output = std::process::Command::new(&*RUSTC_PATH)
        .arg("--print=sysroot")
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .stdout(Stdio::piped())
        .output()?;
    if !output.status.success() {
        anyhow::bail!(
            "Failed to get sysroot. '{RUSTC_PATH:?} --print=sysroot' exited with {}",
            output.status,
        );
    }
    // `rustc` prints a trailing newline.
    Ok(PathBuf::from(
        std::str::from_utf8(&output.stdout)?.trim_end(),
    ))
}

#[allow(clippy::too_many_lines)]
fn main() -> anyhow::Result<()> {
    use opts::Syntax;
    reset_signal_pipe_handler()?;

    let opts = opts::options().run();
    owo_colors::set_override(opts.format.color);

    let sysroot = sysroot()?;
    if opts.format.verbosity > 0 {
        eprintln!("Found sysroot: {}", sysroot.display());
    }

    let unstable = opts
        .cargo
        .unstable
        .iter()
        .flat_map(|x| ["-Z".to_owned(), x.clone()])
        .collect::<Vec<_>>();

    let metadata = MetadataCommand::new()
        .cargo_path(&*CARGO_PATH)
        .manifest_path(&opts.cargo.manifest_path)
        .other_options(unstable)
        .no_deps()
        .exec()?;

    let focus_package = match opts.select_fragment.package {
        Some(name) => metadata
            .packages
            .iter()
            .find(|p| p.name == name)
            .with_context(|| format!("Package '{name}' is not found"))?,
        None if metadata.packages.len() == 1 => &metadata.packages[0],
        None => {
            eprintln!(
                "{:?} refers to multiple packages, you need to specify which one to use",
                opts.cargo.manifest_path
            );
            for package in &metadata.packages {
                eprintln!("\t-p {}", package.name);
            }
            anyhow::bail!("Multiple packages found")
        }
    };

    let focus_artifact = match opts.select_fragment.focus {
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

    let mut cargo_child = spawn_cargo(
        &opts.cargo,
        &opts.format,
        opts.syntax,
        opts.target_cpu.as_deref(),
        focus_package,
        &focus_artifact,
    )?;

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
            _ => {}
        }
    }
    // add some spacing between cargo's output and ours
    eprintln!();
    if !success {
        let status = cargo_child.wait()?;
        eprintln!("Cargo failed with {status}");
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

    match opts.syntax {
        Syntax::Intel | Syntax::Att | Syntax::Wasm => {
            asm::dump_function(opts.to_dump, &asm_path, &sysroot, &opts.format)
        }
        Syntax::McaAtt | Syntax::McaIntel => mca::dump_function(
            opts.to_dump,
            &asm_path,
            &opts.format,
            opts.syntax == Syntax::McaIntel,
            &opts.cargo.target,
            &opts.target_cpu,
        ),
        Syntax::Llvm => llvm::dump_function(opts.to_dump, &asm_path, &opts.format),
        Syntax::Mir => mir::dump_function(opts.to_dump, &asm_path, &opts.format),
    }
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

    // then there's rlib with filenames as following:
    // `filenames`:
    // [..]/target/debug/libfoo.a              <+
    // [..]/target/debug/libfoo.rlib            | <+ Hard linked.
    // Asm files:                               |  | Or same contents at least
    // [..]/target/debug/libfoo-01234567.a     <+  |
    // [..]/target/debug/libfoo-01234567.rlib     <+
    // [..]/target/debug/foo-01234567.s

    if artifact.target.kind.iter().any(|k| k == "rlib") {
        let rlib_path = artifact
            .filenames
            .iter()
            .find(|f| f.extension().map_or(false, |e| e == "rlib"))
            .expect("No rlib?");
        let deps_dir = rlib_path.with_file_name("deps");

        for entry in deps_dir.read_dir()? {
            let maybe_origin = entry?.path();
            if same_contents(&rlib_path, &maybe_origin)? {
                let name = maybe_origin
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .strip_prefix("lib")
                    .unwrap();
                let asm_file = maybe_origin.with_file_name(name).with_extension(expect_ext);
                if asm_file.exists() {
                    return Ok(asm_file);
                }
            }
        }
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
