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
    pub format: Format,
}

#[derive(Debug, Clone)]
pub struct Format {
    pub rust: bool,
    pub color: bool,
}

pub fn opts() -> Options {
    let manifest = long("manifest-path")
        .help("Path to Cargo.toml")
        .argument_os("PATH")
        .map(PathBuf::from)
        .fallback_with(|| std::env::current_dir().map(|x| x.join("Cargo.toml")));

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

    let parser = construct!(Options {
        target,
        manifest,
        dry,
        package,
        frozen,
        locked,
        offline,
        format,
        function,
    });

    /*
    pub fn options() -> OptionParser<(Level, OsString, Command)> {
        Info::default().for_parser(command(
            "hackerman",
            Some("A set of commands to do strange things to the workspace"),
            options_inner(),
        ))
    }*/

    let command_parser = command(
        "asm",
        Some("A command to display raw asm for some rust function"),
        Info::default().for_parser(parser),
    );

    Info::default().for_parser(command_parser).run()
}
