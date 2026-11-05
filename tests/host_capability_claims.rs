//! Binds the matrix to the build: run under each feature set in `docs/render-tcb.md`. A claim of a
//! native contract must come with the public entries that carry it, which this file names so that
//! removing or re-gating one stops it compiling.
//! 把矩阵绑定到构建：在 `docs/render-tcb.md` 所列的每组 feature 下运行。声明原生契约必须有承载它的
//! 公开入口；本文件点名这些入口，删除或改换门控都会编译失败。

use jasper_ui_engine::host_capabilities::{CapabilityStatus, HostCapabilityMatrix};

fn gated(on: bool) -> CapabilityStatus {
    if on {
        CapabilityStatus::NativeContract
    } else {
        CapabilityStatus::FeatureGated
    }
}

#[test]
fn claims_follow_the_features_of_this_build() {
    let matrix = HostCapabilityMatrix::compiled();
    let native = cfg!(feature = "native-blitz-pending");
    assert_eq!(matrix.dpi_and_extent_reconfigure, gated(native));
    assert_eq!(matrix.injected_font_resources, gated(native));
    assert_eq!(matrix.surface_pause_resume_retire, gated(native));
    assert_eq!(
        matrix.cpu_rgba8_surface,
        gated(cfg!(feature = "cpu-pending"))
    );
    assert_eq!(
        matrix.gpu_texture_view,
        gated(cfg!(feature = "gpu-pending"))
    );
    assert_eq!(HostCapabilityMatrix::native(), matrix);
}

#[test]
fn claims_a_build_cannot_show_stay_fixed() {
    let matrix = HostCapabilityMatrix::compiled();
    assert_eq!(
        matrix.gpu_surface_acquire_and_configure,
        CapabilityStatus::HostOwned
    );
    assert_eq!(matrix.gpu_present, CapabilityStatus::HostOwned);
    assert_eq!(matrix.host_vsync, CapabilityStatus::HostOwned);
    assert_eq!(matrix.gpu_hardware_present, CapabilityStatus::Unverified);
    assert_eq!(
        matrix.gpu_device_loss_recovery,
        CapabilityStatus::Unsupported
    );
    assert_eq!(
        matrix.jasper_os_host_integration,
        CapabilityStatus::Unsupported
    );
}

// Compiled, never run: naming an entry with its real arguments is the check.
// 只编译不运行：用真实参数点名入口本身就是检查。
#[cfg(feature = "native-blitz-pending")]
#[allow(dead_code)]
fn native_contract_entries(host: &mut jasper_ui_engine::native::host::HostRenderer) {
    use jasper_ui_engine::native::host::HostRenderer;
    use jasper_ui_engine::scene::{Node, NodeId, Scene};
    let scene = Scene::new(Node::container(NodeId(1)));
    let _ = HostRenderer::with_fonts(1, &scene, &[], 1, 1);
    let _ = host.reconfigure(1, 1, 1, 1, 1.0);
    let _ = host.suspend(1, 1, 1, &mut |_| {});
    let _ = host.resume(1, 1, 1, 1, 1.0);
    let _ = host.retire(1, 1, 1, &mut |_| {});
}

#[cfg(feature = "cpu-pending")]
#[allow(dead_code)]
fn cpu_surface_entries(host: &mut jasper_ui_engine::native::host::HostRenderer, bytes: &mut [u8]) {
    let _ = host.render_to_surface(1, 1, 1, bytes, 4, 1);
    let _ = host.render_to_surface_with_damage(1, 1, 1, bytes, 4, 1);
}

#[cfg(feature = "gpu-pending")]
#[allow(dead_code)]
fn gpu_texture_entries(
    host: &mut jasper_ui_engine::native::host::HostRenderer,
    gpu: &mut jasper_ui_engine::native::host::GpuRenderer,
    view: &wgpu::TextureView,
) {
    let _ = host.render_gpu_to_host_view(1, 1, 1, gpu, wgpu::TextureFormat::Rgba8Unorm, view, 1);
    let _ = host.gpu_presented(1, 1);
}
