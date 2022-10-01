use cargo::{
    core::{
        compiler::{CompileKind, CompileTarget, TargetInfo},
        Workspace,
    },
    ops::{compile, CleanOptions, CompileFilter, CompileOptions, Packages},
    Config,
};
use cargo_show_asm::{
    asm::{self, Item},
    color, llvm, mir, opts,
};
use std::{collections::BTreeMap, ffi::OsStr};

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

    let mut cfg = Config::default()?;
    cfg.configure(
        opts.verbosity,
        false,
        None,
        opts.frozen,
        opts.locked,
        opts.offline,
        &opts.target_dir,
        &[],
        &[],
    )?;

    let ws = Workspace::new(&opts.manifest_path, &cfg)?;
    let package = opts::select_package(&opts, &ws);
    let rustc = cfg.load_global_rustc(Some(&ws))?;
    let kind = match &opts.target {
        Some(t) => CompileKind::Target(CompileTarget::new(t)?),
        None => CompileKind::Host,
    };
    let target_info = TargetInfo::new(&cfg, &[CompileKind::Host], &rustc, kind)?;

    let mut compile_opts = CompileOptions::new(&cfg, cargo::core::compiler::CompileMode::Build)?;

    compile_opts.spec = Packages::Packages(vec![package.clone()]);

    if let Some(focus) = &opts.focus {
        compile_opts.filter = CompileFilter::from(focus.clone());
    }
    compile_opts.cli_features = opts.cli_features.try_into()?;
    compile_opts.build_config.requested_kinds = vec![kind];
    compile_opts.build_config.requested_profile = opts.compile_mode.into();
    compile_opts.build_config.force_rebuild = opts.force_rebuild;

    let mut rustc_args = vec![
        // so only one file gets created
        String::from("-C"),
        String::from("codegen-units=1"),
        // we care about asm
        String::from("--emit"),
        String::from(opts.syntax.emit()),
        // debug info is needed to map to rust source
        String::from("-C"),
        String::from("debuginfo=2"),
    ];

    if let Some(asm_syntax) = opts.syntax.format() {
        rustc_args.push(String::from("-C"));
        rustc_args.push(String::from(asm_syntax));
    }

    if let Some(cpu) = &opts.target_cpu {
        rustc_args.push(String::from("-C"));
        rustc_args.push(format!("target-cpu={}", cpu));
    }
    compile_opts.target_rustc_args = Some(rustc_args);
    compile_opts.build_config.build_plan = opts.dry;

    let mut retrying = false;
    owo_colors::set_override(opts.format.color);

    let target_name = opts.function.as_deref().unwrap_or("");
    let mut target_function = (target_name, opts.nth);

    loop {
        let comp = compile(&ws, &compile_opts)?;
        if opts.dry {
            return Ok(());
        }

        // I see no ways how there can be more than one, let's assert that
        // and deal with the bug reports if any.
        assert!(
            [1, 2].contains(&comp.deps_output.len()),
            "More than one custom target?"
        );

        // by default "clean" cleans only the host target, in case of crosscompilation
        // we need to clean the crosscompiled one
        let mut clean_targets = Vec::new();

        // crosscompilation can produce files for kinds other than Host.
        // If it's present - we prefer non host versions as more interesting one
        // As a side effect this prevents cargo-show-asm from showing things
        // used to compile proc macro. Proper approach would probably be looking
        // for target crate files in both host and target folders, there
        // should be only one. But then there's windows with odd glob crate andt
        // testing that is very painful. Pull requests are welcome
        let output = if comp.deps_output.len() == 1 {
            &comp.deps_output[&CompileKind::Host]
        } else {
            let (cc, path) = comp
                .deps_output
                .iter()
                .find(|(k, _v)| **k != CompileKind::Host)
                .expect("There shouldn't be more than one host target");
            match cc {
                CompileKind::Host => unreachable!("We are filtering host out above..."),
                CompileKind::Target(t) => clean_targets.push(t.short_name().to_string()),
            }
            path
        };

        let output = match opts.focus.clone().and_then(|f| f.correction()) {
            Some(path) => output.with_file_name(path),
            None => output.clone(),
        };

        if opts.verbosity > 0 {
            println!("Scanning {:?}", output);
        }
        let mut source_files = Vec::new();
        let name = &comp.root_crate_names[0];
        for entry in std::fs::read_dir(&output)? {
            let entry = entry?;
            let path = entry.path();

            let ext = match path.extension() {
                Some(ext) => ext,
                None => continue,
            };

            let file_name = match path.file_name().and_then(OsStr::to_str) {
                Some(file_name) => file_name,
                None => continue,
            };

            if ext == opts.syntax.ext() && file_name.starts_with(name) {
                source_files.push(path);
            }
        }

        let mut existing = Vec::new();
        if opts.verbosity > 0 {
            println!("Found some files: {:?}", source_files);
        }

        // this variable exists to deal with the case where there's only
        // one matching function - we might as well show it to the user directly
        let mut single_target;

        let seen = match source_files.len() {
            0 => {
                anyhow::bail!(
                    "Compilation produced no files satisfying {:?}, this is a bug",
                    output.with_file_name("*").with_extension(opts.syntax.ext())
                );
            }
            1 => {
                let file = source_files.remove(0);

                let mut seen;

                loop {
                    seen = match opts.syntax {
                        opts::Syntax::Intel | opts::Syntax::Att => asm::dump_function(
                            target_function,
                            &file,
                            &target_info.sysroot,
                            &opts.format,
                            &mut existing,
                        ),
                        opts::Syntax::Llvm => {
                            llvm::dump_function(target_function, &file, &opts.format, &mut existing)
                        }
                        opts::Syntax::Mir => {
                            mir::dump_function(target_function, &file, &opts.format, &mut existing)
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
                seen
            }
            _ => {
                if retrying {
                    anyhow::bail!(
                        "Compilation produced multiple matching files: {source_files:?}. Do you have several targets (library and binary) producing a file with the same name? Otherwise this is a bug",
                    );
                }
                let clean_opts = CleanOptions {
                    config: &cfg,
                    spec: vec![package.clone()],
                    targets: clean_targets,
                    profile_specified: false,
                    requested_profile: opts.compile_mode.into(),
                    doc: false,
                };
                cargo::ops::clean(&ws, &clean_opts)?;
                retrying = true;
                continue;
            }
        };

        if !seen {
            suggest_name(target_name, opts.format.full_name, &existing)?;
        }
        break;
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
