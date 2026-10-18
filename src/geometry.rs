//! Geometric primitives shared by layout, paint, input and rasterization.
//! 几何基础类型；全部使用 f32 逻辑像素，物理像素比例由宿主负责。

/// A 2D point in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A width/height pair in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
}

/// An axis-aligned rectangle. `origin` is the top-left corner.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub const ZERO: Self = Self {
        origin: Point::ZERO,
        size: Size::ZERO,
    };

    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    pub fn from_origin_size(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    pub fn x(self) -> f32 {
        self.origin.x
    }
    pub fn y(self) -> f32 {
        self.origin.y
    }
    pub fn width(self) -> f32 {
        self.size.width
    }
    pub fn height(self) -> f32 {
        self.size.height
    }
    pub fn right(self) -> f32 {
        self.origin.x + self.size.width
    }
    pub fn bottom(self) -> f32 {
        self.origin.y + self.size.height
    }

    pub fn contains(self, point: Point) -> bool {
        point.x >= self.origin.x
            && point.y >= self.origin.y
            && point.x < self.right()
            && point.y < self.bottom()
    }

    pub fn intersects(self, other: Rect) -> bool {
        self.origin.x < other.right()
            && self.right() > other.origin.x
            && self.origin.y < other.bottom()
            && self.bottom() > other.origin.y
    }

    /// Shrinks the rectangle by the given edges.
    pub fn inset(self, edges: Edges) -> Self {
        let width = (self.size.width - edges.horizontal()).max(0.0);
        let height = (self.size.height - edges.vertical()).max(0.0);
        Self::new(
            self.origin.x + edges.left,
            self.origin.y + edges.top,
            width,
            height,
        )
    }

    pub fn offset(self, dx: f32, dy: f32) -> Self {
        Self::new(
            self.origin.x + dx,
            self.origin.y + dy,
            self.size.width,
            self.size.height,
        )
    }
}

/// Four-sided spacing, used for padding, margin and border.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edges {
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    pub const fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    pub fn horizontal(self) -> f32 {
        self.left + self.right
    }
    pub fn vertical(self) -> f32 {
        self.top + self.bottom
    }
    pub fn size(self) -> Size {
        Size::new(self.horizontal(), self.vertical())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_and_intersects() {
        let r = Rect::new(10.0, 10.0, 20.0, 10.0);
        assert!(r.contains(Point::new(10.0, 10.0)));
        assert!(!r.contains(Point::new(30.0, 10.0)));
        assert!(r.intersects(Rect::new(20.0, 15.0, 20.0, 20.0)));
        assert!(!r.intersects(Rect::new(30.0, 0.0, 5.0, 5.0)));
    }

    #[test]
    fn inset_respects_edges() {
        let r = Rect::new(0.0, 0.0, 100.0, 50.0).inset(Edges::new(5.0, 10.0, 5.0, 10.0));
        assert_eq!(r, Rect::new(10.0, 5.0, 80.0, 40.0));
    }
}
