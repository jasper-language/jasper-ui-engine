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

/// Default packed-RGBA8 budget for the minimal CPU renderer.
pub const DEFAULT_CORE_FRAME_BYTE_BUDGET: usize = 64 * 1024 * 1024;

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
    InvalidFrameSize,
    FrameBudgetExceeded {
        width: u32,
        height: u32,
        required_bytes: u64,
        max_bytes: usize,
    },
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
    max_frame_bytes: usize,
    text: Box<dyn TextEngine>,
}

fn normalize_finite_dimension(value: f32) -> f32 {
    if value.is_finite() {
        value.max(1.0)
    } else {
        value
    }
}

impl CoreRgbaRenderer {
    pub fn new(size: Size, text: Box<dyn TextEngine>) -> Self {
        Self {
            size: Size::new(
                normalize_finite_dimension(size.width),
                normalize_finite_dimension(size.height),
            ),
            clear: Color::TRANSPARENT,
            max_frame_bytes: DEFAULT_CORE_FRAME_BYTE_BUDGET,
            text,
        }
    }

    pub fn with_clear_color(mut self, clear: Color) -> Self {
        self.clear = clear;
        self
    }

    pub fn with_max_frame_bytes(mut self, max_frame_bytes: usize) -> Self {
        self.max_frame_bytes = max_frame_bytes;
        self
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn resize(&mut self, size: Size) {
        self.size = Size::new(
            normalize_finite_dimension(size.width),
            normalize_finite_dimension(size.height),
        );
    }

    pub fn render(&self, scene: &Scene) -> Result<CoreRgbaFrame, CoreRenderError> {
        let (width, height) = self.packed_frame_extent()?;
        let layout = TaffyLayoutEngine.layout(scene, self.size, self.text.as_ref())?;
        let batch = UiBatch::new(paint(&layout, self.clear).ops);
        let mut backend = HeadlessBackend::new(width, height);
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

    fn packed_frame_extent(&self) -> Result<(u32, u32), CoreRenderError> {
        if !self.size.width.is_finite()
            || !self.size.height.is_finite()
            || self.size.width <= 0.0
            || self.size.height <= 0.0
            || self.size.width > 65535.0
            || self.size.height > 65535.0
        {
            return Err(CoreRenderError::InvalidFrameSize);
        }
        let width = (self.size.width + 0.5) as u32;
        let height = (self.size.height + 0.5) as u32;
        let required_bytes = u64::from(width) * u64::from(height) * 4;
        if required_bytes > self.max_frame_bytes as u64 {
            return Err(CoreRenderError::FrameBudgetExceeded {
                width,
                height,
                required_bytes,
                max_bytes: self.max_frame_bytes,
            });
        }
        Ok((width, height))
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
    fn rejects_non_finite_and_oversized_frames_before_allocation() {
        let scene = scene();
        for size in [
            Size::new(f32::INFINITY, 2.0),
            Size::new(2.0, f32::NEG_INFINITY),
            Size::new(f32::NAN, 2.0),
            Size::new(65536.0, 2.0),
        ] {
            let renderer = CoreRgbaRenderer::new(size, Box::new(SimpleTextEngine));
            assert_eq!(
                renderer.render(&scene),
                Err(CoreRenderError::InvalidFrameSize)
            );
        }

        let renderer = CoreRgbaRenderer::new(Size::new(2.0, 2.0), Box::new(SimpleTextEngine))
            .with_max_frame_bytes(15);
        assert_eq!(
            renderer.render(&scene),
            Err(CoreRenderError::FrameBudgetExceeded {
                width: 2,
                height: 2,
                required_bytes: 16,
                max_bytes: 15,
            })
        );

        let mut renderer = CoreRgbaRenderer::new(Size::new(2.0, 2.0), Box::new(SimpleTextEngine))
            .with_max_frame_bytes(16);
        assert!(renderer.render(&scene).is_ok());
        renderer.resize(Size::new(f32::INFINITY, 2.0));
        assert_eq!(
            renderer.render(&scene),
            Err(CoreRenderError::InvalidFrameSize)
        );
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
