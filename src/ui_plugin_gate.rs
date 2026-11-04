//! Pure admission rules of the `ui.*` plugin host namespace, version 1; maps and calls nothing.
//! `ui.*` 插件宿主命名空间第 1 版的纯准入规则；不映射也不调用任何代码。
//!
//! `ui.*` is a versioned UI host namespace, not part of the Native ABI v1 core; see `docs/abi.md`.
//! `ui.*` 是版本化的 UI 宿主命名空间，不属于 Native ABI v1 核心；见 `docs/abi.md`。

/// Value of the manifest key `ui.abi` this host implements.
/// 本宿主实现的清单键 `ui.abi` 取值。
pub const UI_PLUGIN_ABI: &str = "1";
/// Value of the manifest key `ui.kind` for a plugin that paints one node.
/// 绘制单个节点的插件的清单键 `ui.kind` 取值。
pub const UI_PLUGIN_KIND_PAINT_NODE: &str = "paint-node";
/// Capability a `paint-node` plugin must request in `core.requires`.
/// `paint-node` 插件必须在 `core.requires` 中请求的能力。
pub const UI_PAINT_CAPABILITY: &str = "ui.paint";
/// `UiPaintRequestV1::version`.
/// `UiPaintRequestV1::version` 的取值。
pub const UI_PAINT_REQUEST_VERSION: u32 = 1;
/// `UiPaintCommandV1::kind`: opaque-or-blended rectangle fill.
/// `UiPaintCommandV1::kind`：矩形填充（按 alpha 混合）。
pub const UI_PAINT_FILL_RECT: u32 = 1;
/// Commands a plugin may return for one node in one frame.
/// 插件一帧为一个节点最多可返回的命令数。
pub const UI_PAINT_COMMAND_CAPACITY: u32 = 64;

/// What the host tells the plugin; node size is in device pixels.
/// 宿主告知插件的内容；节点尺寸以设备像素计。
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiPaintRequestV1 {
    pub version: u32,
    pub size: u32,
    pub node_width: u32,
    pub node_height: u32,
}

/// One paint command in node-local device pixels; every bit pattern is a value.
/// 一条绘制命令，坐标为节点局部设备像素；任何位模式都是合法取值。
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UiPaintCommandV1 {
    pub kind: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub rgba: [u8; 4],
    pub reserved: [u32; 2],
}

/// Validated commands of one scene node, painted above the scene in slice order.
/// 一个场景节点的已校验命令，按切片顺序绘制在场景之上。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiPaintLayer<'a> {
    pub node: crate::scene::NodeId,
    pub commands: &'a [UiPaintCommandV1],
}

/// A node's device-pixel box in view coordinates; the origin may be off-screen.
/// 节点在视图坐标中的设备像素框；原点可在屏幕之外。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiNodePixelBox {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Why a plugin's paint output was refused; the whole output is refused, never partly drawn.
/// 插件绘制输出被拒绝的原因；整份输出被拒绝，绝不部分绘制。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiPaintError {
    /// The entry returned a negative value or more commands than the buffer holds.
    /// 入口返回负值，或返回的命令数超过缓冲区容量。
    Count(i32),
    /// Command at this index has an unknown kind or nonzero reserved bits.
    /// 该下标的命令种类未知或保留位非零。
    Malformed(usize),
    /// Command at this index is empty or leaves the node.
    /// 该下标的命令为空矩形或超出节点。
    OutOfNode(usize),
}

/// Why the `ui.*` keys of a manifest were refused.
/// 清单的 `ui.*` 键被拒绝的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiManifestError {
    MissingAbi,
    UnsupportedAbi,
    MissingKind,
    UnsupportedKind,
    MissingEntry,
    /// `ui.paint` names a lifecycle export.
    /// `ui.paint` 指向生命周期导出。
    ReservedEntry,
    /// `ui.paint` is not in `core.exports`.
    /// `ui.paint` 不在 `core.exports` 中。
    EntryNotExported,
    /// `ui.paint` capability is not requested in `core.requires`.
    /// `core.requires` 未请求 `ui.paint` 能力。
    CapabilityNotRequested,
}

/// Admits the `ui.*` keys against the declared exports and capability requests.
/// 按声明的导出与能力请求准入 `ui.*` 键。
pub fn ui_manifest_admit<'a, 'b>(
    abi: Option<&str>,
    kind: Option<&str>,
    entry: Option<&'a str>,
    mut exports: impl Iterator<Item = &'b str>,
    mut requires: impl Iterator<Item = &'b str>,
) -> Result<&'a str, UiManifestError> {
    if abi.ok_or(UiManifestError::MissingAbi)? != UI_PLUGIN_ABI {
        return Err(UiManifestError::UnsupportedAbi);
    }
    if kind.ok_or(UiManifestError::MissingKind)? != UI_PLUGIN_KIND_PAINT_NODE {
        return Err(UiManifestError::UnsupportedKind);
    }
    let entry = entry.ok_or(UiManifestError::MissingEntry)?;
    if matches!(entry, "jaso_init" | "jaso_start" | "jaso_stop") {
        return Err(UiManifestError::ReservedEntry);
    }
    if !exports.any(|name| name == entry) {
        return Err(UiManifestError::EntryNotExported);
    }
    if !requires.any(|name| name == UI_PAINT_CAPABILITY) {
        return Err(UiManifestError::CapabilityNotRequested);
    }
    Ok(entry)
}

// BEGIN VERIFIED UI PLUGIN RECT KERNEL
/// A fill is admitted only when nonempty and wholly inside the node; it is rejected, never clipped.
/// 填充只有非空且完全落在节点内才准入；越界拒绝，不裁剪。
pub fn paint_rect_admits(
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    node_width: u32,
    node_height: u32,
) -> bool {
    width > 0
        && height > 0
        && width <= node_width
        && x <= node_width - width
        && height <= node_height
        && y <= node_height - height
}
// END VERIFIED UI PLUGIN RECT KERNEL

// BEGIN VERIFIED UI PLUGIN COUNT KERNEL
/// The command count from an entry's return value, when it fits the buffer.
/// 入口返回值所表示的命令数（须不超过缓冲区容量）。
pub fn paint_count_admits(returned: i32, capacity: u32) -> Option<u32> {
    if returned < 0 {
        return None;
    }
    let count = returned as u32;
    if count > capacity {
        return None;
    }
    Some(count)
}
// END VERIFIED UI PLUGIN COUNT KERNEL

// BEGIN VERIFIED UI PLUGIN RELOC KERNEL
/// The pointer a base-plus-addend relocation writes; site and target must both lie in the image.
/// 「基址 + 加数」重定位写入的指针；写入位置与目标都必须落在映像内。
pub fn relocation_value(base: u64, offset: u32, addend: u64, image_size: u32) -> Option<u64> {
    if image_size < 8 || offset > image_size - 8 {
        return None;
    }
    if addend > image_size as u64 || addend > u64::MAX - base {
        return None;
    }
    Some(base + addend)
}
// END VERIFIED UI PLUGIN RELOC KERNEL

/// Checks a whole paint output; the first offending command decides the error.
/// 检查整份绘制输出；第一条违规命令决定错误。
pub fn validate_paint_commands(
    commands: &[UiPaintCommandV1],
    node_width: u32,
    node_height: u32,
) -> Result<(), UiPaintError> {
    for (index, command) in commands.iter().enumerate() {
        if command.kind != UI_PAINT_FILL_RECT || command.reserved != [0, 0] {
            return Err(UiPaintError::Malformed(index));
        }
        if !paint_rect_admits(
            command.x,
            command.y,
            command.width,
            command.height,
            node_width,
            node_height,
        ) {
            return Err(UiPaintError::OutOfNode(index));
        }
    }
    Ok(())
}

/// Admits an entry's return value and buffer as a validated command slice.
/// 把入口返回值与缓冲区准入为已校验的命令切片。
pub fn admit_paint_output(
    returned: i32,
    buffer: &[UiPaintCommandV1],
    node_width: u32,
    node_height: u32,
) -> Result<&[UiPaintCommandV1], UiPaintError> {
    let capacity = u32::try_from(buffer.len()).unwrap_or(u32::MAX);
    let count = paint_count_admits(returned, capacity).ok_or(UiPaintError::Count(returned))?;
    let commands = buffer
        .get(..count as usize)
        .ok_or(UiPaintError::Count(returned))?;
    validate_paint_commands(commands, node_width, node_height)?;
    Ok(commands)
}

/// Snaps a logical box to device pixels by rounding both edges; None when it is not representable.
/// 将逻辑框两边取整对齐到设备像素；无法表示时为 None。
pub fn node_pixel_box(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    scale: f64,
) -> Option<UiNodePixelBox> {
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let edge = |value: f64| -> Option<i32> {
        let snapped = (value * scale).round();
        // Exact i32 range; the casts below cannot saturate after this check.
        // 精确的 i32 范围；通过此检查后下面的转换不会饱和。
        (snapped.is_finite() && snapped >= i32::MIN as f64 && snapped <= i32::MAX as f64)
            .then_some(snapped as i32)
    };
    let (x, y, width, height) = (x as f64, y as f64, width as f64, height as f64);
    if width < 0.0 || height < 0.0 {
        return None;
    }
    let (left, top) = (edge(x)?, edge(y)?);
    let (right, bottom) = (edge(x + width)?, edge(y + height)?);
    Some(UiNodePixelBox {
        x: left,
        y: top,
        width: right.checked_sub(left)?.try_into().ok()?,
        height: bottom.checked_sub(top)?.try_into().ok()?,
    })
}
