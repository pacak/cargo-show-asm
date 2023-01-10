use bpaf::{construct, long, short, Bpaf, Parser};
use cargo_metadata::Artifact;
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
    /// Package to use if ambigous
    #[bpaf(long, short, argument("SPEC"))]
    pub package: Option<String>,
    #[bpaf(external, optional)]
    pub focus: Option<Focus>,

    // how to compile
    #[bpaf(external)]
    pub cargo: Cargo,

    // how to display
    /// Generate code for a specific CPU
    #[bpaf(external)]
    pub target_cpu: Option<String>,
    #[bpaf(external)]
    pub format: Format,
    #[bpaf(external)]
    pub syntax: Syntax,

    // what to display
    #[bpaf(external)]
    pub to_dump: ToDump,
}

#[derive(Debug, Clone, Bpaf)]
#[allow(clippy::struct_excessive_bools)]
pub struct Cargo {
    #[bpaf(external, hide_usage)]
    pub manifest_path: PathBuf,

    /// Use custom target directory for generated artifacts, create if missing
    #[bpaf(
        env("CARGO_TARGET_DIR"),
        argument("DIR"),
        parse(check_target_dir),
        optional,
        hide_usage
    )]
    pub target_dir: Option<PathBuf>,
    /// Produce a build plan instead of actually building
    #[bpaf(hide_usage)]
    pub dry: bool,
    /// Requires Cargo.lock and cache are up to date
    #[bpaf(hide_usage)]
    pub frozen: bool,
    /// Requires Cargo.lock is up to date
    #[bpaf(hide_usage)]
    pub locked: bool,
    /// Run without accessing the network
    #[bpaf(hide_usage)]
    pub offline: bool,
    #[bpaf(external, hide_usage)]
    pub cli_features: CliFeatures,
    #[bpaf(external)]
    pub compile_mode: CompileMode,
    /// Build for the target triple
    #[bpaf(argument("TRIPLE"))]
    pub target: Option<String>,
}

#[derive(Debug, Clone, Bpaf)]
pub enum ToDump {
    /// Dump the whole asm file
    Everything,

    ByIndex {
        /// Dump name with this index
        #[bpaf(positional("ITEM_INDEX"))]
        value: usize,
    },

    Function {
        /// Dump function with that specific name / filter functions containing this string
        #[bpaf(positional("FUNCTION"), optional)]
        function: Option<String>,

        /// Select specific function when there's several with the same name
        #[bpaf(positional("INDEX"), fallback(0))]
        nth: usize,
    },
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
    /// Do not activate `default` feature
    pub no_default_features: bool,

    /// Activate all available features
    pub all_features: bool,

    /// A feature to activate, can be used multiple times
    #[bpaf(argument("FEATURE"))]
    pub features: Vec<String>,
}

#[derive(Bpaf, Clone, Debug)]
#[bpaf(fallback(CompileMode::Release))]
pub enum CompileMode {
    /// Compile in release mode (default)
    Release,
    /// Compile in dev mode
    Dev,
    Custom(
        /// Build for this specific profile
        #[bpaf(long("profile"), argument("PROFILE"))]
        String,
    ),
}

fn verbosity() -> impl Parser<usize> {
    short('v')
        .long("verbose")
        .help("more verbose output, can be specified multiple times")
        .req_flag(())
        .many()
        .map(|v| v.len())
        .hide_usage()
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

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Bpaf, Copy)]
pub struct Format {
    /// Print interleaved Rust code
    pub rust: bool,

    #[bpaf(external(color_detection), hide_usage)]
    pub color: bool,

    /// Include full demangled name instead of just prefix
    #[bpaf(hide_usage)]
    pub full_name: bool,

    /// Keep all the original labels
    #[bpaf(hide_usage)]
    pub keep_labels: bool,

    /// more verbose output, can be specified multiple times
    #[bpaf(external)]
    pub verbosity: usize,

    /// Try to strip some of the non-assembly instruction information
    pub simplify: bool,
}

#[derive(Debug, Clone, Bpaf, Eq, PartialEq, Copy)]
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
    /// Show WASM, needs wasm32-unknown-unknown target installed
    Wasm,
}

impl Syntax {
    #[must_use]
    pub fn format(&self) -> Option<&str> {
        match self {
            Syntax::Intel => Some("llvm-args=-x86-asm-syntax=intel"),
            Syntax::Att => Some("llvm-args=-x86-asm-syntax=att"),
            Syntax::Wasm | Syntax::Mir | Syntax::Llvm => None,
        }
    }

    #[must_use]
    pub fn emit(&self) -> &str {
        match self {
            Syntax::Intel | Syntax::Att | Syntax::Wasm => "asm",
            Syntax::Llvm => "llvm-ir",
            Syntax::Mir => "mir",
        }
    }

    #[must_use]
    pub fn ext(&self) -> &str {
        match self {
            Syntax::Intel | Syntax::Att | Syntax::Wasm => "s",
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

    // Show available tests (hidden: cargo shows the list as an error)
    #[bpaf(long("test"), hide)]
    TestList,

    Bench(
        /// Show results from a benchmark
        #[bpaf(long("bench"), argument("BENCH"))]
        String,
    ),

    // Show available benchmarks (hidden: cargo shows the list as an error)
    #[bpaf(long("bench"), hide)]
    BenchList,

    Example(
        /// Show results from an example
        #[bpaf(long("example"), argument("EXAMPLE"))]
        String,
    ),

    // Show available examples (hidden: cargo shows the list as an error)
    #[bpaf(long("example"), hide)]
    ExampleList,

    Bin(
        /// Show results from a binary
        #[bpaf(long("bin"), argument("BIN"))]
        String,
    ),

    // Show available binaries (hidden: cargo shows the list as an error)
    #[bpaf(long("bin"), hide)]
    BinList,
}

impl TryFrom<&'_ cargo_metadata::Target> for Focus {
    type Error = anyhow::Error;

    fn try_from(target: &cargo_metadata::Target) -> Result<Self, Self::Error> {
        match target.kind.first().map(|s| &**s) {
            Some("lib" | "rlib") => Ok(Focus::Lib),
            Some("test") => Ok(Focus::Test(target.name.clone())),
            Some("bench") => Ok(Focus::Bench(target.name.clone())),
            Some("example") => Ok(Focus::Example(target.name.clone())),
            Some("bin") => Ok(Focus::Bin(target.name.clone())),
            _ => anyhow::bail!("Unknow target kind: {:?}", target.kind),
        }
    }
}

impl Focus {
    #[must_use]
    pub fn as_parts(&self) -> (&str, Option<&str>) {
        match self {
            Focus::Lib => ("lib", None),
            Focus::Test(name) => ("test", Some(name)),
            Focus::TestList => ("test", None),
            Focus::Bench(name) => ("bench", Some(name)),
            Focus::BenchList => ("bench", None),
            Focus::Example(name) => ("example", Some(name)),
            Focus::ExampleList => ("example", None),
            Focus::Bin(name) => ("bin", Some(name)),
            Focus::BinList => ("bin", None),
        }
    }

    pub fn as_cargo_args(&self) -> impl Iterator<Item = String> {
        let (kind, name) = self.as_parts();
        std::iter::once(format!("--{}", kind)).chain(name.map(ToOwned::to_owned))
    }

    #[must_use]
    pub fn matches_artifact(&self, artifact: &Artifact) -> bool {
        let (kind, name) = self.as_parts();
        let somewhat_matches = kind == "lib" && artifact.target.kind.iter().any(|i| i == "rlib");
        let kind_matches = artifact.target.kind == [kind];
        (somewhat_matches || kind_matches)
            && name.map_or(true, |name| artifact.target.name == *name)
    }
}
