//! wgpu adapter enumeration mapped onto the host-neutral GPU preference policy.

use wgpu::{Adapter, Backends, DeviceType, Instance, InstanceDescriptor};

use crate::graphics::{select_adapter, AdapterInfo, AdapterKind, AdapterSelection, GpuPreference};

/// Creates a wgpu instance for the current platform.
pub fn instance() -> Instance {
    let mut descriptor = InstanceDescriptor::new_without_display_handle();
    descriptor.backends = Backends::all();
    Instance::new(descriptor)
}

/// Enumerates every adapter visible to the instance.
pub fn adapters(instance: &Instance) -> Vec<Adapter> {
    pollster::block_on(instance.enumerate_adapters(Backends::all()))
}

/// Converts wgpu's device class into the engine's coarse class.
pub fn adapter_kind(device_type: DeviceType) -> AdapterKind {
    match device_type {
        DeviceType::IntegratedGpu => AdapterKind::Integrated,
        DeviceType::DiscreteGpu => AdapterKind::Discrete,
        DeviceType::Cpu => AdapterKind::Software,
        DeviceType::VirtualGpu | DeviceType::Other => AdapterKind::Other,
    }
}

/// Snapshot used by the selector and diagnostics.
pub fn describe(adapter: &Adapter) -> AdapterInfo {
    let info = adapter.get_info();
    AdapterInfo {
        name: info.name,
        kind: adapter_kind(info.device_type),
        vendor: info.vendor,
        device: info.device,
    }
}

/// Enumerates adapters and applies the user's preference.
pub fn select(
    instance: &Instance,
    preference: &GpuPreference,
    allow_software: bool,
) -> Option<(Adapter, AdapterSelection)> {
    let adapters = adapters(instance);
    let infos: Vec<AdapterInfo> = adapters.iter().map(describe).collect();
    let selection = select_adapter(&infos, preference, allow_software)?;
    let adapter = adapters.into_iter().nth(selection.index)?;
    Some((adapter, selection))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_wgpu_device_types() {
        assert_eq!(
            adapter_kind(DeviceType::IntegratedGpu),
            AdapterKind::Integrated
        );
        assert_eq!(adapter_kind(DeviceType::DiscreteGpu), AdapterKind::Discrete);
        assert_eq!(adapter_kind(DeviceType::Cpu), AdapterKind::Software);
        assert_eq!(adapter_kind(DeviceType::VirtualGpu), AdapterKind::Other);
    }

    #[test]
    fn enumeration_and_selection_do_not_panic_without_a_gpu() {
        let instance = instance();
        let found = adapters(&instance);
        let selection = select(&instance, &GpuPreference::Auto, true);
        assert_eq!(found.len(), found.len());
        if found.is_empty() {
            assert!(selection.is_none());
        }
    }
}
