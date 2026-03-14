use crate::cached_lines::CachedLines;
use crate::esafeprintln;
use crate::opts::SourcesFrom;
use std::borrow::Cow;
use std::path::{Display, Path, PathBuf};

pub(crate) type SourceFile = (String, Option<(Source, CachedLines)>);

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
        let mut source = std::env::home_dir().expect("No home dir?");

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

/// Returns a closure that trims the paths
pub(crate) fn path_formatter() -> impl for<'p> Fn(&'p Path, &'p mut PathBuf) -> Display<'p> {
    let current_dir = std::env::current_dir().unwrap_or_default();
    let home_dir = std::env::home_dir();
    let home = if std::path::MAIN_SEPARATOR == '/' {
        "~"
    } else {
        "%userprofile%"
    };
    move |path, tmp| {
        if path.is_absolute() {
            if let Ok(rel) = path.strip_prefix(&current_dir) {
                rel
            } else if let Some(path_in_home) = home_dir
                .as_ref()
                .and_then(|home| path.strip_prefix(home).ok())
            {
                tmp.clear();
                tmp.push(home);
                tmp.push(path_in_home);
                &*tmp
            } else {
                path
            }
        } else {
            path
        }
        .display()
    }
}
