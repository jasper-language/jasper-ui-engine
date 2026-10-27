// DEFERRED: extracts archived Skia candidate plus historical surface helpers.
// Not evidence for the current production renderer; do not resume this suite.
use std::{env,fs,path::PathBuf};
fn main(){let root=PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());let mut code=String::new();
for (file,marker) in [("surface.rs","SURFACE ROW"),("surface.rs","SURFACE SPAN"),("../experiments/deferred-skia-integration/text_skia.rs","LOCAL TEXT STORAGE"),("../experiments/deferred-skia-integration/text_skia.rs","TEXT HIT OFFSET")]{
let p=root.join("../../src").join(file);println!("cargo:rerun-if-changed={}",p.display());let s=fs::read_to_string(p).unwrap();let begin=format!("// BEGIN VERIFIED {marker} KERNEL");let end=format!("// END VERIFIED {marker} KERNEL");assert_eq!(s.matches(&begin).count(),1);assert_eq!(s.matches(&end).count(),1);code.push_str(s.split_once(&begin).unwrap().1.split_once(&end).unwrap().0);}
fs::write(PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("actual.rs"),code).unwrap();}
