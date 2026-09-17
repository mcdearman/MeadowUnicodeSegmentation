//! Finds the `unicode-segmentation` source that Cargo fetched and makes its
//! private tables, and its copy of the official Unicode test data, reachable
//! from `main.rs`.
//!
//! The crate's `src/tables.rs` is a private module. It is copied into
//! `OUT_DIR` with its top-level items made `pub`, so that `main.rs` reads the
//! crate's own tables rather than a transcription of them. Inner attributes
//! are dropped, since `include!` cannot take them.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let crate_dir = upstream_dir();
    let tables = crate_dir.join("src/tables.rs");
    let text = std::fs::read_to_string(&tables)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", tables.display()));

    std::fs::write(out.join("upstream.rs"), publicise(&text)).unwrap();

    let testdata = crate_dir.join("tests/testdata/mod.rs");
    let data = std::fs::read_to_string(&testdata)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", testdata.display()));
    std::fs::write(out.join("testdata.rs"), publicise(&data)).unwrap();

    // The rules, which `src/*.mw` port by hand, for `main.rs` to fingerprint.
    let mut rules = String::new();
    for name in ["grapheme.rs", "word.rs", "sentence.rs"] {
        let path = crate_dir.join("src").join(name);
        rules.push_str(&std::fs::read_to_string(&path).unwrap());
        println!("cargo:rerun-if-changed={}", path.display());
    }
    std::fs::write(out.join("rules.rs.txt"), rules).unwrap();

    println!("cargo:rustc-env=UPSTREAM_DIR={}", crate_dir.display());
    println!("cargo:rerun-if-changed={}", tables.display());
    println!("cargo:rerun-if-changed={}", testdata.display());
    println!("cargo:rerun-if-changed=Cargo.toml");
}

/// Where Cargo put the `unicode-segmentation` this build depends on.
fn upstream_dir() -> PathBuf {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let manifest = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("Cargo.toml");
    let out = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--manifest-path"])
        .arg(&manifest)
        .output()
        .expect("could not run `cargo metadata`");
    assert!(out.status.success(), "`cargo metadata` failed");
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let pkg = meta["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "unicode-segmentation")
        .expect("unicode-segmentation is not among the dependencies");
    Path::new(pkg["manifest_path"].as_str().unwrap())
        .parent()
        .unwrap()
        .to_path_buf()
}

/// `text` with its module-level items made `pub`, so that they can be named
/// from outside the module it is included into.
fn publicise(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 1024);
    for line in text.lines() {
        let rewritten = if line.starts_with("#![") {
            continue;
        } else if line.starts_with("fn ")
            || line.starts_with("static ")
            || line.starts_with("const ")
        {
            format!("pub {line}")
        } else {
            line.to_string()
        };
        out.push_str(&rewritten);
        out.push('\n');
    }
    out
}
