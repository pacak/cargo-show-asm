# cargo-show-asm

A cargo subcommand that displays the Assembly, LLVM-IR, MIR and WASM generated for Rust source code.

# Install

```console
$ cargo install cargo-show-asm
```

# Features

- Platform support:

  - OS: Linux and MacOS. Limited support for Windows
  - Rust: nightly and stable.
  - Architectures: `x86`, `x86_64`, `aarch64`, etc.
  - Cross-compilation support.

- Displaying:

  - Assembly in Intel or AT&T syntax.
  - Corresponding Rust source code alongside assembly.
  - llvm-ir.
  - rustc MIR
  - Wasm code
  - llvm-mca analysis

# Usage:

```console
Show the code rustc generates for any function

Usage: [-p SPEC] [--lib | --test TEST | --bench BENCH | --example EXAMPLE | --bin BIN] [--release |
--dev | --profile PROFILE] [--target TRIPLE] -C FLAG... -Z FLAG... [--native | --target-cpu CPU]
[--rust] [--simplify] -M ARG... [--intel | --att | --llvm | --llvm-input | --mir | --wasm |
--mca-intel | --mca-att] [--everything | <ITEM_INDEX> | <FUNCTION> [<INDEX>]]

Usage:
  1. Focus on a single assembly producing target:
     % cargo asm -p isin --lib   # here we are targeting lib in isin crate
  2. Narrow down a function:
     % cargo asm -p isin --lib from_ # here "from_" is part of the function you are interested intel
  3. Get the full results:
     % cargo asm -p isin --lib isin::base36::from_alphanum

Available positional items:
    <ITEM_INDEX>  Dump name with this index
    <FUNCTION>    Dump function with that specific name / filter functions containing this string
    <INDEX>       Select specific function when there's several with the same name

Available options:
    -p, --package <SPEC>  Package to use, defaults to a current one, required for workspace projects,
                          can also point to a dependency
        --lib             Show results from library code
        --test <TEST>     Show results from an integration test
        --bench <BENCH>   Show results from a benchmark
        --example <EXAMPLE>  Show results from an example
        --bin <BIN>       Show results from a binary
        --manifest-path <PATH>  Path to Cargo.toml, defaults to one in current folder
        --target-dir <DIR>  [env:CARGO_TARGET_DIR: N/A]
                          Use custom target directory for generated artifacts, create if missing
        --dry             Produce a build plan instead of actually building
        --frozen          Requires Cargo.lock and cache are up to date
        --locked          Requires Cargo.lock is up to date
        --offline         Run without accessing the network
        --no-default-features  Do not activate `default` feature
        --all-features    Activate all available features
        --features <FEATURE>  A feature to activate, can be used multiple times
        --release         Compile in release mode (default)
        --dev             Compile in dev mode
        --profile <PROFILE>  Build for this specific profile
        --target <TRIPLE>  Build for the target triple
    -C <FLAG>             Codegen flags to rustc, see 'rustc -C help' for details
    -Z <FLAG>             Unstable (nightly-only) flags to Cargo, see 'cargo -Z help' for details
        --native          Optimize for the CPU running the compiler
        --target-cpu <CPU>  Optimize code for a specific CPU, see 'rustc --print target-cpus'
        --rust            Print interleaved Rust code
        --color           Enable color highlighting
        --no-color        Disable color highlighting
        --full-name       Include full demangled name instead of just prefix
        --keep-labels     Keep all the original labels
    -v, --verbose         more verbose output, can be specified multiple times
        --simplify        Try to strip some of the non-assembly instruction information
    -M, --mca-arg <ARG>   Pass parameter to llvm-mca for mca targets
        --intel           Show assembly using Intel style
        --att             Show assembly using AT&T style
        --llvm            Show llvm-ir
        --llvm-input      Show llvm-ir before any LLVM passes
        --mir             Show MIR
        --wasm            Show WASM, needs wasm32-unknown-unknown target installed
        --mca-intel       Show llvm-mca analysis, Intel style asm
        --mca-att         Show llvm-mca analysis, AT&T style asm
        --everything      Dump the whole asm file
    -h, --help            Prints help information
    -V, --version         Prints version information
```

You can start by running `cargo asm` with no parameters - it will suggests how to narrow the
search scope - for workspace crates you need to specify a crate to work with, for crates
defining several targets (lib, binaries, examples) you need to specify exactly which target to
use. In a workspace `cargo asm` lists only workspace members as suggestions but any crate from
workspace tree is available.

Once `cargo asm` focuses on a single target it will run rustc producing assembly file and will
try to list of available public functions:

```console,ignore
$ cargo asm --lib
Try one of those
"<&T as core::fmt::Display>::fmt" [17, 12, 12, 12, 12, 19, 19, 12]
"<&mut W as core::fmt::Write>::write_char" [20]
"<&mut W as core::fmt::Write>::write_fmt" [38]
"<&mut W as core::fmt::Write>::write_str" [90]
"<F as nom::internal::Parser<I,O,E>>::parse" [263]
# ...
```

Name in quotes is demangled rust name, numbers in square brackets represent number of lines
in asm file. Function with the same name can be present in several instances.

Specifying exact function name or a uniquely identifying part of it will print its assembly code

```console,ignore
$ cargo asm --lib "cargo_show_asm::opts::focus::{{closure}}"
```
To pick between different alternatives you can either specify the index

```console,ignore
$ cargo asm --lib "cargo_show_asm::opts::focus::{{closure}}" 2
```
Or start using full names with hex included:

```console,ignore
$ cargo asm --lib --full-name
# ...
$ cargo asm --lib "once_cell::imp::OnceCell<T>::initialize::h9c5c7d5bd745000b"
```

`cargo-show-asm` comes with a built in search function. Just pass partial name
instead of a full one and only matching functions will be listed

```console
$ cargo asm --lib Debug
```

# My function isn't there!

`rustc` will only generate the code for your function if it knows what type it is, including
generic parameters and if it is exported (in case of a library) and not inlined (in case of a
binary, example, test, etc). If your function takes a generic parameter - try making a monomorphic
wrapper around it and make it `pub` and `#[inline(never)]`.

# What about `cargo-asm`?

`cargo-asm` is not maintained: <https://github.com/gnzlbg/cargo-asm/issues/244>. This crate is a reimplementation which addresses a number of its shortcomings, including:

* `cargo-asm` recompiles everything every time with 1 codegen unit, which is slow and also not necessarily what is in your release profile. `cargo-show-asm` avoids that.

* Because of how `cargo-asm` handles demangling the output looks like asm but isn't actually asm. It contains a whole bunch of extra commas which makes reusing it more annoying.

* `cargo-asm` always uses colors unless you pass a flag while `cargo-show-asm` changes its default behavior if output is not sent to a terminal.

* `cargo-show-asm` also supports MIR (note that the formatting of human-readable MIR is not stable).

# Shell completion

`cargo-asm` comes with shell completion generated by [`bpaf`](https://crates.io/crates/bpaf),
use one of the lines below and place it into the place right for your shell.

```console
$ cargo-asm --bpaf-complete-style-bash
$ cargo-asm --bpaf-complete-style-zsh
$ cargo-asm --bpaf-complete-style-fish
$ cargo-asm --bpaf-complete-style-elvish
```

You'll need to use it as `cargo-asm` command rather than `cargo asm` to take advantage of it.


# Colorful line parser output

You can install `cargo-show-asm` with one of two features to get prettier command line
```console
cargo install cargo-show-asm -F bright-color
cargo install cargo-show-asm -F dull-color
```

# License
This project is licensed under either of

* Apache License, Version 2.0, (LICENSE-APACHE or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license (LICENSE-MIT or <http://opensource.org/licenses/MIT>)

at your option.

# Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
