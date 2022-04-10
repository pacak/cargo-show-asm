use cargo_show_asm::{asm, opts};

use std::collections::BTreeSet;

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
        _ => "",
        /*
        opts::Focus::Lib => todo!(),
        opts::Focus::Test(_) => todo!(),
        opts::Focus::Bench(_) => todo!(),
        opts::Focus::Example(_) => todo!(),
        opts::Focus::Bin(_) => todo!(),*/
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
        String::from("llvm-args=-x86-asm-syntax=intel"),
        String::from("-C"),
        String::from("debuginfo=2"),
    ]);

    compile_opts.build_config.build_plan = opts.dry;

    let mut retrying = false;

    loop {
        let comp = compile(&ws, &compile_opts)?;
        let output = &comp.deps_output[&CompileKind::Host];

        let target = opts.function.as_deref().unwrap_or(" ");

        let file_mask = format!(
            "{}/{}{}-*.s",
            output.display(),
            correction,
            &comp.root_crate_names[0]
        );

        let seen;
        let mut existing = BTreeSet::new();
        let mut asm_files = glob::glob(&file_mask)?.collect::<Vec<_>>();

        match asm_files.len() {
            0 => {
                eprintln!("Compilation produced no files satisfying {file_mask}, this is a bug");
                std::process::exit(1);
            }
            1 => {
                let file = asm_files.remove(0)?;
                seen = asm::dump_function(
                    target,
                    &file,
                    &target_info.sysroot,
                    &opts.format,
                    &mut existing,
                )?;
            }
            _ => {
                if retrying {
                    eprintln!(
                        "Compilation produced multiple matching files: {:?}, this is a bug",
                        asm_files
                    );
                    std::process::exit(1);
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
        }

        if !seen {
            eprintln!("Try one of those");
            for x in &existing {
                eprintln!("\t{x}");
            }
            std::process::exit(1);
        }
        break;
    }

    Ok(())
}
