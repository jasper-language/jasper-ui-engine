use std::{env, fs, path::PathBuf};

fn copy(root: &std::path::Path, source: &str, output: &str) {
    let source = root.join(source);
    println!("cargo:rerun-if-changed={}", source.display());
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join(output);
    let extracted = fs::read_to_string(source)
        .unwrap()
        .lines()
        .map(|line| line.strip_prefix("//!").map_or_else(|| line.to_owned(), |text| format!("// {text}")))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(output, extracted).unwrap();
}

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    copy(&root, "../../src/surface_lifecycle.rs", "actual.rs");
    copy(&root, "../../src/host_capabilities.rs", "capabilities.rs");
}
