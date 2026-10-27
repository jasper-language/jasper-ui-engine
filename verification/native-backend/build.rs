use std::{env,fs,path::PathBuf};
fn main() {
 let directory=PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
 let root=directory.join("../..");let mut actual=String::new();
 for (source,marker,suffix) in [("src/geometry.rs","GEOMETRY POINT","TYPE"),("src/geometry.rs","GEOMETRY SIZE","TYPE"),("src/geometry.rs","GEOMETRY RECT","TYPE"),("src/raster.rs","CANVAS STORAGE","TYPE"),("src/surface.rs","SURFACE ROW","KERNEL"),("src/surface.rs","SURFACE SPAN","KERNEL"),("src/surface.rs","SURFACE BORROW","KERNEL"),("src/surface.rs","SURFACE WRITE","KERNEL"),("src/raster.rs","CANVAS IMPORT","KERNEL"),("src/backend.rs","NATIVE BACKEND TRAIT CONTRACT",""),("src/backend.rs","NATIVE BACKEND ERROR","TYPE"),("src/backend.rs","NATIVE BACKEND SUBMISSION","KERNEL"),("src/backend.rs","NATIVE HEADLESS BACKEND","TYPE"),("src/backend.rs","NATIVE BACKEND FRAME IMPORT","KERNEL"),("src/backend.rs","NATIVE HEADLESS BACKEND FORWARDING","KERNEL"),("src/native_adapter/native_scene.rs","NATIVE DPI SCALE ADMISSION","KERNEL"),("src/native_adapter/native_scene.rs","NATIVE FRAME ORCHESTRATION","KERNEL"),("src/native_adapter/native_scene.rs","NATIVE HOST SURFACE ORCHESTRATION","KERNEL")] {
  let path=root.join(source);println!("cargo:rerun-if-changed={}",path.display());
  let text=fs::read_to_string(path).unwrap();
  let label=if suffix.is_empty(){marker.to_owned()}else{format!("{marker} {suffix}")};
  let begin=format!("// BEGIN VERIFIED {label}\n");let end=format!("// END VERIFIED {label}");
  assert_eq!(text.matches(&begin).count(),1);assert_eq!(text.matches(&end).count(),1);
  actual.push_str(text.split_once(&begin).unwrap().1.split_once(&end).unwrap().0);actual.push('\n');
 }
 let cpu_path=root.join("src/native_adapter/cpu_budget.rs");println!("cargo:rerun-if-changed={}",cpu_path.display());
 let cpu_text=fs::read_to_string(cpu_path).unwrap();
 let cpu_begin="// BEGIN VERIFIED CPU FRAME BYTES KERNEL\n";let cpu_end="// END VERIFIED CPU FRAME BYTES KERNEL";
 assert_eq!(cpu_text.matches(cpu_begin).count(),1);assert_eq!(cpu_text.matches(cpu_end).count(),1);
 let cpu=cpu_text.split_once(cpu_begin).unwrap().1.split_once(cpu_end).unwrap().0;
 let out=PathBuf::from(env::var_os("OUT_DIR").unwrap());
 fs::write(out.join("actual.rs"),actual).unwrap();fs::write(out.join("cpu_budget.rs"),cpu).unwrap();
}
