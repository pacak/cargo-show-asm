use cargo_show_asm::*;

use std::collections::BTreeSet;

use cargo::{
    core::{
        compiler::{CompileKind, TargetInfo},
        Workspace,
    },
    ops::{compile, CompileOptions, Packages},
    util::interning::InternedString,
    Config,
};

/// This should be called before calling any cli method or printing any output.
pub fn reset_signal_pipe_handler() -> anyhow::Result<()> {
    #[cfg(target_family = "unix")]
    {
        use nix::sys::signal;
        unsafe {
            signal::signal(signal::Signal::SIGPIPE, signal::SigHandler::SigDfl)?;
        }
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    reset_signal_pipe_handler()?;

    let opts = opts::opts();

    let mut cfg = Config::default()?;
    cfg.configure(
        1,
        false,
        None,
        opts.frozen,
        opts.locked,
        opts.offline,
        &None,
        &[],
        &[],
    )?;

    let ws = Workspace::new(&opts.manifest, &cfg)?;

    let rustc = cfg.load_global_rustc(Some(&ws))?;
    let target_info = TargetInfo::new(&cfg, &[CompileKind::Host], &rustc, CompileKind::Host)?;

    let mut copts = CompileOptions::new(&cfg, cargo::core::compiler::CompileMode::Build)?;

    if let Some(package) = opts.package {
        copts.spec = Packages::Packages(vec![package]);
    } else if let Some(function) = &opts.function {
        if let Some((package, _)) = function.split_once("::") {
            copts.spec = Packages::Packages(vec![package.to_string()]);
        } else {
            todo!("{:?}", function);
        }
    } else {
        eprintln!("You need to specify package/function to use, try one of those");
        todo!("-p xxxxxx");
    }

    copts.build_config.requested_profile = InternedString::new("release");
    copts.target_rustc_args = Some(vec![
        String::from("-C"),
        String::from("codegen-units=1"),
        String::from("--emit"),
        String::from("asm"),
        String::from("-C"),
        String::from("llvm-args=-x86-asm-syntax=intel"),
        String::from("-C"),
        String::from("debuginfo=2"),
    ]);

    copts.build_config.build_plan = opts.dry;
    let comp = compile(&ws, &copts)?;
    let output = comp.deps_output.get(&CompileKind::Host).unwrap();

    let target = opts.function.as_deref().unwrap_or(" ");

    //    let fmt = opts::Format { rust: opts.rust };

    let mut seen = false;
    let mut existing = BTreeSet::new();
    for s_file in glob::glob(&format!(
        "{}/{}-*.s",
        output.display(),
        &comp.root_crate_names[0]
    ))? {
        seen |= asm::dump_function(
            target,
            &s_file?,
            &target_info.sysroot,
            &opts.format,
            &mut existing,
        )?;
    }

    if !seen {
        println!("Try one of those");
        for x in existing.iter() {
            println!("\t{x}");
        }
    }

    /*
        for e in walkdir::WalkDir::new(o) {
            println!("{:?}", e?);
        }

        let f = PathBuf::from(String::from(
            "/home/pacak/ej/master/target/asm/release/deps/tsu_mini_std-c590d4e929fdbd3c.s",
        ));
        let ti = TargetInfo::new_from_triple("x86_64-unknown-linux-gnu".into());
        //  let c=

        let x = cargo_show_asm::asm::run(&[f], &ti);

        todo!("{:?}", x);
    */
    Ok(())
}
