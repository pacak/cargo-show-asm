# cargo-show-asm

> A [`cargo`] subcommand that displays the assembly generated for Rust source code.

# Install

```
cargo install cargo-show-asm
```

#  Features

* Platform support:

  * OS: Linux, and MacOSX.
  * Rust: nightly and stable.
  * Architectures: x86, x86_64, arm, aarch64, powerpc, mips, sparc.

* Displaying:

  * Assembly in Intel or AT&T syntax.
  * Corresponding Rust source code alongside assembly.

* Querying:

  * functions, for example: `foo`:

  ```
  cargo asm crate::path::to::foo
  ```

  * inherent method, for example: `foo` of a type `Foo` (that is, `Foo::foo`):

  ```
  cargo asm crate::path::to::Foo::foo
  ```

  * trait method implementations, for example: `bar` of the trait `Bar` for the type `Foo`:

  ```
  cargo asm "<crate::path::to::Foo as crate::path::to::Bar>::bar"
  ```

  * generic functions, methods, ...

To search for a function named `foo` in some path, one can just type `cargo asm
foo`. The command will return a list of all similarly named functions
independently of the path.

# License
This project is licensed under either of

* Apache License, Version 2.0, (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)

at your option.

# Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
