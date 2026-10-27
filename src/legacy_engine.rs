//! Engine pipeline: scene -> layout -> paint -> UiBatch -> backend.

use crate::backend::{BackendError, RenderBackend};
use crate::batch::UiBatch;
use crate::geometry::Size;
use crate::layout::{LayoutEngine, LayoutError, LayoutTree, TaffyLayoutEngine};
use crate::paint::paint;
use crate::scene::Scene;
use crate::style::Color;
use crate::text::TextEngine;

/// Per-frame statistics for diagnostics and tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
    pub scene_version: u64,
    pub layout_boxes: usize,
    pub paint_ops: usize,
    pub batch_bytes: usize,
}

/// Rendering errors visible to the host.
#[derive(Clone, Debug, PartialEq)]
pub enum EngineError {
    Layout(LayoutError),
    Backend(BackendError),
}

impl From<LayoutError> for EngineError {
    fn from(error: LayoutError) -> Self {
        EngineError::Layout(error)
    }
}

impl From<BackendError> for EngineError {
    fn from(error: BackendError) -> Self {
        EngineError::Backend(error)
    }
}

/// Generic engine over a render backend.
pub struct Engine<B: RenderBackend> {
    layout: TaffyLayoutEngine,
    text: Box<dyn TextEngine>,
    backend: B,
    size: Size,
    clear: Color,
}

impl<B: RenderBackend> Engine<B> {
    pub fn new(size: Size, backend: B, text: Box<dyn TextEngine>) -> Self {
        Self {
            layout: TaffyLayoutEngine,
            text,
            backend,
            size,
            clear: Color::TRANSPARENT,
        }
    }

    pub fn with_clear_color(mut self, clear: Color) -> Self {
        self.clear = clear;
        self
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn resize(&mut self, size: Size) {
        self.size = Size::new(size.width.max(1.0), size.height.max(1.0));
        self.backend.resize(self.size);
    }

    /// Runs a complete frame. `UiBatch` is the only path into the backend,
    /// so a future FFI/IPC backend can consume the same bytes.
    pub fn render(&mut self, scene: &Scene) -> Result<FrameStats, EngineError> {
        let layout = self.layout.layout(scene, self.size, self.text.as_ref())?;
        let display = paint(&layout, self.clear);
        let batch = UiBatch::new(display.ops);
        let bytes = batch.encode();

        self.backend.resize(self.size);
        self.backend.submit(&batch)?;
        self.backend.present()?;

        Ok(FrameStats {
            scene_version: layout.scene_version,
            layout_boxes: layout.boxes.len(),
            paint_ops: batch.ops.len(),
            batch_bytes: bytes.len(),
        })
    }

    /// Exposes the layout tree for hit testing/diagnostics without committing a frame.
    pub fn layout(&self, scene: &Scene) -> Result<LayoutTree, LayoutError> {
        self.layout.layout(scene, self.size, self.text.as_ref())
    }
}

