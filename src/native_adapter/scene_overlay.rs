//! Native projection of Framework overlay declarations (issue #1 P0-A).
//! 原生投影：把 Framework 的浮层声明接到 Blitz DOM、输入与事件边界。
//!
//! Each overlay root is reparented under the document body and absolutely positioned after the
//! scene settles. This lifts it and its subtree out of the source parent's overflow clip, opacity,
//! transform and sibling order without changing source identity, semantics or layout participation
//! inside the overlay subtree. The engine never closes an overlay: outside presses, Escape and a
//! lost anchor are queued as `dismiss-request` values for the Framework.
//! 每个浮层根在场景布局稳定后移到 document body 下并绝对定位。因此浮层及子树脱离源父节点的
//! overflow、透明度、变换与兄弟绘制顺序，同时不改变源身份、语义或浮层子树内部布局。引擎从不自行关闭
//! 浮层：外部按下、Escape 与锚点丢失都作为 `dismiss-request` 交给 Framework。
use super::NativeScene;
use crate::native_view::PointerAction;
use crate::overlay::{
    DismissReason, OverlayAnchor, OverlayEvent, OverlayEventKind, OverlayInfo, OverlayPlacement,
    OverlaySpec, OverlaySuppression, place_overlay,
};
use crate::scene::NodeId;
use blitz_dom::{LocalName, Namespace, QualName};
use std::collections::{BTreeMap, BTreeSet};

/// Native state of the current view's overlay layer.
pub(super) struct OverlayState {
    body: blitz_dom::NodeId,
    /// Overlay roots in paint order, lowest first.
    order: Vec<NodeId>,
    placements: BTreeMap<NodeId, OverlayPlacement>,
    suppressed: BTreeMap<NodeId, OverlaySuppression>,
    /// Engine-created scrim nodes, reclaimed with the candidate document on frame replacement.
    furniture: Vec<blitz_dom::NodeId>,
    events: Vec<OverlayEvent>,
    return_focus: BTreeMap<NodeId, Option<NodeId>>,
}

impl OverlayState {
    pub(super) fn new(body: blitz_dom::NodeId) -> Self {
        Self {
            body,
            order: Vec::new(),
            placements: BTreeMap::new(),
            suppressed: BTreeMap::new(),
            furniture: Vec::new(),
            events: Vec::new(),
            return_focus: BTreeMap::new(),
        }
    }

    pub(super) fn configure(&mut self, view: &mut NativeScene) -> bool {
        let root = view.source.root.id;
        let mut ranked = Vec::<(NodeId, i32, usize)>::new();
        let mut display_order = BTreeMap::<NodeId, usize>::new();
        let mut overlay_ancestor = BTreeSet::<NodeId>::new();

        for (index, node) in view.source_preorder().into_iter().enumerate() {
            display_order.insert(node.id, index);
            let inside_overlay = view
                .source_ancestors(node.id)
                .iter()
                .any(|ancestor| overlay_ancestor.contains(ancestor));
            if inside_overlay {
                overlay_ancestor.insert(node.id);
            }
            let Some(spec) = node.style.overlay else {
                continue;
            };
            // The scene root cannot lift itself; an anchored overlay cannot create a source cycle.
            if node.id == root || !spec.valid() {
                return false;
            }
            if let OverlayAnchor::Node(anchor) = spec.anchor {
                if anchor == node.id || view.source.find(anchor).is_none() {
                    self.suppressed
                        .insert(node.id, OverlaySuppression::AnchorLost);
                } else if super::source_subtree_contains(&view.parents, anchor, node.id) {
                    return false;
                }
            }
            ranked.push((node.id, spec.layer, index));
            overlay_ancestor.insert(node.id);
        }
        ranked.sort_by_key(|(_, layer, source_index)| (*layer, *source_index));
        self.order = ranked.iter().map(|(id, _, _)| *id).collect();

        let elements: Vec<(NodeId, blitz_dom::NodeId)> = self
            .order
            .iter()
            .filter_map(|id| view.nodes.get(id).map(|(element, _)| (*id, *element)))
            .collect();
        if elements.len() != self.order.len() {
            return false;
        }
        let Some(document) = view.document.as_mut() else {
            return false;
        };
        {
            let mut tree = document.mutate();
            for (id, element) in elements {
                let visible = !self.suppressed.contains_key(&id);
                tree.remove_node(element);
                let tree = &mut tree;
                if let Some(spec) = view.source.find(id).and_then(|node| node.style.overlay) {
                    if visible {
                        if let Some(color) = spec.scrim {
                            let scrim = tree.create_element(
                                QualName::new(
                                    None,
                                    Namespace::from("http://www.w3.org/1999/xhtml"),
                                    LocalName::from("div"),
                                ),
                                Vec::new(),
                            );
                            tree.set_style_property(scrim, "position", "absolute");
                            tree.set_style_property(scrim, "left", "0px");
                            tree.set_style_property(scrim, "top", "0px");
                            tree.set_style_property(scrim, "width", "100vw");
                            tree.set_style_property(scrim, "height", "100vh");
                            tree.set_style_property(
                                scrim,
                                "background-color",
                                &format!(
                                    "#{:02x}{:02x}{:02x}{:02x}",
                                    color.r, color.g, color.b, color.a
                                ),
                            );
                            tree.append_children(self.body, &[scrim]);
                            self.furniture.push(scrim);
                        }
                    }
                }
                tree.set_style_property(element, "position", "absolute");
                tree.set_style_property(element, "left", "0px");
                tree.set_style_property(element, "top", "0px");
                if !visible {
                    tree.remove_style_property(element, "display");
                    tree.set_style_property(element, "display", "none");
                }
                tree.append_children(self.body, &[element]);
            }
        }
        document.resolve(0.0);

        let viewport = rect_from_logical(view.viewport_logical());
        let order = self.order.clone();
        for id in order {
            if self.suppressed.contains_key(&id) {
                continue;
            }
            let Some(spec) = view.source.find(id).and_then(|node| node.style.overlay) else {
                continue;
            };
            let Some(element) = view.nodes.get(&id).map(|(element, _)| *element) else {
                return false;
            };
            let anchor = match spec.anchor {
                OverlayAnchor::Viewport => None,
                OverlayAnchor::Point { x, y } => Some(crate::geometry::Rect::new(x, y, 0.0, 0.0)),
                OverlayAnchor::Node(anchor) => {
                    if !view.source_displayed(anchor) {
                        self.suppressed.insert(id, OverlaySuppression::AnchorLost);
                        continue;
                    }
                    let Some(rect) = view.layout_box(anchor) else {
                        self.suppressed.insert(id, OverlaySuppression::AnchorLost);
                        continue;
                    };
                    Some(rect_from_logical(rect))
                }
            };
            let Some(natural) = view.layout_box(id) else {
                return false;
            };
            let mut size = (natural.width.max(0.0), natural.height.max(0.0));
            if spec.match_anchor_width {
                if let Some(anchor) = anchor {
                    size.0 = size.0.max(anchor.width());
                }
            }
            let first = place_overlay(anchor, size, viewport, &spec);
            let Some(document) = view.document.as_mut() else {
                return false;
            };
            {
                let mut tree = document.mutate();
                if let Some(limit) = first.max_width {
                    tree.set_style_property(element, "max-width", &format!("{limit}px"));
                }
                if let Some(limit) = first.max_height {
                    tree.set_style_property(element, "max-height", &format!("{limit}px"));
                }
            }
            document.resolve(0.0);
            let size = view
                .layout_box(id)
                .map(|rect| (rect.width.max(0.0), rect.height.max(0.0)))
                .unwrap_or(size);
            let placed = place_overlay(anchor, size, viewport, &spec);
            let Some(document) = view.document.as_mut() else {
                return false;
            };
            {
                let mut tree = document.mutate();
                tree.set_style_property(element, "left", &format!("{}px", placed.rect.x()));
                tree.set_style_property(element, "top", &format!("{}px", placed.rect.y()));
            }
            document.resolve(0.0);
            let actual = view
                .layout_box(id)
                .map(rect_from_logical)
                .unwrap_or(placed.rect);
            self.placements.insert(
                id,
                OverlayPlacement {
                    rect: actual,
                    ..placed
                },
            );
        }
        for id in self.suppressed.keys() {
            if matches!(
                self.suppressed.get(id),
                Some(OverlaySuppression::AnchorLost)
            ) {
                self.events.push(OverlayEvent {
                    scene_version: view.scene_version,
                    overlay: *id,
                    kind: OverlayEventKind::DismissRequest(DismissReason::AnchorLost),
                });
            }
        }
        let focused = view.focused_node(view.owner, view.scene_version);
        for id in &self.order {
            self.return_focus.entry(*id).or_insert(focused);
        }
        true
    }

    pub(super) fn carry_from(&mut self, previous: &Self) {
        self.events = previous.events.clone();
        for id in &self.order {
            self.return_focus
                .insert(*id, previous.return_focus.get(id).copied().flatten());
        }
    }

    pub(super) fn info(&self, view: &NativeScene) -> Vec<OverlayInfo> {
        self.order
            .iter()
            .enumerate()
            .filter_map(|(rank, id)| {
                Some(OverlayInfo {
                    overlay: *id,
                    rank,
                    spec: view.source.find(*id)?.style.overlay?,
                    placement: self.placements.get(id).copied(),
                    suppressed: self.suppressed.get(id).copied(),
                    return_focus: self.return_focus.get(id).copied().flatten(),
                })
            })
            .collect()
    }

    pub(super) fn take_events(&mut self) -> Vec<OverlayEvent> {
        std::mem::take(&mut self.events)
    }

    /// Topmost interactive overlay in paint order.
    fn top(&self) -> Option<(NodeId, OverlaySpec, OverlayPlacement)> {
        for id in self.order.iter().rev() {
            if self.suppressed.contains_key(id) {
                continue;
            }
            let Some(placement) = self.placements.get(id).copied() else {
                continue;
            };
            // The spec is resolved by the caller-aware helpers below; the placement alone is
            // enough to decide whether a pointer starts outside.
            return Some((*id, OverlaySpec::default(), placement));
        }
        None
    }

    pub(super) fn pointer_gate(
        &mut self,
        view: &NativeScene,
        x: f32,
        y: f32,
        phase: PointerAction,
        primary_pressed: bool,
    ) -> (bool, bool) {
        let Some((overlay, _, placement)) = self.top() else {
            return (false, false);
        };
        let point = crate::geometry::Point::new(x, y);
        if placement.rect.contains(point) {
            return (false, false);
        }
        let Some(spec) = view
            .source
            .find(overlay)
            .and_then(|node| node.style.overlay)
        else {
            return (false, false);
        };
        let on_anchor = match spec.anchor {
            OverlayAnchor::Node(anchor) => view
                .layout_box(anchor)
                .is_some_and(|rect| rect_from_logical(rect).contains(point)),
            _ => false,
        };
        if phase == PointerAction::Down && primary_pressed && spec.dismiss.outside {
            self.events.push(OverlayEvent {
                scene_version: view.scene_version,
                overlay,
                kind: OverlayEventKind::DismissRequest(DismissReason::OutsidePress { on_anchor }),
            });
        }
        (spec.blocks_outside(), spec.blocks_outside())
    }

    pub(super) fn escape_request(&mut self, view: &NativeScene) {
        let Some((overlay, _, _)) = self.top() else {
            return;
        };
        let Some(spec) = view
            .source
            .find(overlay)
            .and_then(|node| node.style.overlay)
        else {
            return;
        };
        if spec.dismiss.escape {
            self.events.push(OverlayEvent {
                scene_version: view.scene_version,
                overlay,
                kind: OverlayEventKind::DismissRequest(DismissReason::Escape),
            });
        }
    }

    pub(super) fn allows_interaction(&self, view: &NativeScene, id: NodeId) -> bool {
        let Some((overlay, _, _)) = self.top() else {
            return true;
        };
        let Some(spec) = view
            .source
            .find(overlay)
            .and_then(|node| node.style.overlay)
        else {
            return true;
        };
        !spec.blocks_outside() || super::source_subtree_contains(&view.parents, id, overlay)
    }

    #[allow(dead_code)]
    pub(super) fn body(&self) -> blitz_dom::NodeId {
        self.body
    }

    #[allow(dead_code)]
    pub(super) fn furniture(&self) -> &[blitz_dom::NodeId] {
        &self.furniture
    }
}

fn rect_from_logical(rect: crate::style_effects::LogicalBox) -> crate::geometry::Rect {
    crate::geometry::Rect::new(rect.x, rect.y, rect.width, rect.height)
}

#[cfg(all(test, feature = "cpu-pending"))]
mod tests {
    use super::*;
    use crate::native_view::PointerAction;
    use crate::overlay::{OverlayAlign, OverlaySide};
    use crate::scene_events::ScenePointerInput;
    use crate::style::{Color, Length, Overflow, Style};
    use jasper_ui_engine::scene::{Node, Scene};
    use jasper_ui_engine::semantics::{Role, Semantics};

    const OWNER: u64 = 91;

    fn style(width: f32, height: f32, color: Color) -> Style {
        Style::default()
            .size(Length::Px(width), Length::Px(height))
            .background(color)
    }

    fn scene(spec: OverlaySpec) -> Scene {
        let mut overlay_style = style(60.0, 40.0, Color::RED);
        overlay_style.overlay = Some(spec);
        Scene::new(
            Node::container(NodeId(1))
                .with_style(style(200.0, 120.0, Color::WHITE))
                .child(
                    Node::container(NodeId(2))
                        .with_style(style(100.0, 40.0, Color::BLACK).overflow(Overflow::Hidden))
                        .child(
                            Node::container(NodeId(3))
                                .with_style(style(80.0, 20.0, Color::BLUE))
                                .with_semantics(
                                    Semantics::default()
                                        .role(Role::Button)
                                        .label("anchor")
                                        .focusable(true),
                                ),
                        )
                        .child(
                            Node::container(NodeId(4))
                                .with_style(overlay_style)
                                .with_semantics(
                                    Semantics::default()
                                        .role(Role::Button)
                                        .label("overlay")
                                        .focusable(true),
                                ),
                        ),
                )
                .child(
                    Node::container(NodeId(5))
                        .with_style(style(30.0, 20.0, Color::GREEN))
                        .with_semantics(
                            Semantics::default()
                                .role(Role::Button)
                                .label("outside")
                                .focusable(true),
                        ),
                ),
        )
    }

    #[test]
    fn overlay_leaves_parent_clip_and_is_placed_below_its_anchor() {
        let source = scene(
            OverlaySpec::anchored(NodeId(3))
                .place(OverlaySide::Bottom, OverlayAlign::Start)
                .offset(44.0, 0.0),
        );
        let mut view = NativeScene::from_scene(OWNER, &source, &[], 200, 120).unwrap();
        let info = view.overlays.info(&view);
        assert_eq!(info.len(), 1);
        let placement = info[0].placement.expect("overlay is displayed");
        assert_eq!(
            placement.rect,
            crate::geometry::Rect::new(0.0, 64.0, 60.0, 40.0)
        );
        assert_eq!(
            view.node_bounds_from_host(OWNER, source.version(), NodeId(4)),
            Some(placement.rect)
        );
        let pixels = view.render_cpu(OWNER, 1 << 24).unwrap();
        let pixel = |x: usize, y: usize| {
            let i = (y * 200 + x) * 4;
            [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
        };
        // The source parent ends at y=40 and clips children; the lifted overlay is visible below
        // its anchor in the viewport.
        let [r, g, b, a] = pixel(5, 70);
        assert!(r > 200 && g < 80 && b < 80 && a == 255, "{r} {g} {b} {a}");
    }

    #[test]
    fn blocking_overlay_consumes_outside_press_and_emits_dismiss_request() {
        let mut spec = OverlaySpec::anchored(NodeId(3)).offset(44.0, 0.0);
        spec.outside_press = crate::overlay::OutsidePress::Block;
        let source = scene(spec);
        let mut view = NativeScene::from_scene(OWNER, &source, &[], 200, 120).unwrap();
        let mut delivered = Vec::new();
        let consumed = view.pointer_input_from_host(
            OWNER,
            source.version(),
            &ScenePointerInput {
                phase: PointerAction::Down,
                position: (150.0, 10.0),
                primary_pressed: true,
                modifiers: Default::default(),
            },
            &mut |event| {
                delivered.push(event.node);
                Default::default()
            },
        );
        assert!(consumed);
        assert!(delivered.is_empty());
        assert_eq!(
            view.take_overlay_events_from_host(OWNER).unwrap(),
            vec![OverlayEvent {
                scene_version: source.version(),
                overlay: NodeId(4),
                kind: OverlayEventKind::DismissRequest(DismissReason::OutsidePress {
                    on_anchor: false
                }),
            }]
        );
    }

    #[test]
    fn blocking_overlay_rejects_external_focus() {
        let mut spec = OverlaySpec::anchored(NodeId(3)).offset(44.0, 0.0);
        spec.modal = true;
        let source = scene(spec);
        let mut view = NativeScene::from_scene(OWNER, &source, &[], 200, 120).unwrap();
        let mut focus = Vec::new();
        assert!(
            !view.focus_from_host(OWNER, source.version(), NodeId(5), &mut |event| focus
                .push(event),)
        );
        assert!(focus.is_empty());
        assert!(
            view.focus_from_host(OWNER, source.version(), NodeId(4), &mut |event| focus
                .push(event),)
        );
    }
}
