//! Host-neutral graphics adapter preference and selection policy.
//! Windows 风格：每个应用可选自动 / 集成 / 独立 / 指定适配器 / 纯软件。
//!
//! 这里只决定“选哪个适配器”，不依赖 wgpu；桌面宿主把真实 `AdapterInfo`
//! 映射进来即可。运行时切换由宿主重建 device/surface，场景状态不受影响。

use std::fmt;
use std::str::FromStr;

/// User preference for the rendering adapter.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum GpuPreference {
    /// Prefer a discrete GPU, then integrated, then software.
    #[default]
    Auto,
    Integrated,
    Discrete,
    /// A case-insensitive substring of the adapter name.
    Named(String),
    /// Force the CPU/software adapter.
    Software,
}

impl GpuPreference {
    pub fn parse(value: &str) -> Option<Self> {
        let text = value.trim();
        let lower = text.to_ascii_lowercase();
        match lower.as_str() {
            "" | "auto" => Some(Self::Auto),
            "integrated" | "igpu" | "integrated-gpu" => Some(Self::Integrated),
            "discrete" | "dgpu" | "discrete-gpu" | "high-performance" => Some(Self::Discrete),
            "software" | "cpu" | "warp" => Some(Self::Software),
            _ if lower.starts_with("name:") => {
                // Preserve the user's original casing for the adapter name.
                let name = text[5..].trim();
                (!name.is_empty()).then(|| Self::Named(name.to_string()))
            }
            _ => None,
        }
    }

    /// Stable key for a per-application settings file.
    pub fn key(&self) -> String {
        match self {
            Self::Auto => "auto".into(),
            Self::Integrated => "integrated".into(),
            Self::Discrete => "discrete".into(),
            Self::Software => "software".into(),
            Self::Named(name) => format!("name:{name}"),
        }
    }
}

impl FromStr for GpuPreference {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value).ok_or(())
    }
}

impl fmt::Display for GpuPreference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.key())
    }
}

/// Coarse adapter class, mapped from wgpu's `DeviceType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdapterKind {
    Integrated,
    Discrete,
    Software,
    Other,
}

/// Minimal adapter description needed by the selector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterInfo {
    pub name: String,
    pub kind: AdapterKind,
    pub vendor: u32,
    pub device: u32,
}

/// Why the selected adapter differs from the first-choice class.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionFallback {
    #[default]
    None,
    /// A named adapter was requested but not found.
    PreferredMissing,
    /// No hardware adapter matched, so the CPU adapter was selected.
    SoftwareFallback,
}

/// Selected adapter and its fallback status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdapterSelection {
    pub index: usize,
    pub fallback: SelectionFallback,
}

/// Selects an adapter index for a preference.
///
/// `allow_software` controls whether a CPU adapter is an acceptable fallback.
/// Hosts should surface `SoftwareFallback` to the user/settings UI.
pub fn select_adapter(
    adapters: &[AdapterInfo],
    preference: &GpuPreference,
    allow_software: bool,
) -> Option<AdapterSelection> {
    if adapters.is_empty() {
        return None;
    }

    if let GpuPreference::Named(needle) = preference {
        let needle = needle.to_ascii_lowercase();
        if let Some(index) = adapters.iter().position(|adapter| {
            adapter.kind != AdapterKind::Software
                && adapter.name.to_ascii_lowercase().contains(&needle)
        }) {
            return Some(AdapterSelection {
                index,
                fallback: SelectionFallback::None,
            });
        }
        let mut selection = select_adapter(adapters, &GpuPreference::Auto, allow_software)?;
        selection.fallback = SelectionFallback::PreferredMissing;
        return Some(selection);
    }

    let order = match preference {
        GpuPreference::Auto => [
            AdapterKind::Discrete,
            AdapterKind::Integrated,
            AdapterKind::Other,
            AdapterKind::Software,
        ],
        GpuPreference::Integrated => [
            AdapterKind::Integrated,
            AdapterKind::Other,
            AdapterKind::Discrete,
            AdapterKind::Software,
        ],
        GpuPreference::Discrete => [
            AdapterKind::Discrete,
            AdapterKind::Other,
            AdapterKind::Integrated,
            AdapterKind::Software,
        ],
        GpuPreference::Software => [
            AdapterKind::Software,
            AdapterKind::Software,
            AdapterKind::Software,
            AdapterKind::Software,
        ],
        GpuPreference::Named(_) => unreachable!("named handled above"),
    };

    for kind in order.iter() {
        if *kind == AdapterKind::Software && !allow_software {
            continue;
        }
        if let Some(index) = adapters.iter().position(|adapter| adapter.kind == *kind) {
            let preferred_kind = order[0];
            let fallback = if *kind == preferred_kind {
                SelectionFallback::None
            } else if *kind == AdapterKind::Software && preferred_kind != AdapterKind::Software {
                SelectionFallback::SoftwareFallback
            } else {
                SelectionFallback::PreferredMissing
            };
            return Some(AdapterSelection { index, fallback });
        }
    }
    None
}

/// Per-application graphics settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphicsSettings {
    pub preference: GpuPreference,
    pub allow_software_fallback: bool,
}

impl Default for GraphicsSettings {
    fn default() -> Self {
        Self {
            preference: GpuPreference::Auto,
            allow_software_fallback: true,
        }
    }
}

impl GraphicsSettings {
    /// Reads `JASPER_GPU` and `JASPER_GPU_ALLOW_SOFTWARE`.
    pub fn from_env() -> Self {
        let preference = std::env::var("JASPER_GPU")
            .ok()
            .and_then(|value| GpuPreference::parse(&value))
            .unwrap_or_default();
        let allow_software_fallback = std::env::var("JASPER_GPU_ALLOW_SOFTWARE")
            .map(|value| !matches!(value.trim(), "0" | "false" | "no" | "off"))
            .unwrap_or(true);
        Self {
            preference,
            allow_software_fallback,
        }
    }

    /// Persisted environment representation used by a settings UI.
    pub fn env_pairs(&self) -> [(&'static str, String); 2] {
        [
            ("JASPER_GPU", self.preference.key()),
            (
                "JASPER_GPU_ALLOW_SOFTWARE",
                if self.allow_software_fallback {
                    "1".into()
                } else {
                    "0".into()
                },
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter(name: &str, kind: AdapterKind) -> AdapterInfo {
        AdapterInfo {
            name: name.into(),
            kind,
            vendor: 0,
            device: 0,
        }
    }

    fn adapters() -> Vec<AdapterInfo> {
        vec![
            adapter("Intel UHD", AdapterKind::Integrated),
            adapter("NVIDIA RTX", AdapterKind::Discrete),
            adapter("llvmpipe", AdapterKind::Software),
        ]
    }

    #[test]
    fn auto_prefers_discrete_and_integrated_prefers_integrated() {
        let list = adapters();
        assert_eq!(
            select_adapter(&list, &GpuPreference::Auto, true),
            Some(AdapterSelection {
                index: 1,
                fallback: SelectionFallback::None
            })
        );
        assert_eq!(
            select_adapter(&list, &GpuPreference::Integrated, true),
            Some(AdapterSelection {
                index: 0,
                fallback: SelectionFallback::None
            })
        );
    }

    #[test]
    fn named_adapter_matches_case_insensitively_with_fallback() {
        let list = adapters();
        assert_eq!(
            select_adapter(&list, &GpuPreference::Named("nvidia".into()), true),
            Some(AdapterSelection {
                index: 1,
                fallback: SelectionFallback::None
            })
        );
        assert_eq!(
            select_adapter(&list, &GpuPreference::Named("missing".into()), true),
            Some(AdapterSelection {
                index: 1,
                fallback: SelectionFallback::PreferredMissing
            })
        );
    }

    #[test]
    fn software_can_be_forced_or_forbidden() {
        let software = vec![adapter("llvmpipe", AdapterKind::Software)];
        assert_eq!(
            select_adapter(&software, &GpuPreference::Auto, true),
            Some(AdapterSelection {
                index: 0,
                fallback: SelectionFallback::SoftwareFallback
            })
        );
        assert_eq!(select_adapter(&software, &GpuPreference::Auto, false), None);
        assert_eq!(
            select_adapter(&software, &GpuPreference::Software, false),
            None
        );
    }

    #[test]
    fn preference_round_trips_through_settings_text() {
        for preference in [
            GpuPreference::Auto,
            GpuPreference::Integrated,
            GpuPreference::Discrete,
            GpuPreference::Software,
            GpuPreference::Named("RTX 3060".into()),
        ] {
            assert_eq!(GpuPreference::parse(&preference.key()), Some(preference));
        }
    }
}
