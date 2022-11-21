# Change Log

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
- number of MacOS specific bugfixes
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
- fix more crosscompilation issues
Thanks to @WIgor

## [0.1.16] - 2022-09-03
- Fix parsing of file directive on nightly
- Bump bpaf
Thanks to @yotamofek

## [0.1.15] - 2022-08-23
- Update bpaf to 0.5.2, should start give more user fiendly suggestions

## [0.1.14] - 2022-08-20
- Also accept target dir from `env:CARGO_TARGET_DIR`

## [0.1.13] - 2022-08-16
- Upgrade cargo dependency

## [0.1.12] - 2022-08-01
- Dump single match as is

## [0.1.11] - 2022-07-23
- Improved crosscompilation support

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
