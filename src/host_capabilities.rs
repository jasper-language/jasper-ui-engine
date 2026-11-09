//! Declared rendering-host capabilities, kept separate from evidence of an actual host.
//! 声明的渲染宿主能力；与任何实际宿主的运行证据分开。

// BEGIN VERIFIED CAPABILITY STATUS TYPE
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
// END VERIFIED CAPABILITY STATUS TYPE

// BEGIN VERIFIED CAPABILITY MATRIX STORAGE TYPE
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
// END VERIFIED CAPABILITY MATRIX STORAGE TYPE

// BEGIN VERIFIED CAPABILITY FEATURES TYPE
/// The Cargo features the claims depend on. Separate from `cfg!` so that every combination can be
/// checked; only `HostCapabilityMatrix::compiled` reads the real ones.
/// 各项声明依赖的 Cargo feature。与 `cfg!` 分开，以便检查所有组合；只有 `compiled` 读取真实值。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CompiledFeatures {
    pub(crate) native_scene: bool,
    pub(crate) cpu_surface: bool,
    pub(crate) gpu_texture: bool,
}
// END VERIFIED CAPABILITY FEATURES TYPE

// BEGIN VERIFIED CAPABILITY MATRIX KERNEL
impl HostCapabilityMatrix {
    /// The claims for one feature combination. A feature only moves a claim between gated and
    /// native; device-loss support follows the GPU feature, while hardware acceptance and the
    /// system host remain fixed claims.
    /// 一种 feature 组合对应的声明。feature 只让声明在门控与原生之间移动；设备丢失契约随
    /// GPU feature 切换，硬件验收和系统宿主则是固定声明。
    pub(crate) const fn for_features(features: CompiledFeatures) -> Self {
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
            dpi_and_extent_reconfigure: native,
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
            gpu_device_loss_recovery: if features.gpu_texture {
                CapabilityStatus::NativeContract
            } else {
                CapabilityStatus::FeatureGated
            },
            host_vsync: CapabilityStatus::HostOwned,
            jasper_os_host_integration: CapabilityStatus::Unsupported,
        }
    }
}
// END VERIFIED CAPABILITY MATRIX KERNEL

impl HostCapabilityMatrix {
    /// Capabilities of this exact compile, without substituting a fixture or a host report for
    /// platform support. The matrix says who owns each operation; it is not a device inventory.
    /// 本次编译的声明能力；不以夹具或宿主报告替代平台支持。矩阵说明各操作归属，不是设备清单。
    pub const fn compiled() -> Self {
        Self::for_features(CompiledFeatures {
            native_scene: cfg!(feature = "native-blitz-pending"),
            cpu_surface: cfg!(feature = "cpu-pending"),
            gpu_texture: cfg!(feature = "gpu-pending"),
        })
    }

    /// The same report as [`Self::compiled`], kept for existing callers.
    /// 与 [`Self::compiled`] 相同的报告，为既有调用方保留。
    pub const fn native() -> Self {
        Self::compiled()
    }
}

#[cfg(test)]
mod tests {
    use super::CapabilityStatus::{
        FeatureGated, HostOwned, NativeContract, Unsupported, Unverified,
    };
    use super::*;

    fn gated(on: bool) -> CapabilityStatus {
        if on {
            NativeContract
        } else {
            FeatureGated
        }
    }

    // The whole input domain has eight values, so the table is checked on all of them.
    // 输入域只有八个值，所以逐个检查判定表。
    #[test]
    fn every_feature_combination_follows_the_table() {
        for bits in 0..8u8 {
            let features = CompiledFeatures {
                native_scene: bits & 1 != 0,
                cpu_surface: bits & 2 != 0,
                gpu_texture: bits & 4 != 0,
            };
            let expected = HostCapabilityMatrix {
                cpu_rgba8_surface: gated(features.cpu_surface),
                dpi_and_extent_reconfigure: gated(features.native_scene),
                injected_font_resources: gated(features.native_scene),
                surface_pause_resume_retire: gated(features.native_scene),
                gpu_texture_view: gated(features.gpu_texture),
                gpu_surface_acquire_and_configure: HostOwned,
                gpu_present: HostOwned,
                gpu_hardware_present: Unverified,
                gpu_device_loss_recovery: gated(features.gpu_texture),
                host_vsync: HostOwned,
                jasper_os_host_integration: Unsupported,
            };
            assert_eq!(HostCapabilityMatrix::for_features(features), expected);
        }
    }

    #[test]
    fn no_combination_claims_what_a_build_cannot_show() {
        for bits in 0..8u8 {
            let matrix = HostCapabilityMatrix::for_features(CompiledFeatures {
                native_scene: bits & 1 != 0,
                cpu_surface: bits & 2 != 0,
                gpu_texture: bits & 4 != 0,
            });
            for claim in [
                matrix.gpu_hardware_present,
                matrix.host_vsync,
                matrix.gpu_present,
                matrix.gpu_surface_acquire_and_configure,
                matrix.jasper_os_host_integration,
            ] {
                assert_ne!(claim, NativeContract);
            }
        }
    }
}
