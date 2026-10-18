//! Minimal platform host contract.
//! 窗口/DPI/重绘由宿主适配；内置宿主和自带宿主分别实现，不把 winit 变成引擎硬依赖。

use crate::geometry::Size;

/// Mouse cursor shape request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cursor {
    #[default]
    Default,
    Pointer,
    Text,
    Crosshair,
    ResizeHorizontal,
    ResizeVertical,
}

/// Host-side window capabilities needed by the engine.
pub trait PlatformHost {
    fn size(&self) -> Size;
    fn scale_factor(&self) -> f32;
    fn request_redraw(&mut self);
    fn set_cursor(&mut self, _cursor: Cursor) {}
}

/// Headless host used by tests and image export.
#[derive(Clone, Debug)]
pub struct HeadlessHost {
    size: Size,
    scale_factor: f32,
    redraw_requests: u32,
    cursor: Cursor,
}

impl HeadlessHost {
    pub fn new(size: Size) -> Self {
        Self {
            size,
            scale_factor: 1.0,
            redraw_requests: 0,
            cursor: Cursor::Default,
        }
    }

    pub fn redraw_requests(&self) -> u32 {
        self.redraw_requests
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }
}

impl PlatformHost for HeadlessHost {
    fn size(&self) -> Size {
        self.size
    }

    fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    fn request_redraw(&mut self) {
        self.redraw_requests += 1;
    }

    fn set_cursor(&mut self, cursor: Cursor) {
        self.cursor = cursor;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_redraw_and_cursor() {
        let mut host = HeadlessHost::new(Size::new(320.0, 200.0));
        host.request_redraw();
        host.set_cursor(Cursor::Pointer);
        assert_eq!(host.redraw_requests(), 1);
        assert_eq!(host.cursor(), Cursor::Pointer);
    }
}
