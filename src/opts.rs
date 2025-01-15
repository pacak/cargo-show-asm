use bpaf::{construct, doc::Style, long, short, Bpaf, Parser};
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
    // here is the code located
    #[bpaf(external)]
    pub select_fragment: SelectFragment,

    // how to compile
    #[bpaf(external)]
    pub code_source: CodeSource,

    // how to display
    /// Pass parameter to llvm-mca for mca targets
    #[bpaf(short('M'), long)]
    pub mca_arg: Vec<String>,
    /// Generate code for a specific CPU
    #[bpaf(external)]
    pub target_cpu: Option<String>,
    #[bpaf(external)]
    pub format: Format,
    #[bpaf(external(syntax_compat))]
    pub syntax: Syntax,

    // what to display
    #[bpaf(external)]
    pub to_dump: ToDump,
}

#[derive(Debug, Clone, Bpaf)]
pub enum CodeSource {
    FromCargo {
        #[bpaf(external(cargo))]
        cargo: Cargo,
    },
    File {
        /// Disassemble or process this file instead of calling cargo,
        ///  requires cargo-show-asm to be compiled with disasm feature
        ///
        /// You can specify executable, rlib or an object file
        #[bpaf(argument("PATH"), hide_usage)]
        file: PathBuf,
    },
}

#[derive(Clone, Debug, Bpaf)]
pub struct SelectFragment {
    // what to compile
    /// Package to use, defaults to a current one,
    ///
    /// required for workspace projects, can also point
    /// to a dependency
    #[bpaf(long, short, argument("SPEC"))]
    pub package: Option<String>,

    #[bpaf(external, optional)]
    pub focus: Option<Focus>,
}

#[derive(Debug, Clone, Bpaf)]
#[allow(clippy::struct_excessive_bools)]
/// Cargo options
#[bpaf(hide_usage)]
pub struct Cargo {
    #[bpaf(external, hide_usage)]
    pub manifest_path: PathBuf,

    /// Override a cargo configuration value
    #[bpaf(argument("KEY=VALUE"))]
    pub config: Vec<String>,

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
    /// Requires Cargo.lock and cache to be up-to-date
    #[bpaf(hide_usage)]
    pub frozen: bool,
    /// Requires Cargo.lock to be up-to-date
    #[bpaf(hide_usage)]
    pub locked: bool,
    /// Run without accessing the network
    #[bpaf(hide_usage)]
    pub offline: bool,
    /// Do not print cargo log messages
    #[bpaf(short, long, hide_usage)]
    pub quiet: bool,
    #[bpaf(external, hide_usage)]
    pub cli_features: CliFeatures,
    #[bpaf(external)]
    pub compile_mode: CompileMode,
    /// Build for the target triple
    #[bpaf(argument("TRIPLE"))]
    pub target: Option<String>,
    /// Codegen flags to rustc, see 'rustc -C help' for details
    #[bpaf(short('C'), argument("FLAG"))]
    pub codegen: Vec<String>,
    /// Unstable (nightly-only) flags to Cargo, see 'cargo -Z help' for details
    // OsString would be better but MetadataCommand takes a vector of strings...
    #[bpaf(short('Z'), argument("FLAG"))]
    pub unstable: Vec<String>,
}

#[derive(Debug, Clone, Bpaf)]
/// Pick item to display from the artifact
#[bpaf(fallback(ToDump::Unspecified))]
pub enum ToDump {
    /// Dump the whole file
    Everything,

    #[bpaf(hide)]
    ByIndex {
        /// Dump name with this index
        #[bpaf(positional("ITEM_INDEX"))]
        value: usize,
    },

    Function {
        /// Dump a function with a given name, filter functions by name
        #[bpaf(positional("FUNCTION"))]
        function: String,

        /// Select specific function when there's several with the same name
        #[bpaf(positional("INDEX"))]
        nth: Option<usize>,
    },

    #[bpaf(skip)]
    Unspecified,
}

fn target_cpu() -> impl Parser<Option<String>> {
    let native = long("native")
        .help("Optimize for the CPU running the compiler")
        .req_flag("native".to_string());
    let cpu = long("target-cpu")
        .help("Optimize code for a specific CPU, see 'rustc --print target-cpus'")
        .argument::<String>("CPU");
    construct!([native, cpu])
        .custom_usage(&[("TARGET-CPU", Style::Metavar)])
        .optional()
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
        /// Build for this specific profile, you can also use `dev` and `release` here
        #[bpaf(env("CARGO_SHOW_ASM_PROFILE"), long("profile"), argument("PROFILE"))]
        String,
    ),
}

fn verbosity() -> impl Parser<usize> {
    let verbose = short('v')
        .long("verbose")
        .help("more verbose output, can be specified multiple times")
        .req_flag(())
        .hide_usage()
        .count();
    let silent = short('s')
        .long("silent")
        .help("print less user-forward information to make consumption by tools easier")
        .req_flag(())
        .hide_usage()
        .count();
    construct!(verbose, silent).map(|(v, q)| (v + 1).saturating_sub(q))
}

fn manifest_path() -> impl Parser<PathBuf> {
    long("manifest-path")
        .help("Path to Cargo.toml, defaults to one in current folder")
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
/// Postprocessing options:
pub struct Format {
    /// Print interleaved Rust code
    pub rust: bool,

    /// Include other called functions, recursively, up to COUNT depth
    #[bpaf(short, long, argument("COUNT"), fallback(0), display_fallback)]
    pub context: usize,

    #[bpaf(external(color_detection), hide_usage)]
    pub color: bool,

    #[bpaf(hide_usage, external)]
    pub name_display: NameDisplay,

    #[bpaf(external, hide_usage)]
    pub redundant_labels: RedundantLabels,

    /// more verbose output, can be specified multiple times
    #[bpaf(external)]
    pub verbosity: usize,

    /// Try to strip some of the non-assembly instruction information
    pub simplify: bool,

    /// Include sections containing string literals and other constants
    pub include_constants: bool,

    /// Keep blank lines
    #[bpaf(short('b'), long, hide_usage)]
    pub keep_blank: bool,

    #[bpaf(external)]
    pub sources_from: SourcesFrom,
}

#[derive(Debug, Clone, Copy, Bpaf)]
#[bpaf(fallback(SourcesFrom::AllSources))]
pub enum SourcesFrom {
    /// Show rust sources from current workspace only
    ThisWorkspace,
    /// Show rust sources from current workspace and from rust registry
    AllCrates,
    /// Show all the rust sources including stdlib and compiler
    AllSources,
}

#[derive(Debug, Clone, Bpaf, Eq, PartialEq, Copy)]
#[bpaf(fallback(RedundantLabels::Strip))]
pub enum RedundantLabels {
    /// Keep all the original labels
    #[bpaf(short('K'), long("keep-labels"))]
    Keep,
    /// Strip redundant labels, but keep spaces in their place
    #[bpaf(short('B'), long("keep-blanks"))]
    Blanks,
    /// Strip redundant labels entirely
    #[bpaf(short('R'), long("reduce-labels"))]
    Strip,
}

#[derive(Debug, Copy, Clone, Bpaf, Eq, PartialEq)]
#[bpaf(fallback(NameDisplay::Short))]
pub enum NameDisplay {
    #[bpaf(long("full-name"))]
    /// Include full demangled name instead of just prefix
    Full,

    /// Include demangled names without hash suffix (default)
    #[bpaf(long("short-name"))]
    Short,

    /// Do not demangle symbol names
    #[bpaf(long("keep-mangled"))]
    Mangled,
}

#[derive(Debug, Clone, Bpaf, Eq, PartialEq, Copy)]
#[bpaf(fallback(OutputType::Asm))]
pub enum OutputType {
    /// Show assembly
    Asm,
    /// Disassembly binaries or object files
    Disasm,
    /// Show llvm-ir
    Llvm,
    /// Show llvm-ir before any LLVM passes
    LlvmInput,
    /// Show MIR
    Mir,
    /// Show WASM, needs wasm32-unknown-unknown target installed
    Wasm,
    /// Show llvm-mca anasysis
    Mca,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Bpaf)]
#[bpaf(fallback(OutputStyle::Intel))]
pub enum OutputStyle {
    /// Use Intel style for assembly
    Intel,
    /// Use AT&T style for assembly
    Att,
}

#[derive(Debug, Clone, Copy, Bpaf)]
#[bpaf(custom_usage(&[("OUTPUT-FORMAT", Style::Metavar)]))]
/// Pick output type:
pub struct Syntax {
    #[bpaf(external)]
    pub output_type: OutputType,
    #[bpaf(external)]
    pub output_style: OutputStyle,
}

fn syntax_compat() -> impl Parser<Syntax> {
    let mca_att = long("mca-att")
        .req_flag(Syntax {
            output_type: OutputType::Mca,
            output_style: OutputStyle::Att,
        })
        .hide();
    let mca_intel = long("mca-intel")
        .req_flag(Syntax {
            output_type: OutputType::Mca,
            output_style: OutputStyle::Intel,
        })
        .hide();
    construct!([syntax(), mca_att, mca_intel])
}

impl Syntax {
    #[must_use]
    pub fn format(&self) -> Option<&str> {
        match self.output_type {
            OutputType::Asm | OutputType::Mca => match self.output_style {
                OutputStyle::Intel => Some("llvm-args=-x86-asm-syntax=intel"),
                OutputStyle::Att => Some("llvm-args=-x86-asm-syntax=att"),
            },
            OutputType::LlvmInput => Some("no-prepopulate-passes"),
            OutputType::Llvm | OutputType::Mir | OutputType::Wasm => None,

            OutputType::Disasm => None,
        }
    }

    #[must_use]
    pub fn emit(&self) -> Option<&str> {
        match self.output_type {
            OutputType::Asm | OutputType::Wasm | OutputType::Mca => Some("asm"),
            OutputType::Llvm | OutputType::LlvmInput => Some("llvm-ir"),
            OutputType::Mir => Some("mir"),

            OutputType::Disasm => None,
        }
    }

    #[must_use]
    pub fn ext(&self) -> Option<&str> {
        match self.output_type {
            OutputType::Asm | OutputType::Wasm | OutputType::Mca => Some("s"),
            OutputType::Llvm | OutputType::LlvmInput => Some("ll"),
            OutputType::Mir => Some("mir"),

            OutputType::Disasm => None,
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
/// Pick artifact for analysis:
#[bpaf(custom_usage(&[("ARTIFACT", Style::Metavar)]))]
pub enum Focus {
    /// Show results from library code
    Lib,

    Test(
        /// Show results from an integration test
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
        use cargo_metadata::TargetKind as T;
        let kind = target
            .kind
            .first()
            .ok_or_else(|| anyhow::anyhow!("No target kinds in target"))?;
        let name = target.name.clone();
        match kind {
            T::Lib | T::RLib | T::CDyLib => Ok(Focus::Lib),
            T::Test => Ok(Focus::Test(name)),
            T::Bench => Ok(Focus::Bench(name)),
            T::Example => Ok(Focus::Example(name)),
            T::Bin => Ok(Focus::Bin(name)),
            // don't bother with handling remaining cases since struct is #[non_exhaustive]
            _ => anyhow::bail!("Unsupported target kind {kind:?}"),
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
        std::iter::once(format!("--{kind}")).chain(name.map(ToOwned::to_owned))
    }

    #[must_use]
    pub fn matches_artifact(&self, artifact: &Artifact) -> bool {
        let (kind, name) = self.as_parts();
        let somewhat_matches =
            kind == "lib" && artifact.target.is_rlib() || artifact.target.is_cdylib();
        let kind = <cargo_metadata::TargetKind as std::str::FromStr>::from_str(kind)
            .expect("cargo_metadata made me do it");
        let kind_matches = artifact.target.kind.contains(&kind);
        (somewhat_matches || kind_matches)
            && name.map_or(true, |name| artifact.target.name == *name)
    }
}

#[cfg(unix)]
#[cfg(test)]
fn write_updated(new_val: &str, path: impl AsRef<std::path::Path>) -> std::io::Result<bool> {
    use std::io::Read;
    use std::io::Seek;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .read(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    let mut current_val = String::new();
    file.read_to_string(&mut current_val)?;
    if current_val != new_val {
        file.set_len(0)?;
        file.seek(std::io::SeekFrom::Start(0))?;
        std::io::Write::write_all(&mut file, new_val.as_bytes())?;
        Ok(false)
    } else {
        Ok(true)
    }
}

#[cfg(unix)]
#[test]
fn docs_are_up_to_date() {
    let usage = options().render_markdown("cargo asm");
    let readme = std::fs::read_to_string("README.tpl").unwrap();
    let docs = readme.replacen("<USAGE>", &usage, 1);
    assert!(write_updated(&docs, "README.md").unwrap());
}
