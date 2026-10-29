//! Synthetic italic direction for the pinned Blitz painter. Parley reports a synthetic skew
//! angle for a run drawn with an upright face; the painter turns it into a glyph transform
//! `Affine::skew(tan(angle), 0)` that is applied in the y-down scene, so the glyph leans
//! backwards. Every glyph run passes through `UprightSyntheticItalic`, which mirrors the sign
//! of exactly that pure horizontal skew and forwards everything else unchanged.
//! 固定版 Blitz 绘制器的合成斜体方向。Parley 为以直立字面绘制的字形串报告合成倾斜角，
//! 绘制器把它变成字形变换 `Affine::skew(tan(angle), 0)` 并施加于 y 向下的场景，字形因而反倾。
//! 所有字形串经过 `UprightSyntheticItalic`：恰为纯水平斜切时取反其符号，其余原样转发。
use anyrender::{Filter,Glyph,NormalizedCoord,PaintRef,PaintScene,RegisterResourceError,RenderContext,ResourceId};
use peniko::kurbo::{Affine,Rect,Shape,Stroke,Vec2};
use peniko::{BlendMode,Color,Fill,FontData,ImageBrushRef,StyleRef};
use std::any::Any;
use std::sync::Arc;

// BEGIN VERIFIED GLYPH SKEW KERNEL
/// IEEE 754 binary64 bit patterns used by the skew decision.
/// 斜切判定使用的 IEEE 754 binary64 位模式。
pub const F64_ONE_BITS: u64 = 0x3ff0_0000_0000_0000;
pub const F64_SIGN_BIT: u64 = 0x8000_0000_0000_0000;
pub const F64_MAGNITUDE_BITS: u64 = 0x7fff_ffff_ffff_ffff;

/// Takes the `f64::to_bits` of a glyph transform `[a, b, c, d, e, f]` and returns the bits of
/// the horizontal skew `c` to draw with: sign-mirrored when the transform is exactly a pure
/// horizontal skew (a = d = 1, b = e = f = ±0), otherwise unchanged. Other coefficients are
/// never altered.
/// 输入字形变换 `[a, b, c, d, e, f]` 各系数的 `f64::to_bits`，返回应使用的水平斜切 `c` 的位：
/// 变换恰为纯水平斜切（a = d = 1，b = e = f = ±0）时符号取反，否则不变。其余系数从不改动。
pub fn upright_skew_x(a: u64, b: u64, c: u64, d: u64, e: u64, f: u64) -> u64 {
    if a == F64_ONE_BITS
        && (b & F64_MAGNITUDE_BITS) == 0
        && d == F64_ONE_BITS
        && (e & F64_MAGNITUDE_BITS) == 0
        && (f & F64_MAGNITUDE_BITS) == 0
    {
        c ^ F64_SIGN_BIT
    } else {
        c
    }
}
// END VERIFIED GLYPH SKEW KERNEL

/// The glyph transform the scene should receive for one run.
/// 单个字形串应交给场景的字形变换。
fn upright_glyph_transform(transform:Option<Affine>)->Option<Affine> {
    transform.map(|transform| {
        let [a,b,c,d,e,f]=transform.as_coeffs();
        let c=f64::from_bits(upright_skew_x(a.to_bits(),b.to_bits(),c.to_bits(),d.to_bits(),e.to_bits(),f.to_bits()));
        Affine::new([a,b,c,d,e,f])
    })
}

/// Forwarding painter that corrects the synthetic italic direction; nothing else changes.
/// Recorded scene fragments use the provided `append_scene`, so their glyph runs are corrected
/// the same way.
/// 修正合成斜体方向的转发绘制器，其余不变。录制场景片段走默认 `append_scene`，其中字形串同样修正。
pub(crate) struct UprightSyntheticItalic<'p,S:PaintScene>(pub(crate) &'p mut S);

impl<S:PaintScene> RenderContext for UprightSyntheticItalic<'_,S> {
    fn try_register_custom_resource(&mut self,resource:Box<dyn Any>)->Result<ResourceId,RegisterResourceError> {
        self.0.try_register_custom_resource(resource)
    }
    fn unregister_resource(&mut self,resource_id:ResourceId) {self.0.unregister_resource(resource_id)}
    fn renderer_specific_context(&self)->Option<Box<dyn Any>> {self.0.renderer_specific_context()}
}

impl<S:PaintScene> PaintScene for UprightSyntheticItalic<'_,S> {
    fn reset(&mut self) {self.0.reset()}
    fn push_layer(&mut self,fill:Fill,blend:impl Into<BlendMode>,alpha:f32,transform:Affine,clip:&impl Shape,
        filter:Option<Arc<Filter>>,backdrop_filter:Option<Arc<Filter>>) {
        self.0.push_layer(fill,blend,alpha,transform,clip,filter,backdrop_filter)
    }
    fn push_clip_layer(&mut self,fill:Fill,transform:Affine,clip:&impl Shape) {self.0.push_clip_layer(fill,transform,clip)}
    fn pop_layer(&mut self) {self.0.pop_layer()}
    fn stroke<'a>(&mut self,style:&Stroke,transform:Affine,brush:impl Into<PaintRef<'a>>,brush_transform:Option<Affine>,shape:&impl Shape) {
        self.0.stroke(style,transform,brush,brush_transform,shape)
    }
    fn fill<'a>(&mut self,style:Fill,transform:Affine,brush:impl Into<PaintRef<'a>>,brush_transform:Option<Affine>,shape:&impl Shape) {
        self.0.fill(style,transform,brush,brush_transform,shape)
    }
    fn draw_glyphs<'a,'s:'a>(&'s mut self,font:&'a FontData,font_size:f32,hint:bool,normalized_coords:&'a [NormalizedCoord],
        embolden:Vec2,style:impl Into<StyleRef<'a>>,brush:impl Into<PaintRef<'a>>,brush_alpha:f32,transform:Affine,
        glyph_transform:Option<Affine>,glyphs:impl Iterator<Item=Glyph>+Clone) {
        self.0.draw_glyphs(font,font_size,hint,normalized_coords,embolden,style,brush,brush_alpha,transform,
            upright_glyph_transform(glyph_transform),glyphs)
    }
    fn draw_box_shadow(&mut self,transform:Affine,rect:Rect,brush:Color,radius:f64,std_dev:f64) {
        self.0.draw_box_shadow(transform,rect,brush,radius,std_dev)
    }
    fn draw_image(&mut self,image:ImageBrushRef,transform:Affine) {self.0.draw_image(image,transform)}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_skew_is_mirrored_and_others_pass_through() {
        let skew=Affine::skew(14f64.to_radians().tan(),0.0);
        assert_eq!(upright_glyph_transform(Some(skew)),Some(Affine::skew(-(14f64.to_radians().tan()),0.0)));
        assert_eq!(upright_glyph_transform(None),None);
        let moved=Affine::translate((3.0,0.0))*skew;
        assert_eq!(upright_glyph_transform(Some(moved)),Some(moved));
        let scaled=Affine::scale(2.0)*skew;
        assert_eq!(upright_glyph_transform(Some(scaled)),Some(scaled));
    }
}
