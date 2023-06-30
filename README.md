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

# cargo asm<br>
<p>Show the code rustc generates for any function</p><p><b>Usage</b>: <tt><b>cargo asm</b></tt> [<tt><b>-p</b></tt>=<tt><i>SPEC</i></tt>] [<tt><i>ARTIFACT</i></tt>] [<tt><b>-M</b></tt>=<tt><i>ARG</i></tt>]... [<tt><i>TARGET-CPU</i></tt>] [<tt><b>--rust</b></tt>] [<tt><b>--simplify</b></tt>] [<tt><i>OUTPUT-FORMAT</i></tt>] (<tt><b>--everything</b></tt> | <tt><i>FUNCTION</i></tt> [<tt><i>INDEX</i></tt>])</p><p>Usage:<br>
 1. Focus on a single assembly producing target:<br>
    % cargo asm -p isin --lib   # here we are targeting lib in isin crate<br>
 2. Narrow down a function:<br>
    % cargo asm -p isin --lib from_ # here "from_" is part of the function you are interested intel<br>
 3. Get the full results:<br>
    % cargo asm -p isin --lib isin::base36::from_alphanum</p><p><div>
<b>Pick artifact for analysis:</b></div><dl><dt><tt><b>    --lib</b></tt></dt>
<dd>Show results from library code</dd>
<dt><tt><b>    --test</b></tt>=<tt><i>TEST</i></tt></dt>
<dd>Show results from an integration test</dd>
<dt><tt><b>    --bench</b></tt>=<tt><i>BENCH</i></tt></dt>
<dd>Show results from a benchmark</dd>
<dt><tt><b>    --example</b></tt>=<tt><i>EXAMPLE</i></tt></dt>
<dd>Show results from an example</dd>
<dt><tt><b>    --bin</b></tt>=<tt><i>BIN</i></tt></dt>
<dd>Show results from a binary</dd>
</dl>
</p><p><div>
<b>Cargo options</b></div><dl><dt><tt><b>    --manifest-path</b></tt>=<tt><i>PATH</i></tt></dt>
<dd>Path to Cargo.toml, defaults to one in current folder</dd>
<dt><tt><b>    --target-dir</b></tt>=<tt><i>DIR</i></tt></dt>
<dd>Use custom target directory for generated artifacts, create if missing</dd>
<dt></dt>
<dd>Uses environment variable <tt><b>CARGO_TARGET_DIR</b></tt></dd>
<dt><tt><b>    --dry</b></tt></dt>
<dd>Produce a build plan instead of actually building</dd>
<dt><tt><b>    --frozen</b></tt></dt>
<dd>Requires Cargo.lock and cache are up to date</dd>
<dt><tt><b>    --locked</b></tt></dt>
<dd>Requires Cargo.lock is up to date</dd>
<dt><tt><b>    --offline</b></tt></dt>
<dd>Run without accessing the network</dd>
<dt><tt><b>    --no-default-features</b></tt></dt>
<dd>Do not activate `default` feature</dd>
<dt><tt><b>    --all-features</b></tt></dt>
<dd>Activate all available features</dd>
<dt><tt><b>    --features</b></tt>=<tt><i>FEATURE</i></tt></dt>
<dd>A feature to activate, can be used multiple times</dd>
<dt><tt><b>    --release</b></tt></dt>
<dd>Compile in release mode (default)</dd>
<dt><tt><b>    --dev</b></tt></dt>
<dd>Compile in dev mode</dd>
<dt><tt><b>    --profile</b></tt>=<tt><i>PROFILE</i></tt></dt>
<dd>Build for this specific profile, you can also use `dev` and `release` here</dd>
<dt></dt>
<dd>Uses environment variable <tt><b>CARGO_SHOW_ASM_PROFILE</b></tt></dd>
<dt><tt><b>    --target</b></tt>=<tt><i>TRIPLE</i></tt></dt>
<dd>Build for the target triple</dd>
<dt><tt><b>-C</b></tt>=<tt><i>FLAG</i></tt></dt>
<dd>Codegen flags to rustc, see 'rustc -C help' for details</dd>
<dt><tt><b>-Z</b></tt>=<tt><i>FLAG</i></tt></dt>
<dd>Unstable (nightly-only) flags to Cargo, see 'cargo -Z help' for details</dd>
</dl>
</p><p><div>
<b>Postprocessing options:</b></div><dl><dt><tt><b>    --rust</b></tt></dt>
<dd>Print interleaved Rust code</dd>
<dt><tt><b>    --color</b></tt></dt>
<dd>Enable color highlighting</dd>
<dt><tt><b>    --no-color</b></tt></dt>
<dd>Disable color highlighting</dd>
<dt><tt><b>    --full-name</b></tt></dt>
<dd>Include full demangled name instead of just prefix</dd>
<dt><tt><b>    --keep-labels</b></tt></dt>
<dd>Keep all the original labels</dd>
<dt><tt><b>-v</b></tt>, <tt><b>--verbose</b></tt></dt>
<dd>more verbose output, can be specified multiple times</dd>
<dt><tt><b>    --simplify</b></tt></dt>
<dd>Try to strip some of the non-assembly instruction information</dd>
</dl>
</p><p><div>
<b>Pick output format:</b></div><dl><dt><tt><b>    --intel</b></tt></dt>
<dd>Show assembly using Intel style</dd>
<dt><tt><b>    --att</b></tt></dt>
<dd>Show assembly using AT&T style</dd>
<dt><tt><b>    --llvm</b></tt></dt>
<dd>Show llvm-ir</dd>
<dt><tt><b>    --llvm-input</b></tt></dt>
<dd>Show llvm-ir before any LLVM passes</dd>
<dt><tt><b>    --mir</b></tt></dt>
<dd>Show MIR</dd>
<dt><tt><b>    --wasm</b></tt></dt>
<dd>Show WASM, needs wasm32-unknown-unknown target installed</dd>
<dt><tt><b>    --mca-intel</b></tt></dt>
<dd>Show llvm-mca analysis, Intel style asm</dd>
<dt><tt><b>    --mca-att</b></tt></dt>
<dd>Show llvm-mca analysis, AT&T style asm</dd>
</dl>
</p><p><div>
<b>Pick item to display from the artifact</b></div><dl><dt><tt><b>    --everything</b></tt></dt>
<dd>Dump the whole file</dd>
<dt><tt><i>FUNCTION</i></tt></dt>
<dd>Dump a function with a given name, filter functions by name</dd>
<dt><tt><i>INDEX</i></tt></dt>
<dd>Select specific function when there's several with the same name</dd>
</dl>
</p><p><div>
<b>Available options:</b></div><dl><dt><tt><b>-p</b></tt>, <tt><b>--package</b></tt>=<tt><i>SPEC</i></tt></dt>
<dd>Package to use, defaults to a current one,<br>
required for workspace projects, can also point to a dependency</dd>
<dt><tt><b>-M</b></tt>, <tt><b>--mca-arg</b></tt>=<tt><i>ARG</i></tt></dt>
<dd>Pass parameter to llvm-mca for mca targets</dd>
<dt><tt><b>    --native</b></tt></dt>
<dd>Optimize for the CPU running the compiler</dd>
<dt><tt><b>    --target-cpu</b></tt>=<tt><i>CPU</i></tt></dt>
<dd>Optimize code for a specific CPU, see 'rustc --print target-cpus'</dd>
<dt><tt><b>-h</b></tt>, <tt><b>--help</b></tt></dt>
<dd>Prints help information</dd>
<dt><tt><b>-V</b></tt>, <tt><b>--version</b></tt></dt>
<dd>Prints version information</dd>
</dl>
</p>
<style>
div.bpaf-doc {
    padding: 14px;
    background-color:var(--code-block-background-color);
    font-family: "Source Code Pro", monospace;
    margin-bottom: 0.75em;
}
div.bpaf-doc dt { margin-left: 1em; }
div.bpaf-doc dd { margin-left: 3em; }
div.bpaf-doc dl { margin-top: 0; padding-left: 1em; }
div.bpaf-doc  { padding-left: 1em; }
</style>

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
