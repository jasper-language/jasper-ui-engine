//! Loss-aware compatibility projection from the legacy Native `Style` into Typed UI style.
//!
//! This is intentionally not a complete reverse IR adapter. It projects every field that the
//! current Typed UiStyle groups can represent and records each non-default Native-only field that
//! would otherwise be silently lost.
//!
//! 旧 Native `Style` 到 Typed UI Style 的损失感知兼容投影。它不是完整反向 IR adapter；
//! 能由当前 Typed Style 组表达的字段会投影，Native-only 字段则逐项登记，禁止静默丢失。

use super::*;
use crate::geometry::Edges as NativeEdges;
use crate::overlay::{
    OverlayAlign as NativeOverlayAlign, OverlayAnchor as NativeOverlayAnchor,
    OverlaySide as NativeOverlaySide,
};
use crate::style::{
    Align as NativeAlign, Color as NativeColor, Display as NativeDisplay,
    FlexDirection as NativeFlexDirection, FlexWrap as NativeFlexWrap,
    FontStyle as NativeFontStyle, GridLine, GridTrack, Inset as NativeInset,
    Justify as NativeJustify, Length as NativeLength, LineHeight as NativeLineHeight,
    Overflow as NativeOverflow, Position as NativePosition, Style as NativeStyle,
    TextAlign as NativeTextAlign, TextAlignVertical as NativeTextAlignVertical,
    TextDirection as NativeTextDirection,
};

/// One non-default Native-only field omitted from the Typed projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeStyleLoss {
    pub path: &'static str,
    pub detail: &'static str,
}

/// Typed style plus the exact fields the projection could not carry.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStyleProjection {
    pub style: TypedStyle,
    pub losses: Vec<NativeStyleLoss>,
}

/// Optional node-local context needed to reverse image and overlay candidate values.
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeStyleContext<'a> {
    /// Exact committed resource carried by the node's image declaration.
    pub image_resource: Option<&'a ResourceRef>,
    /// Already-translated public identity for a `NativeOverlayAnchor::Node`.
    pub overlay_anchor: Option<UiKey>,
}

impl NativeStyleProjection {
    /// True only when no non-default Native-only field was omitted.
    pub fn is_lossless(&self) -> bool {
        self.losses.is_empty()
    }
}

/// Project one admitted Native style. The caller supplies the element kind because Native table
/// display and scroll overflow defaults are element-derived rather than encoded in Typed style.
pub fn project_native_style(
    source: &NativeStyle,
    element: ElementKind,
) -> Result<NativeStyleProjection, UiIrError> {
    project_native_style_with_context(source, element, NativeStyleContext::default())
}

/// Project with exact node-local image and overlay context. Missing context keeps those fields
/// as explicit losses instead of creating guessed public identities.
pub fn project_native_style_with_context(
    source: &NativeStyle,
    element: ElementKind,
    context: NativeStyleContext<'_>,
) -> Result<NativeStyleProjection, UiIrError> {
    let mut losses = Vec::new();
    let expected_display = match element {
        ElementKind::Table => NativeDisplay::Table,
        ElementKind::TableRow => NativeDisplay::TableRow,
        ElementKind::TableCell => NativeDisplay::TableCell,
        _ => NativeDisplay::Flex,
    };
    let grid_display = source.display == NativeDisplay::Grid;
    let grid_can_select_display =
        matches!(element, ElementKind::Container | ElementKind::ScrollView);
    let has_grid_fields = source.grid_columns.is_some()
        || source.grid_rows.is_some()
        || source.grid_column != GridLine::Auto
        || source.grid_row != GridLine::Auto;
    let project_grid = grid_display || has_grid_fields;
    if project_grid
        && grid_can_select_display
        && !grid_display
    {
        losses.push(NativeStyleLoss {
            path: "style.grid",
            detail: "non-default grid fields exist while Native display is not Grid; projecting them would change display",
        });
    } else if source.display != expected_display && !grid_display {
        losses.push(NativeStyleLoss {
            path: "style.display",
            detail: "the Typed v0.1 style groups cannot encode this Native display mode",
        });
    }

    let expected_overflow = if element == ElementKind::ScrollView {
        NativeOverflow::Scroll
    } else {
        NativeOverflow::Visible
    };
    if source.overflow != expected_overflow {
        losses.push(NativeStyleLoss {
            path: "style.overflow",
            detail: "the Typed v0.1 style groups cannot encode Native overflow",
        });
    }
    push_extension_losses(source, &mut losses);

    let include_grid = project_grid
        && (!grid_can_select_display || grid_display || source.display == expected_display);
    let gap = f64::from(source.gap);
    let grid = include_grid.then(|| GridStyle {
        columns: source.grid_columns.as_ref().map(|tracks| {
            tracks
                .iter()
                .copied()
                .map(native_track)
                .collect::<Vec<_>>()
        }),
        rows: source
            .grid_rows
            .as_ref()
            .map(|tracks| tracks.iter().copied().map(native_track).collect()),
        column: Some(native_grid_line(source.grid_column)),
        row: Some(native_grid_line(source.grid_row)),
        gap: Some(gap),
    });
    let flex = FlexStyle {
        direction: Some(match source.direction {
            NativeFlexDirection::Row => FlexDirection::Row,
            NativeFlexDirection::Column => FlexDirection::Column,
        }),
        wrap: Some(match source.flex_wrap {
            NativeFlexWrap::NoWrap => FlexWrap::Nowrap,
            NativeFlexWrap::Wrap => FlexWrap::Wrap,
            NativeFlexWrap::WrapReverse => FlexWrap::WrapReverse,
        }),
        grow: Some(f64::from(source.flex_grow)),
        shrink: Some(f64::from(source.flex_shrink)),
        align_items: Some(match source.align_items {
            NativeAlign::Start => AlignItems::Start,
            NativeAlign::Center => AlignItems::Center,
            NativeAlign::End => AlignItems::End,
            NativeAlign::Baseline => AlignItems::Baseline,
            NativeAlign::Stretch => AlignItems::Stretch,
        }),
        justify_content: Some(match source.justify_content {
            NativeJustify::Start => JustifyContent::Start,
            NativeJustify::Center => JustifyContent::Center,
            NativeJustify::End => JustifyContent::End,
            NativeJustify::SpaceBetween => JustifyContent::SpaceBetween,
            NativeJustify::SpaceAround => JustifyContent::SpaceAround,
            NativeJustify::SpaceEvenly => JustifyContent::SpaceEvenly,
        }),
        gap: Some(gap),
    };
    let typography = TypographyStyle {
        family: source.font_family.as_ref().map(|family| vec![family.clone()]),
        size: Some(f64::from(source.font_size)),
        weight: Some(source.font_weight),
        style: Some(match source.font_style {
            NativeFontStyle::Normal => FontStyle::Normal,
            NativeFontStyle::Italic => FontStyle::Italic,
        }),
        line_height: source.line_height.map(|height| match height {
            NativeLineHeight::Px(value) => Length::px(f64::from(value)),
            NativeLineHeight::Multiple(value) => Length::percent(f64::from(value) * 100.0),
        }),
        align: Some(match source.text_align {
            NativeTextAlign::Start => TextAlign::Start,
            NativeTextAlign::Center => TextAlign::Center,
            NativeTextAlign::End => TextAlign::End,
            NativeTextAlign::Justify => TextAlign::Justify,
        }),
        vertical_align: source.text_align_vertical.map(|align| match align {
            NativeTextAlignVertical::Top => VerticalAlign::Top,
            NativeTextAlignVertical::Center => VerticalAlign::Center,
            NativeTextAlignVertical::Bottom => VerticalAlign::Bottom,
        }),
        direction: Some(match source.text_direction {
            NativeTextDirection::Auto => TextDirection::Auto,
            NativeTextDirection::Ltr => TextDirection::Ltr,
            NativeTextDirection::Rtl => TextDirection::Rtl,
        }),
    };
    let mut style = TypedStyle {
        box_: Some(BoxStyle {
            width: Some(native_length(source.width)),
            height: Some(native_length(source.height)),
            min_width: Some(native_length(source.min_width)),
            max_width: Some(native_length(source.max_width)),
            min_height: Some(native_length(source.min_height)),
            max_height: Some(native_length(source.max_height)),
            padding: Some(native_edges(source.padding)),
            margin: Some(native_edges(source.margin)),
        }),
        flex: Some(flex),
        grid,
        position: Some(PositionStyle {
            kind: Some(match source.position {
                NativePosition::Static => PositionKind::Static,
                NativePosition::Relative => PositionKind::Relative,
                NativePosition::Absolute => PositionKind::Absolute,
                NativePosition::Fixed => PositionKind::Fixed,
                NativePosition::Sticky => PositionKind::Sticky,
            }),
            inset: Some(native_inset(source.inset)),
            z_index: Some(
                source
                    .z_index
                    .map(ZIndex::Value)
                    .unwrap_or(ZIndex::Auto(ZAuto::Auto)),
            ),
            order: Some(source.order),
        }),
        paint: Some(PaintStyle {
            color: Some(native_color(source.color)),
            background: Some(native_color(source.background)),
            border: Some(BorderStyle {
                width: source.border_width,
                color: native_color(source.border_color),
            }),
            radii: Some(Radii {
                top_left: source.corner_radius.top_left,
                top_right: source.corner_radius.top_right,
                bottom_right: source.corner_radius.bottom_right,
                bottom_left: source.corner_radius.bottom_left,
            }),
            opacity: Some(f64::from(source.opacity)),
        }),
        typography: Some(typography),
        image: None,
        overlay: None,
    };
    project_image(source, context.image_resource, &mut style, &mut losses);
    project_overlay(source, context.overlay_anchor, &mut style, &mut losses);
    style.validate("$")?;
    Ok(NativeStyleProjection { style, losses })
}

fn push_extension_losses(source: &NativeStyle, losses: &mut Vec<NativeStyleLoss>) {
    let mut push = |path, detail| losses.push(NativeStyleLoss { path, detail });
    if !source.box_shadows.is_empty() {
        push("style.box_shadows", "Typed v0.1 has no shadow group");
    }
    if source.border_top.is_some() {
        push("style.border_top", "Typed v0.1 has only a uniform border");
    }
    if source.border_right.is_some() {
        push("style.border_right", "Typed v0.1 has only a uniform border");
    }
    if source.border_bottom.is_some() {
        push("style.border_bottom", "Typed v0.1 has only a uniform border");
    }
    if source.border_left.is_some() {
        push("style.border_left", "Typed v0.1 has only a uniform border");
    }
    if source.outline.is_some() {
        push("style.outline", "Typed v0.1 has no outline field");
    }
    if source.text_decoration.underline || source.text_decoration.line_through {
        push("style.text_decoration", "Typed v0.1 has no decoration field");
    }
    if source.hit_slop != NativeEdges::ZERO {
        push("style.hit_slop", "Typed v0.1 has no hit-slop field");
    }
    if source.transform != crate::style::Transform2D::IDENTITY {
        push("style.transform", "Typed v0.1 has no transform group");
    }
    if !source.transitions.is_empty() {
        push("style.transitions", "Typed v0.1 has no motion group");
    }
    if source.background_gradient.is_some() {
        push("style.background_gradient", "Typed v0.1 has no gradient field");
    }
    if source.backdrop.is_some() {
        push("style.backdrop", "Typed v0.1 has no material group");
    }
    if matches!(source.line_height, Some(NativeLineHeight::Multiple(_))) {
        push(
            "style.line_height.multiple",
            "Typed length percentage is projected back by Native as an absolute pixel line height",
        );
    }
}

fn project_image(
    source: &NativeStyle,
    image_resource: Option<&ResourceRef>,
    style: &mut TypedStyle,
    losses: &mut Vec<NativeStyleLoss>,
) {
    let Some(image) = &source.image else {
        return;
    };
    let Some(resource) = image_resource.filter(|resource| {
        resource.id == image.source
            && matches!(resource.kind, ResourceKind::Image | ResourceKind::Svg)
    }) else {
        losses.push(NativeStyleLoss {
            path: "style.image",
            detail: "Typed image requires the node's exact committed Image/SVG ResourceRef",
        });
        return;
    };
    style.image = Some(ImageStyle {
        resource: resource.clone(),
        fit: Some(match image.fit {
            crate::style::ObjectFit::Fill => ImageFit::Fill,
            crate::style::ObjectFit::Contain => ImageFit::Contain,
            crate::style::ObjectFit::Cover => ImageFit::Cover,
            crate::style::ObjectFit::None => ImageFit::None,
            crate::style::ObjectFit::ScaleDown => ImageFit::ScaleDown,
        }),
        align_x: Some(f64::from(image.align_x)),
        align_y: Some(f64::from(image.align_y)),
        tint: image.tint.map(native_color),
    });
}

fn project_overlay(
    source: &NativeStyle,
    overlay_anchor: Option<UiKey>,
    style: &mut TypedStyle,
    losses: &mut Vec<NativeStyleLoss>,
) {
    let Some(overlay) = source.overlay else {
        return;
    };
    let anchor = match overlay.anchor {
        NativeOverlayAnchor::Viewport => OverlayAnchor::Viewport(ViewportAnchor::Viewport),
        NativeOverlayAnchor::Point { x, y } => OverlayAnchor::Point(PointAnchor {
            x: f64::from(x),
            y: f64::from(y),
        }),
        NativeOverlayAnchor::Node(_) => {
            let Some(anchor) = overlay_anchor else {
                losses.push(NativeStyleLoss {
                    path: "style.overlay",
                    detail: "node-anchored overlay requires the anchor's current UiKey",
                });
                return;
            };
            OverlayAnchor::Node(anchor)
        }
    };
    style.overlay = Some(OverlayStyle {
        layer: Some(overlay.layer),
        anchor: Some(anchor),
        side: Some(match overlay.side {
            NativeOverlaySide::Top => OverlaySide::Top,
            NativeOverlaySide::Bottom => OverlaySide::Bottom,
            NativeOverlaySide::Left => OverlaySide::Left,
            NativeOverlaySide::Right => OverlaySide::Right,
        }),
        align: Some(match overlay.align {
            NativeOverlayAlign::Start => OverlayAlign::Start,
            NativeOverlayAlign::Center => OverlayAlign::Center,
            NativeOverlayAlign::End => OverlayAlign::End,
        }),
        offset: Some(f64::from(overlay.offset)),
        cross_offset: Some(f64::from(overlay.cross_offset)),
        margin: Some(f64::from(overlay.margin)),
        modal: Some(overlay.modal),
        scrim: overlay.scrim.map(native_color),
    });
    let mut push = |path, detail| losses.push(NativeStyleLoss { path, detail });
    if overlay.avoid != crate::overlay::OverlayAvoid::default() {
        push(
            "style.overlay.avoid",
            "Typed v0.1 has no overlay collision policy",
        );
    }
    if overlay.match_anchor_width {
        push(
            "style.overlay.match_anchor_width",
            "Typed v0.1 has no match-anchor-width field",
        );
    }
    if overlay.dismiss != crate::overlay::OverlayDismiss::default() {
        push(
            "style.overlay.dismiss",
            "Typed v0.1 dismiss bindings do not encode gesture policy",
        );
    }
    if overlay.outside_press != crate::overlay::OutsidePress::default() {
        push(
            "style.overlay.outside_press",
            "Typed v0.1 has no outside-press policy",
        );
    }
    let expected_focus = if overlay.modal {
        crate::overlay::OverlayFocus {
            trap: true,
            restore: true,
            auto: true,
        }
    } else {
        crate::overlay::OverlayFocus::default()
    };
    if overlay.focus != expected_focus {
        push(
            "style.overlay.focus",
            "Typed v0.1 encodes only the modal-implied focus policy",
        );
    }
}

fn native_length(value: NativeLength) -> Length {
    match value {
        NativeLength::Auto => Length::AUTO,
        NativeLength::Px(value) => Length::px(f64::from(value)),
        NativeLength::Percent(value) => Length::percent(f64::from(value)),
    }
}

fn native_edges(value: NativeEdges) -> Edges {
    Edges {
        top: Length::px(f64::from(value.top)),
        right: Length::px(f64::from(value.right)),
        bottom: Length::px(f64::from(value.bottom)),
        left: Length::px(f64::from(value.left)),
    }
}

fn native_inset(value: NativeInset) -> Edges {
    Edges {
        top: native_length(value.top),
        right: native_length(value.right),
        bottom: native_length(value.bottom),
        left: native_length(value.left),
    }
}

fn native_track(value: GridTrack) -> Track {
    match value {
        GridTrack::Auto => Track::Auto(AutoTrack { auto: true }),
        GridTrack::Px(value) => Track::Value(TrackValue {
            unit: TrackUnit::Px,
            value: f64::from(value),
        }),
        GridTrack::Percent(value) => Track::Value(TrackValue {
            unit: TrackUnit::Percent,
            value: f64::from(value),
        }),
        GridTrack::Fr(value) => Track::Value(TrackValue {
            unit: TrackUnit::Fr,
            value: f64::from(value),
        }),
    }
}

fn native_grid_line(value: GridLine) -> GridPlacement {
    match value {
        GridLine::Auto => GridPlacement {
            start: GridStart::Auto(GridAuto::Auto),
            span: 1,
        },
        GridLine::Line(line) => GridPlacement {
            start: GridStart::Line(line),
            span: 1,
        },
        GridLine::Span(span) => GridPlacement {
            start: GridStart::Auto(GridAuto::Auto),
            span,
        },
    }
}

fn native_color(value: NativeColor) -> UiColor {
    UiColor::new(value.r, value.g, value.b, value.a)
}
