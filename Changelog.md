# Change Log

## [0.2.43] - 2024-12-07
- `-vv` also prints invoked cargo command (#345)
  thanks @zheland
- bump deps

## [0.2.42] - 2024-11-10
- `--quiet` option that gets passed to `cargo
- Also search for context in `.set` statements - for merged functions
  this mean that when you are showing the alias with `-c 1` - the actual
  implementation will show up as well (#338)

## [0.2.41] - 2024-10-13
- make sure not to drop used labels (#318)
- add release-lto profile for slightly smaller/faster version
  thanks @zamazan4ik for the suggestion
- detect and render merged functions (#310)
- update docs (#320)
- smarter approach for detecting constants (#315)
- smarter CI (#79)
- bump deps

## [0.2.40] - 2024-10-01
- more consistend behavior when only one item is detected (#312)
  thanks @zheland
- fixed llvm output for no_mangle functions (#313)
  thanks @zheland
- bump deps

## [0.2.39] - 2024-09-19
- support --config KEY=VAL option that is passed directly to cargo
- bump deps

## [0.2.38] - 2024-07-02
- slightly smarter artifact detection, shouldn't panic with wasm crates
- bump deps

## [0.2.37] - 2024-06-27
- support combination of --everything and --rust
- bump deps

## [0.2.36] - 2024-06-02
- even better support for no_mangle names on windows
  thanks to @evmar
- bump deps

## [0.2.35] - 2024-05-06
- don't include constants by default
- include local jump labels in disassembly
- include instructions in hex in disassembly
- avoid memory addresses with displacement in disassembly
- bump bpaf

## [0.2.34] - 2024-04-25
- don't force debuginfo in llvm, fixes #269

## [0.2.33] - 2024-04-24
- Experimental support for disassembly, `cargo-show-asm` needs to be compiled with "disasm"
  feature.

  With that you can pass `--disasm` flag to disassemble binary artifacts (`.rlib` files or
  executables) created by cargo.

  To work with PGO, BOLT or other optimizations that require non standard build process you
  can pass path to binary directly with `--file`.

  For `cargo-show-asm` to detect symbols in your code you need to disable stripping by adding
  something like this to `Cargo.toml`

  ```
  [profile.release]
  strip = false
  ```

  At the moment interleaving rust source (`--rust`) is not supported
- bump deps


## [0.2.32] - 2024-04-13
- include more instructions in `--simplify`
- handle combinations like `--mca --intel` in addition to `--mca-intel` which is now deprecated
- cosmetic improvements in produced output
- a bunch of internal improvements
- drop `once_cell` dependency

## [0.2.31] - 2024-04-04
- include relevant constants in produced asm output
  this can be disabled with `--no-constants`
- bump deps

## [0.2.30] - 2024-02-11
- Add an option `-c` / `--context` to recursively include functions called from target as
  additional context

## [0.2.29] - 2024-01-23
- fix function selection by index, see https://github.com/pacak/cargo-show-asm/issues/244

## [0.2.28] - 2024-01-17
- Add a set of options to limit rust source code to workspace, regular crates or all available
  code:

        --this-workspace      Show rust sources from current workspace only
        --all-crates          Show rust sources from current workspace and from rust registry
        --all-sources         Show all the rust sources including stdlib and compiler


## [0.2.27] - 2024-01-14
- look for rustc source code in the right place, see https://github.com/pacak/cargo-show-asm/issues/238

## [0.2.26] - 2024-01-09
- avoid using hard to see colors
thanks to @epontan
- bump deps

## [0.2.25] - 2023-12-31
- Improve accuracy of llvm lines, see https://github.com/pacak/cargo-show-asm/pull/229
thanks to @osiewicz
- fix CI

## [0.2.24] - 2023-12-28
- add an option to keep mangled name, thanks to @osiewicz
- add syntax highlight for mangled names
- bump dependencies

## [0.2.23] - 2023-11-26
- Add an option to strip blank lines and make it default, original behavior is accessible
  with `-B` option

## [0.2.22] - 2023-10-10
- better support for no_mangle in macOS
- ignore empty source files - seen them on Windows
- bump a bunch of deps
- add license files

## [0.2.21] - 2023-08-12
- support wonky non-utf8 files produced by rustc/llvm o_O

## [0.2.20] - 2023-06-17
- workaround for fancier debuginfo not supported by cargo-metadata
- usage in README is now generated in markdown

## [0.2.19] - 2023-06-05
- bump bpaf to 0.9.1, usage in README is now generated
- bump deps

## [0.2.18] - 2023-05-11
- you can also specify default profile using `CARGO_SHOW_ASM_PROFILE` env variable
- bump bpaf to 0.8.0, add dull colors by default

## [0.2.17] - 2023-04-11
- look harder for source code, don't panic if it can't be found
- bump deps

## [0.2.16] - 2023-04-04
- drop some dependencies
- support for strange looking file names in dwarf info

## [0.2.15] - 2023-03-09
- Override lto configuration to lto=no, #146

## [0.2.14] - 2023-02-22
- Allow to pass -C flags directly to rustc
- --llvm-input to show llvm-ir before any LLVM passes
- Only generate debug info for LLVM resulting in cleaner
Thanks to @jonasmalacofilho

## [0.2.13] - 2023-02-03
- support cdylib crates
- bump deps

## [0.2.12] - 2023-01-13
- allow to pass -Z flags directly to cargo
- support for llvm-mca

## [0.2.11] - 2023-01-11
- fix filtering by index and name at the same time
- --test, --bench, etc. can be used without argument to list available items
thanks to @danielparks
- bump deps

## [0.2.10] - 2023-01-09
- support for nightly -Z asm-comments

## [0.2.9] - 2023-01-07
- improve error messages
- properly handle exception handling code on Windows
thanks to @al13n321
- support --rust on Windows
thanks to @al13n321

## [0.2.8] - 2023-01-02
- bump dependencies

## [0.2.7] - 2022-11-26
- support mangled names
- fix select-by-index

## [0.2.6] - 2022-11-23
- use color for cargo diagnostics
Thanks to @coolreader18
- support for WASM target
Thanks to @coolreader18

## [0.2.5] - 2022-11-21
- include README.md into docs.rs docs
- dump function by index as well as by name
- improve label colorization and stripping
Thanks to @RustyYato
- bump dependencies

## [0.2.4] - 2022-11-12
- `--simplify` option - to strip some of the things that are not cpu instructions
   from the asm output

## [0.2.3] - 2022-11-05
- support rlib projects + tests

## [0.2.2] - 2022-11-01
- fix `--color` and `--no-color`, regression since 0.2.0

## [0.2.1] - 2022-10-29
- number of macOS specific bugfixes
- update deps
- more detailed output with verbosity flags

## [0.2.0] - 2022-10-22
- replaced libcargo with invoking cargo
Thanks to @oxalica
- renamed `--feature` -> `--features`
- dropped backward compatibility `-no-defaut-features`
- implemented `--everything` to dump the whole file demangled

## [0.1.24] - 2022-10-15
- support custom profiles
- support reading rust sources from registries

## [0.1.23] - 2022-10-11
- update dependenies + bpaf
- optional colorful command line parser output

## [0.1.22] - 2022-10-03
- strip redundant labels by default
- cleaning up
- removing glob in favor of std::fs - windows CI now works
- document completions

## [0.1.21] - 2022-09-29
- options for vendored libgit2 and openssl
- documentation improvements
Thanks to @dtolnay, @matthiasbeyer and @saethlin
- support --native and --target-cpu
- when dumping a function - dump full section

## [0.1.20] - 2022-09-24
- Update cargo version to 0.65
- Bump bpaf
Thanks to @elichai

## [0.1.19] - 2022-09-23
- detect missing sources and suggest to install them

## [0.1.18] - 2022-09-15
- bugfix to package selection in non-virtual workspaces

## [0.1.17] - 2022-09-12
- fix typo in default features
Thanks to @mooli
- fix more cross-compilation issues
Thanks to @WIgor

## [0.1.16] - 2022-09-03
- Fix parsing of file directive on nightly
- Bump bpaf
Thanks to @yotamofek

## [0.1.15] - 2022-08-23
- Update bpaf to 0.5.2, should start give more user friendly suggestions

## [0.1.14] - 2022-08-20
- Also accept target dir from `env:CARGO_TARGET_DIR`

## [0.1.13] - 2022-08-16
- Upgrade cargo dependency

## [0.1.12] - 2022-08-01
- Dump single match as is

## [0.1.11] - 2022-07-23
- Improved cross-compilation support

## [0.1.10] - 2022-07-05
- Upgrade cargo dependency

## [0.1.9] - 2022-07-01
- Upgrade cargo dependency

## [0.1.8] - 2022-06-24
- arm asm bugfixes
- Bump the dependencies, mostly cargo to 0.62

## [0.1.7] - 2022-05-25
- arm asm bugfixes
thanks to @RustyYato

## [0.1.6] - 2022-05-22
- Limited support for MIR

## [0.1.5] - 2022-05-16
- bump dependencies

## [0.1.4] - 2022-04-20
- Limited support for LLVM-IR

## [0.1.3] - 2022-04-20
- Limited support for Windows:
Works a bit better thanks to @nico-abram

## [0.1.2] - 2022-04-15
- Limited support for Windows:
showing asm code for function mostly works, adding rust code annotation doesn't.

## [0.1.1] - 2022-04-14
- First public release
