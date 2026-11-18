//! Minimal Scene to packed CPU RGBA8 renderer for host-independent consumers.
//! 面向宿主无关消费者的最小 Scene → 紧凑 CPU RGBA8 渲染入口。

use crate::backend::{submit_native_frame, HeadlessBackend, RenderBackend};
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

    /// Render to a host-owned backend and present only after RGBA adoption succeeds.
    /// 渲染到宿主持有的后端，并只在 RGBA 接管成功后请求呈现。
    pub fn render_to_backend(
        &self,
        scene: &Scene,
        backend: &mut impl RenderBackend,
    ) -> Result<u64, CoreRenderError> {
        let frame = self.render(scene)?;
        let scene_version = frame.scene_version;
        submit_native_frame(backend, frame.width, frame.height, frame.pixels)?;
        Ok(scene_version)
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
                .with_style(
                    Style::default()
                        .size(crate::style::Length::Px(2.0), crate::style::Length::Px(2.0)),
                )
                .child(
                    Node::rect(NodeId(2)).with_style(
                        Style::default()
                            .size(crate::style::Length::Px(1.0), crate::style::Length::Px(1.0))
                            .background(Color::RED),
                    ),
                ),
        )
    }

    #[test]
    fn renders_tight_rgba8_without_platform_resources() {
        let renderer = CoreRgbaRenderer::new(Size::new(2.0, 2.0), Box::new(SimpleTextEngine));
        let frame = renderer.render(&scene()).unwrap();
        assert_eq!((frame.width, frame.height), (2, 2));
        assert_eq!(frame.pixels.len(), 16);
        assert!(frame
            .pixels
            .iter()
            .skip(3)
            .step_by(4)
            .any(|alpha| *alpha != 0));
        assert_eq!(frame.scene_version, 1);
    }

    #[test]
    fn backend_present_follows_successful_rgba_adoption() {
        #[derive(Default)]
        struct RecordingBackend {
            frame: Option<(u32, u32, usize)>,
            present_calls: u32,
            submit_error: Option<crate::backend::BackendError>,
            present_error: Option<crate::backend::BackendError>,
        }

        impl RenderBackend for RecordingBackend {
            fn resize(&mut self, _: Size) {}

            fn submit(&mut self, _: &UiBatch) -> Result<(), crate::backend::BackendError> {
                Err(crate::backend::BackendError::Unsupported("batch"))
            }

            fn submit_rgba8(
                &mut self,
                width: u32,
                height: u32,
                pixels: Vec<u8>,
            ) -> Result<(), crate::backend::BackendError> {
                if let Some(error) = self.submit_error.clone() {
                    return Err(error);
                }
                self.frame = Some((width, height, pixels.len()));
                Ok(())
            }

            fn present(&mut self) -> Result<(), crate::backend::BackendError> {
                self.present_calls += 1;
                match self.present_error.clone() {
                    Some(error) => Err(error),
                    None => Ok(()),
                }
            }
        }

        let renderer = CoreRgbaRenderer::new(Size::new(2.0, 2.0), Box::new(SimpleTextEngine));
        let mut backend = RecordingBackend::default();
        assert_eq!(
            renderer.render_to_backend(&scene(), &mut backend).unwrap(),
            1
        );
        assert_eq!(backend.frame, Some((2, 2, 16)));
        assert_eq!(backend.present_calls, 1);
    }

    #[test]
    fn backend_submit_failure_does_not_present() {
        struct RejectingBackend {
            present_calls: u32,
        }

        impl RenderBackend for RejectingBackend {
            fn resize(&mut self, _: Size) {}

            fn submit(&mut self, _: &UiBatch) -> Result<(), crate::backend::BackendError> {
                Err(crate::backend::BackendError::Unsupported("batch"))
            }

            fn submit_rgba8(
                &mut self,
                _: u32,
                _: u32,
                _: Vec<u8>,
            ) -> Result<(), crate::backend::BackendError> {
                Err(crate::backend::BackendError::Unsupported(
                    "native RGBA8 frame submission",
                ))
            }

            fn present(&mut self) -> Result<(), crate::backend::BackendError> {
                self.present_calls += 1;
                Ok(())
            }
        }

        let renderer = CoreRgbaRenderer::new(Size::new(2.0, 2.0), Box::new(SimpleTextEngine));
        let mut backend = RejectingBackend { present_calls: 0 };
        assert_eq!(
            renderer.render_to_backend(&scene(), &mut backend),
            Err(CoreRenderError::Backend(
                crate::backend::BackendError::Unsupported("native RGBA8 frame submission")
            ))
        );
        assert_eq!(backend.present_calls, 0);
    }

    #[test]
    fn backend_present_failure_is_reported_after_adoption() {
        struct FailingPresentBackend {
            adopted_len: Option<usize>,
        }

        impl RenderBackend for FailingPresentBackend {
            fn resize(&mut self, _: Size) {}

            fn submit(&mut self, _: &UiBatch) -> Result<(), crate::backend::BackendError> {
                Err(crate::backend::BackendError::Unsupported("batch"))
            }

            fn submit_rgba8(
                &mut self,
                _: u32,
                _: u32,
                pixels: Vec<u8>,
            ) -> Result<(), crate::backend::BackendError> {
                self.adopted_len = Some(pixels.len());
                Ok(())
            }

            fn present(&mut self) -> Result<(), crate::backend::BackendError> {
                Err(crate::backend::BackendError::Io(
                    "host presentation failed".into(),
                ))
            }
        }

        let renderer = CoreRgbaRenderer::new(Size::new(2.0, 2.0), Box::new(SimpleTextEngine));
        let mut backend = FailingPresentBackend { adopted_len: None };
        assert_eq!(
            renderer.render_to_backend(&scene(), &mut backend),
            Err(CoreRenderError::Backend(crate::backend::BackendError::Io(
                "host presentation failed".into()
            )))
        );
        assert_eq!(backend.adopted_len, Some(16));
    }
}
