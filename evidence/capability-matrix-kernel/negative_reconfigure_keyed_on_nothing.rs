use vstd::prelude::*;
verus! {



pub open spec fn capability_gated(on: bool) -> CapabilityStatus {
    if on { CapabilityStatus::NativeContract } else { CapabilityStatus::FeatureGated }
}
/// What this build promises for one host-facing capability.
/// 当前构建对一项宿主能力的承诺。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityStatus {
    /// Engine-owned code implements a host-neutral contract in this build.
    /// 当前构建中由引擎自有代码实现的宿主中立契约。
    NativeContract,
    /// The code exists, but callers must enable the named Cargo feature.
    /// 代码已存在，但调用方必须启用相应 Cargo feature。
    FeatureGated,
    /// The host must acquire, own and synchronize the resource or presentation operation.
    /// 宿主必须取得、持有并同步该资源或呈现操作。
    HostOwned,
    /// No implementation is claimed.
    /// 未声明实现。
    Unsupported,
    /// Implemented to the extent stated, but the relevant acceptance evidence is not present.
    /// 已有实现，但缺少相应验收证据。
    Unverified,
}

/// Stable rendering capabilities reported to embedding hosts.
/// 向嵌入宿主报告的稳定渲染能力。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostCapabilityMatrix {
    pub cpu_rgba8_surface: CapabilityStatus,
    pub dpi_and_extent_reconfigure: CapabilityStatus,
    pub injected_font_resources: CapabilityStatus,
    pub surface_pause_resume_retire: CapabilityStatus,
    pub gpu_texture_view: CapabilityStatus,
    pub gpu_surface_acquire_and_configure: CapabilityStatus,
    pub gpu_present: CapabilityStatus,
    pub gpu_hardware_present: CapabilityStatus,
    pub gpu_device_loss_recovery: CapabilityStatus,
    pub host_vsync: CapabilityStatus,
    pub jasper_os_host_integration: CapabilityStatus,
}

/// The Cargo features the claims depend on. Separate from `cfg!` so that every combination can be
/// checked; only `HostCapabilityMatrix::compiled` reads the real ones.
/// 各项声明依赖的 Cargo feature。与 `cfg!` 分开，以便检查所有组合；只有 `compiled` 读取真实值。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CompiledFeatures {
    pub(crate) native_scene: bool,
    pub(crate) cpu_surface: bool,
    pub(crate) gpu_texture: bool,
}

impl HostCapabilityMatrix {
    /// The claims for one feature combination. A feature only moves a claim between gated and
    /// native; nothing a build cannot show (hardware, device loss, the system host) depends on it.
    /// 一种 feature 组合对应的声明。feature 只让声明在门控与原生之间移动；构建无法证明的项
    /// （硬件、设备丢失、系统宿主）不依赖它。
    pub(crate) const fn for_features(features: CompiledFeatures) -> (result: Self)
        ensures
            result.cpu_rgba8_surface == capability_gated(features.cpu_surface),
            result.dpi_and_extent_reconfigure == capability_gated(features.native_scene),
            result.injected_font_resources == capability_gated(features.native_scene),
            result.surface_pause_resume_retire == capability_gated(features.native_scene),
            result.gpu_texture_view == capability_gated(features.gpu_texture),
            result.gpu_surface_acquire_and_configure == CapabilityStatus::HostOwned,
            result.gpu_present == CapabilityStatus::HostOwned,
            result.gpu_hardware_present == CapabilityStatus::Unverified,
            result.gpu_device_loss_recovery == CapabilityStatus::Unsupported,
            result.host_vsync == CapabilityStatus::HostOwned,
            result.jasper_os_host_integration == CapabilityStatus::Unsupported,
    {
        let native = if features.native_scene {
            CapabilityStatus::NativeContract
        } else {
            CapabilityStatus::FeatureGated
        };
        Self {
            cpu_rgba8_surface: if features.cpu_surface {
                CapabilityStatus::NativeContract
            } else {
                CapabilityStatus::FeatureGated
            },
            dpi_and_extent_reconfigure: CapabilityStatus::NativeContract,
            injected_font_resources: native,
            surface_pause_resume_retire: native,
            gpu_texture_view: if features.gpu_texture {
                CapabilityStatus::NativeContract
            } else {
                CapabilityStatus::FeatureGated
            },
            gpu_surface_acquire_and_configure: CapabilityStatus::HostOwned,
            gpu_present: CapabilityStatus::HostOwned,
            gpu_hardware_present: CapabilityStatus::Unverified,
            gpu_device_loss_recovery: CapabilityStatus::Unsupported,
            host_vsync: CapabilityStatus::HostOwned,
            jasper_os_host_integration: CapabilityStatus::Unsupported,
        }
    }
}

}
