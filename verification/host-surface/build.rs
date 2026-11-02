use std::{env, fs, path::PathBuf};

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let source = root.join("../../src/surface_lifecycle.rs");
    println!("cargo:rerun-if-changed={}", source.display());
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("actual.rs");
    let extracted = fs::read_to_string(source)
        .unwrap()
        .lines()
        .map(|line| line.strip_prefix("//!").map_or_else(|| line.to_owned(), |text| format!("// {text}")))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(output, extracted).unwrap();
}
