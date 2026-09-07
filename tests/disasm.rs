#![cfg(feature = "disasm")]

use std::path::{Path, PathBuf};
use std::process::Command;

fn run_disasm(crate_path: &Path, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_cargo-asm"))
        .arg("--manifest-path")
        .arg(crate_path)
        .arg("--disasm")
        .args(args)
        .output()
        .expect("failed to run cargo-asm");
    assert!(
        out.status.success(),
        "cargo-asm failed {}\n{}\n{}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn disasm_poke() {
    let test_crate = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("tests/disasm-test-crate/Cargo.toml");
    let out = run_disasm(&test_crate, &["--rust", "--lib", "disasm_test_function"]);

    #[cfg(windows)]
    let out = out.replace('\\', '/');

    assert!(out.contains("// tests/disasm-test-crate/src/lib.rs:3"));
    assert!(out.contains("pub fn disasm_test_function"));
    assert!(out.contains("eprintln!(\"hello\");"));
    assert!(!out.contains(".cfi_offset"));
    assert!(!out.contains(".p2align"));
}
