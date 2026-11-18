use std::env;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-check-cfg=cfg(jasper_no_std_core)");

    let incompatible = [
        "DESKTOP",
        "GPU",
        "GPU_PENDING",
        "NATIVE_ENGINE",
        "NATIVE_ENGINE_CPU",
        "NATIVE_BLIT_PENDING",
        "NATIVE_BLIT_CPU_PENDING",
        "CPU_PENDING",
        "ACCESSKIT_PENDING",
        "DESKTOP_ACCESSIBILITY",
        "UI_PLUGIN_PENDING",
        "UI_PLUGIN_PACK",
        "TEXT_PARLEY_PENDING",
        "RENDER_VELLO_CPU_PENDING",
        "RENDER_VELLO_GPU_PENDING",
        "CANVAS_TINY_SKIA",
        "REAL_TEXT",
    ];
    let std_host = incompatible
        .iter()
        .any(|feature| env::var_os(format!("CARGO_FEATURE_{feature}")).is_some());
    if env::var_os("CARGO_FEATURE_NO_STD_CORE").is_some() && !std_host {
        println!("cargo:rustc-cfg=jasper_no_std_core");
    }
}
