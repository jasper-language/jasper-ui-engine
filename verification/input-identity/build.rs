use std::{env,fs,path::PathBuf};
fn main() {
    let root=PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let mut output=String::new();
    for (file,marker) in [("resource.rs","RESOURCE GATE KERNEL"),("scene.rs","NODE ID TYPE"),("input_identity.rs","UI INPUT ADMISSION KERNEL"),("input_identity.rs","UI VIEW OWNER KERNEL"),("input_identity.rs","UI SNAPSHOT ADMISSION KERNEL"),("input_identity.rs","UI UPDATE NEXT KERNEL"),("input_identity.rs","UI INPUT BINDING KERNEL"),("input_identity.rs","UI INPUT PAYLOAD KERNEL"),("input_identity.rs","UI INPUT REPLACE KERNEL"),("composition_identity.rs","UI COMPOSITION IDENTITY KERNEL"),("input.rs","COMPOSITION EVENT TYPE"),("input.rs","PREEDIT EVENT TYPE"),("composition_identity.rs","UI PREEDIT ADMISSION KERNEL"),("composition_identity.rs","NORMALIZED PREEDIT ADMISSION KERNEL"),("composition_identity.rs","UI PREEDIT VIEW KERNEL"),("composition_identity.rs","UI PREEDIT CANCEL KERNEL"),("composition_identity.rs","COMPOSITION CURRENT TOKEN KERNEL"),("composition_identity.rs","UI COMPOSITION RANGE KERNEL"),("semantics.rs","FOCUS STORAGE TYPE"),("semantics.rs","FOCUS CLEANUP KERNEL"),("semantics.rs","FOCUS ERROR TYPE"),("semantics.rs","FOCUS SELECT KERNEL"),("semantics.rs","FOCUS ELIGIBILITY KERNEL"),("keyboard.rs","KEYBOARD STORAGE TYPE"),("keyboard.rs","KEYBOARD FOCUS KERNEL"),("keyboard.rs","KEYBOARD ADMITTED INPUT KERNEL"),("keyboard.rs","KEYBOARD INPUT TRANSFER KERNEL"),("keyboard.rs","KEYBOARD INPUT OPEN CLOSE KERNEL"),("keyboard.rs","KEYBOARD PREEDIT CLOSE KERNEL"),("keyboard.rs","PREEDIT LIFECYCLE DISPATCH KERNEL"),("keyboard.rs","INPUT LIFECYCLE DISPATCH KERNEL")] {
        let path=root.join("../../src").join(file);
        println!("cargo:rerun-if-changed={}",path.display());
        let source=fs::read_to_string(path).unwrap();
        let begin=format!("// BEGIN VERIFIED {marker}");let end=format!("// END VERIFIED {marker}");
        assert_eq!(source.matches(&begin).count(),1);assert_eq!(source.matches(&end).count(),1);
        output.push_str(source.split_once(&begin).unwrap().1.split_once(&end).unwrap().0);
    }
    for (file,marker) in [("geometry.rs","GEOMETRY POINT TYPE"),("geometry.rs","GEOMETRY SIZE TYPE"),("input.rs","POINTER ID TYPE"),("input.rs","POINTER ORDER KERNEL"),("input.rs","POINTER KIND TYPES"),("input.rs","MODIFIERS TYPE"),("input.rs","POINTER EVENT TYPE"),("input.rs","KEY PHASE TYPE"),("input.rs","KEY EVENT TYPE"),("input.rs","SCROLL FOCUS TYPES"),("input.rs","NORMALIZED INPUT TYPE"),("desktop.rs","DESKTOP IME NORMALIZE KERNEL")] {
        let path=root.join("../../src").join(file);println!("cargo:rerun-if-changed={}",path.display());
        let source=fs::read_to_string(path).unwrap();
        let begin=format!("// BEGIN VERIFIED {marker}");let end=format!("// END VERIFIED {marker}");
        assert_eq!(source.matches(&begin).count(),1);assert_eq!(source.matches(&end).count(),1);
        output.push_str(source.split_once(&begin).unwrap().1.split_once(&end).unwrap().0);
    }
    output.push_str("use std::time::Instant;\n");
    let source=fs::read_to_string(root.join("../../src/desktop.rs")).unwrap();
    // DesktopInput now owns the clock-free pointer state and the touch tracker; extract both exact declarations
    // (and the pointer state's own constructor) so the fixture builds the actual storage.
    // DesktopInput 现由无时钟指针状态与触控跟踪组成；逐字抽取二者声明（及指针状态自身构造器），夹具据此构造真实存储。
    for marker in ["DESKTOP POINTER KERNEL","DESKTOP POINTER STATE TYPE","DESKTOP POINTER STATE KERNEL","DESKTOP INPUT TYPE"] {
        let begin=format!("// BEGIN VERIFIED {marker}");let end=format!("// END VERIFIED {marker}");
        assert_eq!(source.matches(&begin).count(),1);assert_eq!(source.matches(&end).count(),1);
        output.push_str(source.split_once(&begin).unwrap().1.split_once(&end).unwrap().0);
    }
    // The touch tracker has no verified marker; take its exact declaration (derives included) verbatim.
    // 触控跟踪无验证标记；逐字取其声明（含 derive）。
    let touch="#[derive(Clone, Debug, Default)]\npub struct DesktopTouchState {";
    assert_eq!(source.matches(touch).count(),1);
    let start=source.find(touch).unwrap();let end=start+source[start..].find("\n}\n").unwrap()+3;
    output.push_str(&source[start..end]);
    output.push_str("impl DesktopInput {\n");
    output.push_str(source.split_once("// BEGIN VERIFIED DESKTOP IME DELEGATE KERNEL").unwrap().1.split_once("// END VERIFIED DESKTOP IME DELEGATE KERNEL").unwrap().0);
    output.push_str("}\n");
    println!("cargo:rerun-if-changed=winit-ime-type.rs");
    let source=fs::read_to_string(root.join("winit-ime-type.rs")).unwrap();
    output.push_str(source.split_once("// BEGIN VERIFIED WINIT IME TYPE").unwrap().1.split_once("// END VERIFIED WINIT IME TYPE").unwrap().0);
    fs::write(PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("actual.rs"),output).unwrap();
}
