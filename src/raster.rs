//! Deterministic CPU rasterizer used by the headless backend and visual tests.
//! 软件光栅只实现基础图元；GPU/Vello 后端的语义由同一显示列表约束。

use std::fs;
use std::io;
use std::path::Path;

use crate::geometry::{Point, Rect};
use crate::style::Color;

/// RGBA8 software framebuffer.
#[derive(Clone, Debug)]
pub struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    clip_stack: Vec<Rect>,
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        Self {
            width,
            height,
            pixels: vec![0; width as usize * height as usize * 4],
            clip_stack: Vec::new(),
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn clear(&mut self, color: Color) {
        for pixel in self.pixels.as_chunks_mut::<4>().0 {
            pixel[0] = color.r;
            pixel[1] = color.g;
            pixel[2] = color.b;
            pixel[3] = color.a;
        }
    }

    pub fn push_clip(&mut self, rect: Rect) {
        let clip = self
            .current_clip()
            .map(|current| intersect(current, rect))
            .unwrap_or(rect);
        self.clip_stack.push(clip);
    }

    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
    }

    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        if color.a == 0 {
            return;
        }
        let Some(clip) = self.effective_clip(rect) else {
            return;
        };
        let x0 = clip.x().floor().max(0.0) as i64;
        let y0 = clip.y().floor().max(0.0) as i64;
        let x1 = clip.right().ceil().min(self.width as f32) as i64;
        let y1 = clip.bottom().ceil().min(self.height as f32) as i64;
        for y in y0..y1 {
            for x in x0..x1 {
                self.blend_pixel(x as u32, y as u32, color);
            }
        }
    }

    pub fn stroke_rect(&mut self, rect: Rect, color: Color, width: f32) {
        let width = width.max(0.0);
        if width == 0.0 || color.a == 0 {
            return;
        }
        let w = width.min(rect.width() / 2.0).min(rect.height() / 2.0);
        if w <= 0.0 {
            return;
        }
        self.fill_rect(Rect::new(rect.x(), rect.y(), rect.width(), w), color);
        self.fill_rect(
            Rect::new(rect.x(), rect.bottom() - w, rect.width(), w),
            color,
        );
        self.fill_rect(
            Rect::new(rect.x(), rect.y() + w, w, rect.height() - 2.0 * w),
            color,
        );
        self.fill_rect(
            Rect::new(rect.right() - w, rect.y() + w, w, rect.height() - 2.0 * w),
            color,
        );
    }

    /// Draws a line with a square brush. This is the deterministic software
    /// path; GPU/Canvas backends may use their own anti-aliased line rasterizer.
    pub fn line(&mut self, from: Point, to: Point, color: Color, width: f32) {
        if color.a == 0 || width <= 0.0 {
            return;
        }
        let brush = width.max(1.0);
        let mut x0 = from.x.round() as i64;
        let mut y0 = from.y.round() as i64;
        let x1 = to.x.round() as i64;
        let y1 = to.y.round() as i64;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let steps = dx.max(-dy);
        for _ in 0..=steps {
            self.fill_rect(
                Rect::new(
                    x0 as f32 - brush / 2.0,
                    y0 as f32 - brush / 2.0,
                    brush,
                    brush,
                ),
                color,
            );
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    pub fn pixel(&self, x: u32, y: u32) -> Option<Color> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = ((y as usize * self.width as usize) + x as usize) * 4;
        Some(Color::new(
            self.pixels[index],
            self.pixels[index + 1],
            self.pixels[index + 2],
            self.pixels[index + 3],
        ))
    }

    pub fn to_ppm(&self) -> Vec<u8> {
        let mut out = format!("P6\n{} {}\n255\n", self.width, self.height).into_bytes();
        out.reserve(self.width as usize * self.height as usize * 3);
        for pixel in self.pixels.as_chunks::<4>().0 {
            // PPM has no alpha; composite over black for deterministic output.
            let alpha = pixel[3] as f32 / 255.0;
            out.push((pixel[0] as f32 * alpha).round() as u8);
            out.push((pixel[1] as f32 * alpha).round() as u8);
            out.push((pixel[2] as f32 * alpha).round() as u8);
        }
        out
    }

    pub fn save_ppm(&self, path: impl AsRef<Path>) -> io::Result<()> {
        fs::write(path, self.to_ppm())
    }

    fn current_clip(&self) -> Option<Rect> {
        self.clip_stack.last().copied()
    }

    fn effective_clip(&self, rect: Rect) -> Option<Rect> {
        let canvas = Rect::new(0.0, 0.0, self.width as f32, self.height as f32);
        let base = self
            .current_clip()
            .map(|clip| intersect(clip, canvas))
            .unwrap_or(canvas);
        let clipped = intersect(base, rect);
        if clipped.width() <= 0.0 || clipped.height() <= 0.0 {
            None
        } else {
            Some(clipped)
        }
    }

    fn blend_pixel(&mut self, x: u32, y: u32, color: Color) {
        if x >= self.width || y >= self.height {
            return;
        }
        let index = ((y as usize * self.width as usize) + x as usize) * 4;
        let dst = Color::new(
            self.pixels[index],
            self.pixels[index + 1],
            self.pixels[index + 2],
            self.pixels[index + 3],
        );
        let out = color.blend_over(dst);
        self.pixels[index] = out.r;
        self.pixels[index + 1] = out.g;
        self.pixels[index + 2] = out.b;
        self.pixels[index + 3] = out.a;
    }
}

fn intersect(a: Rect, b: Rect) -> Rect {
    let x0 = a.x().max(b.x());
    let y0 = a.y().max(b.y());
    let x1 = a.right().min(b.right());
    let y1 = a.bottom().min(b.bottom());
    Rect::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_and_clips_rects() {
        let mut canvas = Canvas::new(10, 10);
        canvas.clear(Color::BLACK);
        canvas.push_clip(Rect::new(2.0, 2.0, 4.0, 4.0));
        canvas.fill_rect(Rect::new(0.0, 0.0, 10.0, 10.0), Color::RED);
        assert_eq!(canvas.pixel(0, 0).unwrap(), Color::BLACK);
        assert_eq!(canvas.pixel(3, 3).unwrap(), Color::RED);
        canvas.pop_clip();
        assert_eq!(canvas.pixel(3, 3).unwrap(), Color::RED);
    }

    #[test]
    fn alpha_blends_over_background() {
        let mut canvas = Canvas::new(1, 1);
        canvas.clear(Color::BLACK);
        canvas.fill_rect(
            Rect::new(0.0, 0.0, 1.0, 1.0),
            Color::new(255, 255, 255, 128),
        );
        let pixel = canvas.pixel(0, 0).unwrap();
        assert!((pixel.r as i32 - 128).abs() <= 1);
        assert_eq!(pixel.a, 255);
    }

    #[test]
    fn draws_lines_with_clipping() {
        let mut canvas = Canvas::new(8, 8);
        canvas.clear(Color::BLACK);
        canvas.line(Point::new(1.0, 1.0), Point::new(6.0, 1.0), Color::RED, 1.0);
        assert_eq!(canvas.pixel(3, 1).unwrap(), Color::RED);
        assert_eq!(canvas.pixel(3, 3).unwrap(), Color::BLACK);
        canvas.push_clip(Rect::new(0.0, 0.0, 3.0, 8.0));
        canvas.line(Point::new(1.0, 5.0), Point::new(6.0, 5.0), Color::BLUE, 1.0);
        assert_eq!(canvas.pixel(2, 5).unwrap(), Color::BLUE);
        assert_eq!(canvas.pixel(5, 5).unwrap(), Color::BLACK);
    }
}
