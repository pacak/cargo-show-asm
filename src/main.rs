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
    color, llvm, mir,
    opts::{self, Focus},
};
use std::collections::BTreeMap;

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

    let correction = opts.focus.as_ref().map_or("", Focus::correction);

    if let Some(focus) = opts.focus {
        compile_opts.filter = CompileFilter::from(focus);
    }
    compile_opts.cli_features = opts.cli_features.try_into()?;
    compile_opts.build_config.requested_profile = opts.compile_mode.into();
    compile_opts.build_config.force_rebuild = opts.force_rebuild;

    let mut rustc_args = vec![
        // so only one file gets created
        String::from("-C"),
        String::from("codegen-units=1"),
        // we care about asm
        String::from("--emit"),
        opts.syntax.emit(),
        String::from("-C"),
        opts.syntax.format(),
        // debug info is needed to map to rust source
        String::from("-C"),
        String::from("debuginfo=2"),
    ];
    if let Some(target) = &opts.target {
        rustc_args.push(String::from("--target"));
        rustc_args.push(target.to_string());
        if let Ok(linker) = cfg.get::<String>(&format!("target.{target}.linker")) {
            rustc_args.push(String::from("-C"));
            rustc_args.push(format!("linker={linker}"));
        }
    }
    compile_opts.target_rustc_args = Some(rustc_args);
    compile_opts.build_config.build_plan = opts.dry;

    let mut retrying = false;
    owo_colors::set_override(opts.format.color);

    let target_name = opts.function.as_deref().unwrap_or("");
    let target = (target_name, opts.nth);

    loop {
        let comp = compile(&ws, &compile_opts)?;
        if opts.dry {
            return Ok(());
        }
        let output = &comp.deps_output[&CompileKind::Host];

        let root;
        #[cfg(not(windows))]
        {
            root = output.display();
        }
        #[cfg(windows)]
        {
            let full = output.canonicalize()?.display().to_string();
            let cur = std::env::current_dir()?
                .canonicalize()?
                .display()
                .to_string();
            let relative = &full[cur.len()..];
            root = format!(
                ".{}{}",
                if relative.starts_with("\\") { "" } else { "\\" },
                relative
            );
        }

        let file_mask = format!(
            "{root}{}{}{}-*.{}",
            std::path::MAIN_SEPARATOR,
            correction,
            &comp.root_crate_names[0],
            opts.syntax.ext(),
        );

        let mut existing = Vec::new();
        let mut asm_files = glob::glob(&file_mask)?.collect::<Vec<_>>();

        let seen = match asm_files.len() {
            0 => {
                anyhow::bail!(
                    "Compilation produced no files satisfying {file_mask}, this is a bug"
                );
            }
            1 => {
                let file = asm_files.remove(0)?;

                match opts.syntax {
                    opts::Syntax::Intel | opts::Syntax::Att => asm::dump_function(
                        target,
                        &file,
                        &target_info.sysroot,
                        &opts.format,
                        &mut existing,
                    )?,
                    opts::Syntax::Llvm => {
                        llvm::dump_function(target, &file, &opts.format, &mut existing)?
                    }
                    opts::Syntax::Mir => {
                        mir::dump_function(target, &file, &opts.format, &mut existing)?
                    }
                }
            }
            _ => {
                if retrying {
                    anyhow::bail!(
                        "Compilation produced multiple matching files: {asm_files:?}, this is a bug",
                    );
                }
                let clean_opts = CleanOptions {
                    config: &cfg,
                    spec: vec![package.clone()],
                    targets: Vec::new(),
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
