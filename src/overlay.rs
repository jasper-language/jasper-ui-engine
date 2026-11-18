//! Scene overlay layer (issue #1 P0-A "layering, clipping and overlays"): the engine-neutral
//! declaration a Framework attaches to a node (`Style::overlay`) to lift it out of its parent's
//! overflow clip and paint order into an overlay layer, anchor it to another node, a point or the
//! viewport, keep it inside the viewport (flip, shift, resize), and give it modal, dismiss and
//! focus behaviour; plus the pure placement algorithm and the public event/query values.
//!
//! Nothing here depends on Blitz or any backend type: the native adapter consumes these values
//! (`native_adapter/scene_overlay.rs`), the Framework only produces and receives them. The engine
//! never closes an overlay by itself: outside presses, Escape and a lost anchor are reported as
//! `OverlayEvent`s (`dismiss-request`) and the Framework decides.
//!
//! 场景浮层层（issue #1 P0-A「层叠、裁剪与浮层」）：Framework 挂在节点上的引擎中立声明（`Style::overlay`），
//! 使节点脱离父节点的 overflow 裁剪与绘制顺序、进入浮层层，锚定到另一节点、一个点或视口，保持在视口内（翻转、平移、
//! 限高），并带模态、关闭请求与焦点行为；另含纯函数定位算法与对外事件/查询值。
//! 本模块不依赖 Blitz 或任何后端类型：原生适配层消费这些值，Framework 只产生与接收它们。引擎从不自行关闭浮层：
//! 外部按下、Escape 与锚点丢失都作为 `OverlayEvent`（`dismiss-request`）报告，由 Framework 决定。
use crate::nostd_prelude::*;
use crate::geometry::Rect;
use crate::scene::NodeId;
use crate::style::Color;

/// Largest accepted `|layer|`.
/// `layer` 绝对值上限。
pub const MAX_OVERLAY_LAYER: i32 = 1_000_000;
/// Largest accepted offset / margin magnitude (logical px), same bound as other effects.
/// 偏移/边距绝对值上限（逻辑像素），与其他效果相同。
pub const MAX_OVERLAY_EXTENT: f32 = crate::style::MAX_EFFECT_EXTENT;
/// Largest accepted `|x|`, `|y|` of a point anchor (logical px).
/// 点锚坐标绝对值上限（逻辑像素）。
pub const MAX_OVERLAY_POINT: f32 = 1_000_000.0;

/// What an overlay is positioned against.
/// 浮层定位所依据的对象。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OverlayAnchor {
    /// Centred in the viewport (dialogs, sheets); `offset` moves it (main = y, cross = x).
    /// 视口居中（对话框、面板）；`offset` 平移（主轴为 y，交叉轴为 x）。
    Viewport,
    /// The border box of a scene node (popover, menu, select, tooltip). The node's layout box
    /// (`node_bounds`, with ancestor scrolling) is used; its E8 transform is not.
    /// 场景节点的边框盒（弹出框、菜单、选择框、提示）。使用其布局盒（`node_bounds`，含祖先滚动），不含 E8 变换。
    Node(NodeId),
    /// A zero-size rectangle at a view-local logical point (context menus).
    /// 视图局部逻辑坐标处的零尺寸矩形（上下文菜单）。
    Point { x: f32, y: f32 },
}

/// Preferred side of the anchor the overlay is placed on.
/// 浮层优先放置在锚点的哪一侧。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverlaySide {
    Top,
    #[default]
    Bottom,
    Left,
    Right,
}

impl OverlaySide {
    /// The opposite side (flip target).
    /// 对侧（翻转目标）。
    pub fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
    fn vertical(self) -> bool {
        matches!(self, Self::Top | Self::Bottom)
    }
    /// Protocol spelling.
    /// 协议拼写。
    pub fn name(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

/// Alignment along the anchor edge (cross axis): start = left/top edges aligned.
/// 沿锚点边（交叉轴）的对齐：start 为左/上边对齐。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverlayAlign {
    #[default]
    Start,
    Center,
    End,
}

/// Viewport collision avoidance; all on by default.
/// 视口边界避让；默认全部开启。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverlayAvoid {
    /// Move to the opposite side when the preferred side is too small and the opposite is larger.
    /// 优先侧空间不足且对侧更大时翻到对侧。
    pub flip: bool,
    /// Slide along the cross axis to stay inside the viewport.
    /// 沿交叉轴平移以留在视口内。
    pub shift: bool,
    /// Limit the size to the available space (`max-height`/`max-width`); the overlay's own
    /// `overflow` decides whether the rest scrolls or is clipped.
    /// 尺寸限制为可用空间（`max-height`/`max-width`）；超出部分滚动还是裁剪由浮层自身 `overflow` 决定。
    pub resize: bool,
}

impl Default for OverlayAvoid {
    fn default() -> Self {
        Self {
            flip: true,
            shift: true,
            resize: true,
        }
    }
}

/// Which gestures produce a `dismiss-request`; both on by default. The engine never closes.
/// 哪些操作产生 `dismiss-request`；默认都开启。引擎从不自行关闭。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverlayDismiss {
    pub outside: bool,
    pub escape: bool,
}

impl Default for OverlayDismiss {
    fn default() -> Self {
        Self {
            outside: true,
            escape: true,
        }
    }
}

/// Focus behaviour. Default: restore only. `modal` implies `trap`.
/// 焦点行为。默认只恢复。`modal` 隐含 `trap`。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverlayFocus {
    /// Tab / Shift+Tab cycle inside the overlay subtree.
    /// Tab / Shift+Tab 在浮层子树内循环。
    pub trap: bool,
    /// When the overlay closes while focus is inside it, focus returns to the node focused
    /// before it opened (if still focusable).
    /// 浮层关闭时若焦点在其内，焦点回到打开前聚焦的节点（若仍可聚焦）。
    pub restore: bool,
    /// When the overlay opens, focus its first focusable node (pre-order, root included).
    /// 浮层打开时聚焦其第一个可聚焦节点（先序，含根）。
    pub auto: bool,
}

impl Default for OverlayFocus {
    fn default() -> Self {
        Self {
            trap: false,
            restore: true,
            auto: false,
        }
    }
}

/// What a press outside a non-modal overlay does to the content under it (modal overlays always
/// block). Either way a `dismiss-request` is reported when `dismiss.outside` is on.
/// 非模态浮层外部按下对其下内容的作用（模态浮层总是阻断）。两种情况下 `dismiss.outside` 开启时都会报告 `dismiss-request`。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutsidePress {
    /// The press also reaches the content under it (web popover light dismiss).
    /// 按下同时到达其下内容（网页 popover 轻关闭）。
    #[default]
    PassThrough,
    /// The press is consumed (desktop menus).
    /// 按下被吞掉（桌面菜单）。
    Block,
}

/// Overlay declaration on a node (`Style::overlay`). The node and its subtree leave the parent's
/// clip, opacity, transform and paint order and are painted in overlay layer order: by `layer`
/// (higher on top), then source pre-order. Layout of the subtree is unchanged (the overlay root
/// is absolutely positioned with its own size); semantics, events and focus keep source identity.
/// 节点上的浮层声明（`Style::overlay`）。节点及其子树脱离父节点的裁剪、透明度、变换与绘制顺序，按浮层层次绘制：
/// 先按 `layer`（越大越靠上），再按源先序。子树布局不变（浮层根绝对定位、保持自身尺寸）；语义、事件与焦点保持源身份。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlaySpec {
    pub layer: i32,
    pub anchor: OverlayAnchor,
    pub side: OverlaySide,
    pub align: OverlayAlign,
    /// Gap between anchor and overlay along the side (main axis), logical px.
    /// 锚点与浮层沿放置方向（主轴）的间距，逻辑像素。
    pub offset: f32,
    /// Extra shift along the alignment (cross) axis, logical px.
    /// 沿对齐（交叉）轴的额外偏移，逻辑像素。
    pub cross_offset: f32,
    pub avoid: OverlayAvoid,
    /// Viewport inset kept free by avoidance, logical px (>= 0).
    /// 避让时保留的视口内边距，逻辑像素（>= 0）。
    pub margin: f32,
    /// Overlay at least as wide as its node anchor (select dropdowns).
    /// 浮层至少与节点锚点同宽（下拉选择）。
    pub match_anchor_width: bool,
    /// Blocks pointer input to everything painted below it, makes it inert for focus /
    /// activation / accessibility actions, traps focus, and is `modal` in AccessKit.
    /// 阻断其下所有内容的指针输入，使其对焦点/激活/无障碍动作惰性，陷阱焦点，AccessKit 中为 `modal`。
    pub modal: bool,
    /// Viewport-covering fill painted right below the overlay (any overlay may have one).
    /// 紧贴浮层之下、覆盖视口的遮罩填充（任何浮层都可设置）。
    pub scrim: Option<Color>,
    pub dismiss: OverlayDismiss,
    pub outside_press: OutsidePress,
    pub focus: OverlayFocus,
}

impl Default for OverlaySpec {
    fn default() -> Self {
        Self {
            layer: 0,
            anchor: OverlayAnchor::Viewport,
            side: OverlaySide::Bottom,
            align: OverlayAlign::Start,
            offset: 0.0,
            cross_offset: 0.0,
            avoid: OverlayAvoid::default(),
            margin: 0.0,
            match_anchor_width: false,
            modal: false,
            scrim: None,
            dismiss: OverlayDismiss::default(),
            outside_press: OutsidePress::PassThrough,
            focus: OverlayFocus::default(),
        }
    }
}

impl OverlaySpec {
    /// Anchored to a node, below it, start-aligned.
    /// 锚定到节点，位于其下方、起始对齐。
    pub fn anchored(anchor: NodeId) -> Self {
        Self {
            anchor: OverlayAnchor::Node(anchor),
            ..Self::default()
        }
    }
    /// Centred in the viewport.
    /// 视口居中。
    pub fn viewport() -> Self {
        Self::default()
    }
    /// A modal dialog: viewport-centred, modal, trapping, auto-focusing and restoring focus.
    /// 模态对话框：视口居中、模态、焦点陷阱、自动聚焦并恢复焦点。
    pub fn modal_dialog(scrim: Option<Color>) -> Self {
        Self {
            modal: true,
            scrim,
            focus: OverlayFocus {
                trap: true,
                restore: true,
                auto: true,
            },
            ..Self::default()
        }
    }
    pub fn layer(mut self, layer: i32) -> Self {
        self.layer = layer;
        self
    }
    pub fn place(mut self, side: OverlaySide, align: OverlayAlign) -> Self {
        self.side = side;
        self.align = align;
        self
    }
    pub fn offset(mut self, main: f32, cross: f32) -> Self {
        self.offset = main;
        self.cross_offset = cross;
        self
    }
    pub fn margin(mut self, margin: f32) -> Self {
        self.margin = margin;
        self
    }
    /// Focus is trapped (explicitly or by `modal`).
    /// 焦点被陷阱（显式或由 `modal` 隐含）。
    pub fn traps_focus(&self) -> bool {
        self.modal || self.focus.trap
    }
    /// Outside presses are consumed (explicitly or by `modal`).
    /// 外部按下被吞掉（显式或由 `modal` 隐含）。
    pub fn blocks_outside(&self) -> bool {
        self.modal || self.outside_press == OutsidePress::Block
    }
    /// Range validation; the native path rejects the whole frame (no clamping) otherwise.
    /// Scene-dependent rules (anchor not inside the overlay, no anchor cycle, not the root)
    /// are checked when the frame is built.
    /// 取值校验；不通过时原生路径整帧拒绝（不钳制）。依赖场景的规则（锚点不在浮层内、无锚定环、非根节点）在构建帧时检查。
    pub fn valid(&self) -> bool {
        let signed = |v: f32| v.is_finite() && v.abs() <= MAX_OVERLAY_EXTENT;
        self.layer.unsigned_abs() <= MAX_OVERLAY_LAYER as u32
            && signed(self.offset)
            && signed(self.cross_offset)
            && self.margin.is_finite()
            && (0.0..=MAX_OVERLAY_EXTENT).contains(&self.margin)
            && match self.anchor {
                OverlayAnchor::Point { x, y } => {
                    x.is_finite()
                        && y.is_finite()
                        && x.abs() <= MAX_OVERLAY_POINT
                        && y.abs() <= MAX_OVERLAY_POINT
                }
                _ => true,
            }
    }
}

// ---------------------------------------------------------------------------------------------
// Placement (pure)
// 定位（纯函数）
// ---------------------------------------------------------------------------------------------

/// Where an overlay ended up, in view-local logical px.
/// 浮层最终位置，视图局部逻辑像素。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayPlacement {
    /// Final border box (size after any resize).
    /// 最终边框盒（含限高/限宽后的尺寸）。
    pub rect: Rect,
    /// Side actually used (after a flip); `Bottom` for viewport anchors.
    /// 实际使用的一侧（翻转后）；视口锚点为 `Bottom`。
    pub side: OverlaySide,
    pub flipped: bool,
    /// Moved along the cross axis (or into the viewport) by avoidance.
    /// 因避让沿交叉轴（或向视口内）移动过。
    pub shifted: bool,
    /// Height / width limit applied by `resize` (None: natural size fits).
    /// `resize` 施加的高/宽限制（None：自然尺寸放得下）。
    pub max_height: Option<f32>,
    pub max_width: Option<f32>,
}

/// Places an overlay of natural border-box size `size` (logical px). `anchor` is None for a
/// viewport anchor, else the anchor rectangle; `viewport` is the visible area. Deterministic and
/// total for finite inputs (non-finite inputs yield a placement at the viewport origin).
/// 放置自然边框盒尺寸为 `size`（逻辑像素）的浮层。视口锚点时 `anchor` 为 None，否则为锚矩形；`viewport` 为可见区域。
/// 对有限输入确定且全定义（非有限输入时放在视口原点）。
pub fn place_overlay(
    anchor: Option<Rect>,
    size: (f32, f32),
    viewport: Rect,
    spec: &OverlaySpec,
) -> OverlayPlacement {
    let finite = |r: Rect| [r.x(), r.y(), r.width(), r.height()].iter().all(|v| v.is_finite());
    let (w, h) = (
        if size.0.is_finite() { size.0.max(0.0) } else { 0.0 },
        if size.1.is_finite() { size.1.max(0.0) } else { 0.0 },
    );
    if !finite(viewport) || anchor.is_some_and(|a| !finite(a)) {
        return OverlayPlacement {
            rect: Rect::new(0.0, 0.0, w, h),
            side: spec.side,
            flipped: false,
            shifted: false,
            max_height: None,
            max_width: None,
        };
    }
    // Usable area: viewport minus the margin, never negative (collapses to its centre line).
    // 可用区域：视口减边距，不为负（退化为中线）。
    let m = spec.margin.max(0.0);
    let (vx0, vx1) = shrink(viewport.x(), viewport.right(), m);
    let (vy0, vy1) = shrink(viewport.y(), viewport.bottom(), m);
    let Some(a) = anchor else {
        // Viewport anchor: centred, then offset; resize/shift keep it inside.
        // 视口锚点：居中后偏移；限高与平移使其留在视口内。
        let (fw, max_width) = fit(w, vx1 - vx0, spec.avoid.resize);
        let (fh, max_height) = fit(h, vy1 - vy0, spec.avoid.resize);
        let x = vx0 + (vx1 - vx0 - fw) / 2.0 + spec.cross_offset;
        let y = vy0 + (vy1 - vy0 - fh) / 2.0 + spec.offset;
        let (cx, sx) = clamp_axis(x, fw, vx0, vx1, spec.avoid.shift);
        let (cy, sy) = clamp_axis(y, fh, vy0, vy1, spec.avoid.shift);
        return OverlayPlacement {
            rect: Rect::new(cx, cy, fw, fh),
            side: OverlaySide::Bottom,
            flipped: false,
            shifted: sx || sy,
            max_height,
            max_width,
        };
    };
    // Main-axis free space on each side of the anchor (after the offset gap).
    // 锚点各侧主轴可用空间（减去偏移间距）。
    let space = |side: OverlaySide| -> f32 {
        match side {
            OverlaySide::Bottom => vy1 - (a.bottom() + spec.offset),
            OverlaySide::Top => (a.y() - spec.offset) - vy0,
            OverlaySide::Right => vx1 - (a.right() + spec.offset),
            OverlaySide::Left => (a.x() - spec.offset) - vx0,
        }
    };
    let preferred = spec.side;
    let main_size = if preferred.vertical() { h } else { w };
    let mut side = preferred;
    let mut flipped = false;
    if spec.avoid.flip
        && main_size > space(preferred)
        && space(preferred.opposite()) > space(preferred)
    {
        side = preferred.opposite();
        flipped = true;
    }
    let avail = space(side).max(0.0);
    let (main, main_limit) = fit(main_size, avail, spec.avoid.resize);
    // Cross axis: aligned to the anchor edge, offset, then shifted into the viewport.
    // 交叉轴：按锚边对齐、偏移，再平移进视口。
    let (cross_lo, cross_len, cross_size, v_lo, v_hi) = if side.vertical() {
        (a.x(), a.width(), w, vx0, vx1)
    } else {
        (a.y(), a.height(), h, vy0, vy1)
    };
    let (cross, cross_limit) = fit(cross_size, v_hi - v_lo, spec.avoid.resize);
    let aligned = match spec.align {
        OverlayAlign::Start => cross_lo,
        OverlayAlign::Center => cross_lo + (cross_len - cross) / 2.0,
        OverlayAlign::End => cross_lo + cross_len - cross,
    } + spec.cross_offset;
    let (cross_pos, shifted) = clamp_axis(aligned, cross, v_lo, v_hi, spec.avoid.shift);
    let main_pos = match side {
        OverlaySide::Bottom => a.bottom() + spec.offset,
        OverlaySide::Top => a.y() - spec.offset - main,
        OverlaySide::Right => a.right() + spec.offset,
        OverlaySide::Left => a.x() - spec.offset - main,
    };
    let (rect, max_height, max_width) = if side.vertical() {
        (
            Rect::new(cross_pos, main_pos, cross, main),
            main_limit,
            cross_limit,
        )
    } else {
        (
            Rect::new(main_pos, cross_pos, main, cross),
            cross_limit,
            main_limit,
        )
    };
    OverlayPlacement {
        rect,
        side,
        flipped,
        shifted,
        max_height,
        max_width,
    }
}

/// `[lo + m, hi - m]`, collapsed to the midpoint when the margin is larger than half.
fn shrink(lo: f32, hi: f32, m: f32) -> (f32, f32) {
    if hi - lo >= 2.0 * m {
        (lo + m, hi - m)
    } else {
        let mid = (lo + hi) / 2.0;
        (mid, mid)
    }
}

/// Size limited to `avail` when resizing is on and it does not fit.
fn fit(size: f32, avail: f32, resize: bool) -> (f32, Option<f32>) {
    let avail = avail.max(0.0);
    if resize && size > avail {
        (avail, Some(avail))
    } else {
        (size, None)
    }
}

/// Clamp `[pos, pos + len]` into `[lo, hi]` when shifting (start edge wins if too long).
fn clamp_axis(pos: f32, len: f32, lo: f32, hi: f32, shift: bool) -> (f32, bool) {
    if !shift {
        return (pos, false);
    }
    let clamped = pos.min(hi - len).max(lo);
    (clamped, clamped != pos)
}

// ---------------------------------------------------------------------------------------------
// Events and queries
// 事件与查询
// ---------------------------------------------------------------------------------------------

/// Why the engine asks the Framework to close an overlay.
/// 引擎请求 Framework 关闭浮层的原因。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DismissReason {
    /// A primary press outside the overlay (and outside overlays anchored inside it).
    /// `on_anchor`: the press was on the overlay's node anchor (toggle buttons usually ignore it).
    /// 浮层外（且不在锚定于其内的浮层中）的主键按下。`on_anchor`：按在浮层的节点锚点上（切换按钮通常忽略）。
    OutsidePress { on_anchor: bool },
    /// Escape whose default the focused node did not prevent.
    /// 聚焦节点未阻止默认行为的 Escape。
    Escape,
    /// The node anchor was removed or is no longer displayed; the overlay is hidden meanwhile.
    /// 节点锚点被删除或不再显示；期间浮层被隐藏。
    AnchorLost,
}

impl DismissReason {
    /// JUI 2 spelling: `event <overlay> dismiss-request <reason>`.
    /// JUI 2 拼写：`event <浮层> dismiss-request <原因>`。
    pub fn protocol_name(self) -> &'static str {
        match self {
            Self::OutsidePress { on_anchor: false } => "outside",
            Self::OutsidePress { on_anchor: true } => "outside-anchor",
            Self::Escape => "escape",
            Self::AnchorLost => "anchor-lost",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayEventKind {
    /// JUI 2 `dismiss-request`; the overlay stays until the Framework removes it.
    /// JUI 2 `dismiss-request`；浮层保留到 Framework 移除为止。
    DismissRequest(DismissReason),
}

/// One overlay event for the Framework (drained with `take_overlay_events`).
/// 交给 Framework 的一条浮层事件（经 `take_overlay_events` 取出）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverlayEvent {
    pub scene_version: u64,
    pub overlay: NodeId,
    pub kind: OverlayEventKind,
}

/// Why an overlay is currently not shown.
/// 浮层当前未显示的原因。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlaySuppression {
    /// Its node anchor is missing or not displayed (a `dismiss-request anchor-lost` was sent).
    /// 节点锚点缺失或未显示（已发送 `dismiss-request anchor-lost`）。
    AnchorLost,
    /// It or a source ancestor has `display: none`.
    /// 自身或某源祖先为 `display: none`。
    NotDisplayed,
}

/// One overlay of the current view, in paint order (`rank` 0 is the lowest).
/// 当前视图的一个浮层，按绘制顺序（`rank` 0 最低）。
#[derive(Clone, Debug, PartialEq)]
pub struct OverlayInfo {
    pub overlay: NodeId,
    pub rank: usize,
    pub spec: OverlaySpec,
    /// None while suppressed.
    /// 被抑制时为 None。
    pub placement: Option<OverlayPlacement>,
    pub suppressed: Option<OverlaySuppression>,
    /// Node focused before the overlay opened (focus-restore target).
    /// 浮层打开前的焦点节点（焦点恢复目标）。
    pub return_focus: Option<NodeId>,
}

// ---------------------------------------------------------------------------------------------
// JUI 2 `xstyle overlay=...`
// ---------------------------------------------------------------------------------------------

/// Parses the `overlay` xstyle value: `none`, or whitespace-separated tokens, each at most once:
/// `layer(<int>)`, `anchor(viewport|<node id>|point,<x>,<y>)`, `side(top|bottom|left|right)`,
/// `align(start|center|end)`, `offset(<main>[,<cross>])`, `avoid(none|flip,shift,resize)`,
/// `margin(<px>)`, `match-width`, `modal`, `scrim(<#color>)`, `dismiss(none|outside,escape)`,
/// `outside(pass|block)`, `focus(none|trap,restore,auto)`. Omitted tokens take the defaults of
/// `OverlaySpec::default()`. Some(None) for `none`; None when malformed or out of range.
/// 解析 `overlay` xstyle 值：`none`，或以空白分隔、每种至多一次的记号（见上）。省略的记号取 `OverlaySpec::default()`。
/// `none` 为 Some(None)；格式错误或越界为 None。
pub fn parse_overlay(value: &str) -> Option<Option<OverlaySpec>> {
    let value = value.trim();
    if value == "none" {
        return Some(None);
    }
    let mut spec = OverlaySpec::default();
    let mut seen: Vec<&str> = Vec::new();
    let mut any = false;
    for token in value.split_whitespace() {
        any = true;
        let (name, args) = match token.find('(') {
            Some(open) => (&token[..open], Some(token[open + 1..].strip_suffix(')')?)),
            None => (token, None),
        };
        if seen.contains(&name) {
            return None;
        }
        seen.push(name);
        match (name, args) {
            ("layer", Some(a)) => {
                if !is_integer(a) {
                    return None;
                }
                spec.layer = a.parse().ok()?;
            }
            ("anchor", Some(a)) => {
                let parts = comma_list(a);
                spec.anchor = match parts.as_slice() {
                    ["viewport"] => OverlayAnchor::Viewport,
                    ["point", x, y] => OverlayAnchor::Point {
                        x: crate::style::parse_number(x)?,
                        y: crate::style::parse_number(y)?,
                    },
                    [id] if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) => {
                        OverlayAnchor::Node(NodeId(id.parse().ok()?))
                    }
                    _ => return None,
                };
            }
            ("side", Some(a)) => {
                spec.side = match a {
                    "top" => OverlaySide::Top,
                    "bottom" => OverlaySide::Bottom,
                    "left" => OverlaySide::Left,
                    "right" => OverlaySide::Right,
                    _ => return None,
                }
            }
            ("align", Some(a)) => {
                spec.align = match a {
                    "start" => OverlayAlign::Start,
                    "center" => OverlayAlign::Center,
                    "end" => OverlayAlign::End,
                    _ => return None,
                }
            }
            ("offset", Some(a)) => match comma_list(a).as_slice() {
                [main] => {
                    spec.offset = crate::style::parse_number(main)?;
                    spec.cross_offset = 0.0;
                }
                [main, cross] => {
                    spec.offset = crate::style::parse_number(main)?;
                    spec.cross_offset = crate::style::parse_number(cross)?;
                }
                _ => return None,
            },
            ("avoid", Some(a)) => {
                let flags = flag_set(a, &["flip", "shift", "resize"])?;
                spec.avoid = OverlayAvoid {
                    flip: flags[0],
                    shift: flags[1],
                    resize: flags[2],
                };
            }
            ("margin", Some(a)) => spec.margin = crate::style::parse_number(a)?,
            ("match-width", None) => spec.match_anchor_width = true,
            ("modal", None) => spec.modal = true,
            ("scrim", Some(a)) => spec.scrim = Some(crate::style::parse_color(a)?),
            ("dismiss", Some(a)) => {
                let flags = flag_set(a, &["outside", "escape"])?;
                spec.dismiss = OverlayDismiss {
                    outside: flags[0],
                    escape: flags[1],
                };
            }
            ("outside", Some(a)) => {
                spec.outside_press = match a {
                    "pass" => OutsidePress::PassThrough,
                    "block" => OutsidePress::Block,
                    _ => return None,
                }
            }
            ("focus", Some(a)) => {
                let flags = flag_set(a, &["trap", "restore", "auto"])?;
                spec.focus = OverlayFocus {
                    trap: flags[0],
                    restore: flags[1],
                    auto: flags[2],
                };
            }
            _ => return None,
        }
    }
    (any && spec.valid()).then_some(Some(spec))
}

/// Canonical `overlay` xstyle value: the tokens that differ from the defaults in a fixed order
/// (`layer(...)` always, so the value is never empty). `parse_overlay` round-trips it.
/// 规范的 `overlay` xstyle 值：与默认值不同的记号按固定顺序输出（总含 `layer(...)`，值不为空）。`parse_overlay` 可往返。
pub fn format_overlay(spec: &OverlaySpec) -> String {
    let d = OverlaySpec::default();
    let n = crate::style::format_number;
    let mut out = vec![format!("layer({})", spec.layer)];
    match spec.anchor {
        OverlayAnchor::Viewport => {}
        OverlayAnchor::Node(id) => out.push(format!("anchor({})", id.0)),
        OverlayAnchor::Point { x, y } => out.push(format!("anchor(point,{},{})", n(x), n(y))),
    }
    if spec.side != d.side {
        out.push(format!("side({})", spec.side.name()));
    }
    if spec.align != d.align {
        out.push(
            match spec.align {
                OverlayAlign::Start => "align(start)",
                OverlayAlign::Center => "align(center)",
                OverlayAlign::End => "align(end)",
            }
            .to_owned(),
        );
    }
    if spec.offset != 0.0 || spec.cross_offset != 0.0 {
        out.push(if spec.cross_offset == 0.0 {
            format!("offset({})", n(spec.offset))
        } else {
            format!("offset({},{})", n(spec.offset), n(spec.cross_offset))
        });
    }
    if spec.avoid != d.avoid {
        out.push(format!(
            "avoid({})",
            flags_text(&[
                ("flip", spec.avoid.flip),
                ("shift", spec.avoid.shift),
                ("resize", spec.avoid.resize)
            ])
        ));
    }
    if spec.margin != 0.0 {
        out.push(format!("margin({})", n(spec.margin)));
    }
    if spec.match_anchor_width {
        out.push("match-width".to_owned());
    }
    if spec.modal {
        out.push("modal".to_owned());
    }
    if let Some(c) = spec.scrim {
        out.push(format!("scrim({})", crate::style::format_color(c)));
    }
    if spec.dismiss != d.dismiss {
        out.push(format!(
            "dismiss({})",
            flags_text(&[("outside", spec.dismiss.outside), ("escape", spec.dismiss.escape)])
        ));
    }
    if spec.outside_press == OutsidePress::Block {
        out.push("outside(block)".to_owned());
    }
    if spec.focus != d.focus {
        out.push(format!(
            "focus({})",
            flags_text(&[
                ("trap", spec.focus.trap),
                ("restore", spec.focus.restore),
                ("auto", spec.focus.auto)
            ])
        ));
    }
    out.join(" ")
}

fn comma_list(text: &str) -> Vec<&str> {
    text.split(',').map(str::trim).collect()
}

fn is_integer(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    !digits.is_empty() && digits.len() <= 10 && digits.bytes().all(|b| b.is_ascii_digit())
}

/// `none` or a comma list of distinct names from `names`; returns which are set.
fn flag_set(text: &str, names: &[&str]) -> Option<Vec<bool>> {
    let mut flags = vec![false; names.len()];
    if text.trim() == "none" {
        return Some(flags);
    }
    for part in text.split(',').map(str::trim) {
        let index = names.iter().position(|n| *n == part)?;
        if flags[index] {
            return None;
        }
        flags[index] = true;
    }
    Some(flags)
}

fn flags_text(flags: &[(&str, bool)]) -> String {
    let set: Vec<&str> = flags.iter().filter(|f| f.1).map(|f| f.0).collect();
    if set.is_empty() {
        "none".to_owned()
    } else {
        set.join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: Rect = Rect::new(0.0, 0.0, 400.0, 300.0);

    fn spec(side: OverlaySide, align: OverlayAlign) -> OverlaySpec {
        OverlaySpec::anchored(NodeId(9)).place(side, align)
    }

    #[test]
    fn each_side_and_alignment_places_against_the_anchor() {
        let a = Rect::new(100.0, 100.0, 80.0, 20.0);
        let size = (60.0, 40.0);
        let p = place_overlay(Some(a), size, VIEW, &spec(OverlaySide::Bottom, OverlayAlign::Start).offset(4.0, 0.0));
        assert_eq!(p.rect, Rect::new(100.0, 124.0, 60.0, 40.0));
        let p = place_overlay(Some(a), size, VIEW, &spec(OverlaySide::Top, OverlayAlign::End));
        assert_eq!(p.rect, Rect::new(120.0, 60.0, 60.0, 40.0));
        let p = place_overlay(Some(a), size, VIEW, &spec(OverlaySide::Right, OverlayAlign::Center));
        assert_eq!(p.rect, Rect::new(180.0, 90.0, 60.0, 40.0));
        let p = place_overlay(Some(a), size, VIEW, &spec(OverlaySide::Left, OverlayAlign::Start).offset(2.0, 3.0));
        assert_eq!(p.rect, Rect::new(38.0, 103.0, 60.0, 40.0));
        assert!(!p.flipped && !p.shifted && p.max_height.is_none());
    }

    #[test]
    fn flips_only_when_the_opposite_side_is_larger() {
        let near_bottom = Rect::new(10.0, 250.0, 50.0, 20.0);
        let p = place_overlay(Some(near_bottom), (50.0, 100.0), VIEW, &spec(OverlaySide::Bottom, OverlayAlign::Start));
        assert!(p.flipped);
        assert_eq!(p.side, OverlaySide::Top);
        assert_eq!(p.rect.y(), 150.0);
        // Without flip: stays below and is resized to the 30 px left.
        let mut s = spec(OverlaySide::Bottom, OverlayAlign::Start);
        s.avoid.flip = false;
        let p = place_overlay(Some(near_bottom), (50.0, 100.0), VIEW, &s);
        assert_eq!((p.side, p.rect.y(), p.rect.height(), p.max_height), (OverlaySide::Bottom, 270.0, 30.0, Some(30.0)));
        // Horizontal flip near the right edge.
        let right = Rect::new(380.0, 10.0, 10.0, 10.0);
        let p = place_overlay(Some(right), (100.0, 20.0), VIEW, &spec(OverlaySide::Right, OverlayAlign::Start));
        assert_eq!((p.side, p.rect.x()), (OverlaySide::Left, 280.0));
    }

    #[test]
    fn shifts_into_the_viewport_and_respects_margin() {
        let a = Rect::new(370.0, 10.0, 20.0, 20.0);
        let p = place_overlay(Some(a), (100.0, 40.0), VIEW, &spec(OverlaySide::Bottom, OverlayAlign::Start).margin(8.0));
        assert!(p.shifted);
        assert_eq!(p.rect.x(), 292.0);
        let mut s = spec(OverlaySide::Bottom, OverlayAlign::Start);
        s.avoid.shift = false;
        let p = place_overlay(Some(a), (100.0, 40.0), VIEW, &s);
        assert_eq!((p.rect.x(), p.shifted), (370.0, false));
    }

    #[test]
    fn resizes_to_the_larger_side_when_neither_fits() {
        let a = Rect::new(10.0, 120.0, 50.0, 20.0);
        // 120 above, 160 below: stays below (larger), limited to 160.
        let p = place_overlay(Some(a), (50.0, 500.0), VIEW, &spec(OverlaySide::Bottom, OverlayAlign::Start));
        assert_eq!((p.side, p.rect.height(), p.max_height), (OverlaySide::Bottom, 160.0, Some(160.0)));
        // Prefers top, which is smaller: flips to bottom.
        let p = place_overlay(Some(a), (50.0, 500.0), VIEW, &spec(OverlaySide::Top, OverlayAlign::Start));
        assert_eq!((p.side, p.flipped, p.rect.y()), (OverlaySide::Bottom, true, 140.0));
        // Too wide for the viewport: width limited and clamped to the left edge.
        let p = place_overlay(Some(a), (900.0, 20.0), VIEW, &spec(OverlaySide::Bottom, OverlayAlign::Start));
        assert_eq!((p.rect.x(), p.rect.width(), p.max_width), (0.0, 400.0, Some(400.0)));
    }

    #[test]
    fn viewport_anchor_centres_offsets_and_limits() {
        let p = place_overlay(None, (200.0, 100.0), VIEW, &OverlaySpec::viewport());
        assert_eq!(p.rect, Rect::new(100.0, 100.0, 200.0, 100.0));
        let p = place_overlay(None, (200.0, 100.0), VIEW, &OverlaySpec::viewport().offset(10.0, -20.0));
        assert_eq!(p.rect, Rect::new(80.0, 110.0, 200.0, 100.0));
        let p = place_overlay(None, (200.0, 900.0), VIEW, &OverlaySpec::viewport().margin(20.0));
        assert_eq!((p.rect.y(), p.rect.height(), p.max_height), (20.0, 260.0, Some(260.0)));
        // Non-finite input never panics.
        let p = place_overlay(Some(Rect::new(f32::NAN, 0.0, 1.0, 1.0)), (1.0, 1.0), VIEW, &OverlaySpec::default());
        assert_eq!(p.rect.x(), 0.0);
    }

    #[test]
    fn xstyle_grammar_round_trips_and_rejects() {
        assert_eq!(parse_overlay("none"), Some(None));
        let s = parse_overlay(
            "layer(5) anchor(42) side(top) align(center) offset(4,-2) avoid(flip,resize) margin(8) \
             match-width modal scrim(#00000080) dismiss(escape) outside(block) focus(trap,auto)",
        )
        .unwrap()
        .unwrap();
        assert_eq!(s.layer, 5);
        assert_eq!(s.anchor, OverlayAnchor::Node(NodeId(42)));
        assert_eq!((s.side, s.align, s.offset, s.cross_offset), (OverlaySide::Top, OverlayAlign::Center, 4.0, -2.0));
        assert_eq!(s.avoid, OverlayAvoid { flip: true, shift: false, resize: true });
        assert!(s.match_anchor_width && s.modal && s.blocks_outside() && s.traps_focus());
        assert_eq!(s.scrim, Some(Color::new(0, 0, 0, 128)));
        assert_eq!(s.dismiss, OverlayDismiss { outside: false, escape: true });
        assert_eq!(s.focus, OverlayFocus { trap: true, restore: false, auto: true });
        assert_eq!(parse_overlay(&format_overlay(&s)), Some(Some(s)));
        let p = parse_overlay("anchor(point,10.5,-3)").unwrap().unwrap();
        assert_eq!(p.anchor, OverlayAnchor::Point { x: 10.5, y: -3.0 });
        assert_eq!(parse_overlay(&format_overlay(&p)), Some(Some(p)));
        assert_eq!(format_overlay(&OverlaySpec::default()), "layer(0)");
        for bad in [
            "",
            "layer(1) layer(2)",
            "layer(x)",
            "layer(2000000)",
            "anchor(-1)",
            "anchor(point,1)",
            "side(up)",
            "offset(1,2,3)",
            "offset(nan)",
            "avoid(flip,flip)",
            "margin(-1)",
            "margin(5000)",
            "scrim(red)",
            "dismiss(click)",
            "outside(eat)",
            "focus(grab)",
            "modal(1)",
            "unknown",
            "layer(1",
        ] {
            assert_eq!(parse_overlay(bad), None, "{bad}");
        }
    }
}
