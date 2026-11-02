//! Declared rendering-host capabilities, kept separate from evidence of an actual host.
//! 声明的渲染宿主能力；与任何实际宿主的运行证据分开。

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

impl HostCapabilityMatrix {
    /// Capabilities of this exact compile, without substituting a fixture or a host report for
    /// platform support. The matrix says who owns each operation; it is not a device inventory.
    /// 本次编译的声明能力；不以夹具或宿主报告替代平台支持。矩阵说明各操作归属，不是设备清单。
    pub const fn compiled() -> Self {
        Self {
            cpu_rgba8_surface: if cfg!(feature = "native-blitz-cpu-pending") {
                CapabilityStatus::NativeContract
            } else {
                CapabilityStatus::FeatureGated
            },
            dpi_and_extent_reconfigure: CapabilityStatus::FeatureGated,
            injected_font_resources: CapabilityStatus::FeatureGated,
            surface_pause_resume_retire: CapabilityStatus::FeatureGated,
            gpu_texture_view: if cfg!(feature = "gpu-pending") {
                CapabilityStatus::NativeContract
            } else {
                CapabilityStatus::FeatureGated
            },
            gpu_surface_acquire_and_configure: CapabilityStatus::HostOwned,
            gpu_present: CapabilityStatus::HostOwned,
            gpu_hardware_present: CapabilityStatus::Unverified,
            gpu_device_loss_recovery: CapabilityStatus::Unverified,
            host_vsync: CapabilityStatus::HostOwned,
            jasper_os_host_integration: CapabilityStatus::Unsupported,
        }
    }

    /// Native CPU rendering also requires the pinned native feature because all three contracts
    /// ship on that surface.
    /// 原生 CPU 渲染同样要求固定原生 feature，因为三项契约共享该入口。
    pub const fn native() -> Self {
        Self {
            dpi_and_extent_reconfigure: if cfg!(feature = "native-blitz-pending") {
                CapabilityStatus::NativeContract
            } else {
                CapabilityStatus::FeatureGated
            },
            injected_font_resources: if cfg!(feature = "native-blitz-pending") {
                CapabilityStatus::NativeContract
            } else {
                CapabilityStatus::FeatureGated
            },
            surface_pause_resume_retire: if cfg!(feature = "native-blitz-pending") {
                CapabilityStatus::NativeContract
            } else {
                CapabilityStatus::FeatureGated
            },
            ..Self::compiled()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_never_claim_the_system_host_or_hardware_gpu() {
        let matrix = HostCapabilityMatrix::native();
        assert_eq!(
            matrix.jasper_os_host_integration,
            CapabilityStatus::Unsupported
        );
        assert_eq!(
            matrix.gpu_device_loss_recovery,
            CapabilityStatus::Unverified
        );
        assert_eq!(
            matrix.gpu_surface_acquire_and_configure,
            CapabilityStatus::HostOwned
        );
        assert_eq!(matrix.gpu_present, CapabilityStatus::HostOwned);
        assert_eq!(
            matrix.gpu_hardware_present,
            CapabilityStatus::Unverified
        );
        assert_eq!(matrix.host_vsync, CapabilityStatus::HostOwned);
        assert_eq!(
            matrix.cpu_rgba8_surface,
            if cfg!(feature = "native-blitz-cpu-pending") {
                CapabilityStatus::NativeContract
            } else {
                CapabilityStatus::FeatureGated
            }
        );
        assert_eq!(
            matrix.gpu_texture_view,
            if cfg!(feature = "gpu-pending") {
                CapabilityStatus::NativeContract
            } else {
                CapabilityStatus::FeatureGated
            }
        );
        assert_eq!(
            matrix.surface_pause_resume_retire,
            if cfg!(feature = "native-blitz-pending") {
                CapabilityStatus::NativeContract
            } else {
                CapabilityStatus::FeatureGated
            }
        );
    }
}
