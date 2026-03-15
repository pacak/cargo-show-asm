use crate::Format;
use crate::cached_lines::CachedLines;
use crate::opts::SourcesFrom;
use crate::{esafeprintln, safeprintln};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

pub(crate) type SourceFile = (String, Option<(Source, CachedLines)>);

pub struct SourceFileIndex<'a> {
    workspace: &'a Path,
    sysroot: &'a Path,

    path_formatter: PathFormatter,
    index: BTreeMap<u64, SourceFile>,
}

impl<'a> SourceFileIndex<'a> {
    pub fn new(workspace: &'a Path, sysroot: &'a Path) -> Self {
        Self {
            workspace,
            sysroot,
            path_formatter: PathFormatter {
                home_dir: env::home_dir(),
                current_dir: env::current_dir().unwrap_or_default(),
            },
            index: Default::default(),
        }
    }

    pub fn get(&self, at: u64) -> Option<&SourceFile> {
        self.index.get(&at)
    }

    /// Cache sourcecode for the file if possible
    pub fn load(&mut self, f: &File<'_>, fmt: &Format) {
        self.index.entry(f.index).or_insert_with(|| {
            let path = f
                .path
                .as_full_path_with_home_dir(self.path_formatter.home_dir.as_deref());
            let pretty_path = self.path_formatter.format_path(&path).display().to_string();
            if fmt.verbosity > 2 {
                safeprintln!("Reading file #{} {}", f.index, path.display());
            }

            if let Some((source, filepath)) = locate_sources(self.sysroot, self.workspace, &path) {
                if fmt.verbosity > 3 {
                    safeprintln!("Resolved name is {filepath:?}");
                }
                let sources = std::fs::read_to_string(&filepath).expect("Can't read a file");
                if sources.is_empty() {
                    if fmt.verbosity > 0 {
                        safeprintln!("Ignoring empty file {filepath:?}!");
                    }
                    (pretty_path, None)
                } else {
                    if fmt.verbosity > 3 {
                        safeprintln!("Got {} bytes", sources.len());
                    }
                    let lines = CachedLines::without_ending(sources);
                    (pretty_path, Some((source, lines)))
                }
            } else {
                if fmt.verbosity > 1 {
                    safeprintln!("File not found {}", path.display());
                }
                (pretty_path, None)
            }
        });
    }
}

struct PathFormatter {
    home_dir: Option<PathBuf>,
    current_dir: PathBuf,
}

impl PathFormatter {
    /// Trims the paths
    fn format_path<'p>(&self, path: &'p Path) -> Cow<'p, Path> {
        let home = if std::path::MAIN_SEPARATOR == '/' {
            "~"
        } else {
            "%userprofile%"
        };
        if path.is_absolute() {
            if let Ok(rel) = path.strip_prefix(&self.current_dir) {
                return rel.into();
            }
            if let Some(path_in_home) = self
                .home_dir
                .as_ref()
                .and_then(|home| path.strip_prefix(home).ok())
            {
                return Path::new(home).join(path_in_home).into();
            }
        }
        return path.into();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File<'a> {
    pub index: u64,
    pub path: FilePath,
    pub md5: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilePath {
    FullPath(String),
    PathAndFileName { path: String, name: String },
}

impl FilePath {
    pub fn as_full_path(&self) -> Cow<'_, Path> {
        match self {
            FilePath::FullPath(path) => Cow::Borrowed(Path::new(path)),
            FilePath::PathAndFileName { path, name } => Cow::Owned(Path::new(path).join(name)),
        }
    }

    /// Optionally expand `~/` to `home_dir`
    ///
    /// Rewritten debug paths may use `~/` for privacy, but that's not a real path,
    /// because `~` is usually expanded by the shell.
    pub fn as_full_path_with_home_dir(&self, home_dir: Option<&Path>) -> Cow<'_, Path> {
        let path = self.as_full_path();

        if let Some(home_dir) = home_dir {
            if let Ok(path_in_home) = path.strip_prefix("~") {
                return Cow::Owned(home_dir.join(path_in_home));
            }
        }

        path
    }
}

#[derive(Debug, Clone)]
pub enum Source {
    Crate,
    External,
    Stdlib,
    Rustc,
}

impl Source {
    pub(crate) fn show_for(&self, from: SourcesFrom) -> bool {
        match self {
            Self::Crate => true,
            Self::External => match from {
                SourcesFrom::ThisWorkspace => false,
                SourcesFrom::AllCrates | SourcesFrom::AllSources => true,
            },
            Self::Rustc | Self::Stdlib => match from {
                SourcesFrom::ThisWorkspace | SourcesFrom::AllCrates => false,
                SourcesFrom::AllSources => true,
            },
        }
    }
}

// DWARF information contains references to source files
// It can point to 3 different items:
// 1. a real file, cargo-show-asm can just read it
// 2. a file from rustlib, sources are under $sysroot/lib/rustlib/src/rust/$suffix
//    Some examples:
//        /rustc/a55dd71d5fb0ec5a6a3a9e8c27b2127ba491ce52/library/core/src/iter/range.rs
//        /private/tmp/rust-20230325-7327-rbrpyq/rustc-1.68.1-src/library/core/src/option.rs
//        /rustc/cc66ad468955717ab92600c770da8c1601a4ff33\\library\\core\\src\\convert\\mod.rs
// 3. a file from prebuilt (?) hashbrown, sources are probably available under
//    cargo registry, most likely under ~/.cargo/registry/$suffix
//    Some examples:
//        /cargo/registry/src/github.com-1ecc6299db9ec823/hashbrown-0.12.3/src/raw/bitmask.rs
//        /Users/runner/.cargo/registry/src/github.com-1ecc6299db9ec823/hashbrown-0.12.3/src/map.rs
// 4. rustc sources:
//    /rustc/89e2160c4ca5808657ed55392620ed1dbbce78d1/compiler/rustc_span/src/span_encoding.rs
//    $sysroot/lib/rustlib/rust-src/rust/compiler/rustc_span/src/span_encoding.rs
pub(crate) fn locate_sources(
    sysroot: &Path,
    workspace: &Path,
    path: &Path,
) -> Option<(Source, PathBuf)> {
    let mut path = Cow::Borrowed(path);
    // a real file that simply exists
    if path.exists() {
        let source = if path.starts_with(workspace) {
            Source::Crate
        } else {
            Source::External
        };

        return Some((source, path.into()));
    }

    let no_rust_src = || {
        esafeprintln!(
            "You need to install rustc sources to be able to see the rust annotations, try\n\
                                       \trustup component add rust-src"
        );
        std::process::exit(1);
    };

    // then during crosscompilation we can get this cursed mix of path names
    //
    // /rustc/cc66ad468955717ab92600c770da8c1601a4ff33\\library\\core\\src\\convert\\mod.rs
    //
    // where one bit comes from the host platform and second bit comes from the target platform
    // This feels like a problem in upstream, but supporting that is not _that_ hard.
    //
    // I think this should take care of Linux and MacOS support
    if (path.starts_with("/rustc/") || path.starts_with("/private/tmp"))
        && path
            .as_os_str()
            .to_str()
            .is_some_and(|s| s.contains('\\') && s.contains('/'))
    {
        let cursed_path = path
            .as_os_str()
            .to_str()
            .expect("They are coming from a text file");
        path = Cow::Owned(PathBuf::from(cursed_path.replace('\\', "/")));
    }

    // /rustc/89e2160c4ca5808657ed55392620ed1dbbce78d1/compiler/rustc_span/src/span_encoding.rs
    if path.starts_with("/rustc") && path.iter().any(|c| c == "compiler") {
        let mut source = sysroot.join("lib/rustlib/rustc-src/rust");
        for part in path.components().skip(3) {
            source.push(part);
        }

        if source.exists() {
            return Some((Source::Rustc, source));
        }
        no_rust_src();
    }

    // rust sources, Linux style
    if path.starts_with("/rustc/") {
        let mut source = sysroot.join("lib/rustlib/src/rust");
        for part in path.components().skip(3) {
            source.push(part);
        }
        if source.exists() {
            return Some((Source::Stdlib, source));
        }
        no_rust_src();
    }

    // rust sources, MacOS style
    if path.starts_with("/private/tmp") && path.components().any(|c| c.as_os_str() == "library") {
        let mut source = sysroot.join("lib/rustlib/src/rust");
        for part in path.components().skip(5) {
            source.push(part);
        }
        if source.exists() {
            return Some((Source::Stdlib, source));
        }
        no_rust_src();
    }

    // cargo registry, Linux and macOS look for cargo/registry and .cargo/registry
    if let Some(ix) = path
        .components()
        .position(|c| c.as_os_str() == "cargo" || c.as_os_str() == ".cargo")
        .and_then(|ix| path.components().nth(ix).zip(Some(ix)))
        .and_then(|(c, ix)| (c.as_os_str() == "registry").then_some(ix))
    {
        // It does what I want as far as *nix is concerned, might not work for Windows...
        #[allow(deprecated)]
        let mut source = env::home_dir().expect("No home dir?");

        source.push(".cargo");
        for part in path.components().skip(ix) {
            source.push(part);
        }
        if source.exists() {
            return Some((Source::External, source));
        }
        panic!("{path:?} looks like it can be a cargo registry reference but we failed to get it");
    }

    None
}
