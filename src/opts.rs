use bpaf::*;
use cargo::ops::CompileFilter;
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

fn focus() -> Parser<Focus> {
    let lib = long("lib").req_flag(Focus::Lib);
    let bin = long("bin").argument("BIN").map(Focus::Bin);
    let test = long("test").argument("TEST").map(Focus::Test);
    let bench = long("bench").argument("BENCH").map(Focus::Bench);
    let example = long("example").argument("EXAMPLE").map(Focus::Example);
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
