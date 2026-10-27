use std::{env,fs,path::PathBuf};
fn main() {
    let root=PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let source=root.join("../../src/native_adapter/native_semantics.rs");
    println!("cargo:rerun-if-changed={}",source.display());
    let text=fs::read_to_string(source).unwrap();
    let begin="// BEGIN VERIFIED NATIVE ACTIVATION ADMISSION KERNEL\n";
    let end="// END VERIFIED NATIVE ACTIVATION ADMISSION KERNEL";
    assert_eq!(text.matches(begin).count(),1);assert_eq!(text.matches(end).count(),1);
    let body=text.split_once(begin).unwrap().1.split_once(end).unwrap().0;
    fs::write(PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("actual.rs"),body).unwrap();
    let source=root.join("../../src/native_adapter/native_scene.rs");
    println!("cargo:rerun-if-changed={}",source.display());
    let text=fs::read_to_string(source).unwrap();
    let begin="// BEGIN VERIFIED SCENE WORK CAPACITY KERNEL\n";
    let end="// END VERIFIED SCENE WORK CAPACITY KERNEL";
    assert_eq!(text.matches(begin).count(),1);assert_eq!(text.matches(end).count(),1);
    let body=text.split_once(begin).unwrap().1.split_once(end).unwrap().0;
    fs::write(PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("capacity.rs"),body).unwrap();
    let mut gate=String::new();
    for (path,marker) in [("../../src/input_identity.rs","UI UPDATE NEXT"),
        ("../../src/native_adapter/native_scene.rs","SCENE WORK CAPACITY"),
        ("../../src/native_adapter/native_scene.rs","SCENE UPDATE ADMISSION")] {
        let source=root.join(path);println!("cargo:rerun-if-changed={}",source.display());
        let text=fs::read_to_string(source).unwrap();
        let begin=format!("// BEGIN VERIFIED {marker} KERNEL\n");
        let end=format!("// END VERIFIED {marker} KERNEL");
        assert_eq!(text.matches(&begin).count(),1);assert_eq!(text.matches(&end).count(),1);
        gate.push_str(text.split_once(&begin).unwrap().1.split_once(&end).unwrap().0);
    }
    fs::write(PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("scene_gate.rs"),gate).unwrap();
    let source=root.join("../../src/native_adapter/native_scene.rs");
    let text=fs::read_to_string(source).unwrap();
    let begin="// BEGIN VERIFIED SOURCE INSERT INDEX KERNEL\n";
    let end="// END VERIFIED SOURCE INSERT INDEX KERNEL";
    assert_eq!(text.matches(begin).count(),1);assert_eq!(text.matches(end).count(),1);
    fs::write(PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("insert_index.rs"),
        text.split_once(begin).unwrap().1.split_once(end).unwrap().0).unwrap();
    let ids=fs::read_to_string(root.join("../../src/scene.rs")).unwrap();
    let ids=ids.split_once("// BEGIN VERIFIED NODE ID TYPE\n").unwrap().1.split_once("// END VERIFIED NODE ID TYPE").unwrap().0;
    let source=fs::read_to_string(root.join("../../src/native_adapter/native_scene.rs")).unwrap();
    let start=source.find("fn source_subtree_contains(").unwrap();
    let end=start+source[start..].find("\n}\n").unwrap()+2;
    fs::write(PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("ancestry.rs"),format!("{ids}\n{}",&source[start..end])).unwrap();
    let roles=fs::read_to_string(root.join("../../src/semantics.rs")).unwrap();
    let begin="// BEGIN VERIFIED ROLE TYPE\n";let end="// END VERIFIED ROLE TYPE";
    assert_eq!(roles.matches(begin).count(),1);assert_eq!(roles.matches(end).count(),1);
    let role=roles.split_once(begin).unwrap().1.split_once(end).unwrap().0;
    let helper=source.split_once("// BEGIN VERIFIED NATIVE BUTTON ROLE KERNEL\n").unwrap().1.split_once("// END VERIFIED NATIVE BUTTON ROLE KERNEL").unwrap().0;
    let begin="// BEGIN VERIFIED SEMANTIC ROLE TAG KERNEL\n";let end="// END VERIFIED SEMANTIC ROLE TAG KERNEL";
    assert_eq!(source.matches(begin).count(),1);assert_eq!(source.matches(end).count(),1);
    fs::write(PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("semantic_tag.rs"),
        format!("{role}\n{helper}\n{}",source.split_once(begin).unwrap().1.split_once(end).unwrap().0)).unwrap();
    println!("cargo:rerun-if-changed={}",root.join("../../src/semantics.rs").display());

}
