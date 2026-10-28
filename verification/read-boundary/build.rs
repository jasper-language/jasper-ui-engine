//! Extract exact production bodies; never maintain a second implementation.
//! 原样提取生产函数体，不维护第二份实现。
use std::{env, fs, path::PathBuf};

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let mut generated = String::new();
    for (file, markers) in [
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
        ("../../src/style.rs", &["COLOR TYPE"][..]),
        (
            "../../src/native_adapter/native_fonts.rs",
            &["FONT ORDER KERNEL"][..],
        ),
        (
            "../../src/native_adapter/frame_schedule.rs",
            &["FRAME REUSE KERNEL", "FRAME DAMAGE TYPE", "DAMAGE SPAN KERNEL", "FRAME DAMAGE KERNEL", "DAMAGE RECT KERNEL", "DAMAGE WRITE KERNEL", "FRAME TICK KERNEL", "FRAME PACE TYPE", "FRAME PACE KERNEL"][..],
        ),
        ("../../src/text.rs", &["TEXT CLASS KERNEL"][..]),
        (
            "../../src/native_adapter/text_geometry.rs",
            &["TEXT OFFSET KERNEL", "TEXT RANGE KERNEL"][..],
        ),
        (
            "../../src/native_adapter/gpu_render.rs",
            &["GPU READBACK LAYOUT KERNEL"][..],
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
            &["PIXEL ERROR TYPE", "PIXEL CONVERT KERNEL"][..],
        ),
        (
            "../../src/scene.rs",
            &["NODE ID TYPE", "VERSION KERNEL", "TRANSACTION GATE KERNEL"][..],
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
