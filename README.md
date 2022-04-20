# cargo-show-asm

A cargo subcommand that displays the assembly generated for Rust source code.

# Install

```
cargo install cargo-show-asm
```

# Features

- Platform support:

  - OS: Linux and MacOSX. Limited support for Windows
  - Rust: nightly and stable.
  - Architectures: x86, x86_64, aarch64, probably more but untested.
  - Crosscompilation support.

Missing operating systems and architenctures might be supported by accident, please make a
ticket if something not working for your favorite platform

- Displaying:

  - Assembly in Intel or AT&T syntax.
  - Corresponding Rust source code alongside assembly.
  - llvm-ir.

# Usage:

You can start by running `cargo asm` with no parameters - it will suggests how to narrow the
search scope - for workspace crates you need to specify a crate to work with, for crates
defining several targets (lib, binaries, examples) you need to specify exactly which target to
use. In a workspace `cargo asm` lists only workspace members as suggestions but any crate from
workspace tree is available.

Once `cargo asm` focues on a single target it will run rustc producing assembly file and will
try to list of available public functions:

```ignore
% cargo asm --lib
Try one of those
"<&T as core::fmt::Display>::fmt" [17, 12, 12, 12, 12, 19, 19, 12]
"<&mut W as core::fmt::Write>::write_char" [20]
"<&mut W as core::fmt::Write>::write_fmt" [38]
"<&mut W as core::fmt::Write>::write_str" [90]
"<F as nom::internal::Parser<I,O,E>>::parse" [263]
...
```

Name in quotes is demangled rust name, numbers in square brackets represent number of lines
in asm file. Function with the same name can be present in several instances.

Specifying exact function name will print its assembly code

```ignore
% cargo asm --lib "cargo_show_asm::opts::focus::{{closure}}"
```
To pick between different alternatives you can either specify the index

```ignore
% cargo asm --lib "cargo_show_asm::opts::focus::{{closure}}" 2
```
Or start using full names with hex included:

```ignore
% cargo asm --lib --full-name
....
% cargo asm --lib "once_cell::imp::OnceCell<T>::initialize::h9c5c7d5bd745000b"
```

`cargo-show-asm` comes with a built in search function. Just pass partial name
instead of a full one and only matching functions will be listed

```
% cargo asm --lib Debug
```

# License
This project is licensed under either of

* Apache License, Version 2.0, (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)

at your option.

# Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
