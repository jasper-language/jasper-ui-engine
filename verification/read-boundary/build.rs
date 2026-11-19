//! Extract exact production bodies; never maintain a second implementation.
//! 原样提取生产函数体，不维护第二份实现。
use std::{env, fs, path::PathBuf};

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let mut generated = String::new();
    for (file, markers) in [
        (
            "../../src/ui_ir/mod.rs",
            &["UI KEY TYPE", "UI EVENT KIND TYPE"][..],
        ),
        (
            "../../src/ui_ir/identity.rs",
            &["IDENTITY ERROR TYPE", "UI IDENTITY NEXT KERNEL"][..],
        ),
        (
            "../../src/ui_ir/events.rs",
            &[
                "UI PERMISSION TYPE",
                "UI EVENT PERMISSION KERNEL",
                "AUTHORITY FAILURE TYPE",
                "AUTHORITY MATCH KERNEL",
                "PERMISSION FAILURE TYPE",
                "PERMISSION MATCH KERNEL",
            ][..],
        ),
        (
            "../../src/ui_ir/mod.rs",
            &["RESOURCE KIND TYPE", "RESOURCE ADMISSION TYPE"][..],
        ),
        (
            "../../src/ui_ir/native.rs",
            &["RESOURCE TRANSITION TYPE", "RESOURCE TRANSITION KERNEL"][..],
        ),
        (
            "../../src/batch.rs",
            &[
                "DECODE RANGE KERNEL",
                "BYTE READ KERNEL",
                "BATCH ERROR TYPE",
                "CURSOR READ KERNEL",
                "SCALAR READ KERNEL",
                "HEADER CONSTANTS TYPE",
                "HEADER READ KERNEL",
                "BOUNDED PAYLOAD KERNEL",
            ][..],
        ),
        (
            "../../src/surface.rs",
            &[
                "SURFACE ROW KERNEL",
                "SURFACE SPAN KERNEL",
                "SURFACE BORROW KERNEL",
                "SURFACE WRITE KERNEL",
                "SURFACE SEGMENT WRITE KERNEL",
                "SURFACE VIEW KERNEL",
                "PIXEL PACK KERNEL",
            ][..],
        ),
        (
            "../../src/geometry.rs",
            &[
                "GEOMETRY POINT TYPE",
                "GEOMETRY SIZE TYPE",
                "GEOMETRY RECT TYPE",
            ][..],
        ),
        (
            "../../src/raster.rs",
            &["CANVAS STORAGE TYPE", "CANVAS PIXEL READ KERNEL", "CANVAS IMPORT KERNEL"][..],
        ),
        ("../../src/style.rs", &["COLOR TYPE", "COLOR CONSTRUCT KERNEL"][..]),
        (
            "../../src/native_adapter/native_fonts.rs",
            &["FONT ORDER KERNEL", "FONT WEIGHT KERNEL", "FONT FAMILY KERNEL", "BOLD ALIAS KERNEL"][..],
        ),
        (
            "../../src/native_adapter/frame_schedule.rs",
            &["FRAME REUSE KERNEL", "FRAME DAMAGE TYPE", "DAMAGE SPAN KERNEL", "FRAME DAMAGE KERNEL", "DAMAGE RECT KERNEL", "DAMAGE WRITE KERNEL", "FRAME TICK KERNEL", "FRAME PACE TYPE", "FRAME PACE KERNEL"][..],
        ),
        ("../../src/text.rs", &["TEXT CLASS KERNEL"][..]),
        (
            "../../src/native_adapter/text_edit.rs",
            &["UTF8 FLOOR BOUNDARY KERNEL", "PREEDIT BUFFER OFFSET KERNEL"][..],
        ),
        (
            "../../src/native_adapter/text_geometry.rs",
            &["TEXT OFFSET KERNEL", "TEXT RANGE KERNEL"][..],
        ),
        (
            "../../src/native_adapter/access_tree.rs",
            &["ACCESS ACTIONS TYPE", "ACCESS ACTIONS KERNEL", "ACCESS CHANGE TYPE", "ACCESS CHANGE KERNEL"][..],
        ),
        (
            "../../src/native_adapter/access_kit.rs",
            &["ACCESS REQUEST TYPE", "ACCESS REQUEST KERNEL"][..],
        ),
        (
            "../../src/native_adapter/gpu_render.rs",
            &["GPU READBACK LAYOUT KERNEL"][..],
        ),
        (
            "../../src/core_renderer.rs",
            &["CORE FRAME BYTES KERNEL", "CORE FRAME BUDGET KERNEL"][..],
        ),
        (
            "../../src/native_adapter/glyph_skew.rs",
            &["GLYPH SKEW KERNEL"][..],
        ),
        (
            "../../src/native_adapter/native_scene.rs",
            &[
                "SCENE UPDATE ERROR TYPE KERNEL",
                "SCENE FRAME COMPLETION KERNEL",
                "SCENE FRAME ORDER KERNEL",
                "SCENE INTERACTION RESTORE KERNEL",
            ][..],
        ),
        (
            "../../src/resource.rs",
            &["RESOURCE GATE KERNEL", "BYTES SLOT KERNEL"][..],
        ),
        (
            "../../src/input.rs",
            &[
                "MODIFIERS TYPE",
                "KEY PHASE TYPE",
                "KEY EVENT TYPE",
                "POINTER ID TYPE",
                "POINTER ORDER KERNEL",
                "POINTER KIND TYPES",
                "POINTER EVENT TYPE",
                "POINTER TRACKER TYPE",
            ][..],
        ),
        (
            "../../src/semantics.rs",
            &[
                "ROLE TYPE",
                "FOCUS STORAGE TYPE",
                "FOCUS CLEANUP KERNEL",
                "FOCUS ERROR TYPE",
                "FOCUS SELECT KERNEL",
            ][..],
        ),
        (
            "../../src/keyboard.rs",
            &[
                "ROLE ACTIVATION KERNEL",
                "COMMAND MODIFIER KERNEL",
                "KEYBOARD STORAGE TYPE",
                "KEYBOARD FOCUS KERNEL",
                "INPUT RETIRE KERNEL",
            ][..],
        ),
        (
            "../../src/pointer.rs",
            &[
                "POINTER ROUTER TYPE",
                "CAPTURE QUERY KERNEL",
                "HOVER CHANGE TYPE",
                "HOVER STORAGE TYPE",
                "HOVER UPDATE KERNEL",
                "HOVER RETIRE KERNEL",
            ][..],
        ),
        (
            "../../src/desktop.rs",
            &[
                "PIXEL ERROR TYPE",
                "PIXEL CONVERT KERNEL",
                "DESKTOP POINTER KERNEL",
                "DESKTOP POINTER STATE TYPE",
                "DESKTOP POINTER STATE KERNEL",
            ][..],
        ),
        ("../../src/handle.rs", &["HANDLE TYPE", "HANDLE KERNEL"][..]),
        (
            "../../src/scene.rs",
            &["NODE ID TYPE", "VERSION KERNEL", "TRANSACTION GATE KERNEL", "SCENE BATCH STAGE KERNEL", "SCENE STATE TYPE", "SCENE BATCH COMMIT KERNEL", "SCENE MOVE ADMISSION KERNEL", "SCENE NODE KIND TYPE", "SCENE NODE TYPE", "SCENE PATCH TYPE", "SCENE ERROR TYPE", "SCENE NODE INSERT KERNEL", "SCENE NODE TAKE KERNEL", "SCENE NODE REMOVE KERNEL", "SCENE MOVE KERNEL", "SCENE NODE FIND KERNEL", "SCENE PATCH KERNEL", "SCENE CHECKED PATCH KERNEL", "SCENE BATCH UNIQUE KERNEL", "SCENE UNIQUE IDS KERNEL", "SCENE IDENTITY SET INSTANCE"][..],
        ),
    ] {
        let source = root.join(file);
        println!("cargo:rerun-if-changed={}", source.display());
        let text = fs::read_to_string(source).unwrap();
        for marker in markers {
            let begin = format!("// BEGIN VERIFIED {marker}");
            let end = format!("// END VERIFIED {marker}");
            assert_eq!(text.matches(&begin).count(), 1, "ambiguous begin marker");
            assert_eq!(text.matches(&end).count(), 1, "ambiguous end marker");
            generated.push_str(
                text.split_once(&begin)
                    .unwrap()
                    .1
                    .split_once(&end)
                    .unwrap()
                    .0,
            );
        }
    }
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("production.rs"),
        generated,
    )
    .unwrap();
}
