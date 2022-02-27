use bpaf::*;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Options {
    pub manifest: PathBuf,
    pub target: Option<PathBuf>,
    pub package: Option<String>,
    pub function: Option<String>,
    pub dry: bool,
    pub frozen: bool,
    pub locked: bool,
    pub offline: bool,
}

pub fn opts() -> Options {
    let manifest = long("manifest-path")
        .help("Path to Cargo.toml")
        .argument_os("PATH")
        .map(PathBuf::from)
        .fallback("Cargo.toml".into());

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

    let parser = construct!(Options {
        target,
        manifest,
        dry,
        package,
        function,
        frozen,
        locked,
        offline
    });

    Info::default().for_parser(parser).run()
}
