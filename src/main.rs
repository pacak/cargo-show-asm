use cargo_show_asm::*;

use std::collections::BTreeSet;

use cargo::{
    core::{
        compiler::{CompileKind, TargetInfo},
        MaybePackage, TargetKind, Workspace,
    },
    ops::{compile, CompileFilter, CompileOptions, FilterRule, LibRule, Packages},
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
        opts.verbosity as u32,
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

    let package = match (ws.root_maybe(), &opts.package) {
        (MaybePackage::Package(p), _) => p,
        (MaybePackage::Virtual(_), None) => {
            eprintln!("{:?} defines a virtual workspace package, you need to specify which member to use with -p xxxx", opts.manifest);
            for package in ws.members() {
                eprintln!("\t-p {}", package.name());
            }
            std::process::exit(1);
        }
        (MaybePackage::Virtual(_), Some(p)) => {
            if let Some(package) = ws.members().find(|package| package.name().as_str() == p) {
                package
            } else {
                eprintln!("{p} is not a valid package name in this workspace");
                std::process::exit(1);
            }
        }
    };

    if package.targets().len() > 1 && opts.focus.is_none() {
        eprintln!(
            "{} defines multiple targets, you need to specify which one to use:",
            package.name()
        );
        for t in package.targets().iter() {
            match t.kind() {
                TargetKind::Lib(_) => print!("--lib"),
                TargetKind::Bin => print!("--bin {}", t.name()),
                TargetKind::Test => print!("--test {}", t.name()),
                TargetKind::Bench => print!("--bench {}", t.name()),
                TargetKind::ExampleLib(_) => todo!(),
                TargetKind::ExampleBin => print!("--example {}", t.name()),
                TargetKind::CustomBuild => continue,
            }
            println!("\tfor {}: {:?}", t.description_named(), t.src_path());
        }

        std::process::exit(1);
    }

    let rustc = cfg.load_global_rustc(Some(&ws))?;
    let target_info = TargetInfo::new(&cfg, &[CompileKind::Host], &rustc, CompileKind::Host)?;

    let mut copts = CompileOptions::new(&cfg, cargo::core::compiler::CompileMode::Build)?;

    copts.spec = Packages::Packages(vec![package.name().to_string()]);

    if let Some(focus) = opts.focus {
        copts.filter = CompileFilter::from(focus);
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
