//! Compiles the Apple Foundation Models helper (`native/apple-fm/main.swift`) with `swiftc` into
//! `OUT_DIR/apple-fm`, which the crate embeds. When `swiftc` or the SDK is missing, or the target
//! is not macOS, the build still succeeds with an empty file and the Apple profile reports
//! `unavailable (helper_missing)`. No Cargo dependency is involved.
use std::{path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=native/apple-fm/main.swift");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=DIARY_SKIP_SWIFT");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let helper = out.join("apple-fm");
    let _ = std::fs::remove_file(&helper);
    let built = try_build(&out, &helper);
    if !built {
        std::fs::write(&helper, []).expect("write empty helper");
    }
}

fn try_build(out: &std::path::Path, helper: &std::path::Path) -> bool {
    if std::env::var_os("DIARY_SKIP_SWIFT").is_some()
        || std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos")
    {
        return false;
    }
    let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        Ok("x86_64") => "x86_64",
        _ => return false,
    };
    let status = Command::new("swiftc")
        .args(["-O", "-target"])
        .arg(format!("{arch}-apple-macos26.0"))
        .arg("-module-cache-path")
        .arg(out.join("swift-module-cache"))
        .arg("native/apple-fm/main.swift")
        .arg("-o")
        .arg(helper)
        .status();
    match status {
        Ok(s) if s.success() => true,
        _ => {
            println!(
                "cargo:warning=apple-fm helper not built (swiftc or macOS 26 SDK unavailable); \
                 the Apple profile will report helper_missing"
            );
            false
        }
    }
}
