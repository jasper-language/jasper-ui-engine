//! Engine-side geometry of the style effects that have no Blitz CSS equivalent here:
//! the outline ring (E9, with offset), rounded-corner hit exclusion (E2) and hit slop (E10).
//! Pure functions over view-local logical rectangles; no document, no painting.
//! 在此没有 Blitz CSS 对应物的样式效果的引擎侧几何：外框环（E9，含偏移）、圆角命中排除（E2）与命中扩展（E10）。
//! 只是对视图局部逻辑矩形的纯函数；不访问文档、不绘制。
use jasper_ui_engine::geometry::Edges;
use jasper_ui_engine::style::{CornerRadii, Outline};

/// Axis-aligned box in view-local logical px: x, y, width, height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LogicalBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl LogicalBox {
    pub(crate) fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }

    /// Grown by `left/top/right/bottom` (negative shrinks); None if the result is empty.
    pub(crate) fn outset(&self, top: f32, right: f32, bottom: f32, left: f32) -> Option<Self> {
        let out = Self {
            x: self.x - left,
            y: self.y - top,
            width: self.width + left + right,
            height: self.height + top + bottom,
        };
        (out.width > 0.0 && out.height > 0.0 && out.x.is_finite() && out.y.is_finite())
            .then_some(out)
    }

    pub(crate) fn intersect(&self, other: &Self) -> Option<Self> {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = (self.x + self.width).min(other.x + other.width);
        let y1 = (self.y + self.height).min(other.y + other.height);
        (x1 > x0 && y1 > y0).then_some(Self {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        })
    }

    /// Euclidean distance from a point to the box (0 inside).
    pub(crate) fn distance(&self, px: f32, py: f32) -> f32 {
        let dx = (self.x - px).max(px - (self.x + self.width)).max(0.0);
        let dy = (self.y - py).max(py - (self.y + self.height)).max(0.0);
        (dx * dx + dy * dy).sqrt()
    }

    /// Nearest point inside the box, kept `inset` (at most half the extent) away from its edges
    /// so the native hit test lands strictly inside.
    pub(crate) fn clamp_inside(&self, px: f32, py: f32, inset: f32) -> (f32, f32) {
        let ix = inset.min(self.width / 2.0);
        let iy = inset.min(self.height / 2.0);
        (
            px.clamp(self.x + ix, self.x + self.width - ix),
            py.clamp(self.y + iy, self.y + self.height - iy),
        )
    }
}

/// CSS radii after the overlap reduction (CSS Backgrounds 3 §5.5): all radii are scaled by one
/// factor so adjacent radii never exceed their side. Order: TL, TR, BR, BL.
/// CSS 圆角重叠缩减后的半径：所有半径按同一系数缩小，使相邻半径之和不超过所在边。顺序：左上、右上、右下、左下。
pub(crate) fn used_radii(width: f32, height: f32, radii: CornerRadii) -> [f32; 4] {
    let r = [
        radii.top_left,
        radii.top_right,
        radii.bottom_right,
        radii.bottom_left,
    ];
    let mut factor: f32 = 1.0;
    for (sum, side) in [
        (r[0] + r[1], width),
        (r[3] + r[2], width),
        (r[0] + r[3], height),
        (r[1] + r[2], height),
    ] {
        if sum > 0.0 {
            factor = factor.min(side.max(0.0) / sum);
        }
    }
    r.map(|v| (v * factor).max(0.0))
}

/// True when the point is inside the box's rectangle but outside its rounded shape, i.e. in a
/// corner cut away by a radius. A node does not receive hits there (nor, if it clips, do its
/// descendants).
/// 点在盒子的矩形内但在其圆角形状之外（即被圆角切掉的角落）时为真。该节点在此不接收命中（若它裁剪，其后代也不接收）。
pub(crate) fn in_cut_corner(rect: LogicalBox, radii: CornerRadii, px: f32, py: f32) -> bool {
    if radii.is_zero() || !rect.contains(px, py) {
        return false;
    }
    let [tl, tr, br, bl] = used_radii(rect.width, rect.height, radii);
    let (x0, y0) = (rect.x, rect.y);
    let (x1, y1) = (rect.x + rect.width, rect.y + rect.height);
    let outside = |cx: f32, cy: f32, r: f32| {
        let (dx, dy) = (px - cx, py - cy);
        dx * dx + dy * dy > r * r
    };
    (tl > 0.0 && px < x0 + tl && py < y0 + tl && outside(x0 + tl, y0 + tl, tl))
        || (tr > 0.0 && px > x1 - tr && py < y0 + tr && outside(x1 - tr, y0 + tr, tr))
        || (br > 0.0 && px > x1 - br && py > y1 - br && outside(x1 - br, y1 - br, br))
        || (bl > 0.0 && px < x0 + bl && py > y1 - bl && outside(x0 + bl, y1 - bl, bl))
}

/// The slop-extended hit box (layout box grown by `slop`).
pub(crate) fn slop_box(rect: LogicalBox, slop: Edges) -> Option<LogicalBox> {
    rect.outset(slop.top, slop.right, slop.bottom, slop.left)
}

/// Outline ring geometry: (outer box, outer radii, inner box and radii if any). Radii follow the
/// border radii like CSS: a rounded corner grows by the distance from the border edge, a square
/// corner stays square. A negative offset larger than the box leaves no hole (solid fill).
/// 外框环几何：（外框盒、外圆角、内框盒与内圆角（若有））。圆角同 CSS 随边框圆角变化：圆角按离边框边缘的距离增大，
/// 直角保持直角。负偏移超过盒子时没有内洞（实心填充）。
pub(crate) fn outline_ring(
    border_box: LogicalBox,
    radii: CornerRadii,
    outline: Outline,
) -> Option<(LogicalBox, [f32; 4], Option<(LogicalBox, [f32; 4])>)> {
    if !(outline.width > 0.0) || outline.color.a == 0 {
        return None;
    }
    let used = used_radii(border_box.width, border_box.height, radii);
    let grow = |d: f32| used.map(|r| if r > 0.0 { (r + d).max(0.0) } else { 0.0 });
    let outer_d = outline.offset + outline.width;
    let outer = border_box.outset(outer_d, outer_d, outer_d, outer_d)?;
    let inner = border_box
        .outset(
            outline.offset,
            outline.offset,
            outline.offset,
            outline.offset,
        )
        .map(|b| (b, grow(outline.offset)));
    Some((outer, grow(outer_d), inner))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jasper_ui_engine::style::Color;

    const B: LogicalBox = LogicalBox {
        x: 10.0,
        y: 20.0,
        width: 100.0,
        height: 40.0,
    };

    #[test]
    fn radii_are_reduced_together() {
        assert_eq!(
            used_radii(100.0, 40.0, CornerRadii::all(8.0)),
            [8.0, 8.0, 8.0, 8.0]
        );
        // 40 tall box: 30+30 > 40, factor 2/3 for every corner.
        let r = used_radii(100.0, 40.0, CornerRadii::all(30.0));
        assert!(r.iter().all(|v| (v - 20.0).abs() < 1e-4));
    }

    #[test]
    fn cut_corner_only_outside_the_arc() {
        let radii = CornerRadii::new(10.0, 0.0, 10.0, 0.0);
        assert!(in_cut_corner(B, radii, 10.5, 20.5)); // top-left tip
        assert!(!in_cut_corner(B, radii, 17.0, 27.0)); // inside the arc
        assert!(!in_cut_corner(B, radii, 109.5, 20.5)); // square top-right
        assert!(in_cut_corner(B, radii, 109.5, 59.5)); // bottom-right tip
        assert!(!in_cut_corner(B, radii, 60.0, 40.0)); // centre
        assert!(!in_cut_corner(B, radii, 5.0, 20.5)); // outside the box entirely
        assert!(!in_cut_corner(B, CornerRadii::ZERO, 10.5, 20.5));
    }

    #[test]
    fn slop_grows_per_side_and_clamps_back_inside() {
        let slop = slop_box(B, Edges::new(1.0, 2.0, 3.0, 4.0)).unwrap();
        assert_eq!(
            slop,
            LogicalBox {
                x: 6.0,
                y: 19.0,
                width: 106.0,
                height: 44.0
            }
        );
        assert!(slop.contains(7.0, 62.0) && !B.contains(7.0, 62.0));
        let (x, y) = B.clamp_inside(7.0, 62.0, 0.5);
        assert!(B.contains(x, y) && x == 10.5 && y == 59.5);
        assert_eq!(B.distance(7.0, 20.0), 3.0);
        assert_eq!(B.distance(50.0, 30.0), 0.0);
    }

    #[test]
    fn ring_offsets_and_radii() {
        let outline = Outline {
            width: 2.0,
            offset: 3.0,
            color: Color::BLACK,
        };
        let (outer, outer_r, inner) =
            outline_ring(B, CornerRadii::new(4.0, 0.0, 0.0, 0.0), outline).unwrap();
        assert_eq!(
            (outer.x, outer.y, outer.width, outer.height),
            (5.0, 15.0, 110.0, 50.0)
        );
        assert_eq!(outer_r, [9.0, 0.0, 0.0, 0.0]);
        let (inner, inner_r) = inner.unwrap();
        assert_eq!((inner.x, inner.width), (7.0, 106.0));
        assert_eq!(inner_r, [7.0, 0.0, 0.0, 0.0]);
        // Inset ring: negative offset.
        let (outer, _, inner) = outline_ring(
            B,
            CornerRadii::ZERO,
            Outline {
                offset: -4.0,
                ..outline
            },
        )
        .unwrap();
        assert_eq!(outer.x, 12.0);
        assert_eq!(inner.unwrap().0.x, 14.0);
        // Zero width or transparent: nothing.
        assert!(outline_ring(
            B,
            CornerRadii::ZERO,
            Outline {
                width: 0.0,
                ..outline
            }
        )
        .is_none());
        assert!(outline_ring(
            B,
            CornerRadii::ZERO,
            Outline {
                color: Color::TRANSPARENT,
                ..outline
            }
        )
        .is_none());
        // Offset swallowing the box: solid, no hole.
        let (_, _, inner) = outline_ring(
            B,
            CornerRadii::ZERO,
            Outline {
                width: 30.0,
                offset: -25.0,
                ..outline
            },
        )
        .unwrap();
        assert!(inner.is_none());
    }
}
