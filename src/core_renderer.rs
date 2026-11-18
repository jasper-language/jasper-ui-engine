//! Minimal Scene to packed CPU RGBA8 renderer for host-independent consumers.
//! 面向宿主无关消费者的最小 Scene → 紧凑 CPU RGBA8 渲染入口。

use crate::backend::{HeadlessBackend, RenderBackend};
use crate::batch::UiBatch;
use crate::geometry::Size;
use crate::layout::{LayoutEngine, LayoutError, TaffyLayoutEngine};
use crate::nostd_prelude::*;
use crate::paint::paint;
use crate::scene::Scene;
use crate::style::Color;
use crate::text::TextEngine;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreRgbaFrame {
    pub width: u32,
    pub height: u32,
    /// Tight, row-major RGBA8 pixels with exactly `width * height * 4` bytes.
    pub pixels: Vec<u8>,
    pub scene_version: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CoreRenderError {
    Layout(LayoutError),
    Backend(crate::backend::BackendError),
}

impl From<LayoutError> for CoreRenderError {
    fn from(error: LayoutError) -> Self {
        Self::Layout(error)
    }
}

impl From<crate::backend::BackendError> for CoreRenderError {
    fn from(error: crate::backend::BackendError) -> Self {
        Self::Backend(error)
    }
}

/// Stateless layout/paint adapter. The host owns presentation and all platform resources.
/// 无状态布局/绘制适配器；呈现与全部平台资源由宿主拥有。
pub struct CoreRgbaRenderer {
    size: Size,
    clear: Color,
    text: Box<dyn TextEngine>,
}

impl CoreRgbaRenderer {
    pub fn new(size: Size, text: Box<dyn TextEngine>) -> Self {
        Self {
            size: Size::new(size.width.max(1.0), size.height.max(1.0)),
            clear: Color::TRANSPARENT,
            text,
        }
    }

    pub fn with_clear_color(mut self, clear: Color) -> Self {
        self.clear = clear;
        self
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn resize(&mut self, size: Size) {
        self.size = Size::new(size.width.max(1.0), size.height.max(1.0));
    }

    pub fn render(&self, scene: &Scene) -> Result<CoreRgbaFrame, CoreRenderError> {
        let layout = TaffyLayoutEngine.layout(scene, self.size, self.text.as_ref())?;
        let batch = UiBatch::new(paint(&layout, self.clear).ops);
        let mut backend = HeadlessBackend::new(
            (self.size.width + 0.5) as u32,
            (self.size.height + 0.5) as u32,
        );
        backend.resize(self.size);
        backend.submit(&batch)?;
        let canvas = backend.canvas();
        Ok(CoreRgbaFrame {
            width: canvas.width(),
            height: canvas.height(),
            pixels: canvas.pixels().to_vec(),
            scene_version: layout.scene_version,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Node, NodeId, Scene};
    use crate::style::Style;
    use crate::text::SimpleTextEngine;

    fn scene() -> Scene {
        Scene::new(
            Node::rect(NodeId(1))
                .with_style(Style::default().size(
                    crate::style::Length::Px(2.0),
                    crate::style::Length::Px(2.0),
                ))
                .child(Node::rect(NodeId(2)).with_style(
                    Style::default()
                        .size(
                            crate::style::Length::Px(1.0),
                            crate::style::Length::Px(1.0),
                        )
                        .background(Color::RED),
                )),
        )
    }

    #[test]
    fn renders_tight_rgba8_without_platform_resources() {
        let renderer = CoreRgbaRenderer::new(Size::new(2.0, 2.0), Box::new(SimpleTextEngine));
        let frame = renderer.render(&scene()).unwrap();
        assert_eq!((frame.width, frame.height), (2, 2));
        assert_eq!(frame.pixels.len(), 16);
        assert!(frame.pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
        assert_eq!(frame.scene_version, 1);
    }
}
