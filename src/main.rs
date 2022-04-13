use std::collections::BTreeMap;

use cargo_show_asm::{
    asm::{self, Item},
    color, opts,
};

use cargo::{
    core::{
        compiler::{CompileKind, TargetInfo},
        Workspace,
    },
    ops::{compile, CleanOptions, CompileFilter, CompileOptions, Packages},
    util::interning::InternedString,
    Config,
};

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

fn main() -> anyhow::Result<()> {
    reset_signal_pipe_handler()?;

    let opts = opts::options().run();

    let mut cfg = Config::default()?;
    cfg.configure(
        u32::try_from(opts.verbosity).unwrap_or(0),
        false,
        None,
        opts.frozen,
        opts.locked,
        opts.offline,
        &None,
        &[],
        &[],
    )?;

    let ws = Workspace::new(&opts.manifest_path, &cfg)?;

    let package = opts::select_package(&opts, &ws);

    let rustc = cfg.load_global_rustc(Some(&ws))?;
    let target_info = TargetInfo::new(&cfg, &[CompileKind::Host], &rustc, CompileKind::Host)?;

    let mut compile_opts = CompileOptions::new(&cfg, cargo::core::compiler::CompileMode::Build)?;

    compile_opts.spec = Packages::Packages(vec![package.clone()]);

    let correction = match opts.focus.as_ref() {
        Some(opts::Focus::Example(_)) => "../examples/",
        Some(
            opts::Focus::Lib | opts::Focus::Test(_) | opts::Focus::Bench(_) | opts::Focus::Bin(_),
        )
        | None => "",
    };

    if let Some(focus) = opts.focus {
        compile_opts.filter = CompileFilter::from(focus);
    }
    compile_opts.build_config.requested_profile = InternedString::new("release");
    compile_opts.target_rustc_args = Some(vec![
        String::from("-C"),
        String::from("codegen-units=1"),
        String::from("--emit"),
        String::from("asm"),
        String::from("-C"),
        opts.syntax.to_string(),
        String::from("-C"),
        String::from("debuginfo=2"),
    ]);

    compile_opts.build_config.build_plan = opts.dry;

    let mut retrying = false;

    loop {
        let comp = compile(&ws, &compile_opts)?;
        let output = &comp.deps_output[&CompileKind::Host];

        let target = (opts.function.as_deref().unwrap_or(" "), opts.nth);

        let file_mask = format!(
            "{}/{}{}-*.s",
            output.display(),
            correction,
            &comp.root_crate_names[0]
        );

        owo_colors::set_override(opts.format.color);

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
                asm::dump_function(
                    target,
                    &file,
                    &target_info.sysroot,
                    &opts.format,
                    &mut existing,
                )?
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
                    requested_profile: InternedString::new("release"),
                    doc: false,
                };
                cargo::ops::clean(&ws, &clean_opts)?;
                retrying = true;
                continue;
            }
        };

        if !seen {
            suggest_name(opts.format.full_name, &existing);
        }
        break;
    }

    Ok(())
}

fn suggest_name(full: bool, items: &[Item]) {
    let names = items.iter().fold(BTreeMap::new(), |mut m, item| {
        m.entry(if full { &item.hashed } else { &item.name })
            .or_insert_with(Vec::new)
            .push(item.len);
        m
    });

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
