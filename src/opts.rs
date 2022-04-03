use bpaf::{command, construct, long, positional, short, Bpaf, Info, OptionParser, Parser};
use cargo::{
    core::{MaybePackage, Target, TargetKind, Workspace},
    ops::CompileFilter,
};
use std::path::PathBuf;

#[derive(Clone, Debug, Bpaf)]
#[bpaf(options)]
pub struct Options {
    #[bpaf(external(parse_manifest_path))]
    pub manifest_path: PathBuf,
    /// Custom target directory for generated artifacts
    #[bpaf(argument_os("DIR"))]
    pub target_dir: Option<PathBuf>,
    /// Package to use if ambigous
    #[bpaf(argument("SPEC"))]
    pub package: Option<String>,
    #[bpaf(external(focus), optional)]
    pub focus: Option<Focus>,
    /// Produce a build plan instead of actually building
    pub dry: bool,
    /// Requires Cargo.lock and cache are up to date
    pub frozen: bool,
    /// Requires Cargo.lock is up to date
    pub locked: bool,
    /// Run without accessing the network
    pub offline: bool,
    #[bpaf(external(format))]
    pub format: Format,
    /// more verbose output, can be specified multuple times
    #[bpaf(external(verbose))]
    pub verbosity: usize,
    #[bpaf(positional("FUNCTION"), optional)]
    pub function: Option<String>,
}

fn verbose() -> Parser<usize> {
    short('v')
        .long("verbose")
        .help("more verbose output, can be specified multuple times")
        .req_flag(())
        .many()
        .map(|v| v.len())
}

fn parse_manifest_path() -> Parser<PathBuf> {
    long("manifest-path")
        .help("Path to Cargo.toml")
        .argument_os("PATH")
        .map(PathBuf::from)
        .parse(|p| {
            if p.is_absolute() {
                Ok(p)
            } else {
                std::env::current_dir()
                    .map(|d| d.join(p))
                    .and_then(|full_path| full_path.canonicalize())
            }
        })
        .fallback_with(|| std::env::current_dir().map(|x| x.join("Cargo.toml")))
}

#[derive(Debug, Clone, Bpaf)]
pub struct Format {
    /// Print interleaved Rust code
    pub rust: bool,

    /// use
    #[bpaf(external(color_detection))]
    pub color: bool,
}

fn color_detection() -> Parser<bool> {
    let yes = long("color")
        .help("Enable color highlighting")
        .req_flag(true);
    let no = long("no-color")
        .help("Disable color highlighting")
        .req_flag(false);
    construct!([yes, no]).fallback_with::<_, &str>(|| Ok(atty::is(atty::Stream::Stdout)))
}

#[derive(Debug, Clone, Bpaf)]
pub enum Focus {
    /// Show results from library code
    Lib,

    Test(
        /// Show results from a test
        #[bpaf(long("test"), argument("TEST"))]
        String,
    ),

    Bench(
        /// Show results from a benchmark
        #[bpaf(long("bench"), argument("BENCH"))]
        String,
    ),

    Example(
        /// Show results from an example
        #[bpaf(long("example"), argument("EXAMPLE"))]
        String,
    ),

    Bin(
        /// Show results from a binary
        #[bpaf(long("bin"), argument("BIN"))]
        String,
    ),
}

impl From<Focus> for CompileFilter {
    fn from(focus: Focus) -> Self {
        let mut lib_only = false;
        let mut bins = Vec::new();
        let mut tests = Vec::new();
        let mut examples = Vec::new();
        let mut benches = Vec::new();
        match focus {
            Focus::Lib => lib_only = true,
            Focus::Test(t) => tests = vec![t],
            Focus::Bench(b) => benches = vec![b],
            Focus::Example(e) => examples = vec![e],
            Focus::Bin(b) => bins = vec![b],
        }
        Self::from_raw_arguments(
            lib_only, bins, false, tests, false, examples, false, benches, false, false,
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
    #[must_use]
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

/*
pub fn opts() -> Options {
    let manifest_path = long("manifest-path")
        .help("Path to Cargo.toml")
        .argument_os("PATH")
        .map(PathBuf::from)
        .parse(|p| {
            if p.is_absolute() {
                Ok(p)
            } else {
                std::env::current_dir()
                    .map(|d| d.join(p))
                    .and_then(|full_path| full_path.canonicalize())
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

    let focus = focus().optional();

    let parser = construct!(Options {
        target,
        manifest_path,
        focus,
        verbosity,
        dry,
        package,
        frozen,
        locked,
        offline,
        format(),
        function,
    });

    let command_parser = command(
        "asm",
        Some("A command to display raw asm for some rust function"),
        Info::default().for_parser(parser),
    );

    Info::default().for_parser(command_parser).run()
}
*/

pub fn opts() -> Options {
    let command_parser = command(
        "asm",
        Some("A command to display raw asm for some rust function"),
        options(),
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
                eprintln!("{:?} defines a virtual workspace package, you need to specify which member to use with -p xxxx", opts.manifest_path);
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
                TargetKind::ExampleBin => eprint!("--example {}", t.name()),
                TargetKind::ExampleLib(_) | TargetKind::CustomBuild => continue,
            }
            eprintln!("\tfor {}: {:?}", t.description_named(), t.src_path());
        }

        std::process::exit(1);
    }
    package.name().to_string()
}
