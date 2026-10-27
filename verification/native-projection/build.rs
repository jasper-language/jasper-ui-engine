use std::{env, fs, path::PathBuf};
fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let source = root.join("../../src/native_adapter/native_view.rs");
    println!("cargo:rerun-if-changed={}", source.display());
    let text = fs::read_to_string(source).unwrap();
    let mut output = String::new();
    for marker in ["NATIVE VIEW TYPE", "NATIVE COMPONENT VERSION KERNEL", "NATIVE HOST CONTEXT KERNEL", "NATIVE VIEW CLOSE KERNEL"] {
        let begin = format!("// BEGIN VERIFIED {marker}\n");
        let end = format!("// END VERIFIED {marker}");
        assert_eq!(text.matches(&begin).count(), 1);
        assert_eq!(text.matches(&end).count(), 1);
        output.push_str(text.split_once(&begin).unwrap().1.split_once(&end).unwrap().0);
    }
    let facade=root.join("../../src/native_adapter/facade.rs");
    println!("cargo:rerun-if-changed={}",facade.display());
    let text=fs::read_to_string(facade).unwrap();
    for marker in ["TEXT DISPLAY TYPE","TEXT DISPLAY VERSION KERNEL","TEXT DISPLAY CLOSE KERNEL"] {
        let begin=format!("// BEGIN VERIFIED {marker}\n");let end=format!("// END VERIFIED {marker}");
        assert_eq!(text.matches(&begin).count(),1);assert_eq!(text.matches(&end).count(),1);
        output.push_str(text.split_once(&begin).unwrap().1.split_once(&end).unwrap().0);
    }
    fs::write(PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("actual.rs"), output).unwrap();
    let mut scene=String::new();
    for (path,markers) in [("../../src/semantics.rs",vec!["ROLE TYPE","SCENE SEMANTICS TYPE"]),
        ("../../src/native_adapter/native_scene.rs",vec!["NATIVE SCENE TYPE","NATIVE SCENE CLOSE KERNEL","SCENE SNAPSHOT IDENTITY KERNEL"]),
        ("../../src/native_adapter/facade.rs",vec!["SCENE HOST TYPE","SCENE HOST CLOSE KERNEL","SCENE HOST IDENTITY KERNEL"])] {
        let source=root.join(path);println!("cargo:rerun-if-changed={}",source.display());let text=fs::read_to_string(source).unwrap();
        for marker in markers {let begin=format!("// BEGIN VERIFIED {marker}\n");let end=format!("// END VERIFIED {marker}");
            assert_eq!(text.matches(&begin).count(),1);assert_eq!(text.matches(&end).count(),1);
            scene.push_str(text.split_once(&begin).unwrap().1.split_once(&end).unwrap().0);}
    }
    let text=fs::read_to_string(root.join("../../src/native_adapter/native_scene.rs")).unwrap();
    assert_eq!(text.matches("    fn source_visible(").count(),1);
    let start=text.find("    fn source_visible(").unwrap();let end=start+text[start..].find("\n    }\n").unwrap()+6;
    scene.push_str(&format!("impl NativeScene {{\n{}\n}}\n",&text[start..end]));
    let begin="// BEGIN VERIFIED SOURCE HIDDEN ANCESTRY KERNEL\n";
    let end="// END VERIFIED SOURCE HIDDEN ANCESTRY KERNEL";
    assert_eq!(text.matches(begin).count(),1);assert_eq!(text.matches(end).count(),1);
    scene.push_str(text.split_once(begin).unwrap().1.split_once(end).unwrap().0);
    fs::write(PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("actual_scene.rs"),scene).unwrap();
    let path=root.join("../../src/native_adapter/capture_policy.rs");
    println!("cargo:rerun-if-changed={}",path.display());let text=fs::read_to_string(path).unwrap();let mut policy=String::new();
    for (marker,suffix) in [("CAPTURE CANCEL ROUTE","TYPE"),("CAPTURE CANCEL POLICY","KERNEL")] {
        let begin=format!("// BEGIN VERIFIED {marker} {suffix}\n");let end=format!("// END VERIFIED {marker} {suffix}");
        assert_eq!(text.matches(&begin).count(),1);assert_eq!(text.matches(&end).count(),1);
        policy.push_str(text.split_once(&begin).unwrap().1.split_once(&end).unwrap().0);
    }
    fs::write(PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("actual_capture_policy.rs"),policy).unwrap();

}
