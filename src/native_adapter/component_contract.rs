//! Rust component update contract, independent of the backend; candidate facade re-exports values.
//! 模块保持私有，经候选库入口导出自有值；不含后端类型，不冻结语言 API。
// BEGIN VERIFIED COMPONENT UPDATE TYPES
#[derive(Clone, Copy)]
pub struct TextAppearance {
    pub font_size_px: u16,
    pub padding_px: u16,
    pub foreground: [u8; 3],
    pub background: [u8; 3],
}

/// Provisional Rust layout update value, not a frozen public subnamespace/API.
#[derive(Clone, Copy)]
pub enum ContentFlow { Row, Column }

/// Provisional synchronously borrowed Rust update contract; no upstream or Signal runtime types.
#[derive(Clone, Copy)]
pub enum ComponentUpdate<'a> {
    AccessibleLabel(Option<&'a str>),
    ButtonCaption(&'a str),
    ButtonDisabled(bool),
    Appearance(TextAppearance),
    Flow { direction: ContentFlow, gap_px: u16 },
}

// END VERIFIED COMPONENT UPDATE TYPES

// BEGIN VERIFIED COMPONENT UPDATE SUPPORT KERNEL
/// Validate own role/value constraints before native mutation.
pub(crate) fn supports_component_update(button: bool, update: ComponentUpdate<'_>) -> bool {
    match update {
        ComponentUpdate::ButtonCaption(_) | ComponentUpdate::ButtonDisabled(_) => button,
        ComponentUpdate::Appearance(appearance) => appearance.font_size_px > 0,
        _ => true,
    }
}
// END VERIFIED COMPONENT UPDATE SUPPORT KERNEL
