use bpaf::{construct, long, short, Bpaf, Parser};
use cargo::{
    core::{MaybePackage, Target, TargetKind, Workspace},
    ops::CompileFilter,
    util::interning::InternedString,
};
use std::path::PathBuf;

#[derive(Clone, Debug, Bpaf)]
#[bpaf(options("asm"), version)]
#[allow(clippy::struct_excessive_bools)]
#[allow(clippy::doc_markdown)]
/// Show the code rustc generates for any function
///
///
///
/// Usage:
///   1. Focus on a single assembly producing target:
///      % cargo asm -p isin --lib   # here we are targeting lib in isin crate
///   2. Narrow down a function:
///      % cargo asm -p isin --lib from_ # here "from_" is part of the function you are interested intel
///   3. Get the full results:
///      % cargo asm -p isin --lib isin::base36::from_alphanum
pub struct Options {
    // what to compile
    #[bpaf(external(parse_manifest_path))]
    pub manifest_path: PathBuf,
    /// Package to use if ambigous
    #[bpaf(long, short, argument("SPEC"))]
    pub package: Option<String>,
    #[bpaf(external(focus), optional)]
    pub focus: Option<Focus>,

    // how to compile
    /// Custom target directory for generated artifacts
    #[bpaf(argument_os("DIR"))]
    pub target_dir: Option<PathBuf>,
    /// Produce a build plan instead of actually building
    pub dry: bool,
    /// Requires Cargo.lock and cache are up to date
    pub frozen: bool,
    /// Requires Cargo.lock is up to date
    pub locked: bool,
    /// Run without accessing the network
    pub offline: bool,
    /// Force Cargo to do a full rebuild and treat each target as changed
    pub force_rebuild: bool,
    #[bpaf(external)]
    pub cli_features: CliFeatures,
    #[bpaf(external, fallback(CompileMode::Release))]
    pub compile_mode: CompileMode,
    /// Build for the target triple
    #[bpaf(argument("TRIPLE"))]
    pub target: Option<String>,

    // how to display
    #[bpaf(external(format))]
    pub format: Format,
    /// more verbose output, can be specified multiple times
    #[bpaf(external(verbose))]
    pub verbosity: u32,
    #[bpaf(external, fallback(Syntax::Intel))]
    pub syntax: Syntax,

    // what to display
    #[bpaf(positional("FUNCTION"), optional)]
    pub function: Option<String>,
    #[bpaf(positional("INDEX"), from_str(usize), fallback(0))]
    pub nth: usize,
}

#[derive(Bpaf, Clone, Debug)]
pub struct CliFeatures {
    /// Do not activate `default` feature
    pub no_defaut_features: bool,
    /// Activate all available features
    pub all_features: bool,
    /// A feature to activate, can be used multiple times
    #[bpaf(argument("FEATURE"))]
    pub feature: Vec<String>,
}

impl TryFrom<CliFeatures> for cargo::core::resolver::features::CliFeatures {
    type Error = anyhow::Error;

    fn try_from(cf: CliFeatures) -> Result<Self, Self::Error> {
        Self::from_command_line(&cf.feature, cf.all_features, !cf.no_defaut_features)
    }
}

// feature, no_defaut_features, all_features

#[derive(Bpaf, Copy, Clone, Debug)]
pub enum CompileMode {
    /// Compile in release mode (default)
    Release,
    /// Compile in dev mode
    Dev,
}

impl From<CompileMode> for InternedString {
    fn from(mode: CompileMode) -> Self {
        InternedString::new(match mode {
            CompileMode::Release => "release",
            CompileMode::Dev => "dev",
        })
    }
}

fn verbose() -> Parser<u32> {
    short('v')
        .long("verbose")
        .help("more verbose output, can be specified multiple times")
        .req_flag(())
        .many()
        .map(|v| v.len().min(u32::MAX as usize) as u32)
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

    #[bpaf(external(color_detection))]
    pub color: bool,

    /// include full demangled name instead of just prefix
    pub full_name: bool,
}

#[derive(Debug, Clone, Bpaf)]
pub enum Syntax {
    /// Generate assembly using Intel style
    Intel,
    /// Generate assembly using AT&T style
    Att,
}

impl ToString for Syntax {
    fn to_string(&self) -> String {
        match self {
            Syntax::Intel => String::from("llvm-args=-x86-asm-syntax=intel"),
            Syntax::Att => String::from("llvm-args=-x86-asm-syntax=att"),
        }
    }
}

fn color_detection() -> Parser<bool> {
    let yes = long("color")
        .help("Enable color highlighting")
        .req_flag(true);
    let no = long("no-color")
        .help("Disable color highlighting")
        .req_flag(false);
    construct!([yes, no]).fallback_with::<_, &str>(|| {
        Ok(supports_color::on(supports_color::Stream::Stdout).is_some())
    })
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

    #[must_use]
    /// a path relative to output directory for this focus item
    pub const fn correction(&self) -> &'static str {
        match self {
            #[cfg(not(windows))]
            Focus::Example(_) => "../examples/",
            #[cfg(windows)]
            Focus::Example(_) => "..\\examples\\",
            Focus::Lib | Focus::Test(_) | Focus::Bench(_) | Focus::Bin(_) => "",
        }
    }
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
