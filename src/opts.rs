use bpaf::{construct, long, short, Bpaf, Parser};
use cargo::{
    core::{MaybePackage, Target, TargetKind, Workspace},
    ops::CompileFilter,
    util::interning::InternedString,
};
use std::path::PathBuf;

fn check_target_dir(path: PathBuf) -> anyhow::Result<PathBuf> {
    if path.is_dir() {
        Ok(path)
    } else {
        std::fs::create_dir(&path)?;
        Ok(std::fs::canonicalize(path)?)
    }
}

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
    #[bpaf(external)]
    pub manifest_path: PathBuf,
    /// Package to use if ambigous
    #[bpaf(long, short, argument("SPEC"))]
    pub package: Option<String>,
    #[bpaf(external, optional)]
    pub focus: Option<Focus>,

    // how to compile
    /// Use custom target directory for generated artifacts, create if missing
    #[bpaf(
        env("CARGO_TARGET_DIR"),
        argument("DIR"),
        parse(check_target_dir),
        optional
    )]
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
    #[bpaf(external)]
    pub compile_mode: CompileMode,
    /// Build for the target triple
    #[bpaf(argument("TRIPLE"))]
    pub target: Option<String>,

    /// Generate code for a specific CPU
    #[bpaf(external)]
    pub target_cpu: Option<String>,

    // how to display
    #[bpaf(external)]
    pub format: Format,
    /// more verbose output, can be specified multiple times
    #[bpaf(external)]
    pub verbosity: u32,
    #[bpaf(external)]
    pub syntax: Syntax,

    // what to display
    #[bpaf(positional("FUNCTION"), optional)]
    pub function: Option<String>,
    #[bpaf(positional("INDEX"), fallback(0))]
    pub nth: usize,
}

fn target_cpu() -> impl Parser<Option<String>> {
    let native = long("native")
        .help("Optimize for the CPU running the compiler")
        .req_flag("native".to_string());
    let cpu = long("target-cpu")
        .help("Optimize code for a specific CPU, see 'rustc --print target-cpus'")
        .argument::<String>("CPU");
    construct!([native, cpu]).optional()
}

#[derive(Bpaf, Clone, Debug)]
pub struct CliFeatures {
    // Previous releases (mis)named this field `no_defaut_features`(sic), resulting in the command
    // line option being `--no-defaut-features` and not the `--no-default-features` used by other
    // Cargo subcommands and which users would normally expect. This attribute retains that as a
    // hidden alias just in case users are still using the misspelt version in their scripts.
    #[bpaf(long, long("no-defaut-features"))]
    /// Do not activate `default` feature
    pub no_default_features: bool,
    /// Activate all available features
    pub all_features: bool,
    /// A feature to activate, can be used multiple times
    #[bpaf(argument("FEATURE"))]
    pub feature: Vec<String>,
}

impl TryFrom<CliFeatures> for cargo::core::resolver::features::CliFeatures {
    type Error = anyhow::Error;

    fn try_from(cf: CliFeatures) -> Result<Self, Self::Error> {
        Self::from_command_line(&cf.feature, cf.all_features, !cf.no_default_features)
    }
}

// feature, no_defaut_features, all_features

#[derive(Bpaf, Copy, Clone, Debug)]
#[bpaf(fallback(CompileMode::Release))]
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

fn verbosity() -> impl Parser<u32> {
    short('v')
        .long("verbose")
        .help("more verbose output, can be specified multiple times")
        .req_flag(())
        .many()
        .map(|v| v.len().min(u32::MAX as usize) as u32)
}

fn manifest_path() -> impl Parser<PathBuf> {
    long("manifest-path")
        .help("Path to Cargo.toml")
        .argument::<PathBuf>("PATH")
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

    /// Include full demangled name instead of just prefix
    pub full_name: bool,

    /// Keep all the original labels
    pub keep_labels: bool,
}

#[derive(Debug, Clone, Bpaf)]
#[bpaf(fallback(Syntax::Intel))]
pub enum Syntax {
    /// Show assembly using Intel style
    #[bpaf(long("intel"), long("asm"))]
    Intel,
    /// Show assembly using AT&T style
    Att,
    /// Show llvm-ir
    Llvm,
    /// Show MIR
    Mir,
}

impl Syntax {
    #[must_use]
    pub fn format(&self) -> String {
        String::from(match self {
            Syntax::Intel => "llvm-args=-x86-asm-syntax=intel",
            Syntax::Att | Syntax::Mir | Syntax::Llvm => "llvm-args=-x86-asm-syntax=att",
        })
    }

    #[must_use]
    pub fn emit(&self) -> String {
        String::from(match self {
            Syntax::Intel | Syntax::Att => "asm",
            Syntax::Llvm => "llvm-ir",
            Syntax::Mir => "mir",
        })
    }

    #[must_use]
    pub const fn ext(&self) -> &str {
        match self {
            Syntax::Intel | Syntax::Att => "s",
            Syntax::Llvm => "ll",
            Syntax::Mir => "mir",
        }
    }
}

fn color_detection() -> impl Parser<bool> {
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
    pub const fn correction(&self) -> Option<&'static str> {
        match self {
            Focus::Example(_) => Some("examples"),
            Focus::Lib | Focus::Test(_) | Focus::Bench(_) | Focus::Bin(_) => None,
        }
    }
}

pub fn select_package(opts: &Options, ws: &Workspace) -> String {
    let package = match (ws.root_maybe(), &opts.package) {
        (_, Some(p)) => {
            if let Some(package) = ws.members().find(|package| package.name().as_str() == p) {
                package
            } else {
                // give up and let rustc to handle the rest
                return p.to_string();
            }
        }
        (MaybePackage::Package(p), None) => p,
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
