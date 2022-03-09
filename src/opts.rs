use bpaf::*;
use cargo::{
    core::{MaybePackage, Target, TargetKind, Workspace},
    ops::CompileFilter,
};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Options {
    pub manifest: PathBuf,
    pub target: Option<PathBuf>,
    pub package: Option<String>,
    pub function: Option<String>,
    pub focus: Option<Focus>,
    pub dry: bool,
    pub frozen: bool,
    pub locked: bool,
    pub offline: bool,
    pub format: Format,
    pub verbosity: usize,
}

#[derive(Debug, Clone)]
pub struct Format {
    pub rust: bool,
    pub color: bool,
}

#[derive(Debug, Clone)]
pub enum Focus {
    Lib,
    Test(String),
    Bench(String),
    Example(String),
    Bin(String),
}

impl From<Focus> for CompileFilter {
    fn from(focus: Focus) -> Self {
        let mut lib_only = false;
        let mut bins = Vec::new();
        let mut tsts = Vec::new();
        let mut exms = Vec::new();
        let mut bens = Vec::new();
        match focus {
            Focus::Lib => lib_only = true,
            Focus::Test(t) => tsts = vec![t],
            Focus::Bench(b) => bens = vec![b],
            Focus::Example(e) => exms = vec![e],
            Focus::Bin(b) => bins = vec![b],
        }
        CompileFilter::from_raw_arguments(
            lib_only, bins, false, tsts, false, exms, false, bens, false, false,
        )
    }
}

impl std::fmt::Display for Focus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Focus::Lib => f.write_str("--lib"),
            Focus::Test(t) => write!(f, "--test {}", t),
            Focus::Bench(b) => write!(f, "--bench {}", b),
            Focus::Example(e) => write!(f, "--example {}", e),
            Focus::Bin(b) => write!(f, "--bin {b}"),
        }
    }
}

impl Focus {
    pub fn matches(&self, target: &Target) -> bool {
        match self {
            Focus::Lib => target.is_lib(),
            Focus::Test(t) => target.is_test() && target.name() == t,
            Focus::Bench(b) => target.is_bench() && target.name() == b,
            Focus::Example(e) => target.is_example() && target.name() == e,
            Focus::Bin(b) => target.is_bin() && target.name() == b,
        }
    }
}

fn focus() -> Parser<Focus> {
    let lib = long("lib")
        .help("Show results from library code")
        .req_flag(Focus::Lib);
    let bin = long("bin")
        .help("Show results from a binary")
        .argument("BIN")
        .map(Focus::Bin);
    let test = long("test")
        .help("Show results from a test")
        .argument("TEST")
        .map(Focus::Test);
    let bench = long("bench")
        .help("Show results from a benchmark")
        .argument("BENCH")
        .map(Focus::Bench);
    let example = long("example")
        .help("Show results from an example")
        .argument("EXAMPLE")
        .map(Focus::Example);
    lib.or_else(bin)
        .or_else(test)
        .or_else(bench)
        .or_else(example)
}

pub fn opts() -> Options {
    let manifest = long("manifest-path")
        .help("Path to Cargo.toml")
        .argument_os("PATH")
        .map(PathBuf::from)
        .parse(|p| {
            if p.is_absolute() {
                Ok(p)
            } else {
                std::env::current_dir()
                    .map(|d| d.join(p))
                    .and_then(|p| p.canonicalize())
            }
        })
        .fallback_with(|| std::env::current_dir().map(|x| x.join("Cargo.toml")));

    let verbosity = short('v')
        .long("verbose")
        .help("more verbose output, can be specified multuple times")
        .req_flag(())
        .many()
        .map(|v| v.len());

    let target = long("target-dir")
        .help("Custom target directory for generated artifacts")
        .argument_os("DIR")
        .map(PathBuf::from)
        .optional();

    let package = long("package")
        .short('p')
        .help("Package to use, if not specified")
        .argument("SPEC")
        .optional();

    let function = positional("FUNCTION").optional();

    let dry = short('d')
        .long("dry")
        .help("Produce a build plan instead of actually building")
        .switch();

    let frozen = long("frozen")
        .help("Require Cargo.lock and cache are up to date")
        .switch();

    let locked = long("locked")
        .help("Require Cargo.lock is up to date")
        .switch();

    let offline = long("offline")
        .help("Run without accessing the network")
        .switch();

    let rust = long("rust")
        .short('r')
        .help("Print interleaved Rust code")
        .switch();

    let color = long("no-color")
        .help("Disable color detection")
        .switch()
        .map(|x| !x);

    let format = construct!(Format { rust, color });

    let focus = focus().optional();

    let parser = construct!(Options {
        target,
        manifest,
        focus,
        verbosity,
        dry,
        package,
        frozen,
        locked,
        offline,
        format,
        function,
    });

    let command_parser = command(
        "asm",
        Some("A command to display raw asm for some rust function"),
        Info::default().for_parser(parser),
    );

    Info::default().for_parser(command_parser).run()
}

pub fn select_package(opts: &Options, ws: &Workspace) -> String {
    let package = match (ws.root_maybe(), &opts.package) {
        (MaybePackage::Package(p), _) => p,
        (MaybePackage::Virtual(_), None) => {
            if let Some(focus) = &opts.focus {
                let mut candidates = Vec::new();
                for p in ws.members() {
                    for t in p.targets() {
                        if focus.matches(t) {
                            candidates.push(p.name());
                        }
                        println!("{:?} {:?}", p, t);
                    }
                }
                match candidates.len() {
                    0 => {
                        eprintln!("Target specification {focus} didn't match any packages");
                        std::process::exit(1);
                    }
                    1 => return candidates.remove(0).to_string(),
                    _ => {
                        eprintln!(
                            "There's multiple targets that match {focus}. Try narrowing the focus by specifying one of those packages:"
                        );
                        for cand in &candidates {
                            eprintln!("\t-p {cand}");
                        }
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("{:?} defines a virtual workspace package, you need to specify which member to use with -p xxxx", opts.manifest);
                for package in ws.members() {
                    eprintln!("\t-p {}", package.name());
                }
                std::process::exit(1);
            }
        }
        (MaybePackage::Virtual(_), Some(p)) => {
            if let Some(package) = ws.members().find(|package| package.name().as_str() == p) {
                package
            } else {
                // give up and let rustc to handle the rest
                return p.to_string();
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
                TargetKind::Lib(_) => eprint!("--lib"),
                TargetKind::Bin => eprint!("--bin {}", t.name()),
                TargetKind::Test => eprint!("--test {}", t.name()),
                TargetKind::Bench => eprint!("--bench {}", t.name()),
                TargetKind::ExampleLib(_) => continue,
                TargetKind::ExampleBin => eprint!("--example {}", t.name()),
                TargetKind::CustomBuild => continue,
            }
            eprintln!("\tfor {}: {:?}", t.description_named(), t.src_path());
        }

        std::process::exit(1);
    }
    package.name().to_string()
}
