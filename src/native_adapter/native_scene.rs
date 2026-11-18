//! Candidate adapter for the existing Rust scene, using native Blitz mutations only.
//! 现有 Rust 场景适配候选；不序列化 HTML，不执行旧布局/绘制算法。
use crate::view_slot::ViewSlot;
/// E14 in-scene text editing (child module: it reads the scene's private state).
/// E14 场景内文本编辑（子模块：读取场景私有状态）。
#[path = "scene_edit.rs"]
mod scene_edit;
/// P0-A scene overlays (child module: it reads the scene's private native state).
/// P0-A 场景浮层（子模块：读取场景私有原生状态）。
#[path = "scene_overlay.rs"]
mod scene_overlay;
use blitz_dom::{BaseDocument, DocumentConfig, LocalName, Namespace, QualName};
use blitz_traits::shell::Viewport;
use jasper_ui_engine::input_identity::next_ui_update_version;
use jasper_ui_engine::{
    scene::{Node, NodeId, NodeKind, Scene, ScenePatch},
    semantics::{Role, Semantics},
    style::*,
};
use std::collections::{BTreeMap, BTreeSet};

/// Publicly consumable reason a scene update was not committed. This contains no backend type
/// and no patch payload; a rejected update leaves the active view at its prior version.
/// 场景更新未提交的可消费原因；不包含后端类型或补丁载荷，拒绝后活动视图保持原版本。
// BEGIN VERIFIED SCENE UPDATE ERROR TYPE KERNEL
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneUpdateError {
    OwnerMismatch,
    Closed,
    EmptyFrame,
    StaleVersion {
        expected: u64,
        current: u64,
    },
    InvalidSuccessor {
        expected: u64,
        next: u64,
    },
    SourceRejected,
    NativeRejected,
    /// The updated scene would hold `required` nodes; the host quota allows `limit`.
    /// 更新后的场景需要 `required` 个节点，宿主额度只允许 `limit` 个。
    NodeQuotaExceeded {
        limit: usize,
        required: usize,
    },
    /// The updated scene would hold `required` bytes of text and semantic strings; the host
    /// quota allows `limit`.
    /// 更新后的场景需要 `required` 字节文字与语义字符串，宿主额度只允许 `limit` 字节。
    TextQuotaExceeded {
        limit: usize,
        required: usize,
    },
}
// END VERIFIED SCENE UPDATE ERROR TYPE KERNEL

impl std::fmt::Display for SceneUpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OwnerMismatch => write!(f, "scene update owner does not match the view"),
            Self::Closed => write!(f, "scene view is closed"),
            Self::EmptyFrame => write!(f, "scene update frame is empty"),
            Self::StaleVersion { expected, current } => write!(
                f,
                "scene update expected version {expected}, current version is {current}"
            ),
            Self::InvalidSuccessor { expected, next } => {
                write!(f, "scene update cannot advance from {expected} to {next}")
            }
            Self::SourceRejected => write!(f, "source scene rejected the update frame"),
            Self::NativeRejected => {
                write!(f, "native scene could not construct the update candidate")
            }
            Self::NodeQuotaExceeded { limit, required } => write!(
                f,
                "scene update needs {required} nodes but the host node quota is {limit}"
            ),
            Self::TextQuotaExceeded { limit, required } => write!(
                f,
                "scene update needs {required} bytes of text but the host text quota is {limit} bytes"
            ),
        }
    }
}

/// Host-configured resource quota for one scene view. The engine itself has no fixed node or
/// per-frame operation limit; `None` (the default) means unlimited. These are the admission
/// measures the engine can count exactly (source nodes; UTF-8 bytes of text content plus
/// semantic label/description/value), not an allocator bound for layout, fonts or rendering.
/// 宿主为单个场景视图配置的资源额度。引擎本身没有固定的节点数或每帧操作数上限；`None`（默认）表示无上限。
/// 这里是引擎能精确计数的准入量（源节点数；文字内容与语义 label/description/value 的 UTF-8 字节数），
/// 不是布局、字体或渲染的分配器上界。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SceneQuota {
    pub max_nodes: Option<usize>,
    pub max_text_bytes: Option<usize>,
}

impl SceneQuota {
    /// No quota: only the host's memory bounds the scene.
    /// 无额度：场景只受宿主内存约束。
    pub const UNLIMITED: Self = Self {
        max_nodes: None,
        max_text_bytes: None,
    };
}

/// Exact source usage (nodes, text bytes) of one scene tree; iterative, saturating.
/// 一棵场景树的精确源用量（节点数、文字字节数）；迭代、饱和计数。
pub(crate) fn scene_usage(root: &Node) -> (usize, usize) {
    let mut nodes = 0usize;
    let mut text = 0usize;
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        nodes = nodes.saturating_add(1);
        if let NodeKind::Text(value) = &node.kind {
            text = text.saturating_add(value.len());
        }
        for value in [
            &node.semantics.label,
            &node.semantics.description,
            &node.semantics.value,
        ] {
            text = text.saturating_add(value.len());
        }
        pending.extend(node.children.iter());
    }
    (nodes, text)
}

/// Admit measured usage against the host quota; nodes are checked before text.
/// 按宿主额度准入实测用量；先查节点数，再查文字字节数。
pub(crate) fn admit_scene_quota(
    quota: SceneQuota,
    nodes: usize,
    text_bytes: usize,
) -> Result<(), SceneUpdateError> {
    if let Some(limit) = quota.max_nodes {
        if nodes > limit {
            return Err(SceneUpdateError::NodeQuotaExceeded {
                limit,
                required: nodes,
            });
        }
    }
    if let Some(limit) = quota.max_text_bytes {
        if text_bytes > limit {
            return Err(SceneUpdateError::TextQuotaExceeded {
                limit,
                required: text_bytes,
            });
        }
    }
    Ok(())
}

impl std::error::Error for SceneUpdateError {}

/// IEEE-754 positive finite values are ordered by their unsigned bit pattern.
/// Keep host DPI admission integer-only so the exact boundary can be verified
/// without delegating our policy to an unmodelled floating-point comparison.
// BEGIN VERIFIED NATIVE DPI SCALE ADMISSION KERNEL
fn admits_native_dpi_scale_bits(bits: u32) -> bool {
    (0x3c80_0000..=0x4280_0000).contains(&bits)
}
// END VERIFIED NATIVE DPI SCALE ADMISSION KERNEL

/// Admit one versioned native CPU render, then submit and present only an exact packed frame.
/// The renderer is an injected third-party boundary; this kernel owns its call eligibility,
/// packed-length check and the transition into the verified backend submission seam.
#[cfg(feature = "cpu-pending")]
// BEGIN VERIFIED NATIVE FRAME ORCHESTRATION KERNEL
fn orchestrate_native_frame(
    owner: u64,
    caller: u64,
    current_version: u64,
    requested_version: u64,
    extent: Option<(u32, u32)>,
    budget: usize,
    render: impl FnOnce(usize) -> Option<Vec<u8>>,
    backend: &mut impl jasper_ui_engine::backend::RenderBackend,
) -> Result<(), jasper_ui_engine::backend::BackendError> {
    use jasper_ui_engine::backend::BackendError;
    if owner != caller || current_version != requested_version {
        return Err(BackendError::Invalid(
            "native frame owner or version".into(),
        ));
    }
    let (width, height) =
        extent.ok_or_else(|| BackendError::Invalid("retired native view".into()))?;
    let expected = crate::cpu_budget::cpu_frame_bytes(width, height, budget)
        .ok_or_else(|| BackendError::Invalid("native frame extent or budget".into()))?;
    let pixels =
        render(budget).ok_or_else(|| BackendError::Invalid("native frame renderer".into()))?;
    if pixels.len() != expected {
        return Err(BackendError::Invalid("native frame packed length".into()));
    }
    jasper_ui_engine::backend::submit_native_frame(backend, width, height, pixels)
}
// END VERIFIED NATIVE FRAME ORCHESTRATION KERNEL

/// Validate an injected software surface before rendering, then copy one exact
/// packed frame row-by-row without touching host padding.
#[cfg(feature = "cpu-pending")]
// BEGIN VERIFIED NATIVE HOST SURFACE ORCHESTRATION KERNEL
fn orchestrate_native_host_surface(
    width: u32,
    height: u32,
    destination: &mut [u8],
    stride: usize,
    max_render_bytes: usize,
    render: impl FnOnce(usize) -> Option<Vec<u8>>,
) -> bool {
    use jasper_ui_engine::surface::{rgba8_row_bytes, surface_span, write_surface_row};
    let Some(row_bytes) = rgba8_row_bytes(width, destination.len()) else {
        return false;
    };
    let height = height as usize;
    if surface_span(row_bytes, height, stride, destination.len()).is_none() {
        return false;
    }
    let Some(expected) = row_bytes.checked_mul(height) else {
        return false;
    };
    let Some(pixels) = render(max_render_bytes) else {
        return false;
    };
    if pixels.len() != expected {
        return false;
    }
    for row in 0..height {
        let start = row * row_bytes;
        let end = start + row_bytes;
        if !write_surface_row(
            destination,
            row_bytes,
            height,
            stride,
            row,
            &pixels[start..end],
        ) {
            return false;
        }
    }
    true
}
// END VERIFIED NATIVE HOST SURFACE ORCHESTRATION KERNEL

// BEGIN VERIFIED NATIVE SCENE TYPE
pub(crate) struct NativeScene {
    source: Scene,
    font_ctx: blitz_dom::FontContext,
    bold_aliases: BTreeSet<u64>,
    document: ViewSlot<BaseDocument>,
    owner: u64,
    scene_version: u64,
    node_count: usize,
    parents: BTreeMap<NodeId, Option<NodeId>>,
    children: BTreeMap<NodeId, Vec<NodeId>>,
    displayed: BTreeMap<NodeId, bool>,
    semantics: BTreeMap<NodeId, Semantics>,
    nodes: BTreeMap<NodeId, (blitz_dom::NodeId, Option<blitz_dom::NodeId>)>,
    frame_dirty: bool,
    frame_cache: Option<(u32, u32, Vec<u8>)>,
    families: BTreeSet<String>,
    last_pointer: Option<(f32, f32)>,
    motion: jasper_ui_engine::motion::MotionState,
    material: MaterialState,
    edit: scene_edit::EditState,
    images: crate::image_resource::ImageStore,
    overlays: scene_overlay::OverlayState,
}
// END VERIFIED NATIVE SCENE TYPE

/// E5 material budget and the report of the last painted frame.
/// E5 材质预算与上一个绘制帧的报告。
#[derive(Clone, Debug, Default)]
pub(crate) struct MaterialState {
    budget: crate::material::BackdropBudget,
    report: crate::material::BackdropReport,
}
impl NativeScene {
    /// Bounded snapshot import; Canvas remains unsupported.
    pub(crate) fn from_scene(
        owner: u64,
        scene: &Scene,
        font: &[u8],
        width: u32,
        height: u32,
    ) -> Option<Self> {
        Self::from_scene_with_context(
            owner,
            scene,
            crate::native_fonts::injected_font_context(font),
            width,
            height,
            usize::MAX,
        )
    }
    /// Host-ordered font stack; measurement and painting share the one Parley layout.
    /// 宿主有序字体栈；测量与绘制共用同一份 Parley 布局。
    pub(crate) fn from_scene_with_fonts(
        owner: u64,
        scene: &Scene,
        fonts: &[&[u8]],
        width: u32,
        height: u32,
    ) -> Option<Self> {
        Self::from_scene_with_context(
            owner,
            scene,
            crate::native_fonts::injected_font_stack(fonts)?,
            width,
            height,
            usize::MAX,
        )
    }
    /// `node_limit` bounds the admission traversal (`scene_work_fits`); `usize::MAX` is unlimited.
    /// `node_limit` 限定准入遍历（`scene_work_fits`）；`usize::MAX` 表示无上限。
    fn from_scene_with_context(
        owner: u64,
        scene: &Scene,
        fonts: crate::native_fonts::InjectedFonts,
        width: u32,
        height: u32,
        node_limit: usize,
    ) -> Option<Self> {
        let crate::native_fonts::InjectedFonts {
            context: mut font_ctx,
            bold_aliases,
        } = fonts;
        // Declared families are admitted against exactly what the injected stack registered.
        // 声明的族名按注入字体栈实际登记的族名准入。
        let families: BTreeSet<String> = font_ctx
            .collection
            .family_names()
            .map(str::to_owned)
            .collect();
        let prepared = prepare_subtree(&scene.root, 0, &BTreeSet::new(), &families, node_limit)?;
        // Existing identity validation runs only after depth/node admission.
        scene.validate().ok()?;
        let stored_font_ctx = font_ctx.clone();
        let mut document = BaseDocument::new(DocumentConfig {
            font_ctx: Some(font_ctx),
            viewport: Some(Viewport {
                window_size: (width, height),
                ..Viewport::default()
            }),
            ..DocumentConfig::default()
        });
        let root = document.root_node().id;
        let body;
        {
            let mut tree = document.mutate();
            let name = |local: &str| {
                QualName::new(
                    None,
                    Namespace::from("http://www.w3.org/1999/xhtml"),
                    LocalName::from(local),
                )
            };
            let html = tree.create_element(name("html"), Vec::new());
            body = tree.create_element(name("body"), Vec::new());
            tree.set_style_property(body, "margin", "0px");
            tree.append_children(html, &[body]);
            tree.append_children(root, &[html]);
        }
        let mut view = Self {
            source: scene.clone(),
            font_ctx: stored_font_ctx,
            bold_aliases,
            document: ViewSlot::new(document),
            owner,
            scene_version: scene.version(),
            node_count: 0,
            nodes: BTreeMap::new(),
            semantics: BTreeMap::new(),
            parents: BTreeMap::new(),
            displayed: BTreeMap::new(),
            children: BTreeMap::new(),
            frame_dirty: true,
            frame_cache: None,
            families,
            last_pointer: None,
            motion: jasper_ui_engine::motion::MotionState::default(),
            material: MaterialState::default(),
            edit: scene_edit::EditState::default(),
            images: crate::image_resource::ImageStore::default(),
            overlays: scene_overlay::OverlayState::new(body),
        };
        if !view.append_prepared_subtree(&scene.root, None, body, prepared) {
            return None;
        }
        if !view.sync_native_focus_participation() {
            return None;
        }
        view.document.as_mut()?.resolve(0.0);
        if !view.apply_source_scrolls(&scene.root) {
            return None;
        }
        let configured = view.with_overlay_state(|overlays, view| overlays.configure(view));
        if !configured {
            return None;
        }
        view.with_overlay_state(|overlays, view| overlays.apply_auto_focus(view));
        Some(view)
    }
    /// Shared native construction for initial snapshots and admitted insertion.
    fn append_prepared_subtree(
        &mut self,
        root: &Node,
        source_parent: Option<NodeId>,
        parent: blitz_dom::NodeId,
        prepared: Vec<Vec<(&'static str, String)>>,
    ) -> bool {
        // Preserve Framework's inherited semantic hiding in native tab participation.
        // Native focus ordering stays in Blitz; raw exported semantics remain unchanged.
        let Some(inherited_hidden) =
            source_hidden_ancestry(&self.parents, &self.semantics, source_parent)
        else {
            return false;
        };
        let Some(document) = self.document.as_mut() else {
            return false;
        };
        let mut tree = document.mutate();
        let name = |local: &str| {
            QualName::new(
                None,
                Namespace::from("http://www.w3.org/1999/xhtml"),
                LocalName::from(local),
            )
        };
        let mut stack = vec![(root, parent, source_parent, inherited_hidden)];
        let mut index = 0;
        while let Some((node, parent, source_parent, inherited_hidden)) = stack.pop() {
            let hidden = inherited_hidden || node.semantics.hidden;
            let element = tree.create_element(
                name(if role_uses_native_button(node.semantics.role) {
                    "button"
                } else {
                    "div"
                }),
                Vec::new(),
            );
            self.semantics.insert(node.id, node.semantics.clone());
            self.parents.insert(node.id, source_parent);
            self.displayed
                .insert(node.id, node.style.display != Display::None);
            self.children.insert(
                node.id,
                node.children.iter().map(|child| child.id).collect(),
            );
            for (key, mut value) in semantic_attributes(&node.semantics) {
                if hidden && key == "tabindex" {
                    value = "-1".to_owned();
                }
                tree.set_attribute(
                    element,
                    QualName::new(None, Namespace::from(""), LocalName::from(key)),
                    &value,
                );
            }
            let text = if let NodeKind::Text(text) = &node.kind {
                let text = tree.create_text_node(text);
                tree.append_children(element, &[text]);
                Some(text)
            } else {
                None
            };
            self.nodes.insert(node.id, (element, text));
            for (property, value) in &prepared[index] {
                tree.set_style_property(element, property, value);
            }
            index += 1;
            tree.append_children(parent, &[element]);
            for child in node.children.iter().rev() {
                stack.push((child, element, Some(node.id), hidden));
            }
        }
        self.node_count = self.nodes.len();
        true
    }
    /// Consume source offsets through native layout data; no scrolling/clamping algorithm.
    fn apply_source_scrolls(&mut self, root: &Node) -> bool {
        let mut pending = vec![root];
        while let Some(source) = pending.pop() {
            let Some(offset) = native_scroll_offset(source.scroll) else {
                return false;
            };
            let Some((element, _)) = self.nodes.get(&source.id) else {
                return false;
            };
            let element = *element;
            let Some(document) = self.document.as_mut() else {
                return false;
            };
            let Some(node) = document.get_node_mut(element) else {
                return false;
            };
            *node.scroll_offset_mut() = offset;
            for child in source.children.iter().rev() {
                pending.push(child);
            }
        }
        true
    }
    /// Apply one complete frame to a candidate source/native view, then swap it into place.
    /// No active document or metadata is mutated before both stages succeed.
    pub(crate) fn apply_patches(
        &mut self,
        caller: u64,
        expected: u64,
        next: u64,
        patches: &[ScenePatch],
    ) -> bool {
        self.apply_patches_result(caller, expected, next, patches)
            .is_ok()
    }
    /// Structured form of frame admission and candidate construction.
    pub(crate) fn apply_patches_result(
        &mut self,
        caller: u64,
        expected: u64,
        next: u64,
        patches: &[ScenePatch],
    ) -> Result<(), SceneUpdateError> {
        self.apply_patches_result_within(caller, expected, next, patches, SceneQuota::UNLIMITED)
    }
    /// Frame transaction under a host quota. There is no per-frame operation limit: the whole
    /// frame is applied to a detached source copy and native candidate, then swapped in, so it is
    /// atomic however large it is. The quota is measured on the updated source before any native
    /// construction; exceeding it rejects the frame like any other failure (no version consumed).
    /// 宿主额度下的帧事务。没有每帧操作数上限：整帧应用到分离的源副本与原生候选上再整体换入，无论多大都
    /// 是原子的。额度在任何原生构造之前对更新后的源场景计量；超额与其他失败一样整帧拒绝（不消耗版本）。
    pub(crate) fn apply_patches_result_within(
        &mut self,
        caller: u64,
        expected: u64,
        next: u64,
        patches: &[ScenePatch],
        quota: SceneQuota,
    ) -> Result<(), SceneUpdateError> {
        let mut exceeded = None;
        let result = self.apply_patches_built_by(
            caller,
            expected,
            next,
            patches,
            |view, source, focused, patches| {
                let (nodes, text) = scene_usage(&source.root);
                if let Err(error) = admit_scene_quota(quota, nodes, text) {
                    exceeded = Some(error);
                    return None;
                }
                view.rebuild_candidate(
                    source,
                    focused,
                    patches,
                    quota.max_nodes.unwrap_or(usize::MAX),
                )
            },
        );
        match (result, exceeded) {
            (Err(SceneUpdateError::NativeRejected), Some(error)) => Err(error),
            (result, _) => result,
        }
    }
    /// The whole frame transaction with its native construction stage as a parameter. Both
    /// stages only hold `&self`, so a stage panic unwinds past a view nothing has written.
    fn apply_patches_built_by(
        &mut self,
        caller: u64,
        expected: u64,
        next: u64,
        patches: &[ScenePatch],
        build: impl FnOnce(&Self, Scene, Option<NodeId>, &[ScenePatch]) -> Option<Self>,
    ) -> Result<(), SceneUpdateError> {
        admit_scene_update(
            self.owner,
            caller,
            self.scene_version,
            expected,
            next,
            patches.len(),
            self.document.is_open(),
        )?;
        let focused = self
            .focused_node(caller, expected)
            .filter(|id| !self.frame_retires_node(*id, patches));
        let view = &*self;
        let candidate = order_scene_frame(
            next,
            || {
                let mut source = view.source.clone();
                let source_ok = source.apply_batch_at(expected, patches).is_ok();
                let source_version = source.version();
                (source_ok, source_version, source)
            },
            |source| build(view, source, focused, patches),
        )?;
        *self = candidate;
        Ok(())
    }
    /// Build and restore host-visible state in a detached candidate document.
    fn rebuild_candidate(
        &self,
        source: Scene,
        focused: Option<NodeId>,
        patches: &[ScenePatch],
        node_limit: usize,
    ) -> Option<Self> {
        let document = self.document.as_ref()?;
        let viewport = document.viewport().clone();
        let viewport_scroll = document.viewport_scroll();
        let pressed = document
            .get_mousedown_node_id()
            .and_then(|native| {
                self.nodes
                    .iter()
                    .find_map(|(id, (element, _))| (*element == native).then_some(*id))
            })
            .filter(|id| !self.frame_retires_node(*id, patches));
        let (width, height) = viewport.window_size;
        let previous_focus = self.focused_node(self.owner, self.scene_version);
        let mut candidate = Self::from_scene_with_context(
            self.owner,
            &source,
            crate::native_fonts::InjectedFonts {
                context: self.font_ctx.clone(),
                bold_aliases: self.bold_aliases.clone(),
            },
            width,
            height,
            node_limit,
        )?;
        restore_candidate_interaction(
            &mut candidate,
            CarriedInteraction {
                viewport,
                scroll: viewport_scroll,
                pressed,
                focused,
                pointer: self.last_pointer,
                frame_cache: self.frame_cache.clone(),
            },
        )?;
        // E7: carry the clock and running transitions; start new ones for restyled nodes whose
        // new style covers the changed property. No transition anywhere: no work.
        let restyled: BTreeSet<NodeId> = patches
            .iter()
            .filter_map(|patch| match patch {
                ScenePatch::UpdateStyle { id, style } if !style.transitions.is_empty() => Some(*id),
                _ => None,
            })
            .collect();
        let mut motion = self.motion.clone();
        if motion.running() || !restyled.is_empty() {
            motion.retarget(
                |id| self.source.find(id).map(|n| &n.style),
                |id| candidate.source.find(id).map(|n| &n.style),
                &restyled,
                |id| self.frame_retires_node(id, patches),
            );
        }
        candidate.motion = motion;
        if candidate.motion.running() {
            let animated: Vec<NodeId> = candidate.motion.animated_nodes().collect();
            candidate.write_presented(&animated);
        }
        // E5: the material budget is a host session setting (kept until the host sets it again)
        // and the report describes the last painted frame until the candidate paints its own.
        // E5：材质预算是宿主会话级设置（保持到宿主再次设置）；报告在候选首次绘制前仍描述上一个绘制帧。
        candidate.material = self.material.clone();
        // E14: the edit session survives frames (engine-local, no version of its own).
        candidate.carry_edit_from(self, patches);
        candidate.images = self.images.clone();
        candidate.images = self.images.clone();
        candidate
            .overlays
            .carry_from(&self.overlays, previous_focus);
        candidate.with_overlay_state(|overlays, candidate| {
            overlays.restore_removed_focus(
                candidate,
                &self.overlays,
                &self.parents,
                previous_focus,
            );
            overlays.apply_auto_focus(candidate);
        });
        Some(candidate)
    }
    /// A frame that removes a node also retires its native press/focus identity. Reusing
    /// the same source NodeId later in the frame must not transfer the old interaction state.
    fn frame_retires_node(&self, id: NodeId, patches: &[ScenePatch]) -> bool {
        patches.iter().any(|patch| match patch {
            ScenePatch::Remove { id: removed } => {
                source_subtree_contains(&self.parents, id, *removed)
            }
            _ => false,
        })
    }
    /// Publish actual final focus after a successful source patch batch.
    pub(crate) fn apply_patches_with_focus_events(
        &mut self,
        caller: u64,
        expected: u64,
        next: u64,
        patches: &[ScenePatch],
        sink: &mut impl FnMut(crate::scene_events::SceneFocusEvent),
    ) -> bool {
        let before = self.focused_node(caller, expected);
        if !self.apply_patches(caller, expected, next, patches) {
            return false;
        }
        let after = self.focused_node(caller, next);
        crate::scene_events::notify_focus_transition(next, before, after, sink);
        true
    }
    /// Native pointer defaults may change focus; report only the final observed transition.
    pub(crate) fn pointer_with_focus_events_from_host(
        &mut self,
        caller: u64,
        version: u64,
        phase: crate::native_view::PointerAction,
        x: f32,
        y: f32,
        pressed: bool,
        pointer_sink: &mut impl FnMut(
            crate::scene_events::SceneEvent,
        ) -> crate::native_events::NativeEventReply,
        focus_sink: &mut impl FnMut(crate::scene_events::SceneFocusEvent),
    ) -> bool {
        let before = self.focused_node(caller, version);
        if !self.pointer_from_host(caller, version, phase, x, y, pressed, pointer_sink) {
            return false;
        }
        let after = self.focused_node(caller, version);
        crate::scene_events::notify_focus_transition(version, before, after, focus_sink);
        true
    }
    /// Owner/version-admitted mouse projection; host capture remains a separate obligation.
    pub(crate) fn pointer_from_host(
        &mut self,
        caller: u64,
        version: u64,
        phase: crate::native_view::PointerAction,
        x: f32,
        y: f32,
        pressed: bool,
        sink: &mut impl FnMut(crate::scene_events::SceneEvent) -> crate::native_events::NativeEventReply,
    ) -> bool {
        self.pointer_input_from_host(
            caller,
            version,
            &crate::scene_events::ScenePointerInput {
                phase,
                position: (x, y),
                primary_pressed: pressed,
                modifiers: Default::default(),
            },
            sink,
        )
    }
    pub(crate) fn pointer_input_from_host(
        &mut self,
        caller: u64,
        version: u64,
        input: &crate::scene_events::ScenePointerInput,
        sink: &mut impl FnMut(crate::scene_events::SceneEvent) -> crate::native_events::NativeEventReply,
    ) -> bool {
        self.invalidate_frame();
        use crate::native_view::PointerAction;
        use blitz_traits::events::*;
        let (x, y) = input.position;
        let phase = input.phase;
        let pressed = input.primary_pressed;
        if caller != self.owner || version != self.scene_version || !x.is_finite() || !y.is_finite()
        {
            return false;
        }
        let (consume, _) = self.with_overlay_state(|overlays, view| {
            overlays.pointer_gate(view, x, y, phase, pressed_primary(input))
        });
        if consume {
            if let Some(document) = self.document.as_mut() {
                if matches!(phase, PointerAction::Down | PointerAction::Up | PointerAction::Cancel) {
                    document.set_mousedown_node_id(None);
                }
            }
            return true;
        }
        self.last_pointer = Some((x, y));
        let adjustment = self.pointer_hit_adjustment(x, y);
        let (target_x, target_y) = adjustment.steer.unwrap_or((x, y));
        let Some(document) = self.document.as_mut() else {
            return false;
        };
        let scroll = document.viewport_scroll();
        let page_x = target_x as f64 + scroll.x;
        let page_y = target_y as f64 + scroll.y;
        if !page_x.is_finite()
            || !page_y.is_finite()
            || page_x.abs() > f32::MAX as f64
            || page_y.abs() > f32::MAX as f64
        {
            return false;
        }
        let mut modifiers = keyboard_types::Modifiers::empty();
        if input.modifiers.shift {
            modifiers.insert(keyboard_types::Modifiers::SHIFT);
        }
        if input.modifiers.control {
            modifiers.insert(keyboard_types::Modifiers::CONTROL);
        }
        if input.modifiers.alt {
            modifiers.insert(keyboard_types::Modifiers::ALT);
        }
        if input.modifiers.meta {
            modifiers.insert(keyboard_types::Modifiers::META);
        }
        let pointer = BlitzPointerEvent {
            id: BlitzPointerId::Mouse,
            is_primary: true,
            coords: PointerCoords {
                page_x: page_x as f32,
                page_y: page_y as f32,
                client_x: target_x,
                client_y: target_y,
                screen_x: target_x,
                screen_y: target_y,
            },
            button: MouseEventButton::Main,
            buttons: if pressed {
                MouseEventButtons::Primary
            } else {
                MouseEventButtons::None
            },
            mods: modifiers,
            details: PointerDetails::default(),
            element: Point::default(),
            active_pointers: Default::default(),
        };
        let event = match phase {
            PointerAction::Down => UiEvent::PointerDown(pointer),
            PointerAction::Move => UiEvent::PointerMove(pointer),
            PointerAction::Up => UiEvent::PointerUp(pointer),
            PointerAction::Cancel => UiEvent::PointerCancel(pointer),
            PointerAction::Leave => UiEvent::PointerLeave(pointer),
        };
        // Native press identity is authoritative; retired/no press cannot activate a replacement.
        let suppress_unpaired_up =
            phase == PointerAction::Up && document.get_mousedown_node_id().is_none();
        let pressed = document.get_mousedown_node_id().and_then(|native| {
            self.nodes.iter().find_map(|(id, (element, text))| {
                (*element == native || *text == Some(native)).then_some(*id)
            })
        });
        let focused_before = document.get_focussed_node_id();
        // First routed event of this sample's own phase: (source target, default prevented).
        let mut hit: Option<(NodeId, bool)> = None;
        let mut observe = |event: crate::scene_events::SceneEvent| {
            let target = event.node;
            let own = event.kind == crate::scene_events::SceneEventKind::Pointer(phase);
            let reply = sink(event);
            if own && hit.is_none() {
                hit = Some((target, reply.prevent_default));
            }
            reply
        };
        let handler = crate::scene_events::SceneEventHandler {
            version,
            suppress_unpaired_up,
            nodes: &self.nodes,
            semantics: &self.semantics,
            parents: &self.parents,
            pressed,
            exclude: adjustment.exclude,
            reported_position: adjustment.steer.map(|_| (x, y)),
            sink: &mut observe,
        };
        blitz_dom::EventDriver::new(document, handler).handle_ui_event(event);
        if matches!(phase, PointerAction::Up | PointerAction::Cancel) {
            document.set_mousedown_node_id(None);
        }
        // Pinned Blitz focuses only native form inputs on press and clears focus after any click
        // it does not recognise. Like a browser: a primary press whose default is not prevented
        // focuses the nearest focus-eligible source ancestor, and the release of a click inside
        // the control that already held focus keeps it there.
        // 固定版 Blitz 按下时只聚焦原生表单输入，且对不认识的点击一律清除焦点。与浏览器一致：未阻止默认
        // 行为的主键按下聚焦最近的可聚焦源祖先；在已持有焦点的控件内完成的点击，抬起后焦点仍留在该控件上。
        let focus_target = match (phase, hit) {
            (PointerAction::Down, Some((target, false))) if pressed_primary(input) => {
                self.nearest_focus_eligible(target)
            }
            (PointerAction::Up, Some((target, _))) => {
                self.nearest_focus_eligible(target).filter(|id| {
                    self.nodes
                        .get(id)
                        .is_some_and(|(element, _)| focused_before == Some(*element))
                })
            }
            _ => None,
        };
        let element = focus_target.and_then(|id| self.nodes.get(&id).map(|(element, _)| *element));
        let Some(document) = self.document.as_mut() else {
            return false;
        };
        if let Some(element) = element {
            if document.get_focussed_node_id() != Some(element) {
                document.set_focus_to(element);
            }
        }
        document.resolve(0.0);
        // E14: blur commits, a newly focused field starts editing, then caret placement.
        self.sync_edit_session(true);
        self.edit_pointer(
            phase,
            hit,
            (x, y),
            pressed_primary(input),
            input.modifiers.shift,
        );
        true
    }
    /// Nearest source ancestor-or-self that `focus_eligible` admits; bounded by the node count.
    fn nearest_focus_eligible(&self, id: NodeId) -> Option<NodeId> {
        let mut current = Some(id);
        let mut remaining = self.parents.len();
        while let Some(cursor) = current {
            if self.focus_eligible(cursor) {
                return Some(cursor);
            }
            if remaining == 0 {
                return None;
            }
            remaining -= 1;
            current = self.parents.get(&cursor).copied().flatten();
        }
        None
    }
    /// Route only to the admitted currently focused source node; no editor/default text commit.
    /// Reports no activation; `key_with_activation_from_host` adds role keyboard activation.
    pub(crate) fn key_from_host(
        &mut self,
        caller: u64,
        version: u64,
        input: &crate::native_keys::HostKeyInput<'_>,
        sink: &mut impl FnMut(
            crate::scene_events::SceneKeyEvent,
        ) -> crate::native_events::NativeEventReply,
        focus: &mut impl FnMut(crate::scene_events::SceneFocusEvent),
    ) -> bool {
        self.key_with_activation_from_host(caller, version, input, sink, focus, &mut |_| {})
    }
    /// `key_from_host` plus role keyboard activation of the focused node
    /// (`role_activates_on_key`): on a non-repeating, non-composing key press without
    /// ctrl/alt/meta whose default the key sink did not prevent, an enabled visible focused
    /// control reports one activation after the key event.
    /// 在 `key_from_host` 基础上加按角色的键盘激活（`role_activates_on_key`）：非重复、非组合输入、
    /// 无 ctrl/alt/meta、且键事件未被阻止默认行为的按下，聚焦的已启用可见控件在键事件之后报告一次激活。
    pub(crate) fn key_with_activation_from_host(
        &mut self,
        caller: u64,
        version: u64,
        input: &crate::native_keys::HostKeyInput<'_>,
        sink: &mut impl FnMut(
            crate::scene_events::SceneKeyEvent,
        ) -> crate::native_events::NativeEventReply,
        focus: &mut impl FnMut(crate::scene_events::SceneFocusEvent),
        activation: &mut impl FnMut(crate::scene_events::SceneActivationEvent),
    ) -> bool {
        self.invalidate_frame();
        use blitz_traits::events::UiEvent;
        if caller != self.owner || version != self.scene_version {
            return false;
        }
        let before = self.focused_node(caller, version);
        let escape = input.key == "Escape" && input.pressed && !input.composing;
        if input.key == "Tab"
            && input.pressed
            && !input.composing
            && !input.modifiers.control
            && !input.modifiers.alt
            && !input.modifiers.meta
        {
            if let Some(target) =
                self.overlays
                    .tab_target(self, before, input.modifiers.shift)
            {
                if let Some(target) = target {
                    let _ = self.focus_from_host(caller, version, target, focus);
                }
                return true;
            }
        }
        if before.is_some_and(|id| !self.overlays.allows_interaction(self, id)) {
            if escape {
                self.with_overlay_state(|overlays, view| overlays.escape_request(view));
            }
            return false;
        }
        let node = match before {
            Some(node) if self.focus_eligible(node) => node,
            None if input.key == "Tab" && input.pressed && !input.composing => {
                let Some(root) = self
                    .parents
                    .iter()
                    .find_map(|(id, parent)| parent.is_none().then_some(*id))
                else {
                    return false;
                };
                root
            }
            _ if escape => {
                self.with_overlay_state(|overlays, view| overlays.escape_request(view));
                return true;
            }
            _ => return false,
        };
        let Some((element, _)) = self.nodes.get(&node) else {
            return false;
        };
        let element = *element;
        let Some(key) = crate::native_keys::native_command_key(input) else {
            return false;
        };
        let Some(document) = self.document.as_mut() else {
            return false;
        };
        let event = if input.pressed {
            UiEvent::KeyDown(key)
        } else {
            UiEvent::KeyUp(key)
        };
        let mut prevented = false;
        let mut observe = |event: crate::scene_events::SceneKeyEvent| {
            let reply = sink(event);
            prevented |= reply.prevent_default;
            reply
        };
        let handler = crate::scene_events::SceneKeyHandler {
            version,
            node,
            element,
            sink: &mut observe,
        };
        blitz_dom::EventDriver::new(document, handler).handle_ui_event(event);
        document.resolve(0.0);
        if escape && !prevented {
            self.with_overlay_state(|overlays, view| overlays.escape_request(view));
        }
        // E14: an unprevented key on the edited field goes to the engine editor.
        let edited = before == Some(node) && !prevented && self.edit_key(node, input);
        let activates = !edited
            && before == Some(node)
            && input.pressed
            && !input.repeat
            && !input.composing
            && !input.modifiers.control
            && !input.modifiers.alt
            && !input.modifiers.meta
            && !prevented
            && self.semantics.get(&node).is_some_and(|s| {
                !s.disabled && crate::scene_events::role_activates_on_key(s.role, input.key)
            })
            && self.source_visible(node);
        if activates {
            activation(crate::scene_events::SceneActivationEvent {
                scene_version: version,
                node,
            });
        }
        self.sync_edit_session(true);
        let after = self.focused_node(caller, version);
        crate::scene_events::notify_focus_transition(version, before, after, focus);
        true
    }
    /// Native focus query projects only own stable scene identity.
    pub(crate) fn focused_node(&self, caller: u64, version: u64) -> Option<NodeId> {
        if caller != self.owner || version != self.scene_version {
            return None;
        }
        let focused = self.document.as_ref()?.get_focussed_node_id()?;
        self.nodes
            .iter()
            .find_map(|(id, (element, _))| (*element == focused).then_some(*id))
    }
    /// Project source participation after native construction/style batches; Blitz orders Tab.
    /// Source semantics stay raw; this does not compute geometry or choose the next focus.
    fn sync_native_focus_participation(&mut self) -> bool {
        let entries: Vec<_> = self
            .nodes
            .iter()
            .filter_map(|(id, (element, _))| {
                let semantics = self.semantics.get(id)?;
                (semantics.focusable || semantics.role == Role::Button)
                    .then(|| (*element, self.focus_eligible(*id)))
            })
            .collect();
        let Some(document) = self.document.as_mut() else {
            return false;
        };
        {
            let mut tree = document.mutate();
            for (element, eligible) in entries {
                tree.set_attribute(
                    element,
                    QualName::new(None, Namespace::from(""), LocalName::from("tabindex")),
                    if eligible { "0" } else { "-1" },
                );
            }
        }
        document.resolve(0.0);
        true
    }
    /// Source ancestry metadata only; native layout and visibility geometry stay upstream.
    fn focus_eligible(&self, id: NodeId) -> bool {
        if !self
            .semantics
            .get(&id)
            .is_some_and(|s| s.focusable && !s.disabled)
        {
            return false;
        }
        self.source_visible(id)
    }
    fn source_visible(&self, id: NodeId) -> bool {
        let mut current = Some(id);
        let mut remaining = self.parents.len();
        loop {
            let cursor = match current {
                Some(cursor) => cursor,
                None => return true,
            };
            if remaining == 0 {
                return false;
            }
            remaining -= 1;
            if !match self.displayed.get(&cursor) {
                Some(value) => *value,
                None => false,
            } {
                return false;
            }
            if match self.semantics.get(&cursor) {
                Some(value) => value.hidden,
                None => true,
            } {
                return false;
            }
            current = match self.parents.get(&cursor) {
                Some(parent) => *parent,
                None => return false,
            };
        }
    }

    /// Logical viewport used by Framework overlay placement. The host supplies a physical
    /// Surface extent plus DPI; the native logical layout coordinate is that extent divided by
    /// the scale.
    /// Framework 浮层定位使用的逻辑视口。宿主提供物理 Surface 尺寸与 DPI；原生逻辑布局坐标为
    /// 该尺寸除以缩放。
    fn viewport_logical(&self) -> crate::style_effects::LogicalBox {
        let viewport = self
            .document
            .as_ref()
            .map(|document| document.viewport().clone())
            .unwrap_or_default();
        let scale = if viewport.hidpi_scale.is_finite() && viewport.hidpi_scale > 0.0 {
            viewport.hidpi_scale
        } else {
            1.0
        };
        crate::style_effects::LogicalBox {
            x: 0.0,
            y: 0.0,
            width: viewport.window_size.0 as f32 / scale,
            height: viewport.window_size.1 as f32 / scale,
        }
    }

    /// Run one overlay operation with separate borrows of the overlay state and scene.
    /// 在对场景与浮层状态分别可变借用的情况下执行一次浮层操作。
    fn with_overlay_state<R>(
        &mut self,
        operation: impl FnOnce(&mut scene_overlay::OverlayState, &mut Self) -> R,
    ) -> R {
        let body = self.overlays.body();
        let mut overlays =
            std::mem::replace(&mut self.overlays, scene_overlay::OverlayState::new(body));
        let result = operation(&mut overlays, self);
        self.overlays = overlays;
        result
    }

    /// Host-authenticated semantic action; no fabricated pointer, key or focus request.
    /// Scene version prevents stale routing, not duplicate actions; host owns replay policy.
    pub(crate) fn activate_from_host(
        &self,
        caller: u64,
        version: u64,
        id: NodeId,
        sink: &mut impl FnMut(crate::scene_events::SceneActivationEvent),
    ) -> bool {
        if caller != self.owner || self.document.as_ref().is_none() || !self.nodes.contains_key(&id)
        {
            return false;
        }
        if !self.overlays.allows_interaction(self, id) {
            return false;
        }
        let can_activate =
            self.semantics.get(&id).is_some_and(|s| {
                crate::scene_events::role_activates_on_click(s.role) && !s.disabled
            }) && self.source_visible(id);
        if !crate::native_semantics::accepts_activation(self.scene_version, version, can_activate) {
            return false;
        }
        sink(crate::scene_events::SceneActivationEvent {
            scene_version: version,
            node: id,
        });
        true
    }
    /// Framework focus request; does not acquire OS focus or an external input lease.
    pub(crate) fn focus_from_host(
        &mut self,
        caller: u64,
        version: u64,
        id: NodeId,
        sink: &mut impl FnMut(crate::scene_events::SceneFocusEvent),
    ) -> bool {
        self.invalidate_frame();
        if caller != self.owner
            || version != self.scene_version
            || !self.focus_eligible(id)
            || !self.overlays.allows_interaction(self, id)
        {
            return false;
        }
        let Some((element, _)) = self.nodes.get(&id) else {
            return false;
        };
        let element = *element;
        let before = self.focused_node(caller, version);
        let Some(document) = self.document.as_mut() else {
            return false;
        };
        document.set_focus_to(element);
        document.resolve(0.0);
        self.sync_edit_session(true);
        let after = self.focused_node(caller, version);
        if before != after {
            if let Some(node) = before {
                sink(crate::scene_events::SceneFocusEvent {
                    scene_version: version,
                    node,
                    focused: false,
                });
            }
            if let Some(node) = after {
                sink(crate::scene_events::SceneFocusEvent {
                    scene_version: version,
                    node,
                    focused: true,
                });
            }
        }
        after == Some(id)
    }
    /// Idempotent native focus clear; external input/capture cancellation remains host-owned.
    pub(crate) fn clear_focus_from_host(
        &mut self,
        caller: u64,
        version: u64,
        sink: &mut impl FnMut(crate::scene_events::SceneFocusEvent),
    ) -> bool {
        self.invalidate_frame();
        if caller != self.owner || version != self.scene_version {
            return false;
        }
        let before = self.focused_node(caller, version);
        let Some(document) = self.document.as_mut() else {
            return false;
        };
        document.clear_focus();
        document.resolve(0.0);
        self.sync_edit_session(true);
        if let Some(node) = before {
            sink(crate::scene_events::SceneFocusEvent {
                scene_version: version,
                node,
                focused: false,
            });
        }
        true
    }
    /// Host deactivation cleanup only; host still owns capture release and native input protocols.
    pub(crate) fn deactivate_from_host(
        &mut self,
        caller: u64,
        version: u64,
        sink: &mut impl FnMut(crate::scene_events::SceneFocusEvent),
    ) -> bool {
        self.invalidate_frame();
        if caller != self.owner || version != self.scene_version {
            return false;
        }
        let before = self.focused_node(caller, version);
        let Some(document) = self.document.as_mut() else {
            return false;
        };
        self.last_pointer = None;
        document.unactive_node();
        document.set_mousedown_node_id(None);
        document.clear_hover();
        document.clear_focus();
        document.resolve(0.0);
        self.sync_edit_session(true);
        crate::scene_events::notify_focus_transition(version, before, None, sink);
        true
    }
    /// Own metadata projection, not a computed accessible-name tree or native OS bridge.
    pub(crate) fn semantics_from_host(&self, caller: u64, id: NodeId) -> Option<&Semantics> {
        if caller != self.owner || !self.document.is_open() {
            return None;
        }
        self.semantics.get(&id)
    }

    pub(crate) fn insert_image_from_host(
        &mut self,
        caller: u64,
        id: &str,
        bytes: &[u8],
        density: f32,
    ) -> Result<crate::image_resource::ImageInfo, crate::image_resource::ImageError> {
        if caller != self.owner {
            return Err(crate::image_resource::ImageError::Rejected);
        }
        if !self.document.is_open() {
            return Err(crate::image_resource::ImageError::Rejected);
        }
        let info = self.images.insert(id, bytes, density)?;
        self.invalidate_frame();
        Ok(info)
    }

    pub(crate) fn remove_image_from_host(
        &mut self,
        caller: u64,
        id: &str,
    ) -> Option<bool> {
        if caller != self.owner || !self.document.is_open() {
            return None;
        }
        let removed = self.images.remove(id);
        self.invalidate_frame();
        Some(removed)
    }

    pub(crate) fn set_image_limits_from_host(
        &mut self,
        caller: u64,
        limits: crate::image_resource::ImageLimits,
    ) -> Option<Result<(), crate::image_resource::ImageError>> {
        if caller != self.owner || !self.document.is_open() {
            return None;
        }
        let result = self.images.set_limits(limits);
        if result.is_ok() {
            self.invalidate_frame();
        }
        Some(result)
    }

    pub(crate) fn image_report_from_host(
        &self,
        caller: u64,
    ) -> Option<crate::image_resource::ImageReport> {
        if caller != self.owner || !self.document.is_open() {
            return None;
        }
        Some(self.images.report())
    }
    pub(crate) fn overlays_from_host(
        &self,
        caller: u64,
        version: u64,
    ) -> Option<Vec<crate::overlay::OverlayInfo>> {
        if caller != self.owner || version != self.scene_version || !self.document.is_open() {
            return None;
        }
        Some(self.overlays.info(self))
    }

    pub(crate) fn take_overlay_events_from_host(
        &mut self,
        caller: u64,
    ) -> Option<Vec<crate::overlay::OverlayEvent>> {
        if caller != self.owner {
            return None;
        }
        Some(self.overlays.take_events())
    }

    /// Host supplies physical surface extent and DPI; scene version is independent.
    /// Candidate scale bound matches text-view policy; no window/GPU/OS operation is owned here.
    pub(crate) fn set_viewport_from_host(
        &mut self,
        caller: u64,
        width: u32,
        height: u32,
        scale: f32,
    ) -> bool {
        self.invalidate_frame();
        if caller != self.owner || !admits_native_dpi_scale_bits(scale.to_bits()) {
            return false;
        }
        let Some(document) = self.document.as_mut() else {
            return false;
        };
        let mut viewport = document.viewport().clone();
        viewport.window_size = (width, height);
        viewport.hidpi_scale = scale;
        document.set_viewport(viewport);
        document.resolve(0.0);
        true
    }
    /// Rebuild the exact device-pixel raster of every displayed image node into Blitz's
    /// background-image slot. The displayed pixels therefore use the same document painting for
    /// CPU and GPU; misses and quota failures paint the node background and are reported.
    /// 把每个已显示图片节点按精确设备像素尺寸生成的光栅放入 Blitz 的 background-image 槽位。
    /// 因此 CPU 与 GPU 共用同一文档绘制；缺失与额度失败保留节点背景并进入报告。
    fn sync_images(&mut self) -> bool {
        self.images.begin_frame();
        let scale = self
            .document
            .as_ref()
            .map(|document| document.viewport().scale_f64())
            .unwrap_or(1.0);
        let images: Vec<_> = self
            .source_preorder()
            .into_iter()
            .filter_map(|node| {
                self.source_displayed(node.id)
                    .then(|| {
                        let style = self.presented_of(node);
                        style.image.clone().map(|image| (node.id, image, style.color))
                    })
                    .flatten()
            })
            .collect();
        for (id, style, color) in images {
            let Some(border) = self.layout_box(id) else {
                continue;
            };
            let Some((element, _)) = self.nodes.get(&id).copied() else {
                return false;
            };
            let Some(edges) = self
                .document
                .as_ref()
                .and_then(|document| document.get_node(element))
                .map(|node| node.final_layout())
            else {
                return false;
            };
            let content_width = (border.width
                - edges.border.left
                - edges.border.right
                - edges.padding.left
                - edges.padding.right)
                .max(0.0);
            let content_height = (border.height
                - edges.border.top
                - edges.border.bottom
                - edges.padding.top
                - edges.padding.bottom)
                .max(0.0);
            let width = (f64::from(content_width) * scale).ceil().max(0.0) as u32;
            let height = (f64::from(content_height) * scale).ceil().max(0.0) as u32;
            let raster = self.images.raster(&style, color, width, height, scale);
            let Some(image_data) = self
                .document
                .as_mut()
                .and_then(|document| document.get_node_mut(element))
                .and_then(|node| node.element_data_mut())
                .and_then(|element| element.background_images.first_mut())
                .and_then(Option::as_mut)
            else {
                self.images
                    .fail(id, &style.source, crate::image_resource::ImageNodeError::Missing);
                continue;
            };
            match raster {
                Ok(raster) => {
                    image_data.status = blitz_dom::node::Status::Ok;
                    image_data.image = blitz_dom::node::ImageData::Raster(raster);
                }
                Err(error) => {
                    image_data.status = blitz_dom::node::Status::Error;
                    image_data.image = blitz_dom::node::ImageData::None;
                    self.images.fail(id, &style.source, error);
                }
            }
        }
        true
    }
    /// Presented opacity product of the node and its ancestors, and the padding boxes of the
    /// clipping ones (the node itself included when `own_clip`). Without E8 transforms on the
    /// chain the boxes are intersected into one; with them each box keeps its own chain matrix.
    /// None: clipped away.
    #[cfg(feature = "cpu-pending")]
    fn paint_context(&self, id: NodeId, own_clip: bool) -> Option<(f32, Vec<OverlayClip>)> {
        let mut alpha = 1.0;
        let mut clips: Vec<OverlayClip> = Vec::new();
        let mut chain = vec![id];
        chain.extend(self.source_ancestors(id));
        for (index, node) in chain.into_iter().enumerate() {
            let Some(style) = self.style_of(node) else {
                continue;
            };
            alpha *= style.opacity;
            if style.overflow != Overflow::Visible && (index > 0 || own_clip) {
                let b = self.padding_box(node)?;
                let matrix = self.chain_matrix(node);
                match clips.last_mut() {
                    Some(last) if matrix.is_none() && last.matrix.is_none() => {
                        last.rect = last.rect.intersect(&b)?;
                    }
                    _ => clips.push(OverlayClip { rect: b, matrix }),
                }
            }
        }
        Some((alpha, clips))
    }
    /// Target style of a source node with its running transition values (E7): what is painted.
    /// 源节点的目标样式叠加运行中过渡的当前值（E7）：即实际绘制的样式。
    fn style_of(&self, id: NodeId) -> Option<std::borrow::Cow<'_, Style>> {
        let node = self.source.find(id)?;
        Some(self.presented_of(node))
    }
    fn presented_of<'a>(&'a self, node: &'a Node) -> std::borrow::Cow<'a, Style> {
        match self.motion.presented(node.id, &node.style) {
            Some(style) => std::borrow::Cow::Owned(style),
            None => std::borrow::Cow::Borrowed(&node.style),
        }
    }
    /// E8: presented transform of a node and every ancestor composed into one view-local
    /// logical affine (`[a, b, c, d, e, f]`, local layout coordinates → view). None when the whole
    /// chain is untransformed.
    /// E8：节点及其所有祖先的呈现变换合成的视图局部逻辑仿射（局部布局坐标 → 视图）；整条链都无变换时为 None。
    fn chain_matrix(&self, id: NodeId) -> Option<[f64; 6]> {
        let mut acc: Option<[f64; 6]> = None;
        for node in std::iter::once(id).chain(self.source_ancestors(id)) {
            let Some(style) = self.style_of(node) else {
                continue;
            };
            if style.transform.is_identity() {
                continue;
            }
            let Some(b) = self.layout_box(node) else {
                continue;
            };
            let m = style.transform.matrix(
                f64::from(b.x),
                f64::from(b.y),
                f64::from(b.width),
                f64::from(b.height),
            );
            acc = Some(match acc {
                None => m,
                Some(inner) => compose(m, inner),
            });
        }
        acc
    }
    /// A view point in `id`'s untransformed layout coordinates (identity without transforms;
    /// None for a singular transform).
    /// 视图点在 `id` 未变换布局坐标中的位置（无变换时不变；奇异变换为 None）。
    fn to_local(&self, id: NodeId, x: f32, y: f32) -> Option<(f32, f32)> {
        match self.chain_matrix(id) {
            None => Some((x, y)),
            Some(m) => {
                let (lx, ly) = apply(invert(m)?, f64::from(x), f64::from(y));
                let out = (lx as f32, ly as f32);
                (out.0.is_finite() && out.1.is_finite()).then_some(out)
            }
        }
    }
    fn to_view(&self, id: NodeId, x: f32, y: f32) -> (f32, f32) {
        match self.chain_matrix(id) {
            None => (x, y),
            Some(m) => {
                let (vx, vy) = apply(m, f64::from(x), f64::from(y));
                (vx as f32, vy as f32)
            }
        }
    }
    /// Inline layout painted for a source text node and its content origin in view-local logical
    /// px: the inline root must be the element or the anonymous box directly inside it.
    #[cfg(feature = "cpu-pending")]
    fn painted_text_layout(&self, id: NodeId) -> Option<(&blitz_dom::node::TextLayout, f64, f64)> {
        let (element, Some(text)) = self.nodes.get(&id)? else {
            return None;
        };
        let document = self.document.as_ref()?;
        let root = document.get_node(*text)?.inline_root_ancestor()?;
        if root.id != *element && root.parent != Some(*element) {
            return None;
        }
        let layout = root.element_data()?.inline_layout_data.as_deref()?;
        let rect = document.get_client_bounding_rect(root.id)?;
        let edges = root.final_layout();
        let x = rect.x + edges.border.left as f64 + edges.padding.left as f64;
        let y = rect.y
            + edges.border.top as f64
            + edges.padding.top as f64
            + layout.block_offset as f64;
        (x.is_finite() && y.is_finite()).then_some((layout, x, y))
    }
    /// E9 text decoration of one text node: every decorating ancestor-or-self contributes its
    /// lines in its own text colour (CSS propagation), one line per laid-out line spanning the
    /// line's glyph runs. Geometry follows Blitz: underline at the font's underline offset,
    /// line-through a third of the ascent above the baseline, thickness floor(metric) >= 1 px.
    /// E9 一个文字节点的文字装饰：每个带装饰的自身或祖先以其文字颜色贡献装饰线（CSS 传播），每个排版行一条，覆盖该行
    /// 的字形串。几何同 Blitz：下划线在字体建议位置，删除线在基线上方三分之一 ascent 处，粗细 floor(度量) 且至少 1px。
    #[cfg(feature = "cpu-pending")]
    fn text_decorations(&self, id: NodeId, out: &mut Vec<Overlay>) {
        let mut decorating = Vec::new();
        for node in std::iter::once(id).chain(self.source_ancestors(id)) {
            if let Some(style) = self.style_of(node) {
                if style.text_decoration != TextDecoration::NONE {
                    decorating.push((style.text_decoration, style.color));
                }
            }
        }
        if decorating.is_empty() || !self.source_displayed(id) {
            return;
        }
        let Some((layout, origin_x, origin_y)) = self.painted_text_layout(id) else {
            return;
        };
        let Some((alpha, clips)) = self.paint_context(id, true) else {
            return;
        };
        let matrix = self.chain_matrix(id);
        let scale = layout.layout.scale() as f64;
        if !(scale > 0.0) || !scale.is_finite() {
            return;
        }
        for line in layout.layout.lines() {
            let mut extent: Option<(f32, f32)> = None;
            let mut geometry = None;
            for item in line.items() {
                let parley::PositionedLayoutItem::GlyphRun(run) = item else {
                    continue;
                };
                if run.advance() <= 0.0 {
                    continue;
                }
                let (x0, x1) = (run.offset(), run.offset() + run.advance());
                extent = Some(extent.map_or((x0, x1), |(a, b)| (a.min(x0), b.max(x1))));
                if geometry.is_none() {
                    geometry = Some((run.baseline(), *run.run().font_metrics()));
                }
            }
            let (Some((x0, x1)), Some((baseline, metrics))) = (extent, geometry) else {
                continue;
            };
            for (decoration, color) in &decorating {
                let mut lines = Vec::new();
                if decoration.underline {
                    lines.push((metrics.underline_offset, metrics.underline_size));
                }
                if decoration.line_through {
                    let size = metrics.strikethrough_size.floor().max(1.0);
                    lines.push((metrics.ascent / 3.0 + size / 2.0, size));
                }
                for (offset, size) in lines {
                    let size = size.floor().max(1.0);
                    let top = baseline - offset;
                    let a = (f32::from(color.a) * alpha).round();
                    if a <= 0.0 {
                        continue;
                    }
                    let Some(outer) = Some(crate::style_effects::LogicalBox {
                        x: (origin_x + x0 as f64 / scale) as f32,
                        y: (origin_y + top as f64 / scale) as f32,
                        width: ((x1 - x0) as f64 / scale) as f32,
                        height: (size as f64 / scale) as f32,
                    })
                    .filter(|b| [b.x, b.y, b.width, b.height].iter().all(|v| v.is_finite())) else {
                        continue;
                    };
                    out.push(Overlay {
                        outer,
                        outer_radii: [0.0; 4],
                        inner: None,
                        rgba: [color.r, color.g, color.b, a.clamp(0.0, 255.0) as u8],
                        clips: clips.clone(),
                        matrix,
                    });
                }
            }
        }
    }
    /// Source nodes in pre-order (paint order of siblings); explicit stack, no recursion.
    fn source_preorder(&self) -> Vec<&Node> {
        let mut out = Vec::new();
        let mut stack = vec![&self.source.root];
        while let Some(node) = stack.pop() {
            out.push(node);
            stack.extend(node.children.iter().rev());
        }
        out
    }
    /// Deepest mapped source node under a view-local sample. The pinned backend performs the
    /// native hit test; this function only projects its ancestor chain back to source identity.
    fn native_pointer_target(&self, x: f32, y: f32) -> Option<NodeId> {
        let document = self.document.as_ref()?;
        let scroll = document.viewport_scroll();
        let mut reverse = BTreeMap::new();
        for (id, (element, text)) in &self.nodes {
            reverse.insert(*element, *id);
            if let Some(text) = text {
                reverse.insert(*text, *id);
            }
        }
        let limit = self.nodes.len().saturating_mul(4).saturating_add(16);
        let hit = document.hit((x as f64 + scroll.x) as f32, (y as f64 + scroll.y) as f32)?;
        let mut cursor = Some(hit.node_id);
        for _ in 0..limit {
            let current = cursor?;
            if let Some(id) = reverse.get(&current) {
                return Some(*id);
            }
            cursor = document.get_node(current)?.parent;
        }
        None
    }
    /// Occlusion- and overlay-aware source target for the same sample coordinates accepted by
    /// `pointer`. This is a read-only query; press events and focus remain in pointer routing.
    pub(crate) fn pointer_target_from_host(
        &self,
        caller: u64,
        version: u64,
        x: f32,
        y: f32,
    ) -> Option<NodeId> {
        if caller != self.owner
            || version != self.scene_version
            || !x.is_finite()
            || !y.is_finite()
            || self
                .overlays
                .blocks_outside_at(self, x, y)
        {
            return None;
        }
        let adjustment = self.pointer_hit_adjustment(x, y);
        let (target_x, target_y) = adjustment.steer.unwrap_or((x, y));
        let target = self.native_pointer_target(target_x, target_y)?;
        let target = match adjustment.exclude {
            Some(excluded) if source_subtree_contains(&self.parents, target, excluded) => {
                self.parents.get(&excluded).copied().flatten()?
            }
            _ => target,
        };
        self.source_visible(target).then_some(target)
    }
    /// E5: displayed nodes whose presented style has a material, with their logical border-box
    /// size and presented radii.
    /// E5：呈现样式带材质的已显示节点，附逻辑边框盒尺寸与呈现圆角。
    #[cfg(feature = "cpu-pending")]
    fn glass_specs(&self) -> BTreeMap<NodeId, crate::material::GlassSpec> {
        let mut out = BTreeMap::new();
        for node in self.source_preorder() {
            let style = self.presented_of(node);
            let Some(backdrop) = style.backdrop else {
                continue;
            };
            if !self.source_displayed(node.id) {
                continue;
            }
            if let Some(b) = self.layout_box(node.id) {
                out.insert(
                    node.id,
                    crate::material::GlassSpec {
                        backdrop,
                        radii: style.corner_radius,
                        width: b.width,
                        height: b.height,
                    },
                );
            }
        }
        out
    }
    /// Displayed (no `display: none` on the node or an ancestor); unlike `source_visible`,
    /// semantic hiding does not hide paint. Bounded by the node count.
    fn source_displayed(&self, id: NodeId) -> bool {
        let mut current = Some(id);
        let mut remaining = self.parents.len() + 1;
        while let Some(cursor) = current {
            if remaining == 0 || !self.displayed.get(&cursor).copied().unwrap_or(false) {
                return false;
            }
            remaining -= 1;
            current = self.parents.get(&cursor).copied().flatten();
        }
        true
    }
    /// Strict source ancestors, nearest first; bounded by the node count.
    fn source_ancestors(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut current = self.parents.get(&id).copied().flatten();
        while let Some(cursor) = current {
            if out.len() > self.parents.len() {
                break;
            }
            out.push(cursor);
            current = self.parents.get(&cursor).copied().flatten();
        }
        out
    }
    /// Border box in view-local logical coordinates, including a node that is itself scrolled.
    fn layout_box(&self, id: NodeId) -> Option<crate::style_effects::LogicalBox> {
        let (element, _) = self.nodes.get(&id)?;
        let document = self.document.as_ref()?;
        let rect = document.get_client_bounding_rect(*element)?;
        // Pinned Blitz subtracts the box's own scroll from its client origin; add it back.
        let scroll = document.get_node(*element)?.scroll_offset();
        let out = crate::style_effects::LogicalBox {
            x: (rect.x + scroll.x) as f32,
            y: (rect.y + scroll.y) as f32,
            width: rect.width as f32,
            height: rect.height as f32,
        };
        ([out.x, out.y, out.width, out.height]
            .iter()
            .all(|v| v.is_finite())
            && out.width >= 0.0
            && out.height >= 0.0)
            .then_some(out)
    }
    /// Padding box (border box minus border widths): the clip of an `overflow` container.
    fn padding_box(&self, id: NodeId) -> Option<crate::style_effects::LogicalBox> {
        let border_box = self.layout_box(id)?;
        let (element, _) = self.nodes.get(&id)?;
        let border = self.document.as_ref()?.get_node(*element)?.final_layout().border;
        border_box.outset(-border.top, -border.right, -border.bottom, -border.left)
    }
    fn clips(&self, id: NodeId) -> bool {
        self.source
            .find(id)
            .is_some_and(|node| node.style.overflow != Overflow::Visible)
    }
    /// The view point lies in a corner of `id` cut away by its presented radii.
    fn cut_by(&self, id: NodeId, x: f32, y: f32) -> bool {
        let Some(style) = self.style_of(id) else {
            return false;
        };
        let radii = style.corner_radius;
        !radii.is_zero()
            && self.layout_box(id).is_some_and(|rect| {
                self.to_local(id, x, y).is_some_and(|(lx, ly)| {
                    crate::style_effects::in_cut_corner(rect, radii, lx, ly)
                })
            })
    }
    /// The view point lies inside `id`'s (transformed) padding box.
    fn inside_padding(&self, id: NodeId, x: f32, y: f32) -> bool {
        self.padding_box(id).is_some_and(|b| {
            self.to_local(id, x, y)
                .is_some_and(|(lx, ly)| b.contains(lx, ly))
        })
    }
    /// Engine-painted overlays in paint order: every text decoration, then every outline ring.
    #[cfg(feature = "cpu-pending")]
    fn overlays(&self) -> Vec<Overlay> {
        let mut rings = Vec::new();
        let preorder = self.source_preorder();
        if preorder
            .iter()
            .all(|n| n.style.outline.is_none() && n.style.text_decoration == TextDecoration::NONE)
        {
            self.edit_overlays(&mut rings);
            return rings;
        }
        for node in &preorder {
            self.text_decorations(node.id, &mut rings);
        }
        // E14: selection, preedit underline and caret above text, below focus rings.
        self.edit_overlays(&mut rings);
        for node in preorder {
            let style = self.presented_of(node);
            let Some(outline) = style.outline else {
                continue;
            };
            if !self.source_displayed(node.id) {
                continue;
            }
            let Some(border_box) = self.layout_box(node.id) else {
                continue;
            };
            let Some((outer, outer_radii, inner)) = crate::style_effects::outline_ring(
                border_box,
                style.corner_radius,
                outline,
            ) else {
                continue;
            };
            // The node's own overflow does not clip its outline; ancestors' does.
            let Some((alpha, clips)) = self.paint_context(node.id, false) else {
                continue;
            };
            let a = (f32::from(outline.color.a) * alpha).round();
            if a <= 0.0 {
                continue;
            }
            rings.push(Overlay {
                outer,
                outer_radii,
                inner,
                rgba: [
                    outline.color.r,
                    outline.color.g,
                    outline.color.b,
                    a.clamp(0.0, 255.0) as u8,
                ],
                clips,
                matrix: self.chain_matrix(node.id),
            });
        }
        rings
    }
    /// E2/E10 pointer targeting on top of the pinned native hit test (which ignores radii and
    /// has no slop). Slop: a sample outside every real box, whose native target is a strict
    /// source ancestor of a slop node, is steered to the nearest point inside that node (nearest
    /// box wins; ties go to the later node in paint order); a real box always beats a slop.
    /// Radius: a sample in a corner cut away from its target (or from a clipping ancestor)
    /// skips that subtree. No slop or radius anywhere: no work and no change.
    /// 在固定原生命中测试（忽略圆角、无扩展）之上的 E2/E10 指针定位。扩展：采样不在任何真实盒内、且原生目标是
    /// 某扩展节点的严格源祖先时，修正到该节点内最近的点（最近者胜，相同时取绘制顺序更后者）；真实盒总优先于扩展。
    /// 圆角：采样落在目标（或裁剪祖先）被圆角切掉的角落时跳过该子树。全场景没有扩展与圆角时不做任何事。
    fn pointer_hit_adjustment(&self, x: f32, y: f32) -> PointerHitAdjustment {
        let preorder = self.source_preorder();
        let has_slop = preorder
            .iter()
            .any(|n| n.style.hit_slop != jasper_ui_engine::geometry::Edges::ZERO);
        let has_radius = preorder
            .iter()
            .any(|n| !self.presented_of(n).corner_radius.is_zero());
        if !has_slop && !has_radius {
            return PointerHitAdjustment::default();
        }
        let natural = |cx: f32, cy: f32| self.native_pointer_target(cx, cy);
        let target = natural(x, y);
        if has_slop {
            let mut best: Option<(f32, NodeId, crate::style_effects::LogicalBox, (f32, f32))> =
                None;
            for node in &preorder {
                let slop = node.style.hit_slop;
                if slop == jasper_ui_engine::geometry::Edges::ZERO
                    || !self.source_displayed(node.id)
                {
                    continue;
                }
                let Some(rect) = self.layout_box(node.id) else {
                    continue;
                };
                // E8: slop geometry lives in the node's untransformed layout frame.
                let Some((lx, ly)) = self.to_local(node.id, x, y) else {
                    continue;
                };
                if rect.contains(lx, ly)
                    || !crate::style_effects::slop_box(rect, slop)
                        .is_some_and(|b| b.contains(lx, ly))
                {
                    continue;
                }
                if let Some(target) = target {
                    if target == node.id
                        || !source_subtree_contains(&self.parents, node.id, target)
                    {
                        continue;
                    }
                }
                if self.source_ancestors(node.id).into_iter().any(|a| {
                    self.clips(a) && (!self.inside_padding(a, x, y) || self.cut_by(a, x, y))
                }) {
                    continue;
                }
                let distance = rect.distance(lx, ly);
                if best.is_none_or(|(d, ..)| distance <= d) {
                    best = Some((distance, node.id, rect, (lx, ly)));
                }
            }
            if let Some((_, id, rect, (lx, ly))) = best {
                let (mut sx, mut sy) = rect.clamp_inside(lx, ly, 0.5);
                let radii = self
                    .style_of(id)
                    .map(|s| s.corner_radius)
                    .unwrap_or_default();
                let (cx, cy) = (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
                let (qx, qy) = (sx, sy);
                for step in 1..=16 {
                    if !crate::style_effects::in_cut_corner(rect, radii, sx, sy) {
                        break;
                    }
                    let t = step as f32 / 16.0;
                    sx = qx + (cx - qx) * t;
                    sy = qy + (cy - qy) * t;
                }
                let (sx, sy) = self.to_view(id, sx, sy);
                if natural(sx, sy).is_some_and(|hit| source_subtree_contains(&self.parents, hit, id))
                {
                    return PointerHitAdjustment {
                        steer: Some((sx, sy)),
                        exclude: None,
                    };
                }
            }
        }
        let mut exclude = None;
        if has_radius {
            let mut current = target;
            for _ in 0..=self.parents.len() {
                let Some(node) = current else {
                    break;
                };
                let mut cut = self.cut_by(node, x, y).then_some(node);
                for ancestor in self.source_ancestors(node) {
                    if self.clips(ancestor) && self.cut_by(ancestor, x, y) {
                        cut = Some(ancestor);
                    }
                }
                let Some(cut) = cut else {
                    break;
                };
                exclude = Some(cut);
                current = self.parents.get(&cut).copied().flatten();
            }
        }
        PointerHitAdjustment {
            steer: None,
            exclude,
        }
    }
    /// Native client-box projection in view-local logical coordinates; no OS screen transform.
    pub(crate) fn node_bounds_from_host(
        &self,
        caller: u64,
        version: u64,
        id: NodeId,
    ) -> Option<jasper_ui_engine::geometry::Rect> {
        if caller != self.owner || version != self.scene_version || !self.source_visible(id) {
            return None;
        }
        let (element, _) = self.nodes.get(&id)?;
        let document = self.document.as_ref()?;
        // Pinned Blitz subtracts the queried box's own scroll from its client origin.
        // Reject that unsupported border-box case; descendants still use upstream geometry.
        let scroll = document.get_node(*element)?.scroll_offset();
        if scroll.x != 0.0 || scroll.y != 0.0 {
            return None;
        }
        let rect = document.get_client_bounding_rect(*element)?;
        if [rect.x, rect.y, rect.width, rect.height]
            .iter()
            .any(|value| !value.is_finite() || value.abs() > f32::MAX as f64)
            || rect.width < 0.0
            || rect.height < 0.0
        {
            return None;
        }
        Some(jasper_ui_engine::geometry::Rect::new(
            rect.x as f32,
            rect.y as f32,
            rect.width as f32,
            rect.height as f32,
        ))
    }

    /// Native scroll extent and current content-space window. This removes the former need for
    /// hosts to trust their own scroll upper bound and gives list virtualization one authoritative
    /// range query.
    /// 原生滚动范围与当前内容空间窗口。宿主不再需要信任自身上界，虚拟列表获得唯一权威范围查询。
    pub(crate) fn scroll_geometry_from_host(
        &self,
        caller: u64,
        version: u64,
        id: NodeId,
    ) -> Option<jasper_ui_engine::scroll::ScrollGeometry> {
        if caller != self.owner || version != self.scene_version || !self.source_visible(id) {
            return None;
        }
        let (element, _) = self.nodes.get(&id)?;
        let source = self.source.find(id)?;
        if source.style.overflow != Overflow::Scroll {
            return None;
        }
        let document = self.document.as_ref()?;
        let node = document.get_node(*element)?;
        let layout = node.final_layout();
        let width = (layout.size.width
            - layout.border.left
            - layout.border.right
            - layout.padding.left
            - layout.padding.right)
            .max(0.0);
        let height = (layout.size.height
            - layout.border.top
            - layout.border.bottom
            - layout.padding.top
            - layout.padding.bottom)
            .max(0.0);
        // Pinned Blitz's `scroll_width/height` expose the scrolled overflow bound once the
        // container has a nonzero offset (the overflow rectangle is shifted by that offset).
        // Recover the full content extent by adding the viewport back only when it overflows.
        let scroll_width = layout.scroll_width().max(0.0);
        let scroll_height = layout.scroll_height().max(0.0);
        let content = (
            if scroll_width > width { width + scroll_width } else { width },
            if scroll_height > height { height + scroll_height } else { height },
        );
        let viewport = jasper_ui_engine::geometry::Size::new(width, height);
        let content = jasper_ui_engine::geometry::Size::new(content.0, content.1);
        let bounds = jasper_ui_engine::scroll::ScrollBounds { viewport, content };
        let offset = jasper_ui_engine::geometry::Point::new(
            source.scroll.x.max(0.0).min(bounds.max_offset().x),
            source.scroll.y.max(0.0).min(bounds.max_offset().y),
        );
        Some(jasper_ui_engine::scroll::ScrollGeometry {
            viewport,
            content,
            offset,
            max_offset: bounds.max_offset(),
        })
    }

    /// Measure a CSS table from the active layout, preserving source row/cell identities.
    /// 从当前活动布局测量 CSS 表格，并保留源行/单元身份。
    pub(crate) fn table_metrics_from_host(
        &self,
        caller: u64,
        version: u64,
        id: NodeId,
    ) -> Option<jasper_ui_engine::table::TableMetrics> {
        if caller != self.owner || version != self.scene_version || !self.source_visible(id) {
            return None;
        }
        let source = self.source.find(id)?;
        if source.style.display != Display::Table {
            return None;
        }
        let rect = self.node_bounds_from_host(caller, version, id)?;
        let mut column_widths = Vec::<f32>::new();
        let mut row_heights = Vec::<f32>::new();
        let mut cells = Vec::new();
        for (row, row_node) in source
            .children
            .iter()
            .filter(|node| node.style.display == Display::TableRow)
            .enumerate()
        {
            let mut row_height = 0.0f32;
            for (column, cell) in row_node
                .children
                .iter()
                .filter(|node| node.style.display == Display::TableCell)
                .enumerate()
            {
                if !self.source_visible(cell.id) {
                    continue;
                }
                let Some(cell_rect) = self.node_bounds_from_host(caller, version, cell.id) else {
                    continue;
                };
                row_height = row_height.max(cell_rect.height());
                while column_widths.len() <= column {
                    column_widths.push(0.0);
                }
                column_widths[column] = column_widths[column].max(cell_rect.width());
                cells.push(jasper_ui_engine::table::TableCellMetrics {
                    node: cell.id,
                    row,
                    column,
                    rect: cell_rect,
                });
            }
            row_heights.push(row_height);
        }
        Some(jasper_ui_engine::table::TableMetrics {
            table: id,
            rect,
            column_widths,
            row_heights,
            cells,
        })
    }
    /// Inline layout of a source text node with its first line-box origin in view-local logical
    /// coordinates. None unless the text's inline root (the element, or the anonymous block a
    /// flex/grid element wraps it in) lays out exactly the source text (collapsed whitespace has
    /// no source mapping), or when the element has its own nonzero scroll (same pinned-geometry
    /// limit as node_bounds).
    fn source_text_layout(
        &self,
        caller: u64,
        version: u64,
        id: NodeId,
    ) -> Option<(&blitz_dom::node::TextLayout, &str, f64, f64)> {
        if caller != self.owner || version != self.scene_version || !self.source_visible(id) {
            return None;
        }
        self.node_text_layout(id)
    }
    /// `source_text_layout` without the caller/version/visibility admission: the layout of the
    /// text the native document holds for `id` (the E14 display text while it is edited).
    /// 不做调用方/版本/可见性准入的 `source_text_layout`：原生文档中 `id` 所持文字的布局（E14 编辑时为显示文本）。
    pub(crate) fn node_text_layout(
        &self,
        id: NodeId,
    ) -> Option<(&blitz_dom::node::TextLayout, &str, f64, f64)> {
        let (element, Some(text)) = self.nodes.get(&id)? else {
            return None;
        };
        let document = self.document.as_ref()?;
        let scroll = document.get_node(*element)?.scroll_offset();
        if scroll.x != 0.0 || scroll.y != 0.0 {
            return None;
        }
        // A flex/grid element wraps its text in an anonymous inline root; the exact text check
        // below rejects any root that also lays out other content.
        let root = document.get_node(*text)?.inline_root_ancestor()?;
        let layout = root.element_data()?.inline_layout_data.as_deref()?;
        let source = document.get_node(*text)?.text_data()?.content.as_str();
        if layout.text != source {
            return None;
        }
        let rect = document.get_client_bounding_rect(root.id)?;
        let edges = root.final_layout();
        let x = rect.x + edges.border.left as f64 + edges.padding.left as f64;
        let y = rect.y
            + edges.border.top as f64
            + edges.padding.top as f64
            + layout.block_offset as f64;
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        Some((layout, source, x, y))
    }
    /// Source-text UTF-8 offset nearest a view-local logical point, projected onto this node's
    /// own text layout (caret placement, not an occlusion-aware hit test).
    pub(crate) fn text_offset_from_host(
        &self,
        caller: u64,
        version: u64,
        id: NodeId,
        x: f32,
        y: f32,
    ) -> Option<usize> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let (text, source, origin_x, origin_y) = self.source_text_layout(caller, version, id)?;
        let scale = text.layout.scale() as f64;
        let (local_x, local_y) = ((x as f64 - origin_x) * scale, (y as f64 - origin_y) * scale);
        if !local_x.is_finite()
            || !local_y.is_finite()
            || local_x.abs() > f32::MAX as f64
            || local_y.abs() > f32::MAX as f64
        {
            return None;
        }
        let offset =
            parley::Cursor::from_point(&text.layout, local_x as f32, local_y as f32).index();
        crate::text_geometry::text_offset_admits(
            text.text.len(),
            source.len(),
            offset,
            source.is_char_boundary(offset),
        )
        .then_some(offset)
    }
    /// Caret rectangle (`width` logical px wide) before a source-text UTF-8 offset, in the same
    /// view-local logical coordinates as node_bounds. A soft line-break offset attaches to the
    /// following line (downstream affinity).
    pub(crate) fn text_caret_from_host(
        &self,
        caller: u64,
        version: u64,
        id: NodeId,
        offset: usize,
        width: f32,
    ) -> Option<jasper_ui_engine::geometry::Rect> {
        self.source_text_layout(caller, version, id)?;
        self.caret_geometry(id, offset, width)
    }
    /// `text_caret_from_host` geometry without caller/version/visibility admission.
    /// 不做调用方/版本/可见性准入的 `text_caret_from_host` 几何。
    pub(crate) fn caret_geometry(
        &self,
        id: NodeId,
        offset: usize,
        width: f32,
    ) -> Option<jasper_ui_engine::geometry::Rect> {
        if !width.is_finite() || width < 0.0 {
            return None;
        }
        let (text, source, origin_x, origin_y) = self.node_text_layout(id)?;
        if !crate::text_geometry::text_offset_admits(
            text.text.len(),
            source.len(),
            offset,
            source.is_char_boundary(offset),
        ) {
            return None;
        }
        let scale = text.layout.scale() as f64;
        if !(scale > 0.0) || !scale.is_finite() {
            return None;
        }
        let caret =
            parley::Cursor::from_byte_index(&text.layout, offset, parley::Affinity::Downstream)
                .geometry(&text.layout, (width as f64 * scale) as f32);
        let rect = [
            origin_x + caret.x0 / scale,
            origin_y + caret.y0 / scale,
            (caret.x1 - caret.x0) / scale,
            (caret.y1 - caret.y0) / scale,
        ];
        if rect
            .iter()
            .any(|value| !value.is_finite() || value.abs() > f32::MAX as f64)
            || rect[2] < 0.0
            || rect[3] < 0.0
        {
            return None;
        }
        Some(jasper_ui_engine::geometry::Rect::new(
            rect[0] as f32,
            rect[1] as f32,
            rect[2] as f32,
            rect[3] as f32,
        ))
    }
    /// Selection rectangles for the source-text UTF-8 range [start, end), one or more per laid-out
    /// line in visual order, in the same view-local logical coordinates as node_bounds. An empty
    /// range has no rectangles. The start attaches downstream and the end upstream, so a range
    /// ending at a soft line break does not reach onto the following line.
    pub(crate) fn text_selection_from_host(
        &self,
        caller: u64,
        version: u64,
        id: NodeId,
        start: usize,
        end: usize,
    ) -> Option<Vec<jasper_ui_engine::geometry::Rect>> {
        self.source_text_layout(caller, version, id)?;
        self.selection_geometry(id, start, end)
    }
    /// `text_selection_from_host` geometry without caller/version/visibility admission.
    /// 不做调用方/版本/可见性准入的 `text_selection_from_host` 几何。
    pub(crate) fn selection_geometry(
        &self,
        id: NodeId,
        start: usize,
        end: usize,
    ) -> Option<Vec<jasper_ui_engine::geometry::Rect>> {
        let (text, source, origin_x, origin_y) = self.node_text_layout(id)?;
        if !crate::text_geometry::text_range_admits(
            text.text.len(),
            source.len(),
            start,
            end,
            source.is_char_boundary(start),
            source.is_char_boundary(end),
        ) {
            return None;
        }
        let scale = text.layout.scale() as f64;
        if !(scale > 0.0) || !scale.is_finite() {
            return None;
        }
        let selection = parley::Selection::new(
            parley::Cursor::from_byte_index(&text.layout, start, parley::Affinity::Downstream),
            parley::Cursor::from_byte_index(&text.layout, end, parley::Affinity::Upstream),
        );
        let mut rects = Vec::new();
        for (bounds, _line) in selection.geometry(&text.layout) {
            let rect = [
                origin_x + bounds.x0 / scale,
                origin_y + bounds.y0 / scale,
                (bounds.x1 - bounds.x0) / scale,
                (bounds.y1 - bounds.y0) / scale,
            ];
            if rect
                .iter()
                .any(|value| !value.is_finite() || value.abs() > f32::MAX as f64)
                || rect[2] < 0.0
                || rect[3] < 0.0
            {
                return None;
            }
            rects.push(jasper_ui_engine::geometry::Rect::new(
                rect[0] as f32,
                rect[1] as f32,
                rect[2] as f32,
                rect[3] as f32,
            ));
        }
        Some(rects)
    }
    /// Versioned host-neutral semantic hierarchy, containing no backend identity.
    pub(crate) fn semantics_snapshot_from_host(
        &self,
        caller: u64,
        version: u64,
    ) -> Option<crate::scene_events::SceneSemanticsSnapshot> {
        if caller != self.owner || version != self.scene_version || !self.document.is_open() {
            return None;
        }
        let root = self
            .parents
            .iter()
            .find_map(|(id, parent)| parent.is_none().then_some(*id))?;
        let mut nodes = Vec::with_capacity(self.semantics.len());
        for (id, semantics) in &self.semantics {
            nodes.push(crate::scene_events::SceneSemanticsNode {
                node: *id,
                semantics: semantics.clone(),
                children: self.children.get(id)?.clone(),
            });
        }
        Some(crate::scene_events::SceneSemanticsSnapshot {
            scene_version: version,
            root,
            nodes,
        })
    }
    /// Visible accessibility tree with node_bounds geometry, scene focus and admitted actions;
    /// hidden or undisplayed subtrees are pruned. Traversal is bounded by the node count.
    pub(crate) fn accessibility_tree_from_host(
        &self,
        caller: u64,
        version: u64,
    ) -> Option<crate::access_tree::AccessibilityTree> {
        use crate::access_tree::{access_actions, AccessibilityNode, AccessibilityTree};
        if caller != self.owner || version != self.scene_version || !self.document.is_open() {
            return None;
        }
        let root = self
            .parents
            .iter()
            .find_map(|(id, parent)| parent.is_none().then_some(*id))?;
        let focused = self
            .focused_node(caller, version)
            .filter(|id| self.source_visible(*id));
        let mut nodes = Vec::new();
        let mut pending = if self.source_visible(root) {
            vec![root]
        } else {
            Vec::new()
        };
        while let Some(id) = pending.pop() {
            if nodes.len() >= self.semantics.len() {
                return None;
            }
            let semantics = self.semantics.get(&id)?;
            let children: Vec<NodeId> = self
                .children
                .get(&id)?
                .iter()
                .copied()
                .filter(|child| self.source_visible(*child))
                .collect();
            pending.extend(children.iter().rev());
            // E14: the edited field reports its edit buffer and selection.
            let edit = self.edit_accessibility(id);
            nodes.push(AccessibilityNode {
                node: id,
                role: semantics.role,
                label: semantics.label.clone(),
                description: semantics.description.clone(),
                value: match &edit {
                    Some((text, _)) => text.clone(),
                    None => semantics.value.clone(),
                },
                text_selection: edit.map(|(_, selection)| selection),
                disabled: semantics.disabled,
                selected: semantics.selected,
                modal: self
                    .source
                    .find(id)
                    .and_then(|node| node.style.overlay)
                    .is_some_and(|overlay| overlay.modal),
                bounds: self.node_bounds_from_host(caller, version, id),
                focused: focused == Some(id),
                // The kernel's `is_button` input is "activatable by role" since E12.
                actions: access_actions(
                    crate::scene_events::role_activates_on_click(semantics.role),
                    semantics.focusable,
                    semantics.disabled,
                ),
                children,
            });
        }
        Some(AccessibilityTree {
            scene_version: version,
            nodes,
            focused,
        })
    }
    /// Host viewport scrolling is separate from per-node source SetScroll offsets.
    /// Host supplies valid extent and authenticates routing; Blitz owns scrolling/painting.
    pub(crate) fn set_viewport_scroll_from_host(
        &mut self,
        caller: u64,
        version: u64,
        x: f64,
        y: f64,
    ) -> bool {
        self.invalidate_frame();
        if caller != self.owner
            || version != self.scene_version
            || !x.is_finite()
            || !y.is_finite()
            || x < 0.0
            || y < 0.0
            || x > u32::MAX as f64
            || y > u32::MAX as f64
        {
            return false;
        }
        let Some(document) = self.document.as_mut() else {
            return false;
        };
        document.set_viewport_scroll(blitz_dom::Point { x, y });
        true
    }
    #[cfg(feature = "cpu-pending")]
    pub(crate) fn render_cpu(&mut self, caller: u64, max_bytes: usize) -> Option<Vec<u8>> {
        self.render_cpu_with_damage(caller, max_bytes)
            .map(|(pixels, _)| pixels)
    }
    /// Damage is relative to the previous frame this view produced, not to host surface content.
    /// 损伤相对本视图上一次产出的帧，而非宿主表面内容。
    #[cfg(feature = "cpu-pending")]
    pub(crate) fn render_cpu_with_damage(
        &mut self,
        caller: u64,
        max_bytes: usize,
    ) -> Option<(Vec<u8>, crate::frame_schedule::FrameDamage)> {
        use crate::frame_schedule::{frame_damage, FrameDamage};
        if caller != self.owner {
            return None;
        }
        let (width, height, scale) = {
            let document = self.document.as_ref()?;
            let viewport = document.viewport();
            (
                viewport.window_size.0,
                viewport.window_size.1,
                viewport.scale_f64(),
            )
        };
        let bytes = crate::cpu_budget::cpu_frame_bytes(width, height, max_bytes)?;
        if let Some((cached_width, cached_height, cached)) = &self.frame_cache {
            if crate::frame_schedule::frame_reuse_admits(
                self.frame_dirty,
                *cached_width,
                *cached_height,
                cached.len(),
                width,
                height,
                bytes,
            ) {
                return Some((cached.clone(), FrameDamage::Unchanged));
            }
        }
        if !self.sync_images() {
            return None;
        }
        let rings = self.overlays();
        let glass = self.glass_specs();
        let budget = self.material.budget;
        let mut frame_material = None;
        let document = self.document.as_mut()?;
        let pixels = anyrender::render_to_buffer::<anyrender_vello_cpu::VelloCpuImageRenderer, _>(
            |scene| {
                frame_material = Some(paint_frame(
                    scene,
                    document,
                    &self.bold_aliases,
                    scale,
                    (width, height),
                    &rings,
                    &glass,
                    budget,
                ));
            },
            width,
            height,
        );
        if let Some((report, _)) = frame_material {
            self.material.report = report;
        }
        if pixels.len() != bytes {
            return None;
        }
        let previous = self
            .frame_cache
            .as_ref()
            .map(|(w, h, cached)| (*w, *h, cached.as_slice()));
        let damage = frame_damage(previous, width, height, &pixels);
        self.frame_cache = Some((width, height, pixels.clone()));
        self.frame_dirty = false;
        Some((pixels, damage))
    }
    /// A source-visible node's device-pixel box, for sizing plugin paint output.
    /// 源可见节点的设备像素框，用于确定插件绘制输出的尺寸。
    pub(crate) fn node_pixel_box_from_host(
        &self,
        caller: u64,
        version: u64,
        id: NodeId,
    ) -> Option<jasper_ui_engine::ui_plugin_gate::UiNodePixelBox> {
        let bounds = self.node_bounds_from_host(caller, version, id)?;
        let scale = self.document.as_ref()?.viewport().scale_f64();
        jasper_ui_engine::ui_plugin_gate::node_pixel_box(
            bounds.x(),
            bounds.y(),
            bounds.width(),
            bounds.height(),
            scale,
        )
    }
    /// One frame with plugin layers above the scene. Every layer is admitted before any painting,
    /// and the result is not a view frame: frame cache, damage base and dirty flag stay untouched.
    /// 场景之上带插件层的一帧。所有层在绘制前全部准入；结果不是视图帧：帧缓存、损伤基准与脏标记都不变。
    #[cfg(feature = "cpu-pending")]
    pub(crate) fn render_cpu_with_paint_layers(
        &mut self,
        caller: u64,
        version: u64,
        layers: &[jasper_ui_engine::ui_plugin_gate::UiPaintLayer<'_>],
        max_bytes: usize,
    ) -> Option<Vec<u8>> {
        use anyrender::PaintScene;
        use jasper_ui_engine::ui_plugin_gate::validate_paint_commands;
        use peniko::kurbo::{Affine, Rect};
        let mut admitted = Vec::with_capacity(layers.len());
        for layer in layers {
            let node = self.node_pixel_box_from_host(caller, version, layer.node)?;
            validate_paint_commands(layer.commands, node.width, node.height).ok()?;
            admitted.push((node, layer.commands));
        }
        if caller != self.owner || version != self.scene_version {
            return None;
        }
        if !self.sync_images() {
            return None;
        }
        let rings = self.overlays();
        let glass = self.glass_specs();
        let budget = self.material.budget;
        let mut frame_material = None;
        let document = self.document.as_mut()?;
        let (width, height) = document.viewport().window_size;
        let bytes = crate::cpu_budget::cpu_frame_bytes(width, height, max_bytes)?;
        let scale = document.viewport().scale_f64();
        let pixels = anyrender::render_to_buffer::<anyrender_vello_cpu::VelloCpuImageRenderer, _>(
            |scene| {
                frame_material = Some(paint_frame(
                    scene,
                    document,
                    &self.bold_aliases,
                    scale,
                    (width, height),
                    &rings,
                    &glass,
                    budget,
                ));
                for (node, commands) in &admitted {
                    for command in commands.iter() {
                        let left = f64::from(node.x) + f64::from(command.x);
                        let top = f64::from(node.y) + f64::from(command.y);
                        let [red, green, blue, alpha] = command.rgba;
                        scene.fill(
                            peniko::Fill::NonZero,
                            Affine::IDENTITY,
                            peniko::Color::from_rgba8(red, green, blue, alpha),
                            None,
                            &Rect::new(
                                left,
                                top,
                                left + f64::from(command.width),
                                top + f64::from(command.height),
                            ),
                        );
                    }
                }
            },
            width,
            height,
        );
        if let Some((report, _)) = frame_material {
            self.material.report = report;
        }
        (pixels.len() == bytes).then_some(pixels)
    }
    /// Some(true) while a possible visible change is not yet painted; None for a foreign
    /// caller or retired view. Hosts skip render calls while this is Some(false).
    /// 存在尚未绘制的可能可见变化时为 Some(true)；非归属调用方或已退役视图为 None。
    pub(crate) fn needs_frame(&self, caller: u64) -> Option<bool> {
        if caller != self.owner || !self.document.is_open() {
            return None;
        }
        Some(self.frame_dirty || self.motion.running())
    }
    /// E7: advance this view's transitions to host time `now_ms` (milliseconds, monotone) and
    /// write the presented values into the native document. The first call attaches the clock:
    /// before it every transition is immediate. Never consumes or changes the scene version.
    /// E7：把本视图的过渡推进到宿主时间 `now_ms`（毫秒，单调）并把呈现值写入原生文档。首次调用即接上时钟；此前
    /// 所有过渡立即生效。从不消耗或改变场景版本。
    pub(crate) fn animate_from_host(
        &mut self,
        caller: u64,
        now_ms: f64,
    ) -> Option<jasper_ui_engine::motion::AnimationTick> {
        if caller != self.owner || !self.document.is_open() || !now_ms.is_finite() {
            return None;
        }
        if !self.motion.running() {
            self.motion.advance(now_ms)?;
            return Some(Default::default());
        }
        let before: Vec<(NodeId, Option<Style>)> = self
            .motion
            .animated_nodes()
            .map(|id| (id, self.style_of(id).map(|s| s.into_owned())))
            .collect();
        self.motion.advance(now_ms)?;
        let changed: Vec<NodeId> = before
            .into_iter()
            .filter(|(id, style)| self.style_of(*id).map(|s| s.into_owned()) != *style)
            .map(|(id, _)| id)
            .collect();
        if !changed.is_empty() {
            self.invalidate_frame();
            self.write_presented(&changed);
        }
        Some(jasper_ui_engine::motion::AnimationTick {
            changed: !changed.is_empty(),
            running: self.motion.running(),
        })
    }
    /// E7: host reduce-motion environment (JUI 2 `env reduce-motion on|off`). On: running
    /// transitions finish at once; later changes of movement properties (size, transform) are
    /// immediate and others use their rule's reduced timing, or are immediate without one.
    /// E7：宿主「减少动态」环境（JUI 2 `env reduce-motion on|off`）。开启时运行中的过渡立即结束；之后移动类属性
    /// （尺寸、变换）的变化立即生效，其余属性用规则的减少动态替代曲线，没有替代则立即生效。
    pub(crate) fn set_reduce_motion_from_host(&mut self, caller: u64, on: bool) -> bool {
        if caller != self.owner || !self.document.is_open() {
            return false;
        }
        let finished = self.motion.set_reduce_motion(on);
        self.edit_reduce_motion(on);
        if !finished.is_empty() {
            self.invalidate_frame();
            self.write_presented(&finished);
        }
        true
    }
    pub(crate) fn reduce_motion_from_host(&self, caller: u64) -> Option<bool> {
        (caller == self.owner && self.document.is_open()).then_some(self.motion.reduce_motion())
    }
    /// E5: per-frame CPU budget for blurred materials (`BackdropBudget::DEFAULT` initially;
    /// `FALLBACK_ONLY` paints every material as its fallback). A change repaints. False for a
    /// foreign caller or a closed view.
    /// E5：模糊材质的每帧 CPU 预算（初始为 `BackdropBudget::DEFAULT`；`FALLBACK_ONLY` 时所有材质画回退色）。
    /// 改变时重绘。非归属方或已关闭视图为 false。
    pub(crate) fn set_backdrop_budget_from_host(
        &mut self,
        caller: u64,
        budget: crate::material::BackdropBudget,
    ) -> bool {
        if caller != self.owner || !self.document.is_open() {
            return false;
        }
        if self.material.budget != budget {
            self.material.budget = budget;
            self.invalidate_frame();
        }
        true
    }
    /// E5: materials of the last painted frame (blurred, degraded with reason). None for a
    /// foreign caller or a closed view.
    /// E5：上一个绘制帧的材质（已模糊、降级及原因）。非归属方或已关闭视图为 None。
    pub(crate) fn backdrop_report_from_host(
        &self,
        caller: u64,
    ) -> Option<crate::material::BackdropReport> {
        if caller != self.owner || !self.document.is_open() {
            return None;
        }
        Some(self.material.report.clone())
    }
    /// E7: some transition is running (the host should keep calling `animate` each refresh tick).
    /// E7：有过渡在运行（宿主应在每个刷新节拍继续调用 `animate`）。
    pub(crate) fn animating(&self, caller: u64) -> Option<bool> {
        if caller != self.owner || !self.document.is_open() {
            return None;
        }
        Some(self.motion.running())
    }
    /// Write the presented style of `ids` into the native document and re-resolve style and
    /// layout. A presented style the native projection refuses falls back to the target.
    /// 把 `ids` 的呈现样式写入原生文档并重新解析样式与布局；原生投影拒绝的呈现样式退回目标值。
    fn write_presented(&mut self, ids: &[NodeId]) {
        let root = self.source.root.id;
        let mut writes = Vec::new();
        for id in ids {
            let (Some(node), Some((element, _))) = (self.source.find(*id), self.nodes.get(id))
            else {
                continue;
            };
            let presented = self.presented_of(node);
            let shape = TextShape::of(node);
            let Some(properties) =
                style_properties(&presented, *id == root, &self.families, *id, shape).or_else(
                    || style_properties(&node.style, *id == root, &self.families, *id, shape),
                )
            else {
                continue;
            };
            writes.push((*element, properties));
        }
        let Some(document) = self.document.as_mut() else {
            return;
        };
        {
            let mut tree = document.mutate();
            for (element, properties) in &writes {
                for (property, value) in properties {
                    tree.set_style_property(*element, property, value);
                }
            }
        }
        document.resolve(0.0);
    }
    /// Host-forced repaint (e.g. lost or replaced presentation surface).
    /// 宿主强制重绘（如呈现表面丢失或更换）。
    pub(crate) fn invalidate_frame_from_host(&mut self, caller: u64) -> bool {
        if caller != self.owner || !self.document.is_open() {
            return false;
        }
        // The host's copy is gone: the next damage report must be Full, not a pixel diff.
        // 宿主副本已失效：下一次损伤必须为 Full，而非像素差分。
        self.frame_cache = None;
        self.invalidate_frame();
        true
    }
    /// Conservative damage: every mutating entry marks the view dirty before admission,
    /// so a rejected call can only cause an extra paint, never a stale frame.
    /// 保守失效：每个修改入口在准入前即标脏，被拒调用至多多绘一帧，绝不复用过期帧。
    fn invalidate_frame(&mut self) {
        self.frame_dirty = true;
    }
    /// Copy backend RGBA8 bytes into a host-injected software surface, preserving padding.
    /// This is client buffer adaptation, not surface acquisition, IPC import or presentation.
    #[cfg(feature = "cpu-pending")]
    pub(crate) fn render_to_host_surface(
        &mut self,
        caller: u64,
        version: u64,
        destination: &mut [u8],
        stride: usize,
        max_render_bytes: usize,
    ) -> bool {
        if caller != self.owner || version != self.scene_version {
            return false;
        }
        let Some(document) = self.document.as_ref() else {
            return false;
        };
        let (width, height) = document.viewport().window_size;
        orchestrate_native_host_surface(
            width,
            height,
            destination,
            stride,
            max_render_bytes,
            |budget| self.render_cpu(caller, budget),
        )
    }
    /// Same as `render_to_host_surface`, also reporting damage relative to the previous frame
    /// this view produced. A failed write drops the cache so the next report is Full.
    /// 同 `render_to_host_surface`，并报告相对本视图上一帧的损伤；写入失败则丢弃缓存，下次为 Full。
    #[cfg(feature = "cpu-pending")]
    pub(crate) fn render_to_host_surface_with_damage(
        &mut self,
        caller: u64,
        version: u64,
        destination: &mut [u8],
        stride: usize,
        max_render_bytes: usize,
    ) -> Option<crate::frame_schedule::FrameDamage> {
        if caller != self.owner || version != self.scene_version {
            return None;
        }
        let (width, height) = self.document.as_ref()?.viewport().window_size;
        let mut damage = None;
        let written = orchestrate_native_host_surface(
            width,
            height,
            destination,
            stride,
            max_render_bytes,
            |budget| {
                let (pixels, found) = self.render_cpu_with_damage(caller, budget)?;
                damage = Some(found);
                Some(pixels)
            },
        );
        if !written {
            if damage.is_some() {
                self.frame_cache = None;
            }
            return None;
        }
        damage
    }
    /// Like `render_to_host_surface_with_damage`, but writes only the damaged pixels: the host
    /// guarantees `destination` holds the frame this view produced last (otherwise it must
    /// invalidate first, making the next damage Full). Surface admission precedes painting;
    /// a failed write drops the cache so the next report is Full.
    /// 同 `render_to_host_surface_with_damage`，但只写受损像素：宿主保证 `destination` 持有本视图
    /// 上一次产出的帧（否则须先失效，使下一次为 Full）。Surface 准入先于绘制；写入失败丢弃缓存。
    #[cfg(feature = "cpu-pending")]
    pub(crate) fn render_to_host_surface_damaged(
        &mut self,
        caller: u64,
        version: u64,
        destination: &mut [u8],
        stride: usize,
        max_render_bytes: usize,
    ) -> Option<crate::frame_schedule::FrameDamage> {
        use jasper_ui_engine::surface::{rgba8_row_bytes, surface_span};
        if caller != self.owner || version != self.scene_version {
            return None;
        }
        let (width, height) = self.document.as_ref()?.viewport().window_size;
        let row_bytes = rgba8_row_bytes(width, destination.len())?;
        surface_span(row_bytes, height as usize, stride, destination.len())?;
        let (pixels, damage) = self.render_cpu_with_damage(caller, max_render_bytes)?;
        if !crate::frame_schedule::write_damaged_frame(
            destination,
            stride,
            width,
            height,
            &pixels,
            damage,
        ) {
            self.frame_cache = None;
            return None;
        }
        Some(damage)
    }
    /// Native GPU frame as compact RGBA8 (premultiplied alpha, as Vello hybrid emits).
    /// 原生 GPU 帧，紧凑 RGBA8（Vello hybrid 输出的预乘 alpha）。
    #[cfg(feature = "gpu-pending")]
    pub(crate) fn render_gpu(
        &mut self,
        caller: u64,
        version: u64,
        gpu: &mut crate::gpu_render::GpuRenderer,
        max_bytes: usize,
    ) -> Result<Vec<u8>, crate::gpu_render::GpuFrameError> {
        self.render_gpu_with_damage(caller, version, gpu, max_bytes)
            .map(|(pixels, _)| pixels)
    }
    /// GPU frames share the frame cache with CPU frames: an unchanged view reuses the last
    /// produced frame without GPU work, and damage is relative to the last produced frame,
    /// whichever path painted it. A device/render/readback failure keeps the previous
    /// baseline (nothing new was produced) and leaves the view dirty.
    /// GPU 帧与 CPU 帧共用帧缓存：未变化视图不做 GPU 工作而复用最近产出的帧；损伤相对最近产出
    /// 的帧，无论由哪条路径绘制。设备/渲染/读回失败时保留原基准（未产出新帧），视图保持待绘。
    #[cfg(feature = "gpu-pending")]
    pub(crate) fn render_gpu_with_damage(
        &mut self,
        caller: u64,
        version: u64,
        gpu: &mut crate::gpu_render::GpuRenderer,
        max_bytes: usize,
    ) -> Result<(Vec<u8>, crate::frame_schedule::FrameDamage), crate::gpu_render::GpuFrameError>
    {
        use crate::frame_schedule::{frame_damage, FrameDamage};
        use crate::gpu_render::GpuFrameError;
        if caller != self.owner || version != self.scene_version {
            return Err(GpuFrameError::Rejected);
        }
        let (width, height, scale) = {
            let document = self.document.as_ref().ok_or(GpuFrameError::Rejected)?;
            let viewport = document.viewport();
            (
                viewport.window_size.0,
                viewport.window_size.1,
                viewport.scale_f64(),
            )
        };
        let bytes = crate::cpu_budget::cpu_frame_bytes(width, height, max_bytes)
            .ok_or(GpuFrameError::Rejected)?;
        if let Some((cached_width, cached_height, cached)) = &self.frame_cache {
            if crate::frame_schedule::frame_reuse_admits(
                self.frame_dirty,
                *cached_width,
                *cached_height,
                cached.len(),
                width,
                height,
                bytes,
            ) {
                return Ok((cached.clone(), FrameDamage::Unchanged));
            }
        }
        if !self.sync_images() {
            return Err(GpuFrameError::Rejected);
        }
        let rings = self.overlays();
        let glass = self.glass_specs();
        let budget = self.material.budget;
        let mut frame_material = None;
        let document = self.document.as_mut().ok_or(GpuFrameError::Rejected)?;
        let pixels = gpu.render(width, height, |scene| {
            frame_material = Some(paint_frame(
                scene,
                document,
                &self.bold_aliases,
                scale,
                (width, height),
                &rings,
                &glass,
                budget,
            ));
        });
        if let Some((report, images)) = frame_material {
            self.material.report = report;
            gpu.release_images(images);
        }
        gpu.release_images(self.images.take_retired());
        let pixels = pixels?;
        if pixels.len() != bytes {
            return Err(GpuFrameError::Readback(format!(
                "expected {bytes} bytes, read {}",
                pixels.len()
            )));
        }
        let previous = self
            .frame_cache
            .as_ref()
            .map(|(w, h, cached)| (*w, *h, cached.as_slice()));
        let damage = frame_damage(previous, width, height, &pixels);
        self.frame_cache = Some((width, height, pixels.clone()));
        self.frame_dirty = false;
        Ok((pixels, damage))
    }
    /// GPU frame copied into a host-injected software surface; surface admission precedes
    /// any GPU work and a failed write leaves the view dirty.
    /// GPU 帧复制进宿主注入的软件 Surface；Surface 准入先于任何 GPU 工作，写入失败视图保持待绘。
    #[cfg(feature = "gpu-pending")]
    pub(crate) fn render_gpu_to_host_surface(
        &mut self,
        caller: u64,
        version: u64,
        gpu: &mut crate::gpu_render::GpuRenderer,
        destination: &mut [u8],
        stride: usize,
        max_render_bytes: usize,
    ) -> Result<(), crate::gpu_render::GpuFrameError> {
        self.render_gpu_to_host_surface_with_damage(
            caller,
            version,
            gpu,
            destination,
            stride,
            max_render_bytes,
        )
        .map(|_| ())
    }
    /// `render_gpu_to_host_surface` that also reports damage relative to the last produced
    /// frame. A frame produced but not written drops the cache so the next report is Full.
    /// 同 `render_gpu_to_host_surface`，并报告相对最近产出帧的损伤；已产出未写入则丢弃缓存，下次为 Full。
    #[cfg(feature = "gpu-pending")]
    pub(crate) fn render_gpu_to_host_surface_with_damage(
        &mut self,
        caller: u64,
        version: u64,
        gpu: &mut crate::gpu_render::GpuRenderer,
        destination: &mut [u8],
        stride: usize,
        max_render_bytes: usize,
    ) -> Result<crate::frame_schedule::FrameDamage, crate::gpu_render::GpuFrameError> {
        use crate::gpu_render::GpuFrameError;
        if caller != self.owner || version != self.scene_version {
            return Err(GpuFrameError::Rejected);
        }
        let (width, height) = self
            .document
            .as_ref()
            .ok_or(GpuFrameError::Rejected)?
            .viewport()
            .window_size;
        let mut failure = None;
        let mut damage = None;
        let written = orchestrate_native_host_surface(
            width,
            height,
            destination,
            stride,
            max_render_bytes,
            |budget| match self.render_gpu_with_damage(caller, version, gpu, budget) {
                Ok((pixels, found)) => {
                    damage = Some(found);
                    Some(pixels)
                }
                Err(error) => {
                    failure = Some(error);
                    None
                }
            },
        );
        if written {
            if let Some(damage) = damage {
                return Ok(damage);
            }
        }
        if damage.is_some() {
            self.frame_cache = None;
        }
        self.invalidate_frame();
        Err(failure.unwrap_or(GpuFrameError::Rejected))
    }
    /// Like `render_gpu_to_host_surface_with_damage`, but writes only the damaged pixels; the
    /// host guarantees `destination` holds the frame this view produced last (otherwise it
    /// invalidates first). Surface admission precedes GPU work; a rejected or failed call
    /// writes nothing, and a failed write drops the cache so the next report is Full.
    /// 同 `render_gpu_to_host_surface_with_damage`，但只写受损像素；宿主保证 `destination` 持有本视图
    /// 上一次产出的帧（否则先失效）。Surface 准入先于 GPU 工作；被拒或失败不写任何字节，写入失败丢弃缓存。
    #[cfg(feature = "gpu-pending")]
    pub(crate) fn render_gpu_to_host_surface_damaged(
        &mut self,
        caller: u64,
        version: u64,
        gpu: &mut crate::gpu_render::GpuRenderer,
        destination: &mut [u8],
        stride: usize,
        max_render_bytes: usize,
    ) -> Result<crate::frame_schedule::FrameDamage, crate::gpu_render::GpuFrameError> {
        use crate::gpu_render::GpuFrameError;
        use jasper_ui_engine::surface::{rgba8_row_bytes, surface_span};
        if caller != self.owner || version != self.scene_version {
            return Err(GpuFrameError::Rejected);
        }
        let (width, height) = self
            .document
            .as_ref()
            .ok_or(GpuFrameError::Rejected)?
            .viewport()
            .window_size;
        let row_bytes = rgba8_row_bytes(width, destination.len()).ok_or(GpuFrameError::Rejected)?;
        surface_span(row_bytes, height as usize, stride, destination.len())
            .ok_or(GpuFrameError::Rejected)?;
        let (pixels, damage) =
            self.render_gpu_with_damage(caller, version, gpu, max_render_bytes)?;
        if !crate::frame_schedule::write_damaged_frame(
            destination,
            stride,
            width,
            height,
            &pixels,
            damage,
        ) {
            self.frame_cache = None;
            self.invalidate_frame();
            return Err(GpuFrameError::Rejected);
        }
        Ok(damage)
    }
    /// Render directly into a host-owned wgpu texture view. The host owns the Surface, texture
    /// acquisition, resize/configuration and present call; the engine only submits one admitted
    /// Vello hybrid frame into the supplied view.
    /// 直接绘制到宿主持有的 wgpu 纹理视图；Surface、纹理取得/重配与 present 归宿主，
    /// 引擎只向该视图提交一个已准入的 Vello hybrid 帧。
    #[cfg(feature = "gpu-pending")]
    pub(crate) fn render_gpu_to_host_view(
        &mut self,
        caller: u64,
        version: u64,
        gpu: &mut crate::gpu_render::GpuRenderer,
        format: wgpu::TextureFormat,
        view: &wgpu::TextureView,
        max_render_bytes: usize,
    ) -> Result<(), crate::gpu_render::GpuFrameError> {
        use crate::gpu_render::GpuFrameError;
        if caller != self.owner || version != self.scene_version {
            return Err(GpuFrameError::Rejected);
        }
        let (width, height, scale) = {
            let document = self.document.as_ref().ok_or(GpuFrameError::Rejected)?;
            let viewport = document.viewport();
            (
                viewport.window_size.0,
                viewport.window_size.1,
                viewport.scale_f64(),
            )
        };
        crate::cpu_budget::cpu_frame_bytes(width, height, max_render_bytes)
            .ok_or(GpuFrameError::Rejected)?;
        self.invalidate_frame();
        if !self.sync_images() {
            return Err(GpuFrameError::Rejected);
        }
        let rings = self.overlays();
        let glass = self.glass_specs();
        let budget = self.material.budget;
        let mut frame_material = None;
        let document = self.document.as_mut().ok_or(GpuFrameError::Rejected)?;
        let result = gpu.render_to_host_view(width, height, format, view, |scene| {
            frame_material = Some(paint_frame(
                scene,
                document,
                &self.bold_aliases,
                scale,
                (width, height),
                &rings,
                &glass,
                budget,
            ));
        });
        if let Some((report, images)) = frame_material {
            self.material.report = report;
            gpu.release_images(images);
        }
        gpu.release_images(self.images.take_retired());
        if result.is_ok() {
            // Presentation pixels live in the host's texture, not in this CPU cache.
            self.frame_cache = None;
            self.frame_dirty = false;
        }
        result
    }
    /// Render through the native painter then the existing backend submission/presentation seam.
    /// 经原生绘制后接入既有后端提交/呈现接缝；不生成或重放旧 PaintOp。
    /// Backend submit/present failures propagate; external presentation rollback is not promised.
    /// 后端提交/呈现失败原样传播，不承诺外部呈现回滚。
    #[cfg(feature = "cpu-pending")]
    pub(crate) fn render_to_backend(
        &mut self,
        caller: u64,
        version: u64,
        backend: &mut impl jasper_ui_engine::backend::RenderBackend,
        budget: usize,
    ) -> Result<(), jasper_ui_engine::backend::BackendError> {
        let extent = self
            .document
            .as_ref()
            .map(|document| document.viewport().window_size);
        orchestrate_native_frame(
            self.owner,
            caller,
            self.scene_version,
            version,
            extent,
            budget,
            |render_budget| self.render_cpu(caller, render_budget),
            backend,
        )
    }

    /// Report final native focus loss after successful view retirement.
    /// Host still owns pointer release and external input-session cancellation.
    pub(crate) fn close_with_focus_events(
        &mut self,
        caller: u64,
        version: u64,
        sink: &mut impl FnMut(crate::scene_events::SceneFocusEvent),
    ) -> bool {
        if caller != self.owner || version != self.scene_version {
            return false;
        }
        let before = self.focused_node(caller, version);
        if !self.close(caller) {
            return false;
        }
        crate::scene_events::notify_focus_transition(version, before, None, sink);
        true
    }
}
// BEGIN VERIFIED NATIVE SCENE CLOSE KERNEL
impl NativeScene {
    pub(crate) fn close(&mut self, caller: u64) -> bool {
        caller == self.owner && self.document.close()
    }
}
// END VERIFIED NATIVE SCENE CLOSE KERNEL
/// Relation over temporary source-ID topology only; native deletion remains upstream.
fn source_subtree_contains(
    parents: &BTreeMap<NodeId, Option<NodeId>>,
    node: NodeId,
    ancestor: NodeId,
) -> bool {
    let mut current = Some(node);
    let mut remaining = parents.len();
    loop {
        let cursor = match current {
            Some(cursor) => cursor,
            None => return false,
        };
        if cursor.0 == ancestor.0 {
            return true;
        }
        if remaining == 0 {
            return false;
        }
        remaining -= 1;
        current = match parents.get(&cursor) {
            Some(parent) => *parent,
            None => None,
        };
    }
}

/// Source admission shared by initial import and single-subtree insertion; the node count is
/// bounded only by `node_limit` (the host quota, `usize::MAX` when unlimited).
/// 初始导入与单子树插入共用的源准入；节点数只受 `node_limit`（宿主额度，无上限时为 `usize::MAX`）约束。
fn prepare_subtree(
    root: &Node,
    base_depth: usize,
    existing: &BTreeSet<NodeId>,
    families: &BTreeSet<String>,
    node_limit: usize,
) -> Option<Vec<Vec<(&'static str, String)>>> {
    let mut stack = vec![(root, base_depth)];
    let mut prepared = Vec::new();
    let mut seen = BTreeSet::new();
    while let Some((node, depth)) = stack.pop() {
        if depth > 128
            || existing.contains(&node.id)
            || !seen.insert(node.id)
            || matches!(node.kind, NodeKind::Canvas(_))
            || native_scroll_offset(node.scroll).is_none()
        {
            return None;
        }
        prepared.push(style_properties(
            &node.style,
            depth == 0,
            families,
            node.id,
            TextShape::of(node),
        )?);
        if !scene_work_fits(
            existing.len().checked_add(prepared.len())?,
            stack.len(),
            node.children.len(),
            node_limit,
        ) {
            return None;
        }
        for child in node.children.iter().rev() {
            stack.push((child, depth + 1));
        }
    }
    Some(prepared)
}

// BEGIN VERIFIED SOURCE INSERT INDEX KERNEL
/// Match existing ScenePatch insertion semantics: oversized index appends.
pub(crate) fn source_insert_index(index: usize, children: usize) -> usize {
    index.min(children)
}
// END VERIFIED SOURCE INSERT INDEX KERNEL

// BEGIN VERIFIED SCENE UPDATE ADMISSION KERNEL
/// Classify all update-admission rejection reasons without touching source or native state.
/// 在不接触来源或原生状态的情况下分类全部更新准入拒绝原因。
pub(crate) fn admit_scene_update(
    owner: u64,
    caller: u64,
    current: u64,
    expected: u64,
    next: u64,
    count: usize,
    open: bool,
) -> Result<(), SceneUpdateError> {
    if owner != caller {
        return Err(SceneUpdateError::OwnerMismatch);
    }
    if !open {
        return Err(SceneUpdateError::Closed);
    }
    if count == 0 {
        return Err(SceneUpdateError::EmptyFrame);
    }
    if current != expected {
        return Err(SceneUpdateError::StaleVersion { expected, current });
    }
    match next_ui_update_version(current, expected) {
        Some(successor) if next == successor => Ok(()),
        _ => Err(SceneUpdateError::InvalidSuccessor { expected, next }),
    }
}

/// Compatibility boolean projection of the structured admission decision.
/// 结构化准入决策的兼容布尔投影。
pub(crate) fn accepts_scene_update(
    owner: u64,
    caller: u64,
    current: u64,
    expected: u64,
    next: u64,
    count: usize,
    open: bool,
) -> bool {
    admit_scene_update(owner, caller, current, expected, next, count, open).is_ok()
}
// END VERIFIED SCENE UPDATE ADMISSION KERNEL

// BEGIN VERIFIED SCENE FRAME COMPLETION KERNEL
/// Require a successful source frame at the exact successor and a detached native
/// candidate before the caller may replace the active view.
pub(crate) fn complete_scene_frame(
    source_ok: bool,
    source_version: u64,
    next: u64,
    candidate_ready: bool,
) -> Result<(), SceneUpdateError> {
    if !source_ok || source_version != next {
        return Err(SceneUpdateError::SourceRejected);
    }
    if !candidate_ready {
        return Err(SceneUpdateError::NativeRejected);
    }
    Ok(())
}
// END VERIFIED SCENE FRAME COMPLETION KERNEL

// BEGIN VERIFIED SCENE FRAME ORDER KERNEL
/// Run the detached frame stages in their only legal order: the builder receives only a source
/// that succeeded at the exact successor, and neither stage is handed the active view's storage.
/// The caller replaces the active view after `Ok`, so a stage panic leaves nothing to undo.
pub(crate) fn order_scene_frame<S, C>(
    next: u64,
    apply_source: impl FnOnce() -> (bool, u64, S),
    build_candidate: impl FnOnce(S) -> Option<C>,
) -> Result<C, SceneUpdateError> {
    let (source_ok, source_version, source) = apply_source();
    let candidate = if source_ok && source_version == next {
        build_candidate(source)
    } else {
        None
    };
    if let Err(error) = complete_scene_frame(source_ok, source_version, next, candidate.is_some()) {
        return Err(error);
    }
    match candidate {
        Some(candidate) => Ok(candidate),
        None => Err(SceneUpdateError::NativeRejected),
    }
}
// END VERIFIED SCENE FRAME ORDER KERNEL

// BEGIN VERIFIED SCENE INTERACTION RESTORE KERNEL
/// Interaction state one frame carries from the active view into its detached candidate.
pub(crate) struct CarriedInteraction<I, V, S, P, C> {
    viewport: V,
    scroll: S,
    pressed: Option<I>,
    focused: Option<I>,
    pointer: Option<P>,
    frame_cache: C,
}

/// The candidate's own writes. `None` means it lost its document or a node mapping.
pub(crate) trait RestoreCandidate<I, V, S, P, C> {
    fn restore_viewport(&mut self, viewport: V, scroll: S) -> Option<()>;
    fn press_admitted(&self, id: I) -> bool;
    fn restore_press(&mut self, id: I) -> Option<()>;
    fn focus_admitted(&self, id: I) -> bool;
    fn restore_focus(&mut self, id: I) -> Option<()>;
    fn restore_hover(&mut self, pointer: P) -> Option<()>;
    fn resolve_restored(&mut self) -> Option<()>;
    fn adopt_pointer_and_cache(&mut self, pointer: Option<P>, frame_cache: C);
}

/// Replay carried state in its only legal order: viewport, then each identity the candidate still
/// admits, then hover, then one resolve; the pointer and frame cache are adopted only after that.
/// A failed write rejects the whole candidate, whereas an identity it no longer admits just drops.
/// 按唯一合法顺序重放：视口、候选仍准入的身份、悬停、一次解析，之后才采纳指针与帧缓存。
/// 写失败则整个候选作废；候选不再准入的身份只是丢弃。
fn restore_candidate_interaction<I: Copy, V, S, P: Copy, C, T: RestoreCandidate<I, V, S, P, C>>(
    candidate: &mut T,
    carried: CarriedInteraction<I, V, S, P, C>,
) -> Option<()> {
    let CarriedInteraction {
        viewport,
        scroll,
        pressed,
        focused,
        pointer,
        frame_cache,
    } = carried;
    candidate.restore_viewport(viewport, scroll)?;
    if let Some(id) = pressed {
        if candidate.press_admitted(id) {
            candidate.restore_press(id)?;
        }
    }
    if let Some(id) = focused {
        if candidate.focus_admitted(id) {
            candidate.restore_focus(id)?;
        }
    }
    if let Some(position) = pointer {
        candidate.restore_hover(position)?;
    }
    candidate.resolve_restored()?;
    candidate.adopt_pointer_and_cache(pointer, frame_cache);
    Some(())
}
// END VERIFIED SCENE INTERACTION RESTORE KERNEL

type CachedFrame = Option<(u32, u32, Vec<u8>)>;
impl RestoreCandidate<NodeId, Viewport, blitz_dom::util::Point<f64>, (f32, f32), CachedFrame>
    for NativeScene
{
    fn restore_viewport(
        &mut self,
        viewport: Viewport,
        scroll: blitz_dom::util::Point<f64>,
    ) -> Option<()> {
        let document = self.document.as_mut()?;
        document.set_viewport(viewport);
        document.set_viewport_scroll(scroll);
        Some(())
    }
    fn press_admitted(&self, id: NodeId) -> bool {
        self.source_visible(id)
    }
    fn restore_press(&mut self, id: NodeId) -> Option<()> {
        let native = self.nodes.get(&id)?.0;
        self.document.as_mut()?.set_mousedown_node_id(Some(native));
        Some(())
    }
    fn focus_admitted(&self, id: NodeId) -> bool {
        self.focus_eligible(id)
    }
    fn restore_focus(&mut self, id: NodeId) -> Option<()> {
        let native = self.nodes.get(&id)?.0;
        self.document.as_mut()?.set_focus_to(native);
        Some(())
    }
    fn restore_hover(&mut self, (x, y): (f32, f32)) -> Option<()> {
        self.document.as_mut()?.set_hover_to(x, y);
        Some(())
    }
    fn resolve_restored(&mut self) -> Option<()> {
        self.document.as_mut()?.resolve(0.0);
        Some(())
    }
    fn adopt_pointer_and_cache(&mut self, pointer: Option<(f32, f32)>, frame_cache: CachedFrame) {
        self.last_pointer = pointer;
        self.frame_cache = frame_cache;
    }
}

fn pressed_primary(input: &crate::scene_events::ScenePointerInput) -> bool {
    input.primary_pressed
}

// BEGIN VERIFIED SCENE WORK CAPACITY KERNEL
/// Candidate admission count against the host node limit (`usize::MAX` when unlimited); exact
/// and overflow-free. Not an allocator bound for text, fonts or upstream scratch.
/// 按宿主节点上限（无上限时为 `usize::MAX`）的候选准入计数；精确且无溢出。不是文字、字体或上游暂存的分配上界。
pub(crate) fn scene_work_fits(
    prepared: usize,
    pending: usize,
    incoming: usize,
    limit: usize,
) -> bool {
    if prepared > limit || pending > limit - prepared {
        return false;
    }
    incoming <= limit - prepared - pending
}
// END VERIFIED SCENE WORK CAPACITY KERNEL

fn semantic_attributes(semantics: &Semantics) -> Vec<(&'static str, String)> {
    let mut values = Vec::new();
    let role = match semantics.role {
        Role::Button => Some("button"),
        Role::TextField => Some("textbox"),
        Role::TextArea => Some("textbox"),
        Role::Checkbox => Some("checkbox"),
        Role::Radio => Some("radio"),
        Role::Switch => Some("switch"),
        Role::Slider => Some("slider"),
        Role::Image => Some("img"),
        Role::Link => Some("link"),
        Role::List => Some("list"),
        Role::ListItem => Some("listitem"),
        Role::Table => Some("table"),
        Role::TableRow => Some("row"),
        Role::TableCell => Some("cell"),
        Role::Dialog => Some("dialog"),
        Role::Popover => Some("group"),
        Role::Tooltip => Some("tooltip"),
        Role::Menu => Some("menu"),
        Role::MenuItem => Some("menuitem"),
        Role::ScrollView => Some("region"),
        Role::Heading(_) => Some("heading"),
        Role::Status => Some("status"),
        Role::Live(_) => Some("region"),
        Role::Alert => Some("alert"),
        Role::Form => Some("form"),
        Role::Container | Role::Text | Role::Unknown => None,
    };
    if let Some(role) = role {
        values.push(("role", role.to_owned()));
    }
    match semantics.role {
        Role::Heading(level) => values.push(("aria-level", level.get().to_string())),
        Role::TextArea => values.push(("aria-multiline", "true".to_owned())),
        Role::Live(politeness) => values.push((
            "aria-live",
            match politeness {
                jasper_ui_engine::semantics::LivePoliteness::Polite => "polite",
                jasper_ui_engine::semantics::LivePoliteness::Assertive => "assertive",
            }
            .to_owned(),
        )),
        _ => {}
    }
    for (key, value) in [
        ("aria-label", &semantics.label),
        ("aria-description", &semantics.description),
        ("aria-valuetext", &semantics.value),
    ] {
        if !value.is_empty() {
            values.push((key, value.clone()));
        }
    }
    values.push(("aria-disabled", semantics.disabled.to_string()));
    values.push(("aria-hidden", semantics.hidden.to_string()));
    if semantics.selected {
        values.push(("aria-selected", "true".to_owned()));
    }
    if semantics.disabled && semantics.role == Role::Button {
        values.push(("disabled", String::new()));
    }
    // Explicitly suppress native default button tab participation when Framework says false.
    if semantics.focusable || semantics.role == Role::Button {
        values.push((
            "tabindex",
            if semantics.focusable && !semantics.disabled {
                "0"
            } else {
                "-1"
            }
            .to_owned(),
        ));
    }
    values
}

/// One engine-painted overlay (outline ring, or a text decoration line as a hole-less box) in
/// view-local logical units, alpha already folded in.
#[cfg(feature = "cpu-pending")]
struct Overlay {
    outer: crate::style_effects::LogicalBox,
    outer_radii: [f32; 4],
    inner: Option<(crate::style_effects::LogicalBox, [f32; 4])>,
    rgba: [u8; 4],
    /// Ancestor overflow clips, outermost last.
    clips: Vec<OverlayClip>,
    /// E8 chain matrix of the decorated/outlined node (None: untransformed).
    matrix: Option<[f64; 6]>,
}

/// One overlay clip: a padding box in its node's layout frame and that node's chain matrix.
#[cfg(feature = "cpu-pending")]
#[derive(Clone, Copy, Debug)]
struct OverlayClip {
    rect: crate::style_effects::LogicalBox,
    matrix: Option<[f64; 6]>,
}

/// `outer ∘ inner` for `[a, b, c, d, e, f]` affines.
fn compose(outer: [f64; 6], inner: [f64; 6]) -> [f64; 6] {
    let [a1, b1, c1, d1, e1, f1] = outer;
    let [a2, b2, c2, d2, e2, f2] = inner;
    [
        a1 * a2 + c1 * b2,
        b1 * a2 + d1 * b2,
        a1 * c2 + c1 * d2,
        b1 * c2 + d1 * d2,
        a1 * e2 + c1 * f2 + e1,
        b1 * e2 + d1 * f2 + f1,
    ]
}

fn apply(m: [f64; 6], x: f64, y: f64) -> (f64, f64) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

/// Inverse affine; None when singular (e.g. scale 0) or not finite.
fn invert(m: [f64; 6]) -> Option<[f64; 6]> {
    let [a, b, c, d, e, f] = m;
    let det = a * d - b * c;
    if !det.is_finite() || det.abs() < 1e-12 {
        return None;
    }
    let out = [
        d / det,
        -b / det,
        -c / det,
        a / det,
        (c * f - d * e) / det,
        (b * e - a * f) / det,
    ];
    out.iter().all(|v| v.is_finite()).then_some(out)
}

/// One frame of the document plus engine overlays into `scene`, shared by every render path.
/// Without materials this is the direct Blitz paint (unchanged pixels); with materials the frame
/// is recorded, rewritten by `material::compose` (backdrops rasterised by the CPU renderer for
/// both paths) and replayed. Returns the material report and the backdrop image ids.
/// 把文档与引擎叠加层画进 `scene` 的一帧，所有渲染路径共用。无材质时即直接的 Blitz 绘制（像素不变）；有材质时整帧录制、
/// 经 `material::compose` 改写（两条路径的背景都由 CPU 渲染器光栅化）后回放。返回材质报告与背景图像 id。
#[cfg(feature = "cpu-pending")]
#[allow(clippy::too_many_arguments)]
fn paint_frame(
    scene: &mut impl anyrender::PaintScene,
    document: &mut BaseDocument,
    bold_aliases: &BTreeSet<u64>,
    scale: f64,
    (width, height): (u32, u32),
    rings: &[Overlay],
    glass: &BTreeMap<NodeId, crate::material::GlassSpec>,
    budget: crate::material::BackdropBudget,
) -> (crate::material::BackdropReport, Vec<u64>) {
    use anyrender::PaintScene;
    use peniko::kurbo::Affine;
    if glass.is_empty() {
        blitz_paint::paint_scene(
            &mut crate::glyph_skew::UprightSyntheticItalic {
                scene: &mut *scene,
                bold_aliases,
            },
            document,
            scale,
            width,
            height,
            0,
            0,
        );
        paint_overlays(scene, rings, scale);
        return (crate::material::BackdropReport::default(), Vec::new());
    }
    let mut recording = anyrender::recording::Scene::new();
    blitz_paint::paint_scene(
        &mut crate::glyph_skew::UprightSyntheticItalic {
            scene: &mut recording,
            bold_aliases,
        },
        document,
        scale,
        width,
        height,
        0,
        0,
    );
    let composed = crate::material::compose(
        recording.commands,
        glass,
        budget,
        (width, height),
        |prefix, open, region| {
            let offset = region.raster_transform();
            Some(anyrender::render_to_buffer::<
                anyrender_vello_cpu::VelloCpuImageRenderer,
                _,
            >(
                |backdrop| {
                    crate::material::replay(backdrop, prefix, offset);
                    for _ in 0..open {
                        backdrop.pop_layer();
                    }
                },
                region.width,
                region.height,
            ))
        },
    );
    crate::material::replay(scene, &composed.commands, Affine::IDENTITY);
    paint_overlays(scene, rings, scale);
    (composed.report, composed.images)
}

/// Paint overlays after the document: above every node, inside ancestor overflow clips.
/// Shared by the CPU and GPU paths (both paint through the same anyrender scene).
/// 在文档之后绘制叠加层：位于全部节点之上，受祖先 overflow 裁剪。CPU 与 GPU 路径共用（同一 anyrender 场景）。
#[cfg(feature = "cpu-pending")]
fn paint_overlays(scene: &mut impl anyrender::PaintScene, rings: &[Overlay], scale: f64) {
    use peniko::kurbo::{Affine, Rect, RoundedRect, RoundedRectRadii, Shape};
    let transform = Affine::scale(scale);
    let rect = |b: &crate::style_effects::LogicalBox| {
        Rect::new(
            f64::from(b.x),
            f64::from(b.y),
            f64::from(b.x + b.width),
            f64::from(b.y + b.height),
        )
    };
    let rounded = |b: &crate::style_effects::LogicalBox, r: &[f32; 4]| {
        RoundedRect::from_rect(
            rect(b),
            RoundedRectRadii::new(
                f64::from(r[0]),
                f64::from(r[1]),
                f64::from(r[2]),
                f64::from(r[3]),
            ),
        )
    };
    let with = |matrix: &Option<[f64; 6]>| match matrix {
        None => transform,
        Some(m) => transform * Affine::new(*m),
    };
    for ring in rings {
        let mut path = rounded(&ring.outer, &ring.outer_radii).to_path(0.05);
        if let Some((inner, radii)) = &ring.inner {
            path.extend(rounded(inner, radii).to_path(0.05));
        }
        for clip in ring.clips.iter().rev() {
            scene.push_clip_layer(peniko::Fill::NonZero, with(&clip.matrix), &rect(&clip.rect));
        }
        let [r, g, b, a] = ring.rgba;
        scene.fill(
            peniko::Fill::EvenOdd,
            with(&ring.matrix),
            peniko::Color::from_rgba8(r, g, b, a),
            None,
            &path,
        );
        for _ in &ring.clips {
            scene.pop_layer();
        }
    }
}

/// Result of E2/E10 pointer targeting: a steered native sample point, or a source subtree the
/// sample misses through a rounded corner.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct PointerHitAdjustment {
    steer: Option<(f32, f32)>,
    exclude: Option<NodeId>,
}

/// What a source node lays out as text: E15 line height / vertical alignment apply to Text
/// nodes only; an E14 text field also keeps its whitespace (`pre-wrap`) so the edited text maps
/// byte for byte onto its layout.
/// 源节点作为文字如何排版：E15 行高/垂直对齐只作用于 Text 节点；E14 文本框另保留空白（`pre-wrap`），
/// 使编辑文本与其布局逐字节对应。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextShape {
    NotText,
    Text,
    Field,
}

impl TextShape {
    pub(crate) fn of(node: &Node) -> Self {
        match node.kind {
            NodeKind::Text(_)
                if matches!(node.semantics.role, Role::TextField | Role::TextArea) =>
            {
                TextShape::Field
            }
            NodeKind::Text(_) => TextShape::Text,
            _ => TextShape::NotText,
        }
    }
}

fn style_properties(
    style: &Style,
    root: bool,
    families: &BTreeSet<String>,
    id: NodeId,
    shape: TextShape,
) -> Option<Vec<(&'static str, String)>> {
    let mut values = Vec::new();
    for (key, length) in [
        ("width", style.width),
        ("height", style.height),
        ("min-width", style.min_width),
        ("max-width", style.max_width),
        ("min-height", style.min_height),
        ("max-height", style.max_height),
    ] {
        let value = match length {
            Length::Auto if root && key == "width" => "100vw".to_owned(),
            Length::Auto if root && key == "height" => "100vh".to_owned(),
            Length::Auto => {
                if key.starts_with("max-") {
                    "none".to_owned()
                } else {
                    "auto".to_owned()
                }
            }
            Length::Px(v) if v.is_finite() && v >= 0.0 => format!("{v}px"),
            Length::Percent(v) if v.is_finite() && v >= 0.0 => format!("{v}%"),
            _ => return None,
        };
        values.push((key, value));
    }
    for (key, value) in [
        ("padding-top", style.padding.top),
        ("padding-right", style.padding.right),
        ("padding-bottom", style.padding.bottom),
        ("padding-left", style.padding.left),
        ("margin-top", style.margin.top),
        ("margin-right", style.margin.right),
        ("margin-bottom", style.margin.bottom),
        ("margin-left", style.margin.left),
        ("gap", style.gap),
        ("border-width", style.border_width),
        ("font-size", style.font_size),
    ] {
        if !value.is_finite() || value < 0.0 || (key == "font-size" && value == 0.0) {
            return None;
        }
        values.push((key, format!("{value}px")));
    }
    if !style.flex_grow.is_finite() || style.flex_grow < 0.0 {
        return None;
    }
    values.push(("flex-grow", style.flex_grow.to_string()));
    if !style.flex_shrink.is_finite() || style.flex_shrink < 0.0 {
        return None;
    }
    values.push(("flex-shrink", style.flex_shrink.to_string()));
    if !crate::native_fonts::font_weight_admits(style.font_weight) {
        return None;
    }
    values.push(("font-weight", style.font_weight.to_string()));
    // Every node states its family (no inheritance), like color and size; the generic keeps
    // the host-injected order for glyphs the named family lacks.
    // 每个节点都显式声明族（不继承），与颜色、字号一致；通用族保持宿主注入顺序，用于所选族缺字回退。
    values.push((
        "font-family",
        match &style.font_family {
            None => "sans-serif".to_owned(),
            Some(name)
                if crate::native_fonts::font_family_admits(name.as_bytes())
                    && families.contains(name) =>
            {
                format!("\"{name}\", sans-serif")
            }
            Some(_) => return None,
        },
    ));
    for (key, color) in [
        ("background-color", style.background),
        ("border-color", style.border_color),
        ("color", style.color),
    ] {
        values.push((
            key,
            format!(
                "#{:02x}{:02x}{:02x}{:02x}",
                color.r, color.g, color.b, color.a
            ),
        ));
    }
    for (key, value) in [
        (
            "display",
            match style.display {
                Display::Block => "block",
                Display::Flex => "flex",
                Display::Grid => "grid",
                Display::Table => "table",
                Display::TableRow => "table-row",
                Display::TableCell => "table-cell",
                Display::None => "none",
            },
        ),
        (
            "flex-direction",
            match style.direction {
                FlexDirection::Row => "row",
                FlexDirection::Column => "column",
            },
        ),
        (
            "flex-wrap",
            match style.flex_wrap {
                FlexWrap::NoWrap => "nowrap",
                FlexWrap::Wrap => "wrap",
                FlexWrap::WrapReverse => "wrap-reverse",
            },
        ),
        (
            "align-items",
            match style.align_items {
                Align::Start => "flex-start",
                Align::Center => "center",
                Align::End => "flex-end",
                Align::Baseline => "baseline",
                Align::Stretch => "stretch",
            },
        ),
        (
            "justify-content",
            match style.justify_content {
                Justify::Start => "flex-start",
                Justify::Center => "center",
                Justify::End => "flex-end",
                Justify::SpaceBetween => "space-between",
                Justify::SpaceAround => "space-around",
                Justify::SpaceEvenly => "space-evenly",
            },
        ),
        (
            "overflow",
            match style.overflow {
                Overflow::Visible => "visible",
                Overflow::Hidden => "hidden",
                Overflow::Scroll => "scroll",
            },
        ),
        (
            "font-style",
            match style.font_style {
                FontStyle::Normal => "normal",
                FontStyle::Italic => "italic",
            },
        ),
        ("border-style", "solid"),
        ("box-sizing", "border-box"),
    ] {
        values.push((key, value.to_owned()));
    }
    extended_style_properties(style, id, &mut values)?;
    text_style_properties(style, shape, &mut values)?;
    Some(values)
}

/// E15 (and the E14 field whitespace) on Blitz CSS, written after the base properties so the
/// later declarations win. Nothing is written for the defaults, so existing pixels are unchanged.
/// Vertical alignment moves the anonymous text box inside the element's flex container: the main
/// axis (`justify-content`) of a column, the cross axis (`align-items`) of a row; a block element
/// becomes a one-item stretched column, which lays its text out exactly as the block did. The
/// element's own box (layout size, hit area, `node_bounds`) is untouched.
/// E15（及 E14 文本框空白）到 Blitz CSS 的投影，写在基础属性之后以覆盖之。默认值不写任何属性，现有像素不变。
/// 垂直对齐移动元素 flex 容器里的匿名文字盒：列方向用主轴（`justify-content`），行方向用交叉轴（`align-items`）；
/// 块元素改为单项拉伸的列，其文字排版与块相同。元素自身的盒（布局尺寸、命中区、`node_bounds`）不变。
fn text_style_properties(
    style: &Style,
    shape: TextShape,
    values: &mut Vec<(&'static str, String)>,
) -> Option<()> {
    use jasper_ui_engine::style::{LineHeight, TextAlignVertical};
    if shape == TextShape::NotText {
        return Some(());
    }
    match style.line_height {
        None => {}
        Some(h) if !h.valid() => return None,
        Some(LineHeight::Px(v)) => values.push(("line-height", format!("{v}px"))),
        Some(LineHeight::Multiple(v)) => values.push(("line-height", format!("{v}"))),
    }
    values.push((
        "text-align",
        match style.text_align {
            TextAlign::Start => "start",
            TextAlign::Center => "center",
            TextAlign::End => "end",
            TextAlign::Justify => "justify",
        }
        .to_owned(),
    ));
    if style.text_direction != TextDirection::Auto {
        values.push((
            "direction",
            match style.text_direction {
                TextDirection::Ltr => "ltr",
                TextDirection::Rtl => "rtl",
                TextDirection::Auto => unreachable!(),
            }
            .to_owned(),
        ));
    }
    if let Some(align) = style.text_align_vertical {
        let value = match align {
            TextAlignVertical::Top => "flex-start",
            TextAlignVertical::Center => "center",
            TextAlignVertical::Bottom => "flex-end",
        };
        match (style.display, style.direction) {
            (Display::None, _) => {}
            (Display::Flex, FlexDirection::Column) => {
                values.push(("justify-content", value.to_owned()))
            }
            (Display::Flex, FlexDirection::Row) => values.push(("align-items", value.to_owned())),
            (Display::Block, _) => {
                values.push(("display", "flex".to_owned()));
                values.push(("flex-direction", "column".to_owned()));
                values.push(("align-items", "stretch".to_owned()));
                values.push(("justify-content", value.to_owned()));
            }
            _ => {}
        }
    }
    if shape == TextShape::Field {
        values.push(("white-space", "pre-wrap".to_owned()));
    }
    Some(())
}

/// E2/E3/E4/E9/E10 projection onto Blitz CSS. Every node states every property (defaults
/// included), like the base subset. Outline, text decoration and hit slop have no CSS here:
/// pinned Blitz has no `outline-offset` and propagates decorations only inside one inline
/// formatting context (not from a flex/block ancestor), so the engine paints both after the
/// document (`paint_overlays`); the slop is applied by pointer routing.
/// E2/E3/E4/E9/E10 到 Blitz CSS 的投影。与基础子集一样，每个节点显式写出全部属性（含默认值）。外框、文字装饰与
/// 命中扩展不经 CSS：固定版 Blitz 没有 `outline-offset`，且文字装饰只在同一行内格式化上下文内传播（不从 flex/块
/// 祖先传下），因此二者由引擎在文档之后绘制（`paint_overlays`）；命中扩展由指针路由处理。
fn extended_style_properties(
    style: &Style,
    id: NodeId,
    values: &mut Vec<(&'static str, String)>,
) -> Option<()> {
    use jasper_ui_engine::style::Side;
    if !style.extended_style_valid() {
        return None;
    }
    let hex = |c: jasper_ui_engine::style::Color| {
        format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a)
    };
    let r = style.corner_radius;
    for (key, value) in [
        ("border-top-left-radius", r.top_left),
        ("border-top-right-radius", r.top_right),
        ("border-bottom-right-radius", r.bottom_right),
        ("border-bottom-left-radius", r.bottom_left),
    ] {
        values.push((key, format!("{value}px")));
    }
    // Per-side longhands follow the uniform shorthands above, so they override only their side.
    for (side, width_key, color_key) in [
        (Side::Top, "border-top-width", "border-top-color"),
        (Side::Right, "border-right-width", "border-right-color"),
        (Side::Bottom, "border-bottom-width", "border-bottom-color"),
        (Side::Left, "border-left-width", "border-left-color"),
    ] {
        let border = style.border_of(side);
        values.push((width_key, format!("{}px", border.width)));
        values.push((color_key, hex(border.color)));
    }
    values.push((
        "box-shadow",
        if style.box_shadows.is_empty() {
            "none".to_owned()
        } else {
            style
                .box_shadows
                .iter()
                .map(|s| {
                    format!(
                        "{}{}px {}px {}px {}px {}",
                        if s.inset { "inset " } else { "" },
                        s.x,
                        s.y,
                        s.blur,
                        s.spread,
                        hex(s.color)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        },
    ));
    values.push(("opacity", style.opacity.to_string()));
    // E8: CSS order translate → rotate → scale about `transform-origin` (Blitz applies it to
    // paint and hit testing, not layout). Identity is `none`, so an untransformed node creates
    // no stacking context and paints exactly as before. A vanishing scale is kept invertible.
    let t = style.transform;
    let keep = |v: f32| {
        if v.abs() < 1e-4 {
            if v.is_sign_negative() {
                -1e-4
            } else {
                1e-4
            }
        } else {
            v
        }
    };
    values.push((
        "transform",
        if t.is_identity() {
            "none".to_owned()
        } else {
            format!(
                "translate({}px, {}px) rotate({}deg) scale({}, {})",
                t.translate_x,
                t.translate_y,
                t.rotate,
                keep(t.scale_x),
                keep(t.scale_y)
            )
        },
    ));
    values.push((
        "transform-origin",
        format!("{}% {}%", t.origin_x * 100.0, t.origin_y * 100.0),
    ));
    values.push((
        "position",
        match style.position {
            Position::Static => "static",
            Position::Relative => "relative",
            Position::Absolute => "absolute",
            Position::Fixed => "fixed",
            Position::Sticky => "sticky",
        }
        .to_owned(),
    ));
    for (key, length) in [
        ("top", style.inset.top),
        ("right", style.inset.right),
        ("bottom", style.inset.bottom),
        ("left", style.inset.left),
    ] {
        values.push((key, css_length(length)?));
    }
    values.push((
        "z-index",
        style
            .z_index
            .map_or_else(|| "auto".to_owned(), |value| value.to_string()),
    ));
    values.push(("order", style.order.to_string()));
    if let Some(tracks) = &style.grid_columns {
        values.push(("grid-template-columns", css_grid_tracks(tracks)?));
    }
    if let Some(tracks) = &style.grid_rows {
        values.push(("grid-template-rows", css_grid_tracks(tracks)?));
    }
    if let Some(value) = css_grid_line(style.grid_column) {
        values.push(("grid-column", value));
    }
    if let Some(value) = css_grid_line(style.grid_row) {
        values.push(("grid-row", value));
    }
    // E6: CSS gradient image above `background-color`, clipped with the background (rounded
    // border box). None is `none`, so a node without a gradient paints exactly as before.
    values.push((
        "background-image",
        match (&style.image, &style.background_gradient) {
            (Some(image), None) => format!(
                "url(\"about:blank#jasper-image-{}\")",
                image.source
            ),
            (None, None) => "none".to_owned(),
            (None, Some(g)) => css_gradient(g),
            (Some(_), Some(_)) => return None,
        },
    ));
    if style.image.is_some() {
        values.push(("background-size", "100% 100%".to_owned()));
        values.push(("background-repeat", "no-repeat".to_owned()));
        values.push(("background-origin", "content-box".to_owned()));
        values.push(("background-clip", "content-box".to_owned()));
    }
    // E5: the backdrop filter only tags the node's effect layer (`material::tag_css`); neither
    // renderer applies it, the engine composes the material itself (`material.rs`).
    values.push((
        "backdrop-filter",
        match style.backdrop {
            None => "none".to_owned(),
            Some(_) => crate::material::tag_css(id),
        },
    ));
    Some(())
}

fn css_length(length: Length) -> Option<String> {
    match length {
        Length::Auto => Some("auto".to_owned()),
        Length::Px(value) if value.is_finite() && value >= 0.0 => Some(format!("{value}px")),
        Length::Percent(value) if value.is_finite() && value >= 0.0 => {
            Some(format!("{value}%"))
        }
        _ => None,
    }
}

fn css_grid_tracks(tracks: &[GridTrack]) -> Option<String> {
    if !crate::style::valid_grid_tracks(tracks) {
        return None;
    }
    Some(
        tracks
            .iter()
            .map(|track| match track {
                GridTrack::Auto => "auto".to_owned(),
                GridTrack::Px(value) => format!("{value}px"),
                GridTrack::Percent(value) => format!("{value}%"),
                GridTrack::Fr(value) => format!("{value}fr"),
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}

fn css_grid_line(line: GridLine) -> Option<String> {
    match line {
        GridLine::Auto => None,
        GridLine::Line(value) if value != 0 => Some(value.to_string()),
        GridLine::Span(value) if value > 0 => Some(format!("auto / span {value}")),
        _ => None,
    }
}

/// CSS text of an E6 gradient (`Gradient::valid` holds).
fn css_gradient(g: &jasper_ui_engine::style::Gradient) -> String {
    use jasper_ui_engine::style::GradientShape;
    let stops = g
        .stops
        .iter()
        .map(|s| {
            format!(
                "#{:02x}{:02x}{:02x}{:02x} {}%",
                s.color.r,
                s.color.g,
                s.color.b,
                s.color.a,
                s.position * 100.0
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    match g.shape {
        GradientShape::Linear { angle } => format!("linear-gradient({angle}deg, {stops})"),
        GradientShape::Radial {
            center_x,
            center_y,
            radius,
        } => format!(
            "radial-gradient({} at {}% {}%, {stops})",
            match radius {
                None => "farthest-corner".to_owned(),
                Some(r) => format!("circle {r}px"),
            },
            center_x * 100.0,
            center_y * 100.0
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jasper_ui_engine::scene::{Node, NodeId};

    #[test]
    fn typed_v0_flex_and_text_values_project_to_blitz_css() {
        let style = Style::default()
            .direction(FlexDirection::Row)
            .flex_wrap(FlexWrap::WrapReverse)
            .flex_shrink(0.25)
            .align_items(Align::Baseline)
            .justify_content(Justify::SpaceEvenly)
            .text_align(TextAlign::End)
            .text_direction(TextDirection::Rtl);
        let values = style_properties(
            &style,
            false,
            &BTreeSet::new(),
            NodeId(2),
            TextShape::Text,
        )
        .unwrap();
        let values: BTreeMap<_, _> = values.into_iter().collect();
        assert_eq!(values["flex-wrap"], "wrap-reverse");
        assert_eq!(values["flex-shrink"], "0.25");
        assert_eq!(values["align-items"], "baseline");
        assert_eq!(values["justify-content"], "space-evenly");
        assert_eq!(values["text-align"], "end");
        assert_eq!(values["direction"], "rtl");

        let defaults = style_properties(
            &Style::default(),
            false,
            &BTreeSet::new(),
            NodeId(3),
            TextShape::Text,
        )
        .unwrap()
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        assert_eq!(defaults["flex-wrap"], "nowrap");
        assert_eq!(defaults["flex-shrink"], "1");
        assert_eq!(defaults["align-items"], "stretch");
        assert_eq!(defaults["justify-content"], "flex-start");
        assert_eq!(defaults["text-align"], "start");
        assert!(!defaults.contains_key("direction"));
    }

    #[test]
    fn visibility_rejects_cyclic_or_incomplete_source_metadata_without_native_focus() {
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let button = Node::text(NodeId(2), "Action")
            .with_semantics(Semantics::default().role(Role::Button).focusable(true));
        let source = Scene::new(Node::container(NodeId(1)).child(button));
        let version = source.version();
        let mut view = NativeScene::from_scene(10, &source, &bytes, 160, 80).unwrap();
        assert!(view.source_visible(NodeId(2)));
        view.parents.insert(NodeId(1), Some(NodeId(2)));
        assert!(!view.source_visible(NodeId(2)));
        assert!(!view.focus_from_host(10, version, NodeId(2), &mut |_| panic!("cyclic focus")));
        assert!(
            !view.activate_from_host(10, version, NodeId(2), &mut |_| panic!("cyclic activation"))
        );
        view.parents.insert(NodeId(1), None);
        assert!(view.source_visible(NodeId(2)));
        view.semantics.get_mut(&NodeId(1)).unwrap().hidden = true;
        assert!(!view.source_visible(NodeId(2)));
        view.semantics.get_mut(&NodeId(1)).unwrap().hidden = false;
        view.displayed.remove(&NodeId(2));
        assert!(!view.source_visible(NodeId(2)));
        assert_eq!(view.focused_node(10, version), None);
    }

    #[test]
    fn source_ancestry_is_bounded_even_for_cyclic_metadata() {
        let parents = BTreeMap::from([
            (NodeId(1), None),
            (NodeId(2), Some(NodeId(1))),
            (NodeId(3), Some(NodeId(2))),
        ]);
        assert!(source_subtree_contains(&parents, NodeId(3), NodeId(1)));
        assert!(source_subtree_contains(&parents, NodeId(2), NodeId(2)));
        assert!(!source_subtree_contains(&parents, NodeId(1), NodeId(3)));
        assert!(!source_subtree_contains(&parents, NodeId(99), NodeId(1)));
        let cycle = BTreeMap::from([(NodeId(1), Some(NodeId(2))), (NodeId(2), Some(NodeId(1)))]);
        assert!(!source_subtree_contains(&cycle, NodeId(1), NodeId(3)));
        assert!(source_subtree_contains(&cycle, NodeId(1), NodeId(2)));
        assert!(!source_subtree_contains(
            &BTreeMap::new(),
            NodeId(1),
            NodeId(2)
        ));
        assert!(source_subtree_contains(
            &BTreeMap::new(),
            NodeId(1),
            NodeId(1)
        ));
    }

    #[test]
    fn host_semantic_hierarchy_tracks_source_frame_order_and_retirement() {
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let mut scene =
            Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2), "Display only")));
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 160, 80).unwrap();
        let version = scene.version();
        let before = native.semantics_snapshot_from_host(10, version).unwrap();
        assert_eq!(before.root, NodeId(1));
        assert_eq!(before.nodes[1].semantics.label, "");
        assert!(native.semantics_snapshot_from_host(11, version).is_none());
        let metadata = Semantics {
            role: Role::Button,
            label: "Explicit name".into(),
            description: "Host metadata".into(),
            ..Default::default()
        };
        let patches = [
            ScenePatch::Insert {
                parent: NodeId(1),
                index: 0,
                node: Node::text(NodeId(3), "Caption").with_semantics(metadata.clone()),
            },
            ScenePatch::Remove { id: NodeId(2) },
        ];
        scene.apply_batch_at(version, &patches).unwrap();
        assert!(native.apply_patches(10, version, scene.version(), &patches));
        assert!(native.semantics_snapshot_from_host(10, version).is_none());
        let after = native
            .semantics_snapshot_from_host(10, scene.version())
            .unwrap();
        assert_eq!(after.scene_version, scene.version());
        assert_eq!(after.nodes.len(), 2);
        assert_eq!(after.nodes[0].children, vec![NodeId(3)]);
        assert_eq!(after.nodes[1].semantics, metadata);
        assert!(native.close(10));
        assert!(native
            .semantics_snapshot_from_host(10, scene.version())
            .is_none());
    }

    #[test]
    #[cfg(feature = "cpu-pending")]
    fn host_viewport_scroll_changes_pixels_and_rejects_stale_routing() {
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let scene = Scene::new(
            Node::container(NodeId(1))
                .child(Node::text(NodeId(2), "Viewport 中文"))
                .child(
                    Node::rect(NodeId(3)).with_style(
                        Style::default()
                            .size(Length::Px(80.0), Length::Px(120.0))
                            .background(Color::RED),
                    ),
                ),
        );
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 160, 80).unwrap();
        let version = scene.version();
        let before = native.render_cpu(10, 1024 * 1024).unwrap();
        assert!(native.set_viewport_scroll_from_host(10, version, 0.0, 24.0));
        let scrolled = native.render_cpu(10, 1024 * 1024).unwrap();
        assert!(before != scrolled);
        for (caller, v, x, y) in [
            (11, version, 0.0, 0.0),
            (10, version + 1, 0.0, 0.0),
            (10, version, f64::NAN, 0.0),
            (10, version, 0.0, f64::INFINITY),
            (10, version, -1.0, 0.0),
            (10, version, 0.0, u32::MAX as f64 + 1.0),
        ] {
            assert!(!native.set_viewport_scroll_from_host(caller, v, x, y));
            assert!(scrolled == native.render_cpu(10, 1024 * 1024).unwrap());
        }
        assert_eq!(native.snapshot_identity(10), Some((version, 3)));
        assert!(native.set_viewport_scroll_from_host(10, version, 0.0, 0.0));
        assert!(before == native.render_cpu(10, 1024 * 1024).unwrap());
        assert!(native.close(10));
        assert!(!native.set_viewport_scroll_from_host(10, version, 0.0, 0.0));
    }

    #[test]
    fn semantic_activation_routes_own_node_without_requiring_keyboard_focus() {
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let mut button = Node::text(NodeId(2), "Save");
        button.semantics = Semantics {
            role: Role::Button,
            ..Default::default()
        };
        let mut disabled = button.clone();
        disabled.id = NodeId(3);
        disabled.semantics.disabled = true;
        let mut hidden = Node::container(NodeId(4)).child(button.clone());
        hidden.children[0].id = NodeId(5);
        hidden.semantics.hidden = true;
        let mut absent = Node::container(NodeId(6)).child(button.clone());
        absent.children[0].id = NodeId(7);
        absent.style.display = Display::None;
        let scene = Scene::new(
            Node::container(NodeId(1))
                .child(button)
                .child(disabled)
                .child(hidden)
                .child(absent),
        );
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 160, 80).unwrap();
        let version = scene.version();
        let mut events = Vec::new();
        for (caller, v, id) in [
            (11, version, 2),
            (10, version + 1, 2),
            (10, version, 1),
            (10, version, 3),
            (10, version, 5),
            (10, version, 7),
            (10, version, 999),
        ] {
            assert!(!native.activate_from_host(caller, v, NodeId(id), &mut |e| events.push(e)));
        }
        assert!(events.is_empty());
        assert_eq!(native.focused_node(10, version), None);
        assert!(native.activate_from_host(10, version, NodeId(2), &mut |e| events.push(e)));
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].node, NodeId(2));
        assert_eq!(events[0].scene_version, version);
        assert_eq!(native.focused_node(10, version), None);
        assert!(native.close(10));
        assert!(!native.activate_from_host(10, version, NodeId(2), &mut |e| events.push(e)));
        assert_eq!(events.len(), 1);
    }

    #[test]
    #[cfg(feature = "cpu-pending")]
    fn native_scene_copies_into_injected_host_stride_without_touching_padding() {
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let scene = Scene::new(
            Node::container(NodeId(1)).child(Node::text(NodeId(2), "Software host 中文")),
        );
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 160, 80).unwrap();
        let version = scene.version();
        let packed = native.render_cpu(10, 1024 * 1024).unwrap();
        let row = 160 * 4;
        let stride = row + 16;
        let mut destination = vec![0xa5; 79 * stride + row];
        for (caller, v, s, budget) in [
            (11, version, stride, 1024 * 1024),
            (10, version + 1, stride, 1024 * 1024),
            (10, version, row - 1, 1024 * 1024),
            (10, version, stride, 1),
        ] {
            assert!(!native.render_to_host_surface(caller, v, &mut destination, s, budget));
            assert!(destination.iter().all(|byte| *byte == 0xa5));
        }
        assert!(native.render_to_host_surface(10, version, &mut destination, stride, 1024 * 1024));
        let view = jasper_ui_engine::surface::Rgba8SurfaceView::new(&destination, 160, 80, stride)
            .unwrap();
        for y in 0..80 {
            assert_eq!(view.row(y).unwrap(), &packed[y * row..(y + 1) * row]);
        }
        for y in 0..79 {
            assert!(destination[y * stride + row..(y + 1) * stride]
                .iter()
                .all(|byte| *byte == 0xa5));
        }
        let mut short = vec![0xa5; destination.len() - 1];
        assert!(!native.render_to_host_surface(10, version, &mut short, stride, 1024 * 1024));
        assert!(short.iter().all(|byte| *byte == 0xa5));
    }

    #[test]
    #[cfg(feature = "cpu-pending")]
    fn mixed_source_frame_preflights_new_targets_then_matches_authoritative_scene() {
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let mut scene =
            Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2), "Original")));
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 320, 240).unwrap();
        let version = scene.version();
        let before = native.render_cpu(10, 1024 * 1024).unwrap();
        let invalid = [
            ScenePatch::Insert {
                parent: NodeId(1),
                index: 0,
                node: Node::text(NodeId(3), "Must not appear"),
            },
            ScenePatch::UpdateText {
                id: NodeId(99),
                text: "No target".into(),
            },
        ];
        assert!(!native.apply_patches(10, version, version + 1, &invalid));
        assert!(before == native.render_cpu(10, 1024 * 1024).unwrap());
        assert!(!native.nodes.contains_key(&NodeId(3)));
        let patches = [
            ScenePatch::Insert {
                parent: NodeId(1),
                index: 0,
                node: Node::container(NodeId(3)).child(Node::text(NodeId(4), "Old")),
            },
            ScenePatch::UpdateText {
                id: NodeId(4),
                text: "New 中文".into(),
            },
            ScenePatch::Insert {
                parent: NodeId(3),
                index: usize::MAX,
                node: Node::text(NodeId(5), "Temporary"),
            },
            ScenePatch::UpdateStyle {
                id: NodeId(3),
                style: Style::default().background(Color::BLUE),
            },
            ScenePatch::Remove { id: NodeId(2) },
            ScenePatch::Remove { id: NodeId(5) },
        ];
        scene.apply_batch_at(version, &patches).unwrap();
        assert!(native.apply_patches(10, version, scene.version(), &patches));
        assert_eq!(native.snapshot_identity(10), Some((scene.version(), 3)));
        assert_eq!(native.children.get(&NodeId(1)), Some(&vec![NodeId(3)]));
        assert_eq!(native.children.get(&NodeId(3)), Some(&vec![NodeId(4)]));
        let mut reference = NativeScene::from_scene(10, &scene, &bytes, 320, 240).unwrap();
        assert!(
            native.render_cpu(10, 1024 * 1024).unwrap()
                == reference.render_cpu(10, 1024 * 1024).unwrap()
        );
        let version = scene.version();
        let patches = [
            ScenePatch::Remove { id: NodeId(3) },
            ScenePatch::Insert {
                parent: NodeId(1),
                index: 0,
                node: Node::text(NodeId(3), "Replaced kind"),
            },
        ];
        scene.apply_batch_at(version, &patches).unwrap();
        assert!(native.apply_patches(10, version, scene.version(), &patches));
        assert_eq!(native.snapshot_identity(10), Some((scene.version(), 2)));
        assert!(!native.nodes.contains_key(&NodeId(4)));
        let mut reference = NativeScene::from_scene(10, &scene, &bytes, 320, 240).unwrap();
        assert!(
            native.render_cpu(10, 1024 * 1024).unwrap()
                == reference.render_cpu(10, 1024 * 1024).unwrap()
        );
    }

    #[test]
    #[cfg(feature = "cpu-pending")]
    fn source_scene_clamps_out_of_range_insert_index_to_append() {
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let mut scene =
            Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2), "First")));
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        let version = scene.version();
        let patches = [ScenePatch::Insert {
            parent: NodeId(1),
            index: usize::MAX,
            node: Node::text(NodeId(3), "Append"),
        }];
        scene.apply_batch_at(version, &patches).unwrap();
        assert!(native.apply_patches(10, version, scene.version(), &patches));
        assert_eq!(
            native.children.get(&NodeId(1)),
            Some(&vec![NodeId(2), NodeId(3)])
        );
        let mut reference = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        assert!(
            native.render_cpu(10, 1024 * 1024).unwrap()
                == reference.render_cpu(10, 1024 * 1024).unwrap()
        );
    }

    #[test]
    #[cfg(feature = "cpu-pending")]
    fn source_insert_subtree_preserves_order_identity_and_snapshot_pixels() {
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let mut scene = Scene::new(
            Node::container(NodeId(1))
                .child(Node::text(NodeId(2), "First"))
                .child(Node::text(NodeId(3), "Last")),
        );
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 320, 240).unwrap();
        let version = scene.version();
        let before = native.render_cpu(10, 1024 * 1024).unwrap();
        for patch in [
            ScenePatch::Insert {
                parent: NodeId(1),
                index: 0,
                node: Node::text(NodeId(2), "Duplicate"),
            },
            ScenePatch::Insert {
                parent: NodeId(99),
                index: 0,
                node: Node::text(NodeId(9), "Bad parent"),
            },
        ] {
            assert!(!native.apply_patches(10, version, version + 1, &[patch]));
        }
        assert!(before == native.render_cpu(10, 1024 * 1024).unwrap());
        let mut button = Node::text(NodeId(5), "Inserted button");
        button.semantics = Semantics {
            role: Role::Button,
            focusable: true,
            ..Default::default()
        };
        let incoming = Node::container(NodeId(4))
            .child(button)
            .child(Node::text(NodeId(6), "Inserted 中文"));
        let patches = [ScenePatch::Insert {
            parent: NodeId(1),
            index: 1,
            node: incoming,
        }];
        scene.apply_batch_at(version, &patches).unwrap();
        assert!(native.apply_patches(10, version, scene.version(), &patches));
        assert_eq!(
            native.children.get(&NodeId(1)),
            Some(&vec![NodeId(2), NodeId(4), NodeId(3)])
        );
        let mut reference = NativeScene::from_scene(10, &scene, &bytes, 320, 240).unwrap();
        assert!(
            native.render_cpu(10, 1024 * 1024).unwrap()
                == reference.render_cpu(10, 1024 * 1024).unwrap(),
            "inserted native subtree must match source order"
        );
        assert!(native.focus_from_host(10, scene.version(), NodeId(5), &mut |_| {}));
        for (id, index) in [(NodeId(7), 0), (NodeId(8), 4)] {
            let version = scene.version();
            let patches = [ScenePatch::Insert {
                parent: NodeId(1),
                index,
                node: Node::text(id, "Edge insertion"),
            }];
            scene.apply_batch_at(version, &patches).unwrap();
            assert!(native.apply_patches(10, version, scene.version(), &patches));
        }
        assert_eq!(native.snapshot_identity(10), Some((scene.version(), 8)));
        let mut reference = NativeScene::from_scene(10, &scene, &bytes, 320, 240).unwrap();
        // Focus styling is transient; compare with equal native focus state.
        assert!(reference.focus_from_host(10, scene.version(), NodeId(5), &mut |_| {}));
        assert!(
            native.render_cpu(10, 1024 * 1024).unwrap()
                == reference.render_cpu(10, 1024 * 1024).unwrap()
        );
    }

    #[test]
    #[cfg(feature = "cpu-pending")]
    fn source_subtree_removal_cleans_descendant_identity_semantics_and_focus() {
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let mut button = Node::text(NodeId(3), "Nested");
        button.semantics = Semantics {
            role: Role::Button,
            focusable: true,
            ..Default::default()
        };
        let mut scene = Scene::new(
            Node::container(NodeId(1))
                .child(
                    Node::container(NodeId(2))
                        .child(button)
                        .child(Node::text(NodeId(4), "Other nested")),
                )
                .child(Node::rect(NodeId(5))),
        );
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        let version = scene.version();
        assert!(native.focus_from_host(10, version, NodeId(3), &mut |_| {}));
        let before = native.render_cpu(10, 1024 * 1024).unwrap();
        let invalid = [
            ScenePatch::Remove { id: NodeId(2) },
            ScenePatch::UpdateText {
                id: NodeId(4),
                text: "late".into(),
            },
        ];
        assert!(!native.apply_patches(10, version, version + 1, &invalid));
        assert!(before == native.render_cpu(10, 1024 * 1024).unwrap());
        let patches = [ScenePatch::Remove { id: NodeId(2) }];
        scene.apply_batch_at(version, &patches).unwrap();
        let mut focus = Vec::new();
        assert!(native.apply_patches_with_focus_events(
            10,
            version,
            scene.version(),
            &patches,
            &mut |e| focus.push((e.node, e.focused))
        ));
        assert_eq!(focus, vec![(NodeId(3), false)]);
        assert_eq!(native.snapshot_identity(10), Some((scene.version(), 2)));
        for id in [NodeId(2), NodeId(3), NodeId(4)] {
            assert!(native.semantics_from_host(10, id).is_none());
            assert!(!native.nodes.contains_key(&id));
        }
        let mut reference = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        assert!(
            native.render_cpu(10, 1024 * 1024).unwrap()
                == reference.render_cpu(10, 1024 * 1024).unwrap()
        );
    }

    #[test]
    #[cfg(feature = "cpu-pending")]
    fn source_leaf_removal_preflights_order_and_retires_node_focus_metadata() {
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let mut button = Node::text(NodeId(2), "Remove");
        button.semantics = Semantics {
            role: Role::Button,
            focusable: true,
            ..Default::default()
        };
        let mut scene = Scene::new(
            Node::container(NodeId(1))
                .child(button)
                .child(Node::rect(NodeId(3))),
        );
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        let version = scene.version();
        assert!(native.focus_from_host(10, version, NodeId(2), &mut |_| {}));
        let before = native.render_cpu(10, 1024 * 1024).unwrap();
        let invalid = [
            ScenePatch::Remove { id: NodeId(2) },
            ScenePatch::UpdateText {
                id: NodeId(2),
                text: "Late".into(),
            },
        ];
        assert!(!native.apply_patches(10, version, version + 1, &invalid));
        assert!(before == native.render_cpu(10, 1024 * 1024).unwrap());
        assert_eq!(native.focused_node(10, version), Some(NodeId(2)));
        assert!(!native.apply_patches(
            10,
            version,
            version + 1,
            &[ScenePatch::Remove { id: NodeId(1) }]
        ));
        let patches = [
            ScenePatch::UpdateText {
                id: NodeId(2),
                text: "Before remove".into(),
            },
            ScenePatch::Remove { id: NodeId(2) },
            ScenePatch::UpdateStyle {
                id: NodeId(3),
                style: Style::default()
                    .size(Length::Px(80.0), Length::Px(30.0))
                    .background(Color::GREEN),
            },
        ];
        scene.apply_batch_at(version, &patches).unwrap();
        let mut focus = Vec::new();
        assert!(native.apply_patches_with_focus_events(
            10,
            version,
            scene.version(),
            &patches,
            &mut |event| focus.push((event.node, event.focused))
        ));
        assert_eq!(focus, vec![(NodeId(2), false)]);
        assert_eq!(native.snapshot_identity(10), Some((scene.version(), 2)));
        assert!(native.semantics_from_host(10, NodeId(2)).is_none());
        let mut reference = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        assert!(
            native.render_cpu(10, 1024 * 1024).unwrap()
                == reference.render_cpu(10, 1024 * 1024).unwrap()
        );
    }

    #[test]
    fn scene_retirement_notifies_focus_loss_once_after_owner_version_admission() {
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let mut button = Node::text(NodeId(2), "Save");
        button.semantics = Semantics {
            role: Role::Button,
            focusable: true,
            ..Default::default()
        };
        let scene = Scene::new(Node::container(NodeId(1)).child(button));
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        let version = scene.version();
        assert!(native.focus_from_host(10, version, NodeId(2), &mut |_| {}));
        assert!(!native.close_with_focus_events(11, version, &mut |_| panic!("foreign close")));
        assert!(!native.close_with_focus_events(10, version + 1, &mut |_| panic!("stale close")));
        assert_eq!(native.focused_node(10, version), Some(NodeId(2)));
        let mut lost = Vec::new();
        assert!(
            native.close_with_focus_events(10, version, &mut |e| lost.push((
                e.scene_version,
                e.node,
                e.focused
            )))
        );
        assert_eq!(lost, vec![(version, NodeId(2), false)]);
        assert!(!native.close_with_focus_events(10, version, &mut |_| panic!("duplicate close")));
        assert!(native.snapshot_identity(10).is_none());
    }

    #[test]
    fn pointer_and_style_batch_publish_actual_scene_focus_transitions_once() {
        use crate::{native_events::NativeEventReply, native_view::PointerAction};
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let mut button = Node::text(NodeId(2), "Save");
        button.semantics = Semantics {
            role: Role::Button,
            focusable: true,
            ..Default::default()
        };
        let mut scene = Scene::new(Node::container(NodeId(1)).child(button));
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        let mut notifications = Vec::new();
        let version = scene.version();
        // Framework may request focus explicitly; a later press inside it keeps it (no event).
        assert!(
            native.focus_from_host(10, version, NodeId(2), &mut |e| notifications.push((
                e.scene_version,
                e.node,
                e.focused
            )))
        );
        assert!(native.pointer_with_focus_events_from_host(
            10,
            version,
            PointerAction::Down,
            12.0,
            12.0,
            true,
            &mut |_| NativeEventReply::default(),
            &mut |_| panic!("unchanged pointer focus")
        ));
        assert_eq!(notifications, vec![(version, NodeId(2), true)]);
        assert!(native.pointer_with_focus_events_from_host(
            10,
            version,
            PointerAction::Up,
            12.0,
            12.0,
            false,
            &mut |_| NativeEventReply::default(),
            &mut |_| panic!("a click inside the focused button keeps its focus")
        ));
        assert_eq!(notifications, vec![(version, NodeId(2), true)]);
        // A press outside any focusable control blurs it, as in a browser.
        assert!(native.pointer_with_focus_events_from_host(
            10,
            version,
            PointerAction::Down,
            300.0,
            190.0,
            true,
            &mut |_| NativeEventReply::default(),
            &mut |e| notifications.push((e.scene_version, e.node, e.focused))
        ));
        assert!(native.pointer_with_focus_events_from_host(
            10,
            version,
            PointerAction::Up,
            300.0,
            190.0,
            false,
            &mut |_| NativeEventReply::default(),
            &mut |e| notifications.push((e.scene_version, e.node, e.focused))
        ));
        assert_eq!(
            notifications,
            vec![(version, NodeId(2), true), (version, NodeId(2), false)]
        );
        assert!(
            native.focus_from_host(10, version, NodeId(2), &mut |e| notifications.push((
                e.scene_version,
                e.node,
                e.focused
            )))
        );
        let patches = [ScenePatch::UpdateStyle {
            id: NodeId(1),
            style: Style::default().display(Display::None),
        }];
        assert!(!native.apply_patches_with_focus_events(
            11,
            version,
            version + 1,
            &patches,
            &mut |_| panic!("foreign patch focus")
        ));
        scene.apply_batch_at(version, &patches).unwrap();
        assert!(native.apply_patches_with_focus_events(
            10,
            version,
            scene.version(),
            &patches,
            &mut |e| notifications.push((e.scene_version, e.node, e.focused))
        ));
        assert_eq!(
            notifications,
            vec![
                (version, NodeId(2), true),
                (version, NodeId(2), false),
                (version, NodeId(2), true),
                (scene.version(), NodeId(2), false)
            ]
        );
        assert!(!native.apply_patches_with_focus_events(
            10,
            version,
            scene.version(),
            &patches,
            &mut |_| panic!("stale patch focus")
        ));
    }

    #[test]
    fn scene_focus_rejects_display_none_and_hidden_ancestors() {
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let mut child = Node::text(NodeId(2), "Focus");
        child.semantics = Semantics {
            role: Role::Button,
            focusable: true,
            ..Default::default()
        };
        let mut scene = Scene::new(Node::container(NodeId(1)).child(child));
        scene.root.style.display = Display::None;
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        assert!(!native.focus_from_host(10, scene.version(), NodeId(2), &mut |_| {}));
        scene.root.style.display = Display::Flex;
        scene.root.semantics.hidden = true;
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        assert!(!native.focus_from_host(10, scene.version(), NodeId(2), &mut |_| {}));
        scene.root.semantics.hidden = false;
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        assert!(native.focus_from_host(10, scene.version(), NodeId(2), &mut |_| {}));
        let expected = scene.version();
        let patches = [ScenePatch::UpdateStyle {
            id: NodeId(1),
            style: Style::default().display(Display::None),
        }];
        scene.apply_batch_at(expected, &patches).unwrap();
        assert!(native.apply_patches(10, expected, scene.version(), &patches));
        assert!(native.focused_node(10, scene.version()).is_none());
        assert!(
            !native.focus_from_host(10, scene.version(), NodeId(2), &mut |_| panic!(
                "hidden after patch"
            ))
        );
    }

    #[test]
    fn scene_native_focus_checks_semantics_and_notifies_actual_transitions() {
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let mut first = Node::text(NodeId(2), "First");
        first.semantics = Semantics {
            role: Role::Button,
            focusable: true,
            ..Default::default()
        };
        let mut second = Node::text(NodeId(3), "Second");
        second.semantics = first.semantics.clone();
        let mut disabled = Node::text(NodeId(4), "Disabled");
        disabled.semantics = Semantics {
            disabled: true,
            ..first.semantics.clone()
        };
        let mut hidden = Node::text(NodeId(5), "Hidden");
        hidden.semantics = Semantics {
            hidden: true,
            ..first.semantics.clone()
        };
        let scene = Scene::new(
            Node::container(NodeId(1)).with_children(vec![first, second, disabled, hidden]),
        );
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        let version = scene.version();
        let mut events = Vec::new();
        for (caller, v, id) in [
            (11, version, NodeId(2)),
            (10, version + 1, NodeId(2)),
            (10, version, NodeId(4)),
            (10, version, NodeId(5)),
            (10, version, NodeId(1)),
        ] {
            assert!(!native.focus_from_host(caller, v, id, &mut |_| panic!("rejected focus")));
        }
        assert!(
            native.focus_from_host(10, version, NodeId(2), &mut |e| events.push((
                e.scene_version,
                e.node,
                e.focused
            )))
        );
        assert!(native.focus_from_host(10, version, NodeId(2), &mut |_| panic!("duplicate focus")));
        assert!(
            native.focus_from_host(10, version, NodeId(3), &mut |e| events.push((
                e.scene_version,
                e.node,
                e.focused
            )))
        );
        assert_eq!(native.focused_node(10, version), Some(NodeId(3)));
        assert!(!native.clear_focus_from_host(11, version, &mut |_| panic!("foreign clear")));
        assert!(
            native.clear_focus_from_host(10, version, &mut |e| events.push((
                e.scene_version,
                e.node,
                e.focused
            )))
        );
        assert!(native.clear_focus_from_host(10, version, &mut |_| panic!("duplicate clear")));
        assert_eq!(
            events,
            vec![
                (version, NodeId(2), true),
                (version, NodeId(2), false),
                (version, NodeId(3), true),
                (version, NodeId(3), false)
            ]
        );
        assert!(native.close(10));
        assert!(native.focused_node(10, version).is_none());
        assert!(!native.focus_from_host(10, version, NodeId(2), &mut |_| panic!("closed focus")));
    }

    #[test]
    fn nested_button_content_keeps_pointer_target_but_activates_button_owner() {
        use crate::{
            native_events::NativeEventReply, native_view::PointerAction,
            scene_events::SceneEventKind,
        };
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let mut button = Node::container(NodeId(2)).child(Node::text(NodeId(3), "Nested save"));
        button.semantics = Semantics::default().role(Role::Button);
        let scene = Scene::new(Node::container(NodeId(1)).child(button));
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        let mut events = Vec::new();
        for (phase, pressed) in [(PointerAction::Down, true), (PointerAction::Up, false)] {
            assert!(native.pointer_from_host(
                10,
                scene.version(),
                phase,
                12.0,
                12.0,
                pressed,
                &mut |e| {
                    events.push(e);
                    NativeEventReply::default()
                }
            ));
        }
        assert!(
            events
                .iter()
                .any(|e| e.kind == SceneEventKind::Pointer(PointerAction::Down)
                    && e.node == NodeId(3))
        );
        assert!(events
            .iter()
            .any(|e| e.kind == SceneEventKind::Activate && e.node == NodeId(2)));
    }

    #[test]
    fn scene_pointer_routes_own_node_identity_and_native_activation() {
        use crate::{
            native_events::NativeEventReply, native_view::PointerAction,
            scene_events::SceneEventKind,
        };
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let mut button = Node::text(NodeId(2), "Save");
        button.semantics = Semantics::default().role(Role::Button);
        let scene = Scene::new(Node::container(NodeId(1)).child(button));
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        let version = scene.version();
        let mut events = Vec::new();
        assert!(!native.pointer_from_host(
            11,
            version,
            PointerAction::Down,
            12.0,
            12.0,
            true,
            &mut |_| panic!("foreign")
        ));
        assert!(!native.pointer_from_host(
            10,
            version + 1,
            PointerAction::Down,
            12.0,
            12.0,
            true,
            &mut |_| panic!("stale")
        ));
        assert!(native.pointer_from_host(
            10,
            version,
            PointerAction::Down,
            12.0,
            12.0,
            true,
            &mut |e| {
                events.push(e);
                NativeEventReply::default()
            }
        ));
        assert!(native.pointer_from_host(
            10,
            version,
            PointerAction::Up,
            12.0,
            12.0,
            false,
            &mut |e| {
                events.push(e);
                NativeEventReply::default()
            }
        ));
        assert!(events
            .iter()
            .any(|e| e.kind == SceneEventKind::Activate && e.node == NodeId(2)));
        assert!(events.iter().all(|e| e.scene_version == version));
        assert!(events
            .iter()
            .filter(|e| e.kind == SceneEventKind::Pointer(PointerAction::Down))
            .all(|e| e.node == NodeId(2) && e.position == (12.0, 12.0)));
        assert!(native.close(10));
        assert!(!native.pointer_from_host(
            10,
            version,
            PointerAction::Down,
            12.0,
            12.0,
            true,
            &mut |_| panic!("closed")
        ));
    }

    #[test]
    #[cfg(feature = "cpu-pending")]
    fn scene_consumes_host_physical_extent_dpi_and_zero_surface_lifecycle() {
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let scene =
            Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2), "Scale 中文")));
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        let baseline = native.render_cpu(10, 1024 * 1024).unwrap();
        for (caller, scale) in [(11, 2.0), (10, 0.0), (10, f32::NAN), (10, f32::INFINITY)] {
            assert!(!native.set_viewport_from_host(caller, 640, 400, scale));
            assert!(baseline == native.render_cpu(10, 1024 * 1024).unwrap());
        }
        assert!(native.set_viewport_from_host(10, 640, 400, 2.0));
        assert_eq!(
            native.render_cpu(10, 2 * 1024 * 1024).unwrap().len(),
            640 * 400 * 4
        );
        assert_eq!(native.snapshot_identity(10), Some((scene.version(), 2)));
        assert!(native.set_viewport_from_host(10, 0, 0, 2.0));
        assert!(native.render_cpu(10, 1024 * 1024).is_none());
        assert!(native.set_viewport_from_host(10, 320, 200, 1.0));
        assert!(baseline == native.render_cpu(10, 1024 * 1024).unwrap());
        assert!(native.close(10));
        assert!(!native.set_viewport_from_host(10, 320, 200, 1.0));
    }

    #[test]
    fn scene_semantics_preserve_all_fields_without_claiming_os_actions() {
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let supplied = Semantics {
            role: Role::Button,
            label: "Save 中文".into(),
            description: "Save this file".into(),
            value: "pending".into(),
            disabled: true,
            focusable: true,
            hidden: false,
            selected: false,
        };
        let mut node = Node::text(NodeId(2), "Save");
        node.semantics = supplied.clone();
        let scene = Scene::new(Node::container(NodeId(1)).child(node));
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        assert_eq!(native.semantics_from_host(10, NodeId(2)), Some(&supplied));
        let (id, _) = native.nodes.get(&NodeId(2)).unwrap();
        let element = native
            .document
            .as_ref()
            .unwrap()
            .get_node(*id)
            .unwrap()
            .element_data()
            .unwrap();
        assert_eq!(element.name.local, LocalName::from("button"));
        assert_eq!(
            element.attr(LocalName::from("aria-label")),
            Some("Save 中文")
        );
        assert_eq!(element.attr(LocalName::from("tabindex")), Some("-1"));
        assert!(element.attr(LocalName::from("disabled")).is_some());
        assert!(native.close(10));
        assert!(native.semantics_from_host(10, NodeId(2)).is_none());
    }

    #[test]
    #[cfg(feature = "cpu-pending")]
    fn source_patch_batch_updates_same_native_tree_and_preflights_rejection() {
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let mut scene =
            Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2), "Before")));
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        let before = native.render_cpu(10, 1024 * 1024).unwrap();
        let version = scene.version();
        let invalid = [
            ScenePatch::UpdateText {
                id: NodeId(2),
                text: "Must not appear".into(),
            },
            ScenePatch::UpdateText {
                id: NodeId(1),
                text: "Container is not text".into(),
            },
        ];
        assert!(!native.apply_patches(10, version, version + 1, &invalid));
        assert!(before == native.render_cpu(10, 1024 * 1024).unwrap());
        let oversized = vec![
            ScenePatch::UpdateText {
                id: NodeId(2),
                text: "x".into()
            };
            4097
        ];
        // No per-frame operation limit; only a host quota refuses a frame for size.
        assert_eq!(
            native.apply_patches_result_within(
                10,
                version,
                version + 1,
                &oversized,
                SceneQuota {
                    max_nodes: Some(1),
                    max_text_bytes: None
                }
            ),
            Err(SceneUpdateError::NodeQuotaExceeded {
                limit: 1,
                required: 2
            })
        );
        assert!(before == native.render_cpu(10, 1024 * 1024).unwrap());
        let patches = [
            ScenePatch::UpdateText {
                id: NodeId(2),
                text: "After 中文".into(),
            },
            ScenePatch::UpdateStyle {
                id: NodeId(2),
                style: Style::default().background(Color::BLUE),
            },
        ];
        assert!(!native.apply_patches(11, version, version + 1, &patches));
        assert!(!native.apply_patches(10, version, version + 2, &patches));
        scene.apply_batch_at(version, &patches).unwrap();
        assert!(native.apply_patches(10, version, scene.version(), &patches));
        assert_eq!(native.snapshot_identity(10), Some((scene.version(), 2)));
        let after = native.render_cpu(10, 1024 * 1024).unwrap();
        assert!(after != before);
        let mut reference = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        assert!(
            after == reference.render_cpu(10, 1024 * 1024).unwrap(),
            "patched tree must match source snapshot import"
        );
        assert!(!native.apply_patches(10, version, scene.version(), &patches));
        assert!(native.close(10));
        assert!(!native.apply_patches(10, scene.version(), scene.version() + 1, &patches));
    }

    /// Everything a host can observe of the active view without repainting it.
    #[cfg(feature = "cpu-pending")]
    fn active_view_state(native: &NativeScene, version: u64) -> String {
        let document = native.document.as_ref().unwrap();
        let scroll = document.viewport_scroll();
        format!(
            "{:?} {:?} {:?} {:?} {} {:?} {:?} {:?} {:?}",
            native.snapshot_identity(10),
            native.focused_node(10, version),
            document.get_mousedown_node_id(),
            native.last_pointer,
            native.frame_dirty,
            native
                .frame_cache
                .as_ref()
                .map(|(w, h, pixels)| (*w, *h, pixels.len())),
            (scroll.x, scroll.y),
            native.nodes,
            native.source.version(),
        )
    }

    #[test]
    #[cfg(feature = "cpu-pending")]
    fn frame_stage_panic_or_rejection_leaves_active_view_then_same_frame_commits() {
        use crate::{native_events::NativeEventReply, native_view::PointerAction};
        use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let mut button = Node::text(NodeId(2), "Before");
        button.semantics = Semantics {
            role: Role::Button,
            focusable: true,
            ..Default::default()
        };
        button.style = Style::default().height(Length::Px(80.0));
        let filler =
            Node::container(NodeId(3)).with_style(Style::default().height(Length::Px(800.0)));
        let mut scene = Scene::new(Node::container(NodeId(1)).with_children(vec![button, filler]));
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        let version = scene.version();
        assert!(native.set_viewport_scroll_from_host(10, version, 0.0, 4.0));
        assert!(native.focus_from_host(10, version, NodeId(2), &mut |_| {}));
        assert!(native.pointer_from_host(
            10,
            version,
            PointerAction::Down,
            12.0,
            12.0,
            true,
            &mut |_| NativeEventReply::default()
        ));
        let before = native.render_cpu(10, 8 * 1024 * 1024).unwrap();
        let state = active_view_state(&native, version);
        // Non-vacuous: the state this test protects is actually live.
        assert_eq!(native.focused_node(10, version), Some(NodeId(2)));
        assert!(native
            .document
            .as_ref()
            .unwrap()
            .get_mousedown_node_id()
            .is_some());
        assert_eq!(native.last_pointer, Some((12.0, 12.0)));
        assert_eq!(native.document.as_ref().unwrap().viewport_scroll().y, 4.0);
        assert!(!native.frame_dirty && native.frame_cache.is_some());
        let patches = [ScenePatch::UpdateText {
            id: NodeId(2),
            text: "After".into(),
        }];
        let next = version + 1;
        // Repaint from the active document, not the cached frame.
        let mut assert_untouched = |native: &mut NativeScene| {
            assert_eq!(active_view_state(native, version), state);
            assert!(native.invalidate_frame_from_host(10));
            assert!(before == native.render_cpu(10, 8 * 1024 * 1024).unwrap());
            assert_eq!(active_view_state(native, version), state);
        };
        // Panic before any native construction.
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            native.apply_patches_built_by(10, version, next, &patches, |_, _, _, _| {
                resume_unwind(Box::new("builder"))
            })
        }));
        assert!(outcome.is_err());
        assert_untouched(&mut native);
        // Panic with a complete real candidate alive: unwinding drops its Blitz document.
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            native.apply_patches_built_by(
                10,
                version,
                next,
                &patches,
                |view, source, focused, frame| {
                    let candidate = view.rebuild_candidate(source, focused, frame, usize::MAX);
                    assert!(candidate.is_some());
                    resume_unwind(Box::new("after candidate"))
                },
            )
        }));
        assert!(outcome.is_err());
        assert_untouched(&mut native);
        // A real candidate discarded by a native rejection.
        assert_eq!(
            native.apply_patches_built_by(
                10,
                version,
                next,
                &patches,
                |view, source, focused, frame| {
                    assert!(view
                        .rebuild_candidate(source, focused, frame, usize::MAX)
                        .is_some());
                    None
                }
            ),
            Err(SceneUpdateError::NativeRejected)
        );
        assert_untouched(&mut native);
        // A rejected source never reaches the builder.
        let invalid = [ScenePatch::UpdateText {
            id: NodeId(1),
            text: "Container is not text".into(),
        }];
        assert_eq!(
            native.apply_patches_built_by(10, version, next, &invalid, |_, _, _, _| {
                panic!("builder ran on a rejected source")
            }),
            Err(SceneUpdateError::SourceRejected)
        );
        assert_untouched(&mut native);
        // Nothing poisoned: the same frame commits and restores interaction state.
        scene.apply_batch_at(version, &patches).unwrap();
        assert!(native.apply_patches(10, version, next, &patches));
        assert_eq!(native.snapshot_identity(10), Some((next, 3)));
        assert_eq!(native.focused_node(10, next), Some(NodeId(2)));
        assert!(native
            .document
            .as_ref()
            .unwrap()
            .get_mousedown_node_id()
            .is_some());
        assert_eq!(native.last_pointer, Some((12.0, 12.0)));
        assert_eq!(native.document.as_ref().unwrap().viewport_scroll().y, 4.0);
        assert!(native.frame_dirty && native.frame_cache.is_some());
        let after = native.render_cpu(10, 8 * 1024 * 1024).unwrap();
        assert!(after != before);
        let mut reference = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        assert!(reference.set_viewport_scroll_from_host(10, next, 0.0, 4.0));
        assert!(after == reference.render_cpu(10, 8 * 1024 * 1024).unwrap());
    }

    /// The success path of the replay on a real document, read back from the replaced view.
    #[test]
    #[cfg(feature = "cpu-pending")]
    fn committed_frame_replays_interaction_state_and_drops_what_the_candidate_refuses() {
        use crate::{native_events::NativeEventReply, native_view::PointerAction};
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let button = |id: u64, height: f32| {
            let mut node = Node::text(NodeId(id), "Go");
            node.semantics = Semantics {
                role: Role::Button,
                focusable: true,
                ..Default::default()
            };
            node.style = Style::default().height(Length::Px(height));
            node
        };
        let filler =
            Node::container(NodeId(4)).with_style(Style::default().height(Length::Px(800.0)));
        let scene = Scene::new(Node::container(NodeId(1)).with_children(vec![
            button(2, 80.0),
            button(3, 80.0),
            filler,
        ]));
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        let version = scene.version();
        assert!(native.set_viewport_from_host(10, 400, 300, 2.0));
        assert!(native.set_viewport_scroll_from_host(10, version, 0.0, 6.0));
        assert!(native.pointer_from_host(
            10,
            version,
            PointerAction::Down,
            12.0,
            12.0,
            true,
            &mut |_| NativeEventReply::default()
        ));
        let pressed_native = native.document.as_ref().unwrap().get_mousedown_node_id();
        let native_id = |native: &NativeScene, id: NodeId| native.nodes.get(&id).unwrap().0;
        let press = [NodeId(2), NodeId(3)]
            .into_iter()
            .find(|id| pressed_native == Some(native_id(&native, *id)))
            .expect("the press landed on a button");
        let focus = if press == NodeId(2) {
            NodeId(3)
        } else {
            NodeId(2)
        };
        assert!(native.focus_from_host(10, version, focus, &mut |_| {}));
        native.render_cpu(10, 16 * 1024 * 1024).unwrap();
        let viewport = native.document.as_ref().unwrap().viewport().clone();
        let cached = native.frame_cache.clone();
        assert!(native
            .document
            .as_ref()
            .unwrap()
            .get_hover_node_id()
            .is_some());
        assert!(cached.is_some());

        // Both identities survive and every carried value reaches the replaced view.
        let next = version + 1;
        assert!(native.apply_patches(
            10,
            version,
            next,
            &[ScenePatch::UpdateText {
                id: press,
                text: "Ok".into(),
            }],
        ));
        let document = native.document.as_ref().unwrap();
        assert_eq!(document.viewport().window_size, viewport.window_size);
        assert_eq!(document.viewport().hidpi_scale, viewport.hidpi_scale);
        assert_eq!(document.viewport_scroll().y, 6.0);
        assert_eq!(
            document.get_mousedown_node_id(),
            Some(native_id(&native, press))
        );
        assert!(document.get_hover_node_id().is_some());
        assert_eq!(native.focused_node(10, next), Some(focus));
        assert_eq!(native.last_pointer, Some((12.0, 12.0)));
        assert_eq!(native.frame_cache, cached);

        // Hiding both identities drops them without rejecting the frame.
        let last = next + 1;
        let hidden = Style::default()
            .height(Length::Px(80.0))
            .display(crate::style::Display::None);
        assert!(native.apply_patches(
            10,
            next,
            last,
            &[
                ScenePatch::UpdateStyle {
                    id: press,
                    style: hidden.clone(),
                },
                ScenePatch::UpdateStyle {
                    id: focus,
                    style: hidden,
                },
            ],
        ));
        let document = native.document.as_ref().unwrap();
        assert_eq!(document.get_mousedown_node_id(), None);
        assert_eq!(native.focused_node(10, last), None);
        assert_eq!(native.last_pointer, Some((12.0, 12.0)));
        assert_eq!(document.viewport_scroll().y, 6.0);
    }

    #[test]
    #[cfg(feature = "cpu-pending")]
    fn existing_scene_imports_native_tree_and_rejects_unsupported_inputs() {
        let bytes =
            std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required"))
                .unwrap();
        let scene = Scene::new(
            Node::container(NodeId(1))
                .child(Node::text(NodeId(2), "Existing 中文"))
                .child(
                    Node::rect(NodeId(3)).with_style(
                        Style::default()
                            .size(Length::Px(80.0), Length::Px(30.0))
                            .background(Color::RED),
                    ),
                ),
        );
        let mut native = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        assert_eq!(native.snapshot_identity(10), Some((scene.version(), 3)));
        assert_eq!(native.snapshot_identity(11), None);
        assert!(native.render_cpu(11, 1024 * 1024).is_none());
        let pixels = native.render_cpu(10, 1024 * 1024).unwrap();
        assert!(pixels.chunks_exact(4).any(|p| p[0] > 150 && p[1] < 100));
        let mut duplicate = scene.clone();
        duplicate.root.children[1].id = NodeId(2);
        assert!(NativeScene::from_scene(10, &duplicate, &bytes, 320, 200).is_none());
        let mut invalid = scene.clone();
        invalid.root.style.gap = f32::NAN;
        assert!(NativeScene::from_scene(10, &invalid, &bytes, 320, 200).is_none());
        invalid = scene.clone();
        invalid.root.semantics.label = "Preserved semantics".into();
        let semantic_view = NativeScene::from_scene(10, &invalid, &bytes, 320, 200).unwrap();
        assert_eq!(
            semantic_view
                .semantics_from_host(10, NodeId(1))
                .unwrap()
                .label,
            "Preserved semantics"
        );
        assert!(semantic_view.semantics_from_host(11, NodeId(1)).is_none());
        invalid = scene.clone();
        invalid.root.scroll.x = f32::NAN;
        assert!(NativeScene::from_scene(10, &invalid, &bytes, 320, 200).is_none());
        let wide = Scene::new(
            Node::container(NodeId(0))
                .with_children((1..4097).map(|id| Node::text(NodeId(id), "x")).collect()),
        );
        // Node count is unlimited: a scene beyond the former 4096 cap imports.
        let wide_view = NativeScene::from_scene(10, &wide, &bytes, 320, 200).unwrap();
        assert_eq!(
            wide_view.snapshot_identity(10),
            Some((wide.version(), 4097))
        );
        let mut deep = Node::text(NodeId(0), "x");
        for id in 1..131 {
            deep = Node::container(NodeId(id)).child(deep);
        }
        assert!(NativeScene::from_scene(10, &Scene::new(deep), &bytes, 320, 200).is_none());
        assert!(!native.close(11));
        assert!(native.close(10));
        assert!(native.render_cpu(10, 1024 * 1024).is_none());
    }
}

// BEGIN VERIFIED SOURCE HIDDEN ANCESTRY KERNEL
fn source_hidden_ancestry(
    parents: &BTreeMap<NodeId, Option<NodeId>>,
    semantics: &BTreeMap<NodeId, Semantics>,
    ancestor: Option<NodeId>,
) -> Option<bool> {
    let mut current = ancestor;
    let mut hidden = false;
    let mut remaining = parents.len();
    loop {
        let id = match current {
            Some(id) => id,
            None => return Some(hidden),
        };
        if remaining == 0 {
            return None;
        }
        remaining -= 1;
        let Some(semantic) = semantics.get(&id) else {
            return None;
        };
        hidden |= semantic.hidden;
        current = match parents.get(&id) {
            Some(parent) => *parent,
            None => return None,
        };
    }
}
// END VERIFIED SOURCE HIDDEN ANCESTRY KERNEL

/// Candidate finite nonnegative source-offset admission; native owns scroll geometry.
fn native_scroll_offset(
    offset: jasper_ui_engine::geometry::Point,
) -> Option<blitz_dom::Point<f64>> {
    if !offset.x.is_finite()
        || !offset.y.is_finite()
        || offset.x < 0.0
        || offset.y < 0.0
        || offset.x as f64 > u32::MAX as f64
        || offset.y as f64 > u32::MAX as f64
    {
        return None;
    }
    Some(blitz_dom::Point {
        x: offset.x as f64,
        y: offset.y as f64,
    })
}

#[cfg(test)]
mod semantic_attribute_update_tests {
    use super::*;
    #[test]
    fn native_semantic_update_clears_stale_attributes_on_same_element() {
        let bytes = std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
        let initial = Semantics {
            role: Role::Link,
            label: "Open".into(),
            description: "Resource".into(),
            value: "old".into(),
            disabled: true,
            hidden: true,
            focusable: true,
            selected: false,
        };
        let mut source = Scene::new(
            Node::container(NodeId(1)).child(Node::text(NodeId(2), "Text").with_semantics(initial)),
        );
        let mut native = NativeScene::from_scene(10, &source, &bytes, 160, 80).unwrap();
        let element_id = native.nodes[&NodeId(2)].0;
        let old = source.version();
        let patches = [ScenePatch::UpdateSemantics {
            id: NodeId(2),
            semantics: Semantics::default().role(Role::Text),
        }];
        source.apply_batch_at(old, &patches).unwrap();
        assert!(native.apply_patches(10, old, source.version(), &patches));
        assert_eq!(native.nodes[&NodeId(2)].0, element_id);
        let element = native
            .document
            .as_ref()
            .unwrap()
            .get_node(element_id)
            .unwrap()
            .element_data()
            .unwrap();
        for key in [
            "role",
            "aria-label",
            "aria-description",
            "aria-valuetext",
            "disabled",
            "tabindex",
        ] {
            assert_eq!(
                element.attr(LocalName::from(key)),
                None,
                "stale native attribute {key}"
            );
        }
        assert_eq!(element.attr(LocalName::from("aria-hidden")), Some("false"));
        assert_eq!(
            element.attr(LocalName::from("aria-disabled")),
            Some("false")
        );
    }
}

// BEGIN VERIFIED SCENE SNAPSHOT IDENTITY KERNEL
impl NativeScene {
    pub(crate) fn snapshot_identity(&self, caller: u64) -> Option<(u64, usize)> {
        if caller != self.owner || !self.document.is_open() {
            return None;
        }
        Some((self.scene_version, self.node_count))
    }
}
// END VERIFIED SCENE SNAPSHOT IDENTITY KERNEL

// BEGIN VERIFIED SEMANTIC ROLE TAG KERNEL
/// Only Button selects a different native element tag in append_prepared_subtree.
pub(crate) fn semantic_role_update_preserves_tag(current: Role, next: Role) -> bool {
    role_uses_native_button(current) == role_uses_native_button(next)
}
// END VERIFIED SEMANTIC ROLE TAG KERNEL

#[cfg(test)]
mod source_box_sizing_tests {
    use super::*;
    use jasper_ui_engine::geometry::Edges;
    #[test]
    fn source_declared_size_includes_padding_and_border() {
        let bytes = std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
        let style = Style {
            display: Display::Block,
            width: Length::Px(80.0),
            height: Length::Px(40.0),
            padding: Edges::all(5.0),
            border_width: 2.0,
            ..Default::default()
        };
        let source =
            Scene::new(Node::container(NodeId(1)).child(Node::rect(NodeId(2)).with_style(style)));
        let view = NativeScene::from_scene(10, &source, &bytes, 320, 200).unwrap();
        let node = view
            .document
            .as_ref()
            .unwrap()
            .get_node(view.nodes[&NodeId(2)].0)
            .unwrap();
        assert_eq!(node.final_layout().size.width, 80.0);
        assert_eq!(node.final_layout().size.height, 40.0);
    }
}

#[cfg(test)]
mod source_root_viewport_tests {
    use super::*;
    #[test]
    fn auto_source_root_fills_native_logical_viewport() {
        let bytes = std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
        let mut scene =
            Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2), "Small content")));
        let mut view = NativeScene::from_scene(10, &scene, &bytes, 320, 200).unwrap();
        let root = view
            .document
            .as_ref()
            .unwrap()
            .get_node(view.nodes[&NodeId(1)].0)
            .unwrap();
        assert_eq!(root.final_layout().size.width, 320.0);
        assert_eq!(root.final_layout().size.height, 200.0);
        assert!(view.set_viewport_from_host(10, 320, 200, 2.0));
        let size = |view: &NativeScene| {
            view.document
                .as_ref()
                .unwrap()
                .get_node(view.nodes[&NodeId(1)].0)
                .unwrap()
                .final_layout()
                .size
        };
        assert_eq!((size(&view).width, size(&view).height), (160.0, 100.0));
        for (style, expected) in [
            (
                Style::default().size(Length::Px(80.0), Length::Px(40.0)),
                (80.0, 40.0),
            ),
            (Style::default(), (160.0, 100.0)),
        ] {
            let old = scene.version();
            let patches = [ScenePatch::UpdateStyle {
                id: NodeId(1),
                style,
            }];
            scene.apply_batch_at(old, &patches).unwrap();
            assert!(view.apply_patches(10, old, scene.version(), &patches));
            assert_eq!((size(&view).width, size(&view).height), expected);
        }
        assert!(view.set_viewport_from_host(10, 640, 400, 2.0));
        assert_eq!((size(&view).width, size(&view).height), (320.0, 200.0));
    }
}

// BEGIN VERIFIED NATIVE BUTTON ROLE KERNEL
pub(crate) fn role_uses_native_button(role: Role) -> bool {
    matches!(role, Role::Button)
}
// END VERIFIED NATIVE BUTTON ROLE KERNEL

#[cfg(test)]
mod deactivation_hover_tests {
    use super::*;
    #[test]
    fn deactivation_clears_actual_native_hover_and_press() {
        let font = std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
        let source = Scene::new(
            Node::container(NodeId(1)).child(
                Node::text(NodeId(2), "Save")
                    .with_semantics(Semantics::default().role(Role::Button).focusable(true)),
            ),
        );
        let mut view = NativeScene::from_scene(10, &source, &font, 160, 80).unwrap();
        assert!(view.pointer_from_host(
            10,
            source.version(),
            crate::native_view::PointerAction::Down,
            12.0,
            12.0,
            true,
            &mut |_| Default::default()
        ));
        assert!(view
            .document
            .as_ref()
            .unwrap()
            .get_hover_node_id()
            .is_some());
        assert!(view
            .document
            .as_ref()
            .unwrap()
            .get_mousedown_node_id()
            .is_some());
        assert!(
            !view.deactivate_from_host(11, source.version(), &mut |_| panic!("foreign cleanup"))
        );
        assert!(view
            .document
            .as_ref()
            .unwrap()
            .get_hover_node_id()
            .is_some());
        assert!(view.deactivate_from_host(10, source.version(), &mut |_| {}));
        let doc = view.document.as_ref().unwrap();
        assert!(doc.get_hover_node_id().is_none());
        assert!(doc.get_mousedown_node_id().is_none());
        assert_ne!(doc.get_focussed_node_id(), Some(view.nodes[&NodeId(2)].0));
        assert_eq!(view.focused_node(10, source.version()), None);
    }
}
