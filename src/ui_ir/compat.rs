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
    let style = TypedStyle {
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
    if source.image.is_some() {
        push(
            "style.image",
            "Typed image requires a ResourceRef that a bare Native Style does not carry",
        );
    }
    if source.overlay.is_some() {
        push(
            "style.overlay",
            "Typed overlay requires UiKey anchors that a bare Native Style does not carry",
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
