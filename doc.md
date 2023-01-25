## Finding code to analyze

Code in a typical cargo project can be located in a package itself or it can belong to any external or workspace dependency package. Further code can belong to a library, integration test, or any binary package might contain. To access code located in unit test (code you usually run with <tt>cargo test</tt>) you should pick a library and compile it in the test profile: <tt><b>\-\-profile test</b></tt>

## Available options

<dl>
<dt><tt><b>-p</b></tt>, <tt><b>--package</b></tt><tt>=</tt><tt><i>SPEC</i></tt></dt>
<dd>Package to use, defaults to a current one, required for workspace projects, can also point
to a dependency</dd>
<dt><tt><b>--lib</b></tt></dt>
<dd>Show results from library code</dd>
<dt><tt><b>--test</b></tt><tt>=</tt><tt><i>TEST</i></tt></dt>
<dd>Show results from an integration test</dd>
<dt><tt><b>--bench</b></tt><tt>=</tt><tt><i>BENCH</i></tt></dt>
<dd>Show results from a benchmark</dd>
<dt><tt><b>--example</b></tt><tt>=</tt><tt><i>EXAMPLE</i></tt></dt>
<dd>Show results from an example</dd>
<dt><tt><b>--bin</b></tt><tt>=</tt><tt><i>BIN</i></tt></dt>
<dd>Show results from a binary</dd></dl>

## Compiling code with cargo

<tt>cargo-show-asm</tt> lets <tt>cargo</tt> to handle the compilation and allows you to pass parameters directly to <tt>cargo</tt>.

## Available options

<dl>
<dt><tt><b>--manifest-path</b></tt><tt>=</tt><tt><i>PATH</i></tt></dt>
<dd>Path to Cargo.toml, defaults to one in current folder</dd>
<dt><tt><b>--target-dir</b></tt><tt>=</tt><tt><i>DIR</i></tt></dt>
<dd>Use custom target directory for generated artifacts, create if missing</dd>
<dt><tt><b>--dry</b></tt></dt>
<dd>Produce a build plan instead of actually building</dd>
<dt><tt><b>--frozen</b></tt></dt>
<dd>Requires Cargo.lock and cache are up to date</dd>
<dt><tt><b>--locked</b></tt></dt>
<dd>Requires Cargo.lock is up to date</dd>
<dt><tt><b>--offline</b></tt></dt>
<dd>Run without accessing the network</dd>
<dt><tt><b>--no-default-features</b></tt></dt>
<dd>Do not activate `default` feature</dd>
<dt><tt><b>--all-features</b></tt></dt>
<dd>Activate all available features</dd>
<dt><tt><b>--features</b></tt><tt>=</tt><tt><i>FEATURE</i></tt></dt>
<dd>A feature to activate, can be used multiple times</dd>
<dt><tt><b>--release</b></tt></dt>
<dd>Compile in release mode (default)</dd>
<dt><tt><b>--dev</b></tt></dt>
<dd>Compile in dev mode</dd>
<dt><tt><b>--profile</b></tt><tt>=</tt><tt><i>PROFILE</i></tt></dt>
<dd>Build for this specific profile</dd>
<dt><tt><b>--target</b></tt><tt>=</tt><tt><i>TRIPLE</i></tt></dt>
<dd>Build for the target triple</dd>
<dt><tt><b>-Z</b></tt><tt>=</tt><tt><i>FLAG</i></tt></dt>
<dd>Unstable (nightly-only) flags to Cargo, see 'cargo -Z help' for details</dd></dl>

## Picking the output format

<tt>cargo-show-asm</tt> can generate output in many different formats:

## Available options

<dl>
<dt><tt><b>--intel</b></tt></dt>
<dd>Show assembly using Intel style</dd>
<dt><tt><b>--att</b></tt></dt>
<dd>Show assembly using AT&T style</dd>
<dt><tt><b>--llvm</b></tt></dt>
<dd>Show llvm-ir</dd>
<dt><tt><b>--mir</b></tt></dt>
<dd>Show MIR</dd>
<dt><tt><b>--wasm</b></tt></dt>
<dd>Show WASM, needs wasm32-unknown-unknown target installed</dd>
<dt><tt><b>--mca-intel</b></tt></dt>
<dd>Show llvm-mca analysis, Intel style asm</dd>
<dt><tt><b>--mca-att</b></tt></dt>
<dd>Show llvm-mca analysis, AT&T style asm</dd></dl>