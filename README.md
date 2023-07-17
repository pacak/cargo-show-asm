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

# cargo asm

Show the code rustc generates for any function

**Usage**: **`cargo asm`** \[**`-p`**=_`SPEC`_\] \[_`ARTIFACT`_\] \[**`-M`**=_`ARG`_\]... \[_`TARGET-CPU`_\] \[**`--rust`**\] \[**`--simplify`**\] \[_`OUTPUT-FORMAT`_\] \[**`--everything`** | _`FUNCTION`_ \[_`INDEX`_\]\]

 Usage:
 1. Focus on a single assembly producing target:

  ```text
   % cargo asm -p isin --lib   # here we are targeting lib in isin crate


  ```
 2. Narrow down a function:

  ```text
   % cargo asm -p isin --lib from_ # here "from_" is part of the function you are interested intel


  ```
 3. Get the full results:

  ```text
   % cargo asm -p isin --lib isin::base36::from_alphanum
  ```


**Pick artifact for analysis:**
- **`    --lib`** &mdash; 
  Show results from library code
- **`    --test`**=_`TEST`_ &mdash; 
  Show results from an integration test
- **`    --bench`**=_`BENCH`_ &mdash; 
  Show results from a benchmark
- **`    --example`**=_`EXAMPLE`_ &mdash; 
  Show results from an example
- **`    --bin`**=_`BIN`_ &mdash; 
  Show results from a binary



**Cargo options**
- **`    --manifest-path`**=_`PATH`_ &mdash; 
  Path to Cargo.toml, defaults to one in current folder
- **`    --target-dir`**=_`DIR`_ &mdash; 
  Use custom target directory for generated artifacts, create if missing
   
  Uses environment variable **`CARGO_TARGET_DIR`**
- **`    --dry`** &mdash; 
  Produce a build plan instead of actually building
- **`    --frozen`** &mdash; 
  Requires Cargo.lock and cache are up to date
- **`    --locked`** &mdash; 
  Requires Cargo.lock is up to date
- **`    --offline`** &mdash; 
  Run without accessing the network
- **`    --no-default-features`** &mdash; 
  Do not activate `default` feature
- **`    --all-features`** &mdash; 
  Activate all available features
- **`    --features`**=_`FEATURE`_ &mdash; 
  A feature to activate, can be used multiple times
- **`    --release`** &mdash; 
  Compile in release mode (default)
- **`    --dev`** &mdash; 
  Compile in dev mode
- **`    --profile`**=_`PROFILE`_ &mdash; 
  Build for this specific profile, you can also use `dev` and `release` here
   
  Uses environment variable **`CARGO_SHOW_ASM_PROFILE`**
- **`    --target`**=_`TRIPLE`_ &mdash; 
  Build for the target triple
- **`-C`**=_`FLAG`_ &mdash; 
  Codegen flags to rustc, see 'rustc -C help' for details
- **`-Z`**=_`FLAG`_ &mdash; 
  Unstable (nightly-only) flags to Cargo, see 'cargo -Z help' for details



**Postprocessing options:**
- **`    --rust`** &mdash; 
  Print interleaved Rust code
- **`    --color`** &mdash; 
  Enable color highlighting
- **`    --no-color`** &mdash; 
  Disable color highlighting
- **`    --full-name`** &mdash; 
  Include full demangled name instead of just prefix
- **`    --keep-labels`** &mdash; 
  Keep all the original labels
- **`-v`**, **`--verbose`** &mdash; 
  more verbose output, can be specified multiple times
- **`    --simplify`** &mdash; 
  Try to strip some of the non-assembly instruction information



**Pick output format:**
- **`    --intel`** &mdash; 
  Show assembly using Intel style
- **`    --att`** &mdash; 
  Show assembly using AT&T style
- **`    --llvm`** &mdash; 
  Show llvm-ir
- **`    --llvm-input`** &mdash; 
  Show llvm-ir before any LLVM passes
- **`    --mir`** &mdash; 
  Show MIR
- **`    --wasm`** &mdash; 
  Show WASM, needs wasm32-unknown-unknown target installed
- **`    --mca-intel`** &mdash; 
  Show llvm-mca analysis, Intel style asm
- **`    --mca-att`** &mdash; 
  Show llvm-mca analysis, AT&T style asm



**Pick item to display from the artifact**
- **`    --everything`** &mdash; 
  Dump the whole file
- _`FUNCTION`_ &mdash; 
  Dump a function with a given name, filter functions by name
- _`INDEX`_ &mdash; 
  Select specific function when there's several with the same name



**Available options:**
- **`-p`**, **`--package`**=_`SPEC`_ &mdash; 
  Package to use, defaults to a current one,

  required for workspace projects, can also point to a dependency
- **`-M`**, **`--mca-arg`**=_`ARG`_ &mdash; 
  Pass parameter to llvm-mca for mca targets
- **`    --native`** &mdash; 
  Optimize for the CPU running the compiler
- **`    --target-cpu`**=_`CPU`_ &mdash; 
  Optimize code for a specific CPU, see 'rustc --print target-cpus'
- **`-h`**, **`--help`** &mdash; 
  Prints help information
- **`-V`**, **`--version`** &mdash; 
  Prints version information




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
