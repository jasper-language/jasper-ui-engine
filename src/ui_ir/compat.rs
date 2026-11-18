//! Loss-aware compatibility projection from the legacy Native `Style` into Typed UI style.
//!
//! This is intentionally not a complete reverse IR adapter. It projects every field that the
//! current Typed UiStyle groups can represent and records each non-default Native-only field that
//! would otherwise be silently lost.
//!
//! 旧 Native `Style` 到 Typed UI Style 的损失感知兼容投影。它不是完整反向 IR adapter；
//! 能由当前 Typed Style 组表达的字段会投影，Native-only 字段则逐项登记，禁止静默丢失。

use alloc::collections::{BTreeMap, BTreeSet};
use core::fmt::Write as _;
use super::*;
use crate::geometry::Edges as NativeEdges;
use crate::overlay::{
    OverlayAlign as NativeOverlayAlign, OverlayAnchor as NativeOverlayAnchor,
    OverlaySide as NativeOverlaySide,
};
use crate::scene::{Node, NodeId, NodeKind, Scene, ScenePatch};
use crate::semantics::{LivePoliteness, Role, Semantics};
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

/// Loss-aware projection of one Native style into JUI 2 `style`/`xstyle` pairs.
#[derive(Clone, Debug, PartialEq)]
pub struct Jui2StyleProjection {
    /// Base fields carried by a JUI 2 `style` line.
    pub base: Vec<(String, String)>,
    /// Extension fields carried by a JUI 2 `xstyle` line; host capability negotiation is external.
    pub xstyle: Vec<(String, String)>,
    pub losses: Vec<NativeStyleLoss>,
}

/// Loss-aware projection of Native semantics into JUI 2 `sem` fields.
#[derive(Clone, Debug, PartialEq)]
pub struct Jui2SemanticsProjection {
    pub fields: Vec<(String, String)>,
    pub losses: Vec<NativeStyleLoss>,
}

/// One loss-aware initial JUI 2 frame. This is not an incremental or reversible adapter.
#[derive(Clone, Debug, PartialEq)]
pub struct Jui2CompatibilityFrame {
    pub text: String,
    pub losses: Vec<NativeNodeLoss>,
}

/// One host-visible JUI 2 event projected from public identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Jui2Event {
    pub target: UiKey,
    pub kind: Jui2EventKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Jui2EventKind {
    HoverIn,
    HoverOut,
    PointerDown,
    PointerUp,
    Click,
    FocusIn,
    FocusOut,
    Key(String),
    TextCommit(String),
    ImeCancel,
    DismissRequest(String),
}

/// Reverse projection of a raw Legacy Scene with caller-supplied public identity.
#[derive(Clone, Debug, PartialEq)]
pub struct LegacySceneProjection {
    pub node: UiNode,
    pub losses: BTreeMap<UiKey, Vec<NativeNodeLoss>>,
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

/// Project a raw Legacy Scene into Typed UI nodes.
///
/// The Scene has no public identity or event-binding store. Every node therefore requires an
/// explicit `NodeId -> UiKey` mapping, and event bindings are reported as lost rather than
/// invented. Image resource generations are likewise unavailable and are reported through the
/// existing style losses.
pub fn project_legacy_scene(
    scene: &Scene,
    identities: &BTreeMap<NodeId, UiKey>,
) -> Result<LegacySceneProjection, UiIrError> {
    scene.validate().map_err(|error| UiIrError::Invalid {
        path: "$.legacy-scene".into(),
        detail: format!("{error:?}"),
    })?;
    let mut used = BTreeSet::new();
    let mut losses = BTreeMap::new();
    let node = project_legacy_scene_node(
        &scene.root,
        identities,
        &mut used,
        &mut losses,
    )?;
    losses
        .entry(node.key)
        .or_default()
        .push(NativeNodeLoss {
            path: "events".into(),
            detail: "Legacy Scene has no event-binding store; bindings cannot be recovered".into(),
        });
    Ok(LegacySceneProjection { node, losses })
}

fn project_legacy_scene_node(
    source: &Node,
    identities: &BTreeMap<NodeId, UiKey>,
    used: &mut BTreeSet<UiKey>,
    losses: &mut BTreeMap<UiKey, Vec<NativeNodeLoss>>,
) -> Result<UiNode, UiIrError> {
    let key = identities.get(&source.id).copied().ok_or_else(|| UiIrError::Invalid {
        path: "$.legacy-scene.identity".into(),
        detail: format!("no UiKey supplied for NodeId({})", source.id.0),
    })?;
    if key.generation == 0 || !used.insert(key) {
        return Err(UiIrError::Invalid {
            path: "$.legacy-scene.identity".into(),
            detail: format!(
                "invalid or duplicate UiKey mapping {}:{}",
                key.semantic_id, key.generation
            ),
        });
    }
    let element_kind = legacy_element_kind(&source.kind, source.semantics.role)?;
    losses.entry(key).or_default().push(NativeNodeLoss {
        path: "element.kind".into(),
        detail: "Legacy Scene does not store ElementKind; it was inferred from NodeKind and semantic role"
            .into(),
    });
    let overlay_anchor = match source.style.overlay.map(|overlay| overlay.anchor) {
        Some(NativeOverlayAnchor::Node(anchor)) => identities.get(&anchor).copied(),
        _ => None,
    };
    let style = project_native_style_with_context(
        &source.style,
        element_kind,
        NativeStyleContext {
            image_resource: None,
            overlay_anchor,
        },
    )?;
    losses
        .entry(key)
        .or_default()
        .extend(style.losses.into_iter().map(|loss| NativeNodeLoss {
            path: loss.path.into(),
            detail: loss.detail.into(),
        }));
    let semantics = super::native::project_compatibility_semantics(
        &source.semantics,
        "$.legacy-scene.semantics",
        losses,
        key,
    )?;
    let content = match &source.kind {
        NodeKind::Text(text) => Some(text.clone()),
        _ => None,
    };
    let mut children = Vec::with_capacity(source.children.len());
    for child in &source.children {
        children.push(project_legacy_scene_node(
            child, identities, used, losses,
        )?);
    }
    Ok(UiNode {
        key,
        element: UiElement {
            kind: element_kind,
            content,
            attributes: None,
        },
        semantics,
        style: Some(style.style),
        events: None,
        children: (!children.is_empty()).then_some(children),
    })
}

fn legacy_element_kind(kind: &NodeKind, role: Role) -> Result<ElementKind, UiIrError> {
    if matches!(kind, NodeKind::Canvas(_)) {
        return Err(UiIrError::Invalid {
            path: "$.legacy-scene.kind".into(),
            detail: "Typed UI v0.1 has no Canvas element kind".into(),
        });
    }
    Ok(match role {
        Role::Button => ElementKind::Button,
        Role::TextField => ElementKind::TextField,
        Role::TextArea => ElementKind::TextArea,
        Role::Checkbox => ElementKind::Checkbox,
        Role::Radio => ElementKind::Radio,
        Role::Switch => ElementKind::Switch,
        Role::Slider => ElementKind::Slider,
        Role::Image => ElementKind::Image,
        Role::Link => ElementKind::Link,
        Role::ListItem => ElementKind::ListItem,
        Role::TableCell => ElementKind::TableCell,
        Role::MenuItem => ElementKind::MenuItem,
        Role::Tooltip => ElementKind::Tooltip,
        _ => {
            if matches!(kind, NodeKind::Text(_)) {
                ElementKind::Text
            } else {
                ElementKind::Container
            }
        }
    })
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
    } else if source.display != expected_display
        && !(grid_can_select_display && grid_display)
    {
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

/// Project a Native style into the unchanged legacy JUI 2 transport vocabulary.
///
/// This is not a reverse Typed IR mapping. JUI 2 does not carry `UiKey`, events, resource
/// generations or environments, and extension keys still require the host's `hello` capability
/// declaration.
pub fn project_native_style_to_jui2(source: &NativeStyle) -> Jui2StyleProjection {
    let mut base = Vec::new();
    let mut losses = Vec::new();
    let defaults = NativeStyle::default();

    for (key, value) in [
        ("width", source.width),
        ("height", source.height),
        ("min-width", source.min_width),
        ("max-width", source.max_width),
        ("min-height", source.min_height),
        ("max-height", source.max_height),
    ] {
        if value != NativeLength::Auto {
            base.push((key.into(), jui2_length(value)));
        }
    }
    if source.padding != NativeEdges::ZERO {
        base.push(("padding".into(), jui2_edges(source.padding)));
    }
    if source.margin != NativeEdges::ZERO {
        base.push(("margin".into(), jui2_edges(source.margin)));
    }
    if source.gap != defaults.gap {
        base.push(("gap".into(), jui2_number(source.gap)));
    }
    if source.flex_grow != defaults.flex_grow {
        base.push(("flex-grow".into(), jui2_number(source.flex_grow)));
    }
    if source.flex_wrap != defaults.flex_wrap {
        losses.push(NativeStyleLoss {
            path: "style.flex_wrap",
            detail: "JUI 2 base style vocabulary has no flex-wrap field",
        });
    }
    if source.flex_shrink != defaults.flex_shrink {
        losses.push(NativeStyleLoss {
            path: "style.flex_shrink",
            detail: "JUI 2 base style vocabulary has no flex-shrink field",
        });
    }
    if source.font_size != defaults.font_size {
        base.push(("font-size".into(), jui2_number(source.font_size)));
    }
    if source.direction != defaults.direction {
        base.push((
            "direction".into(),
            match source.direction {
                NativeFlexDirection::Row => "row".into(),
                NativeFlexDirection::Column => "column".into(),
            },
        ));
    }
    match source.display {
        NativeDisplay::Flex => {}
        NativeDisplay::Block => base.push(("display".into(), "block".into())),
        NativeDisplay::None => base.push(("display".into(), "none".into())),
        NativeDisplay::Grid
        | NativeDisplay::Table
        | NativeDisplay::TableRow
        | NativeDisplay::TableCell => losses.push(NativeStyleLoss {
            path: "style.display",
            detail: "JUI 2 base style vocabulary has no Grid/Table display mode",
        }),
    }
    if source.align_items != defaults.align_items {
        base.push((
            "align-items".into(),
            match source.align_items {
                NativeAlign::Start => "start",
                NativeAlign::Center => "center",
                NativeAlign::End => "end",
                NativeAlign::Stretch => "stretch",
                NativeAlign::Baseline => {
                    losses.push(NativeStyleLoss {
                        path: "style.align_items",
                        detail: "JUI 2 base style vocabulary has no baseline alignment",
                    });
                    "start"
                }
            }
            .into(),
        ));
    }
    if source.justify_content != defaults.justify_content {
        base.push((
            "justify-content".into(),
            match source.justify_content {
                NativeJustify::Start => "start",
                NativeJustify::Center => "center",
                NativeJustify::End => "end",
                NativeJustify::SpaceBetween => "space-between",
                NativeJustify::SpaceAround | NativeJustify::SpaceEvenly => {
                    losses.push(NativeStyleLoss {
                        path: "style.justify_content",
                        detail: "JUI 2 base style vocabulary has no space-around/space-evenly",
                    });
                    "start"
                }
            }
            .into(),
        ));
    }
    if source.background != defaults.background {
        base.push(("background".into(), jui2_color(source.background)));
    }
    if source.color != defaults.color {
        base.push(("color".into(), jui2_color(source.color)));
    }
    if source.border_width != defaults.border_width
        || source.border_color != defaults.border_color
    {
        base.push((
            "border".into(),
            format!(
                "{} {}",
                jui2_number(source.border_width),
                jui2_color(source.border_color)
            ),
        ));
    }
    if source.font_weight != defaults.font_weight {
        base.push(("weight".into(), source.font_weight.to_string()));
    }
    if source.font_style != defaults.font_style {
        base.push((
            "font-style".into(),
            match source.font_style {
                NativeFontStyle::Normal => "normal".into(),
                NativeFontStyle::Italic => "italic".into(),
            },
        ));
    }
    if let Some(family) = &source.font_family {
        base.push(("family".into(), family.clone()));
    }
    if source.overflow != defaults.overflow {
        base.push((
            "overflow".into(),
            match source.overflow {
                NativeOverflow::Visible => "visible",
                NativeOverflow::Hidden => "hidden",
                NativeOverflow::Scroll => "scroll",
            }
            .into(),
        ));
    }
    if source.text_align != defaults.text_align {
        losses.push(NativeStyleLoss {
            path: "style.text_align",
            detail: "JUI 2 base style vocabulary has no text alignment field",
        });
    }
    if source.text_direction != defaults.text_direction {
        losses.push(NativeStyleLoss {
            path: "style.text_direction",
            detail: "JUI 2 base style vocabulary has no text direction field",
        });
    }
    Jui2StyleProjection {
        base,
        xstyle: source
            .xstyle_pairs()
            .into_iter()
            .map(|(key, value)| (key.into(), value))
            .collect(),
        losses,
    }
}

/// Project Native semantics into the unchanged legacy JUI 2 transport vocabulary.
pub fn project_native_semantics_to_jui2(source: &Semantics) -> Jui2SemanticsProjection {
    let mut fields = Vec::new();
    let mut losses = Vec::new();
    let role = match source.role {
        Role::Container => "container",
        Role::Text => "text",
        Role::Button => "button",
        Role::TextField => "text-field",
        Role::TextArea => {
            losses.push(NativeStyleLoss {
                path: "semantics.role",
                detail: "JUI 2 role vocabulary has no text-area role",
            });
            "text-field"
        }
        Role::Checkbox => "checkbox",
        Role::Radio => "radio",
        Role::Switch => "switch",
        Role::Slider => "slider",
        Role::Image => "image",
        Role::Link => "link",
        Role::List => "list",
        Role::ListItem => "list-item",
        Role::Table | Role::TableRow | Role::TableCell => {
            losses.push(NativeStyleLoss {
                path: "semantics.role",
                detail: "JUI 2 role vocabulary has no table roles",
            });
            "container"
        }
        Role::Dialog => "dialog",
        Role::Popover => {
            losses.push(NativeStyleLoss {
                path: "semantics.role",
                detail: "JUI 2 role vocabulary has no popover role",
            });
            "container"
        }
        Role::Tooltip => {
            losses.push(NativeStyleLoss {
                path: "semantics.role",
                detail: "JUI 2 role vocabulary has no tooltip role",
            });
            "text"
        }
        Role::Menu => "menu",
        Role::MenuItem => "menu-item",
        Role::ScrollView => "scroll-view",
        Role::Heading(level) => {
            fields.push(("level".into(), level.get().to_string()));
            "heading"
        }
        Role::Status => "status",
        Role::Live(politeness) => {
            fields.push((
                "live".into(),
                match politeness {
                    LivePoliteness::Polite => "polite",
                    LivePoliteness::Assertive => "assertive",
                }
                .into(),
            ));
            "live"
        }
        Role::Alert => "alert",
        Role::Form => "form",
        Role::Unknown => {
            losses.push(NativeStyleLoss {
                path: "semantics.role",
                detail: "JUI 2 role vocabulary has no unknown role",
            });
            "container"
        }
    };
    fields.insert(0, ("role".into(), role.into()));
    for (key, value) in [
        ("label", &source.label),
        ("description", &source.description),
        ("value", &source.value),
    ] {
        if !value.is_empty() {
            fields.push((key.into(), value.clone()));
        }
    }
    for (key, value) in [
        ("disabled", source.disabled),
        ("focusable", source.focusable),
        ("hidden", source.hidden),
    ] {
        if value {
            fields.push((key.into(), "true".into()));
        }
    }
    if source.selected {
        losses.push(NativeStyleLoss {
            path: "semantics.selected",
            detail: "JUI 2 semantics vocabulary has no selected field",
        });
    }
    Jui2SemanticsProjection { fields, losses }
}

/// Encode the current Native document as a fresh JUI 2 initial frame.
///
/// JUI 2 receives backend `NodeId`s only. `UiKey`, event bindings, resource payloads and
/// generations, environment, and the current document revision are reported as losses rather
/// than promoted into public identity.
pub fn project_jui2_initial_frame(
    document: &NativeUiDocument,
) -> Result<Jui2CompatibilityFrame, UiIrError> {
    let snapshot = document.project_compatibility_snapshot()?;
    let mut losses = vec![NativeNodeLoss {
        path: "$.jui2.identity".into(),
        detail: "JUI 2 carries backend numeric node IDs, not UiKey identity".into(),
    }];
    if compatibility_node_has_events(&snapshot.root) {
        losses.push(NativeNodeLoss {
            path: "$.jui2.events".into(),
            detail: "JUI 2 frame transport has no event-binding declaration".into(),
        });
    }
    if !snapshot.resources.is_empty() {
        losses.push(NativeNodeLoss {
            path: "$.jui2.resources".into(),
            detail: "resource payloads, kinds and generations travel outside JUI 2 frames".into(),
        });
    }
    if snapshot.environment != UiEnvironment::default() {
        losses.push(NativeNodeLoss {
            path: "$.jui2.environment".into(),
            detail: "JUI 2 environment is host state, not an initial frame operation".into(),
        });
    }
    if document.revision() != 2 {
        losses.push(NativeNodeLoss {
            path: "$.jui2.revision".into(),
            detail: format!(
                "initial frame resets revision to 2; document revision is {}",
                document.revision()
            ),
        });
    }

    let mut text = String::from("frame\t1\t2\n");
    encode_jui2_node(
        document,
        &document.scene().root,
        None,
        0,
        &mut text,
        &mut losses,
    )?;
    text.push_str("end\n");
    Ok(Jui2CompatibilityFrame { text, losses })
}

/// Encode one already-admitted Native scene update as a JUI 2 successor frame.
///
/// JUI 2 has no identity-preserving Move operation. Such a patch is represented as
/// remove+insert for compatibility and the interaction loss is reported.
pub fn project_jui2_scene_update(
    document: &NativeUiDocument,
    update: &NativeSceneUpdate,
) -> Result<Jui2CompatibilityFrame, UiIrError> {
    if update.expected_revision.checked_add(1) != Some(update.next_revision)
        || document.revision() != update.next_revision
    {
        return Err(UiIrError::Invalid {
            path: "$.jui2.update".into(),
            detail: "scene update revisions do not name the current document successor".into(),
        });
    }
    let snapshot = document.project_compatibility_snapshot()?;
    let mut losses = vec![NativeNodeLoss {
        path: "$.jui2.identity".into(),
        detail: "JUI 2 update transport carries backend NodeIds, not UiKey identity".into(),
    }];
    if compatibility_node_has_events(&snapshot.root) {
        losses.push(NativeNodeLoss {
            path: "$.jui2.events".into(),
            detail: "JUI 2 updates do not carry event bindings".into(),
        });
    }
    if !snapshot.resources.is_empty() {
        losses.push(NativeNodeLoss {
            path: "$.jui2.resources".into(),
            detail: "resource payloads and generations travel outside JUI 2 updates".into(),
        });
    }
    if snapshot.environment != UiEnvironment::default() {
        losses.push(NativeNodeLoss {
            path: "$.jui2.environment".into(),
            detail: "JUI 2 environment is host state, not a scene frame operation".into(),
        });
    }

    let mut text = format!(
        "frame\t{}\t{}\n",
        update.expected_revision, update.next_revision
    );
    for patch in &update.patches {
        match patch {
            ScenePatch::Insert { parent, index, node } => {
                encode_jui2_node(
                    document,
                    node,
                    Some(*parent),
                    *index,
                    &mut text,
                    &mut losses,
                )?;
            }
            ScenePatch::UpdateStyle { id, style } => {
                let path = format!("$.jui2.update.style.{}", id.0);
                let id = jui2_numeric(id.0, &format!("{path}.id"))?;
                let style = project_native_style_to_jui2(style);
                if !style.base.is_empty() {
                    text.push_str("style\t");
                    text.push_str(&id);
                    for (key, value) in &style.base {
                        text.push('\t');
                        text.push_str(key);
                        text.push('=');
                        text.push_str(&jui2_escape(value));
                    }
                    text.push('\n');
                }
                if !style.xstyle.is_empty() {
                    text.push_str("xstyle\t");
                    text.push_str(&id);
                    for (key, value) in &style.xstyle {
                        text.push('\t');
                        text.push_str(key);
                        text.push('=');
                        text.push_str(&jui2_escape(value));
                    }
                    text.push('\n');
                }
                losses.extend(style.losses.into_iter().map(|loss| NativeNodeLoss {
                    path: format!("{path}.{}", loss.path),
                    detail: loss.detail.into(),
                }));
            }
            ScenePatch::UpdateSemantics { id, semantics } => {
                let path = format!("$.jui2.update.semantics.{}", id.0);
                let id = jui2_numeric(id.0, &format!("{path}.id"))?;
                let semantics = project_native_semantics_to_jui2(semantics);
                text.push_str("sem\t");
                text.push_str(&id);
                for (key, value) in &semantics.fields {
                    text.push('\t');
                    text.push_str(key);
                    text.push('=');
                    text.push_str(&jui2_escape(value));
                }
                text.push('\n');
                losses.extend(
                    semantics
                        .losses
                        .into_iter()
                        .map(|loss| NativeNodeLoss {
                            path: format!("{path}.{}", loss.path),
                            detail: loss.detail.into(),
                        }),
                );
            }
            ScenePatch::UpdateText { id, text: value } => {
                let id = jui2_numeric(id.0, "$.jui2.update.text.id")?;
                writeln!(text, "text\t{id}\t{}", jui2_escape(value))
                    .expect("writing JUI 2 text to String cannot fail");
            }
            ScenePatch::SetCanvas { id, .. } => {
                let id = jui2_numeric(id.0, "$.jui2.update.canvas.id")?;
                losses.push(NativeNodeLoss {
                    path: format!("$.jui2.update.canvas.{id}"),
                    detail: "JUI 2 frame transport has no Canvas command".into(),
                });
            }
            ScenePatch::SetScroll { id, offset } => {
                let id = jui2_numeric(id.0, "$.jui2.update.scroll.id")?;
                let x = jui2_transport_int(offset.x, "$.jui2.update.scroll.x")?;
                let y = jui2_transport_int(offset.y, "$.jui2.update.scroll.y")?;
                writeln!(text, "scroll\t{id}\t{x}\t{y}")
                    .expect("writing JUI 2 text to String cannot fail");
            }
            ScenePatch::Move {
                id,
                parent,
                index,
            } => {
                let node = document.scene().find(*id).ok_or_else(|| UiIrError::Invalid {
                    path: "$.jui2.update.move".into(),
                    detail: "moved node is absent from the final document".into(),
                })?;
                let id = jui2_numeric(id.0, "$.jui2.update.move.id")?;
                losses.push(NativeNodeLoss {
                    path: format!("$.jui2.update.move.{id}"),
                    detail: "JUI 2 has no Move; remove+insert cannot preserve focus or press state"
                        .into(),
                });
                writeln!(text, "remove\t{id}")
                    .expect("writing JUI 2 text to String cannot fail");
                encode_jui2_node(
                    document,
                    node,
                    Some(*parent),
                    *index,
                    &mut text,
                    &mut losses,
                )?;
            }
            ScenePatch::Remove { id } => {
                let id = jui2_numeric(id.0, "$.jui2.update.remove.id")?;
                writeln!(text, "remove\t{id}")
                    .expect("writing JUI 2 text to String cannot fail");
            }
        }
    }
    text.push_str("end\n");
    Ok(Jui2CompatibilityFrame { text, losses })
}

/// Encode one Typed target event as an existing JUI 2 event line.
pub fn project_jui2_event(
    document: &NativeUiDocument,
    event: &Jui2Event,
) -> Result<String, UiIrError> {
    let node = document.node_for_key(event.target).ok_or_else(|| UiIrError::Invalid {
        path: "$.jui2.event.target".into(),
        detail: "event target is not active in the document".into(),
    })?;
    let id = jui2_numeric(node.0, "$.jui2.event.target")?;
    let (kind, payload) = match &event.kind {
        Jui2EventKind::HoverIn => ("hover", "in"),
        Jui2EventKind::HoverOut => ("hover", "out"),
        Jui2EventKind::PointerDown => ("pointer-down", ""),
        Jui2EventKind::PointerUp => ("pointer-up", ""),
        Jui2EventKind::Click => ("click", ""),
        Jui2EventKind::FocusIn => ("focus", "in"),
        Jui2EventKind::FocusOut => ("focus", "out"),
        Jui2EventKind::Key(value) => ("key", value.as_str()),
        Jui2EventKind::TextCommit(value) => ("text-commit", value.as_str()),
        Jui2EventKind::ImeCancel => ("ime-cancel", ""),
        Jui2EventKind::DismissRequest(value) => ("dismiss-request", value.as_str()),
    };
    Ok(format!("event\t{id}\t{kind}\t{}", jui2_escape(payload)))
}

fn compatibility_node_has_events(node: &UiNode) -> bool {
    node.events.as_ref().is_some_and(|events| !events.is_empty())
        || node
            .children
            .as_deref()
            .unwrap_or_default()
            .iter()
            .any(compatibility_node_has_events)
}

fn encode_jui2_node(
    document: &NativeUiDocument,
    node: &Node,
    parent: Option<NodeId>,
    index: usize,
    text: &mut String,
    losses: &mut Vec<NativeNodeLoss>,
) -> Result<(), UiIrError> {
    let key = document
        .key_for_node(node.id)
        .ok_or_else(|| UiIrError::Invalid {
            path: "$.jui2".into(),
            detail: "JUI 2 projection lacks a node UiKey".into(),
        })?;
    let path = format!("$.jui2.node[{}:{}]", key.semantic_id, key.generation);
    let id = jui2_numeric(node.id.0, &format!("{path}.id"))?;
    let parent = match parent {
        Some(id) => jui2_numeric(id.0, &format!("{path}.parent"))?,
        None => "0".into(),
    };
    let index = jui2_numeric(index as u64, &format!("{path}.index"))?;
    let kind = match node.kind {
        NodeKind::Container => "container",
        NodeKind::Text(_) => "text",
        NodeKind::Rect => "rect",
        NodeKind::Canvas(_) => {
            return Err(UiIrError::Invalid {
                path: format!("{path}.kind"),
                detail: "JUI 2 compatibility frame cannot encode a Canvas node".into(),
            })
        }
    };
    writeln!(text, "insert\t{parent}\t{index}\t{id}\t{kind}")
        .expect("writing JUI 2 text to String cannot fail");
    if let NodeKind::Text(value) = &node.kind {
        writeln!(text, "text\t{id}\t{}", jui2_escape(value))
            .expect("writing JUI 2 text to String cannot fail");
    }
    let style = project_native_style_to_jui2(&node.style);
    if !style.base.is_empty() {
        text.push_str("style\t");
        text.push_str(&id);
        for (key, value) in &style.base {
            text.push('\t');
            text.push_str(key);
            text.push('=');
            text.push_str(&jui2_escape(value));
        }
        text.push('\n');
    }
    if !style.xstyle.is_empty() {
        text.push_str("xstyle\t");
        text.push_str(&id);
        for (key, value) in &style.xstyle {
            text.push('\t');
            text.push_str(key);
            text.push('=');
            text.push_str(&jui2_escape(value));
        }
        text.push('\n');
    }
    losses.extend(style.losses.into_iter().map(|loss| NativeNodeLoss {
        path: format!("{path}.{}", loss.path),
        detail: loss.detail.into(),
    }));
    let semantics = project_native_semantics_to_jui2(&node.semantics);
    text.push_str("sem\t");
    text.push_str(&id);
    for (key, value) in &semantics.fields {
        text.push('\t');
        text.push_str(key);
        text.push('=');
        text.push_str(&jui2_escape(value));
    }
    text.push('\n');
    losses.extend(
        semantics
            .losses
            .into_iter()
            .map(|loss| NativeNodeLoss {
                path: format!("{path}.{}", loss.path),
                detail: loss.detail.into(),
            }),
    );
    for (child_index, child) in node.children.iter().enumerate() {
        encode_jui2_node(
            document,
            child,
            Some(node.id),
            child_index,
            text,
            losses,
        )?;
    }
    Ok(())
}

fn jui2_numeric(value: u64, path: &str) -> Result<String, UiIrError> {
    if value > 999_999_999 {
        return Err(UiIrError::Invalid {
            path: path.into(),
            detail: "JUI 2 transport accepts at most nine decimal digits".into(),
        });
    }
    Ok(value.to_string())
}

fn jui2_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            _ => out.push(ch),
        }
    }
    out
}

fn jui2_transport_int(value: f32, path: &str) -> Result<String, UiIrError> {
    if !value.is_finite()
        || value.fract() != 0.0
        || value.abs() > 999_999_999.0
    {
        return Err(UiIrError::Invalid {
            path: path.into(),
            detail: "JUI 2 integer transport requires an exact signed nine-digit value".into(),
        });
    }
    Ok((value as i64).to_string())
}

fn jui2_number(value: f32) -> String {
    value.to_string()
}

fn jui2_length(value: NativeLength) -> String {
    match value {
        NativeLength::Auto => "auto".into(),
        NativeLength::Px(value) => jui2_number(value),
        NativeLength::Percent(value) => format!("{}%", jui2_number(value)),
    }
}

fn jui2_edges(value: NativeEdges) -> String {
    [value.top, value.right, value.bottom, value.left]
        .map(jui2_number)
        .join(" ")
}

fn jui2_color(value: NativeColor) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", value.r, value.g, value.b, value.a)
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
