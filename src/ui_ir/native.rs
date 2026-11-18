//! Native compatibility mapping from validated Typed UI IR to the existing Rust scene.
//!
//! Backend `NodeId`s are allocated only in this mapping and are never exposed as Framework
//! identity. Any semantic or style field the current Native scene cannot represent is reported
//! and rejects the frame by default.

use super::events::{
    check_authority_values, check_permission, AdmittedEvent, AuthorityRef, UiEventAdmissionError,
};
use super::*;
use crate::geometry::Edges as NativeEdges;
use crate::overlay::{
    OverlayAlign as NativeOverlayAlign, OverlayAnchor as NativeOverlayAnchor,
    OverlaySide as NativeOverlaySide, OverlaySpec,
};
use crate::scene::{Node, NodeId, NodeKind, Scene, SceneError, ScenePatch};
use crate::semantics::{HeadingLevel, Role, Semantics};
use crate::style::{
    Align as NativeAlign, Color as NativeColor, CornerRadii, Display as NativeDisplay,
    FlexDirection as NativeFlexDirection, FontStyle as NativeFontStyle, GridLine, GridTrack,
    ImageStyle as NativeImageStyle, Inset as NativeInset, Justify as NativeJustify,
    Length as NativeLength, LineHeight as NativeLineHeight, ObjectFit as NativeObjectFit,
    Position as NativePosition, Style as NativeStyle, TextAlignVertical as NativeVerticalAlign,
};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SupportLevel {
    Required,
    Optional,
    HostOwned,
    WebOnly,
    NativeOnly,
    Candidate,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MappingIssueKind {
    InformationLoss,
    Unsupported,
    InvalidState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MappingIssue {
    pub path: String,
    pub property: Option<&'static str>,
    pub support: SupportLevel,
    pub kind: MappingIssueKind,
    pub detail: String,
}

impl fmt::Display for MappingIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {:?} at {:?} ({:?}): {}",
            self.path, self.kind, self.property, self.support, self.detail
        )
    }
}

impl std::error::Error for MappingIssue {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MappedProperty {
    pub support: SupportLevel,
    pub host_accepted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeUiFrameError {
    InvalidIr(UiIrError),
    Revision { expected: u64, actual: u64 },
    AuthorityMismatch,
    MissingRoot,
    DuplicateRoot,
    Mapping(MappingIssue),
    Scene(String),
}

impl fmt::Display for NativeUiFrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIr(error) => write!(f, "invalid UI IR: {error}"),
            Self::Revision { expected, actual } => {
                write!(
                    f,
                    "frame expects revision {expected}, current revision is {actual}"
                )
            }
            Self::AuthorityMismatch => {
                write!(f, "frame identity domain, owner or session changed")
            }
            Self::MissingRoot => write!(f, "the mapped scene has no root insertion"),
            Self::DuplicateRoot => write!(f, "the scene already has a root"),
            Self::Mapping(issue) => write!(f, "{issue}"),
            Self::Scene(error) => write!(f, "native scene rejected the frame: {error}"),
        }
    }
}

impl std::error::Error for NativeUiFrameError {}

impl From<UiIrError> for NativeUiFrameError {
    fn from(value: UiIrError) -> Self {
        Self::InvalidIr(value)
    }
}

impl From<MappingIssue> for NativeUiFrameError {
    fn from(value: MappingIssue) -> Self {
        Self::Mapping(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Authority {
    identity_domain: String,
    owner: String,
    session: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResourceLife {
    Prepared,
    Committed,
}

#[derive(Clone, Debug)]
struct ResourceState {
    resource: ResourceRef,
    life: ResourceLife,
}

/// Validated, executable Native document profile for one admitted IR session.
#[derive(Clone, Debug)]
pub struct NativeUiDocument {
    authority: Authority,
    revision: u64,
    scene: Scene,
    next_node_id: u64,
    key_to_node: BTreeMap<UiKey, NodeId>,
    node_to_key: BTreeMap<NodeId, UiKey>,
    element_kinds: BTreeMap<UiKey, ElementKind>,
    event_bindings: BTreeMap<UiKey, BTreeMap<UiEventKind, EventBinding>>,
    resource_refs: BTreeMap<String, ResourceRef>,
    resource_generations: BTreeMap<String, (ResourceKind, u64)>,
    resources: BTreeMap<String, ResourceState>,
    environment: UiEnvironment,
    mapped_properties: BTreeMap<&'static str, MappedProperty>,
}

impl NativeUiDocument {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, NativeUiFrameError> {
        Self::from_frame(&UiFrame::from_json_slice(bytes)?)
    }

    /// Construct the initial Native document. The first admitted frame must branch from revision
    /// one and contain exactly one root insertion.
    pub fn from_frame(frame: &UiFrame) -> Result<Self, NativeUiFrameError> {
        frame.validate()?;
        if frame.expected_revision != 1 || frame.next_revision != 2 {
            return Err(NativeUiFrameError::Revision {
                expected: 1,
                actual: frame.expected_revision,
            });
        }
        let authority = Authority {
            identity_domain: frame.identity_domain.clone(),
            owner: frame.owner.clone(),
            session: frame.session.clone(),
        };
        let mut builder = DocumentBuilder::new(authority);
        builder.apply_ops(&frame.ops)?;
        builder.finish(frame.next_revision)
    }

    /// Atomically apply a successor frame. Mapping is performed on a detached clone; an error
    /// leaves the previous document and revision untouched.
    pub fn apply_frame(&mut self, frame: &UiFrame) -> Result<(), NativeUiFrameError> {
        frame.validate()?;
        let authority = Authority {
            identity_domain: frame.identity_domain.clone(),
            owner: frame.owner.clone(),
            session: frame.session.clone(),
        };
        if authority != self.authority {
            return Err(NativeUiFrameError::AuthorityMismatch);
        }
        if frame.expected_revision != self.revision {
            return Err(NativeUiFrameError::Revision {
                expected: frame.expected_revision,
                actual: self.revision,
            });
        }
        let builder = DocumentBuilder {
            authority: self.authority.clone(),
            revision: self.revision,
            scene: self.scene.clone(),
            next_node_id: self.next_node_id,
            key_to_node: self.key_to_node.clone(),
            node_to_key: self.node_to_key.clone(),
            element_kinds: self.element_kinds.clone(),
            event_bindings: self.event_bindings.clone(),
            resource_refs: self.resource_refs.clone(),
            resource_generations: self.resource_generations.clone(),
            resources: self.resources.clone(),
            environment: self.environment.clone(),
            mapped_properties: self.mapped_properties.clone(),
        };
        let next = builder.apply_successor(frame)?;
        *self = next;
        Ok(())
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn identity_domain(&self) -> &str {
        &self.authority.identity_domain
    }

    pub fn owner(&self) -> &str {
        &self.authority.owner
    }

    pub fn session(&self) -> &str {
        &self.authority.session
    }

    pub fn node_for_key(&self, key: UiKey) -> Option<NodeId> {
        self.key_to_node.get(&key).copied()
    }

    pub fn key_for_node(&self, node: NodeId) -> Option<UiKey> {
        self.node_to_key.get(&node).copied()
    }

    pub fn event_binding(&self, key: UiKey, event: UiEventKind) -> Option<&EventBinding> {
        self.event_bindings.get(&key)?.get(&event)
    }

    pub fn resource_ref(&self, id: &str) -> Option<&ResourceRef> {
        self.resource_refs.get(id)
    }

    /// Resource currently prepared or committed in this document.
    pub fn active_resource(&self, id: &str) -> Option<&ResourceRef> {
        self.resources.get(id).map(|state| &state.resource)
    }

    pub fn environment(&self) -> &UiEnvironment {
        &self.environment
    }

    pub fn mapped_properties(&self) -> &BTreeMap<&'static str, MappedProperty> {
        &self.mapped_properties
    }

    /// Admit an event against this document's current owner/session/revision and internal map.
    /// A stale `UiKey` does not fall back to a newly allocated backend node.
    pub fn admit_event(
        &self,
        context: &UiEventContext,
        event: UiEventKind,
    ) -> Result<AdmittedEvent, UiEventAdmissionError> {
        check_authority_values(
            AuthorityRef {
                identity_domain: &context.identity_domain,
                owner: &context.owner,
                session: &context.session,
                revision: context.frame_revision,
            },
            AuthorityRef {
                identity_domain: &self.authority.identity_domain,
                owner: &self.authority.owner,
                session: &self.authority.session,
                revision: self.revision,
            },
        )?;
        if !self.key_to_node.contains_key(&context.target) {
            return Err(UiEventAdmissionError::StaleGeneration(context.target));
        }
        let binding = self.event_binding(context.target, event).ok_or(
            UiEventAdmissionError::EventNotBound {
                target: context.target,
                event,
            },
        )?;
        check_permission(&context.permission_context, event, binding)?;
        Ok(AdmittedEvent {
            target: context.target,
            event,
            action: binding.action.clone(),
        })
    }
}

#[derive(Clone, Debug)]
struct DocumentBuilder {
    authority: Authority,
    revision: u64,
    scene: Scene,
    next_node_id: u64,
    key_to_node: BTreeMap<UiKey, NodeId>,
    node_to_key: BTreeMap<NodeId, UiKey>,
    element_kinds: BTreeMap<UiKey, ElementKind>,
    event_bindings: BTreeMap<UiKey, BTreeMap<UiEventKind, EventBinding>>,
    resource_refs: BTreeMap<String, ResourceRef>,
    resource_generations: BTreeMap<String, (ResourceKind, u64)>,
    resources: BTreeMap<String, ResourceState>,
    environment: UiEnvironment,
    mapped_properties: BTreeMap<&'static str, MappedProperty>,
}

impl DocumentBuilder {
    fn new(authority: Authority) -> Self {
        Self {
            authority,
            revision: 0,
            scene: Scene::new(Node::container(NodeId(0))),
            next_node_id: 1,
            key_to_node: BTreeMap::new(),
            node_to_key: BTreeMap::new(),
            element_kinds: BTreeMap::new(),
            event_bindings: BTreeMap::new(),
            resource_refs: BTreeMap::new(),
            resource_generations: BTreeMap::new(),
            resources: BTreeMap::new(),
            environment: UiEnvironment::default(),
            mapped_properties: BTreeMap::new(),
        }
    }

    fn finish(mut self, revision: u64) -> Result<NativeUiDocument, NativeUiFrameError> {
        if self.key_to_node.is_empty() {
            return Err(NativeUiFrameError::MissingRoot);
        }
        // Scene starts at version one. Advance exactly once to the admitted frame revision.
        self.advance_scene_version(1, revision)?;
        Ok(NativeUiDocument {
            authority: self.authority,
            revision,
            scene: self.scene,
            next_node_id: self.next_node_id,
            key_to_node: self.key_to_node,
            node_to_key: self.node_to_key,
            element_kinds: self.element_kinds,
            event_bindings: self.event_bindings,
            resource_refs: self.resource_refs,
            resource_generations: self.resource_generations,
            resources: self.resources,
            environment: self.environment,
            mapped_properties: self.mapped_properties,
        })
    }

    fn apply_successor(mut self, frame: &UiFrame) -> Result<NativeUiDocument, NativeUiFrameError> {
        let expected = self.revision;
        self.advance_scene_version(expected, frame.next_revision)?;
        self.apply_ops(&frame.ops)?;
        Ok(NativeUiDocument {
            authority: self.authority,
            revision: frame.next_revision,
            scene: self.scene,
            next_node_id: self.next_node_id,
            key_to_node: self.key_to_node,
            node_to_key: self.node_to_key,
            element_kinds: self.element_kinds,
            event_bindings: self.event_bindings,
            resource_refs: self.resource_refs,
            resource_generations: self.resource_generations,
            resources: self.resources,
            environment: self.environment,
            mapped_properties: self.mapped_properties,
        })
    }

    fn advance_scene_version(
        &mut self,
        expected: u64,
        next: u64,
    ) -> Result<(), NativeUiFrameError> {
        if next != expected.saturating_add(1) {
            return Err(NativeUiFrameError::Revision {
                expected,
                actual: next,
            });
        }
        let root = self.scene.root.id;
        let style = self.scene.root.style.clone();
        self.scene
            .apply_batch_at(expected, &[ScenePatch::UpdateStyle { id: root, style }])
            .map_err(native_scene_error)
    }

    fn apply_ops(&mut self, ops: &[UiOp]) -> Result<(), NativeUiFrameError> {
        for (index, op) in ops.iter().enumerate() {
            let path = format!("$.ops[{index}]");
            match op {
                UiOp::Insert(op) => self.insert(&path, op)?,
                UiOp::Update(op) => self.update(&path, op)?,
                UiOp::Move(op) => self.move_node(&path, op)?,
                UiOp::Remove(op) => self.remove(&path, op)?,
                UiOp::Resource(op) => self.resource(&path, op)?,
                UiOp::Env(op) => self.update_environment(&path, &op.env)?,
            }
        }
        Ok(())
    }

    fn insert(&mut self, path: &str, op: &InsertOp) -> Result<(), NativeUiFrameError> {
        self.reserve_node_keys(path, &op.node)?;
        let node = self.build_node(path, &op.node)?;
        if op.parent.is_none() {
            if self.key_to_node.len() != count_nodes(&op.node) {
                return Err(NativeUiFrameError::DuplicateRoot);
            }
            if op.index != 0 {
                return Err(issue(
                    path,
                    None,
                    SupportLevel::Required,
                    MappingIssueKind::InvalidState,
                    "a root insertion must use index zero",
                ));
            }
            self.scene = Scene::try_new(node).map_err(native_scene_error)?;
            return Ok(());
        }
        let parent_key = match op.parent {
            Some(parent) => parent,
            None => {
                return Err(issue(
                    path,
                    None,
                    SupportLevel::Required,
                    MappingIssueKind::InvalidState,
                    "root insertion did not produce a scene",
                ))
            }
        };
        let parent = self.key_to_node.get(&parent_key).copied().ok_or_else(|| {
            issue(
                path,
                None,
                SupportLevel::Required,
                MappingIssueKind::InvalidState,
                format!(
                    "parent identity {}:{} is not active",
                    parent_key.semantic_id, parent_key.generation
                ),
            )
        })?;
        let parent_node = find_node_mut(&mut self.scene.root, parent).ok_or_else(|| {
            issue(
                path,
                None,
                SupportLevel::Required,
                MappingIssueKind::InvalidState,
                "mapped parent disappeared",
            )
        })?;
        let index = usize_index(path, op.index)?;
        if index > parent_node.children.len() {
            return Err(issue(
                path,
                None,
                SupportLevel::Required,
                MappingIssueKind::InvalidState,
                "insertion index exceeds the parent child count",
            ));
        }
        parent_node.children.insert(index, node);
        Ok(())
    }

    fn reserve_node_keys(&mut self, path: &str, node: &UiNode) -> Result<(), NativeUiFrameError> {
        if self.key_to_node.contains_key(&node.key) {
            return Err(issue(
                path,
                None,
                SupportLevel::Required,
                MappingIssueKind::InvalidState,
                format!(
                    "identity {}:{} is already active or was not retired",
                    node.key.semantic_id, node.key.generation
                ),
            ));
        }
        let id = NodeId(self.next_node_id);
        self.next_node_id = self.next_node_id.checked_add(1).ok_or_else(|| {
            issue(
                path,
                None,
                SupportLevel::Required,
                MappingIssueKind::InvalidState,
                "backend node identity space is exhausted",
            )
        })?;
        self.key_to_node.insert(node.key, id);
        self.node_to_key.insert(id, node.key);
        self.element_kinds.insert(node.key, node.element.kind);
        for (index, child) in node
            .children
            .as_deref()
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            self.reserve_node_keys(&format!("{path}.node.children[{index}]"), child)?;
        }
        Ok(())
    }

    fn build_node(&mut self, path: &str, node: &UiNode) -> Result<Node, NativeUiFrameError> {
        let key = node.key;
        let id = self.key_to_node[&key];
        if node
            .element
            .attributes
            .as_ref()
            .is_some_and(|attributes| !attributes.is_empty())
        {
            return Err(issue(
                &format!("{path}.node.element.attributes"),
                None,
                SupportLevel::Unsupported,
                MappingIssueKind::InformationLoss,
                "element attributes have no UI IR v0 Native representation",
            ));
        }
        let kind = native_node_kind(&node.element);
        let semantics = self.map_semantics(&format!("{path}.node.semantics"), &node.semantics)?;
        let style = if let Some(typed) = &node.style {
            self.map_style(&format!("{path}.node.style"), typed, node.element.kind)?
        } else {
            default_style(node.element.kind)
        };
        let mut native = Node::new(id, kind)
            .with_style(style)
            .with_semantics(semantics);
        if let Some(events) = &node.events {
            self.event_bindings.insert(
                key,
                events
                    .iter()
                    .cloned()
                    .map(|event| (event.event, event))
                    .collect(),
            );
        }
        for (index, child) in node
            .children
            .as_deref()
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            native
                .children
                .push(self.build_node(&format!("{path}.node.children[{index}]"), child)?);
        }
        Ok(native)
    }

    fn map_semantics(
        &self,
        path: &str,
        source: &UiSemantics,
    ) -> Result<Semantics, NativeUiFrameError> {
        if source.selected {
            return Err(issue(
                &format!("{path}.selected"),
                None,
                SupportLevel::Required,
                MappingIssueKind::InformationLoss,
                "the existing Rust Semantics value has no selected state",
            ));
        }
        if source.role != SemanticRole::Heading && source.level.is_some() {
            return Err(issue(
                &format!("{path}.level"),
                None,
                SupportLevel::Required,
                MappingIssueKind::InformationLoss,
                "a non-heading semantic level has no Native representation",
            ));
        }
        let role = match source.role {
            SemanticRole::Container => Role::Container,
            SemanticRole::Text => Role::Text,
            SemanticRole::Button => Role::Button,
            SemanticRole::Link => Role::Link,
            SemanticRole::TextField => Role::TextField,
            SemanticRole::Checkbox => Role::Checkbox,
            SemanticRole::Radio => Role::Radio,
            SemanticRole::Switch => Role::Switch,
            SemanticRole::Slider => Role::Slider,
            SemanticRole::List => Role::List,
            SemanticRole::ListItem => Role::ListItem,
            SemanticRole::Image => Role::Image,
            SemanticRole::Dialog => Role::Dialog,
            SemanticRole::Menu => Role::Menu,
            SemanticRole::MenuItem => Role::MenuItem,
            SemanticRole::ScrollView => Role::ScrollView,
            SemanticRole::Heading => Role::Heading(
                HeadingLevel::new(source.level.ok_or_else(|| {
                    issue(
                        &format!("{path}.level"),
                        None,
                        SupportLevel::Required,
                        MappingIssueKind::InvalidState,
                        "Native heading semantics require an explicit level",
                    )
                })?)
                .ok_or_else(|| {
                    issue(
                        &format!("{path}.level"),
                        None,
                        SupportLevel::Required,
                        MappingIssueKind::InvalidState,
                        "heading level is not in 1..=6",
                    )
                })?,
            ),
            SemanticRole::Status => Role::Status,
            SemanticRole::Alert => Role::Alert,
            SemanticRole::Form => Role::Form,
            SemanticRole::TextArea
            | SemanticRole::Table
            | SemanticRole::TableRow
            | SemanticRole::TableCell
            | SemanticRole::Popover
            | SemanticRole::Tooltip
            | SemanticRole::Live => {
                return Err(issue(
                    &format!("{path}.role"),
                    None,
                    SupportLevel::Required,
                    MappingIssueKind::InformationLoss,
                    format!(
                        "Native semantic role {:?} is not representable",
                        source.role
                    ),
                ))
            }
        };
        Ok(Semantics {
            role,
            label: source.label.clone(),
            description: source.description.clone(),
            value: source.value.clone(),
            disabled: source.disabled,
            focusable: source.focusable,
            hidden: source.hidden,
        })
    }

    fn map_style(
        &mut self,
        path: &str,
        source: &TypedStyle,
        element: ElementKind,
    ) -> Result<NativeStyle, NativeUiFrameError> {
        let mut style = default_style(element);
        if let Some(value) = &source.box_ {
            if let Some(value) = value.width {
                self.mark("box.width", SupportLevel::Required, true);
                style.width = native_length(&format!("{path}.box.width"), value)?;
            }
            if let Some(value) = value.height {
                self.mark("box.height", SupportLevel::Required, true);
                style.height = native_length(&format!("{path}.box.height"), value)?;
            }
            if let Some(value) = value.min_width {
                self.mark("box.min_width", SupportLevel::Required, true);
                style.min_width = native_length(&format!("{path}.box.min_width"), value)?;
            }
            if let Some(value) = value.max_width {
                self.mark("box.max_width", SupportLevel::Required, true);
                style.max_width = native_length(&format!("{path}.box.max_width"), value)?;
            }
            if let Some(value) = value.min_height {
                self.mark("box.min_height", SupportLevel::Required, true);
                style.min_height = native_length(&format!("{path}.box.min_height"), value)?;
            }
            if let Some(value) = value.max_height {
                self.mark("box.max_height", SupportLevel::Required, true);
                style.max_height = native_length(&format!("{path}.box.max_height"), value)?;
            }
            if let Some(value) = &value.padding {
                self.mark("box.padding", SupportLevel::Required, true);
                style.padding = native_edges(&format!("{path}.box.padding"), "box.padding", value)?;
            }
            if let Some(value) = &value.margin {
                self.mark("box.margin", SupportLevel::Required, true);
                style.margin = native_edges(&format!("{path}.box.margin"), "box.margin", value)?;
            }
        }

        let mut flex_gap = None;
        if let Some(value) = &source.flex {
            if let Some(value) = value.direction {
                self.mark("flex.direction", SupportLevel::Required, true);
                style.direction = match value {
                    FlexDirection::Row => NativeFlexDirection::Row,
                    FlexDirection::Column => NativeFlexDirection::Column,
                };
            }
            if let Some(value) = value.wrap {
                self.mark("flex.wrap", SupportLevel::Required, true);
                if value != FlexWrap::Nowrap {
                    return Err(issue(
                        &format!("{path}.flex.wrap"),
                        Some("flex.wrap"),
                        SupportLevel::Required,
                        MappingIssueKind::InformationLoss,
                        "wrapped flex lines are not represented by the current Native Style",
                    ));
                }
            }
            if let Some(value) = value.grow {
                self.mark("flex.grow", SupportLevel::Required, true);
                style.flex_grow = native_f32(&format!("{path}.flex.grow"), value, true)?;
            }
            if let Some(value) = value.shrink {
                self.mark("flex.shrink", SupportLevel::Required, true);
                native_f32(&format!("{path}.flex.shrink"), value, true)?;
                return Err(issue(
                    &format!("{path}.flex.shrink"),
                    Some("flex.shrink"),
                    SupportLevel::Required,
                    MappingIssueKind::Unsupported,
                    "the current Native Style has no flex shrink field",
                ));
            }
            if let Some(value) = value.align_items {
                self.mark("flex.align_items", SupportLevel::Required, true);
                style.align_items = match value {
                    AlignItems::Start => NativeAlign::Start,
                    AlignItems::Center => NativeAlign::Center,
                    AlignItems::End => NativeAlign::End,
                    AlignItems::Stretch => NativeAlign::Stretch,
                    AlignItems::Baseline => {
                        return Err(issue(
                            &format!("{path}.flex.align_items"),
                            Some("flex.align_items"),
                            SupportLevel::Required,
                            MappingIssueKind::Unsupported,
                            "Native Style has no baseline alignment",
                        ))
                    }
                };
            }
            if let Some(value) = value.justify_content {
                self.mark("flex.justify_content", SupportLevel::Required, true);
                style.justify_content = match value {
                    JustifyContent::Start => NativeJustify::Start,
                    JustifyContent::Center => NativeJustify::Center,
                    JustifyContent::End => NativeJustify::End,
                    JustifyContent::SpaceBetween => NativeJustify::SpaceBetween,
                    JustifyContent::SpaceAround | JustifyContent::SpaceEvenly => {
                        return Err(issue(
                            &format!("{path}.flex.justify_content"),
                            Some("flex.justify_content"),
                            SupportLevel::Required,
                            MappingIssueKind::Unsupported,
                            "Native Style has no space-around or space-evenly distribution",
                        ))
                    }
                };
            }
            if let Some(value) = value.gap {
                self.mark("flex.gap", SupportLevel::Required, true);
                flex_gap = Some(native_f32(&format!("{path}.flex.gap"), value, true)?);
            }
        }

        let mut grid_gap = None;
        if let Some(value) = &source.grid {
            if matches!(element, ElementKind::Container | ElementKind::ScrollView) {
                style.display = NativeDisplay::Grid;
            }
            if let Some(tracks) = &value.columns {
                self.mark("grid.columns", SupportLevel::Candidate, false);
                style.grid_columns = Some(
                    tracks
                        .iter()
                        .map(|track| native_track(&format!("{path}.grid.columns"), track))
                        .collect::<Result<_, _>>()?,
                );
            }
            if let Some(tracks) = &value.rows {
                self.mark("grid.rows", SupportLevel::Candidate, false);
                style.grid_rows = Some(
                    tracks
                        .iter()
                        .map(|track| native_track(&format!("{path}.grid.rows"), track))
                        .collect::<Result<_, _>>()?,
                );
            }
            if let Some(value) = &value.column {
                self.mark("grid.column", SupportLevel::Candidate, false);
                style.grid_column = native_grid_line(&format!("{path}.grid.column"), value)?;
            }
            if let Some(value) = &value.row {
                self.mark("grid.row", SupportLevel::Candidate, false);
                style.grid_row = native_grid_line(&format!("{path}.grid.row"), value)?;
            }
            if let Some(value) = value.gap {
                self.mark("grid.gap", SupportLevel::Candidate, false);
                grid_gap = Some(native_f32(&format!("{path}.grid.gap"), value, true)?);
            }
        }
        if let (Some(flex), Some(grid)) = (flex_gap, grid_gap) {
            if flex != grid {
                return Err(issue(
                    path,
                    Some("grid.gap"),
                    SupportLevel::Candidate,
                    MappingIssueKind::InformationLoss,
                    "Native Style has one gap field but flex.gap and grid.gap differ",
                ));
            }
        }
        style.gap = flex_gap.or(grid_gap).unwrap_or(0.0);

        if let Some(value) = &source.position {
            if let Some(value) = value.kind {
                self.mark("position.kind", SupportLevel::Candidate, false);
                style.position = match value {
                    PositionKind::Static => NativePosition::Static,
                    PositionKind::Relative => NativePosition::Relative,
                    PositionKind::Absolute => NativePosition::Absolute,
                    PositionKind::Fixed => NativePosition::Fixed,
                    PositionKind::Sticky => NativePosition::Sticky,
                };
            }
            if let Some(value) = &value.inset {
                self.mark("position.inset", SupportLevel::Candidate, false);
                let top = native_length(&format!("{path}.position.inset.top"), value.top)?;
                let right = native_length(&format!("{path}.position.inset.right"), value.right)?;
                let bottom = native_length(&format!("{path}.position.inset.bottom"), value.bottom)?;
                let left = native_length(&format!("{path}.position.inset.left"), value.left)?;
                style.inset = NativeInset::new(top, right, bottom, left);
            }
            if value.z_index.is_some() {
                self.mark("position.z_index", SupportLevel::Candidate, false);
            }
            style.z_index = match value.z_index {
                Some(ZIndex::Auto(_)) | None => None,
                Some(ZIndex::Value(value)) => Some(value),
            };
            if let Some(value) = value.order {
                self.mark("position.order", SupportLevel::Required, false);
                style.order = value;
            }
        }

        if let Some(value) = &source.paint {
            if let Some(value) = value.color {
                self.mark("paint.color", SupportLevel::Required, true);
                style.color = native_color(value);
            }
            if let Some(value) = value.background {
                self.mark("paint.background", SupportLevel::Required, true);
                style.background = native_color(value);
            }
            if let Some(value) = &value.border {
                self.mark("paint.border", SupportLevel::Required, true);
                style.border_width = value.width;
                style.border_color = native_color(value.color);
            }
            if let Some(value) = value.radii {
                self.mark("paint.radii", SupportLevel::Required, true);
                style.corner_radius = CornerRadii {
                    top_left: value.top_left,
                    top_right: value.top_right,
                    bottom_right: value.bottom_right,
                    bottom_left: value.bottom_left,
                };
            }
            if let Some(value) = value.opacity {
                self.mark("paint.opacity", SupportLevel::Required, true);
                style.opacity = native_f32(&format!("{path}.paint.opacity"), value, true)?;
            }
        }

        if let Some(value) = &source.typography {
            if let Some(families) = &value.family {
                self.mark("typography.family", SupportLevel::Required, true);
                if families.len() != 1 {
                    return Err(issue(
                        &format!("{path}.typography.family"),
                        Some("typography.family"),
                        SupportLevel::Required,
                        MappingIssueKind::InformationLoss,
                        "per-node family fallback lists exceed the current Native family field",
                    ));
                }
                style.font_family = Some(families[0].clone());
            }
            if let Some(value) = value.size {
                self.mark("typography.size", SupportLevel::Required, true);
                style.font_size = native_f32(&format!("{path}.typography.size"), value, false)?;
            }
            if let Some(value) = value.weight {
                self.mark("typography.weight", SupportLevel::Required, true);
                style.font_weight = value;
            }
            if let Some(value) = value.style {
                self.mark("typography.style", SupportLevel::Required, true);
                style.font_style = match value {
                    FontStyle::Normal => NativeFontStyle::Normal,
                    FontStyle::Italic => NativeFontStyle::Italic,
                };
            }
            if let Some(value) = value.line_height {
                self.mark("typography.line_height", SupportLevel::Required, true);
                let length =
                    match native_length(&format!("{path}.typography.line_height"), value)? {
                        NativeLength::Auto => return Err(issue(
                            &format!("{path}.typography.line_height"),
                            Some("typography.line_height"),
                            SupportLevel::Required,
                            MappingIssueKind::Unsupported,
                            "auto line-height is not expressible in the Native line-height field",
                        )),
                        NativeLength::Px(value) => value,
                        NativeLength::Percent(value) => style.font_size * value / 100.0,
                    };
                if !length.is_finite() || length <= 0.0 {
                    return Err(issue(
                        &format!("{path}.typography.line_height"),
                        Some("typography.line_height"),
                        SupportLevel::Required,
                        MappingIssueKind::Unsupported,
                        "line-height must resolve to a positive finite pixel value",
                    ));
                }
                style.line_height = Some(NativeLineHeight::Px(length));
            }
            if let Some(value) = value.align {
                self.mark("typography.align", SupportLevel::Required, true);
                if value != TextAlign::Start {
                    return Err(issue(
                        &format!("{path}.typography.align"),
                        Some("typography.align"),
                        SupportLevel::Required,
                        MappingIssueKind::Unsupported,
                        "the current Native Style has no horizontal text-align field",
                    ));
                }
            }
            if let Some(value) = value.vertical_align {
                self.mark("typography.vertical_align", SupportLevel::Required, true);
                style.text_align_vertical = match value {
                    VerticalAlign::Top => Some(NativeVerticalAlign::Top),
                    VerticalAlign::Center => Some(NativeVerticalAlign::Center),
                    VerticalAlign::Bottom => Some(NativeVerticalAlign::Bottom),
                    VerticalAlign::Baseline => {
                        return Err(issue(
                            &format!("{path}.typography.vertical_align"),
                            Some("typography.vertical_align"),
                            SupportLevel::Required,
                            MappingIssueKind::Unsupported,
                            "Native vertical text alignment has no baseline mode",
                        ))
                    }
                };
            }
            if let Some(value) = value.direction {
                self.mark("typography.direction", SupportLevel::Required, true);
                if value != TextDirection::Auto {
                    return Err(issue(
                        &format!("{path}.typography.direction"),
                        Some("typography.direction"),
                        SupportLevel::Required,
                        MappingIssueKind::Unsupported,
                        "an explicit bidi direction is not carried by the current Native Style",
                    ));
                }
            }
        }

        if let Some(value) = &source.image {
            self.mark("image.resource", SupportLevel::Candidate, false);
            if !matches!(value.resource.kind, ResourceKind::Image | ResourceKind::Svg) {
                return Err(issue(
                    &format!("{path}.image.resource.kind"),
                    Some("image.resource"),
                    SupportLevel::Candidate,
                    MappingIssueKind::Unsupported,
                    "the Native image projection accepts image or SVG resources",
                ));
            }
            self.observe_declared_resource(
                &format!("{path}.image.resource"),
                value.resource.clone(),
            )?;
            let mut image = NativeImageStyle::new(value.resource.id.clone());
            if let Some(value) = value.fit {
                self.mark("image.fit", SupportLevel::Candidate, false);
                image.fit = match value {
                    ImageFit::Fill => NativeObjectFit::Fill,
                    ImageFit::Contain => NativeObjectFit::Contain,
                    ImageFit::Cover => NativeObjectFit::Cover,
                    ImageFit::None => NativeObjectFit::None,
                    ImageFit::ScaleDown => NativeObjectFit::ScaleDown,
                };
            }
            if let Some(value) = value.align_x {
                self.mark("image.align_x", SupportLevel::Candidate, false);
                image.align_x = native_f32(&format!("{path}.image.align_x"), value, true)?;
            }
            if let Some(value) = value.align_y {
                self.mark("image.align_y", SupportLevel::Candidate, false);
                image.align_y = native_f32(&format!("{path}.image.align_y"), value, true)?;
            }
            if let Some(value) = value.tint {
                self.mark("image.tint", SupportLevel::Candidate, false);
                image.tint = Some(native_color(value));
            }
            style.image = Some(image);
        }

        if let Some(value) = &source.overlay {
            let mut overlay = OverlaySpec::default();
            if let Some(value) = value.layer {
                self.mark("overlay.layer", SupportLevel::Candidate, false);
                overlay.layer = value;
            }
            if let Some(value) = &value.anchor {
                self.mark("overlay.anchor", SupportLevel::Candidate, false);
                overlay.anchor = match value {
                    OverlayAnchor::Viewport(_) => NativeOverlayAnchor::Viewport,
                    OverlayAnchor::Node(key) => NativeOverlayAnchor::Node(
                        self.key_to_node.get(key).copied().ok_or_else(|| {
                            issue(
                                &format!("{path}.overlay.anchor"),
                                Some("overlay.anchor"),
                                SupportLevel::Candidate,
                                MappingIssueKind::InvalidState,
                                format!(
                                    "overlay anchor {}:{} is not active",
                                    key.semantic_id, key.generation
                                ),
                            )
                        })?,
                    ),
                    OverlayAnchor::Point(value) => {
                        let x = native_f32(&format!("{path}.overlay.anchor.x"), value.x, false)?;
                        let y = native_f32(&format!("{path}.overlay.anchor.y"), value.y, false)?;
                        if x.abs() > crate::overlay::MAX_OVERLAY_POINT
                            || y.abs() > crate::overlay::MAX_OVERLAY_POINT
                        {
                            return Err(issue(
                                &format!("{path}.overlay.anchor"),
                                Some("overlay.anchor"),
                                SupportLevel::Candidate,
                                MappingIssueKind::Unsupported,
                                "overlay point exceeds the Native admission bound",
                            ));
                        }
                        NativeOverlayAnchor::Point { x, y }
                    }
                };
            }
            if let Some(value) = value.side {
                self.mark("overlay.side", SupportLevel::Candidate, false);
                overlay.side = match value {
                    OverlaySide::Top => NativeOverlaySide::Top,
                    OverlaySide::Bottom => NativeOverlaySide::Bottom,
                    OverlaySide::Left => NativeOverlaySide::Left,
                    OverlaySide::Right => NativeOverlaySide::Right,
                };
            }
            if let Some(value) = value.align {
                self.mark("overlay.align", SupportLevel::Candidate, false);
                overlay.align = match value {
                    OverlayAlign::Start => NativeOverlayAlign::Start,
                    OverlayAlign::Center => NativeOverlayAlign::Center,
                    OverlayAlign::End => NativeOverlayAlign::End,
                };
            }
            if let Some(value) = value.offset {
                self.mark("overlay.offset", SupportLevel::Candidate, false);
                overlay.offset = native_f32(&format!("{path}.overlay.offset"), value, false)?;
            }
            if let Some(value) = value.cross_offset {
                self.mark("overlay.cross_offset", SupportLevel::Candidate, false);
                overlay.cross_offset =
                    native_f32(&format!("{path}.overlay.cross_offset"), value, false)?;
            }
            if let Some(value) = value.margin {
                self.mark("overlay.margin", SupportLevel::Candidate, false);
                overlay.margin = native_f32(&format!("{path}.overlay.margin"), value, true)?;
            }
            if let Some(value) = value.modal {
                self.mark("overlay.modal", SupportLevel::Candidate, false);
                overlay.modal = value;
                if value {
                    overlay.focus.trap = true;
                    overlay.focus.auto = true;
                }
            }
            if let Some(value) = value.scrim {
                self.mark("overlay.scrim", SupportLevel::Candidate, false);
                overlay.scrim = Some(native_color(value));
            }
            if [overlay.offset, overlay.cross_offset, overlay.margin]
                .into_iter()
                .any(|value| value.abs() > crate::overlay::MAX_OVERLAY_EXTENT)
            {
                return Err(issue(
                    path,
                    Some("overlay.offset"),
                    SupportLevel::Candidate,
                    MappingIssueKind::Unsupported,
                    "overlay extent exceeds the Native admission bound",
                ));
            }
            style.overlay = Some(overlay);
        }

        Ok(style)
    }

    fn mark(&mut self, property: &'static str, support: SupportLevel, host_accepted: bool) {
        self.mapped_properties.insert(
            property,
            MappedProperty {
                support,
                host_accepted,
            },
        );
    }

    fn observe_declared_resource(
        &mut self,
        path: &str,
        resource: ResourceRef,
    ) -> Result<(), NativeUiFrameError> {
        if let Some(previous) = self.resource_refs.get(&resource.id) {
            if previous != &resource {
                return Err(issue(
                    path,
                    Some("image.resource"),
                    SupportLevel::Candidate,
                    MappingIssueKind::InvalidState,
                    "one resource id was declared with different generation or kind",
                ));
            }
        } else {
            self.resource_refs.insert(resource.id.clone(), resource);
        }
        Ok(())
    }

    fn update(&mut self, path: &str, op: &UpdateOp) -> Result<(), NativeUiFrameError> {
        let node_id = self.active_node(path, op.key)?;
        let old_element_kind = self.element_kinds.get(&op.key).copied();
        let element_kind = op
            .fields
            .element
            .as_ref()
            .map(|element| element.kind)
            .or(old_element_kind)
            .unwrap_or(ElementKind::Container);
        if let Some(element) = &op.fields.element {
            if element
                .attributes
                .as_ref()
                .is_some_and(|attributes| !attributes.is_empty())
            {
                return Err(issue(
                    &format!("{path}.fields.element.attributes"),
                    None,
                    SupportLevel::Unsupported,
                    MappingIssueKind::InformationLoss,
                    "element attributes have no UI IR v0 Native representation",
                ));
            }
        }
        let semantics = if let Some(value) = &op.fields.semantics {
            Some(self.map_semantics(&format!("{path}.fields.semantics"), value)?)
        } else {
            None
        };
        let style = if let Some(value) = &op.fields.style {
            Some(self.map_style(&format!("{path}.fields.style"), value, element_kind)?)
        } else {
            None
        };
        let node = find_node_mut(&mut self.scene.root, node_id)
            .ok_or_else(|| stale_mapping(path, op.key))?;
        if let Some(element) = &op.fields.element {
            node.kind = native_node_kind(element);
            apply_element_defaults(&mut node.style, element.kind);
            self.element_kinds.insert(op.key, element.kind);
        }
        if let Some(semantics) = semantics {
            node.semantics = semantics;
        }
        if let Some(style) = style {
            node.style = style;
        }
        if let Some(events) = &op.fields.events {
            self.event_bindings.insert(
                op.key,
                events
                    .iter()
                    .cloned()
                    .map(|event| (event.event, event))
                    .collect(),
            );
        }
        Ok(())
    }

    fn move_node(&mut self, path: &str, op: &MoveOp) -> Result<(), NativeUiFrameError> {
        let key_id = self.active_node(path, op.key)?;
        let parent_id = self.active_node(path, op.parent)?;
        if self.scene.root.id == key_id {
            return Err(issue(
                path,
                None,
                SupportLevel::Required,
                MappingIssueKind::InvalidState,
                "the scene root cannot be moved",
            ));
        }
        let moved =
            remove_node(&mut self.scene.root, key_id).ok_or_else(|| stale_mapping(path, op.key))?;
        if find_node(&moved, parent_id).is_some() {
            return Err(issue(
                path,
                None,
                SupportLevel::Required,
                MappingIssueKind::InvalidState,
                "a node cannot be moved into its own subtree",
            ));
        }
        let parent = find_node_mut(&mut self.scene.root, parent_id)
            .ok_or_else(|| stale_mapping(path, op.parent))?;
        let index = usize_index(path, op.index)?;
        if index > parent.children.len() {
            return Err(issue(
                path,
                None,
                SupportLevel::Required,
                MappingIssueKind::InvalidState,
                "move index exceeds the parent child count",
            ));
        }
        parent.children.insert(index, moved);
        Ok(())
    }

    fn remove(&mut self, path: &str, op: &RemoveOp) -> Result<(), NativeUiFrameError> {
        let node_id = self.active_node(path, op.key)?;
        if self.scene.root.id == node_id {
            return Err(issue(
                path,
                None,
                SupportLevel::Required,
                MappingIssueKind::InvalidState,
                "the scene root cannot be removed",
            ));
        }
        let removed = remove_node(&mut self.scene.root, node_id)
            .ok_or_else(|| stale_mapping(path, op.key))?;
        let mut retired = Vec::new();
        collect_keys(&removed, &self.node_to_key, &mut retired);
        for key in retired {
            if let Some(node) = self.key_to_node.remove(&key) {
                self.node_to_key.remove(&node);
            }
            self.element_kinds.remove(&key);
            self.event_bindings.remove(&key);
        }
        Ok(())
    }

    fn resource(&mut self, path: &str, op: &ResourceOp) -> Result<(), NativeUiFrameError> {
        let resource = &op.resource;
        if self
            .resource_refs
            .get(&resource.id)
            .is_some_and(|declared| declared != resource)
        {
            return Err(issue(
                path,
                None,
                SupportLevel::Candidate,
                MappingIssueKind::InvalidState,
                "resource operation conflicts with the generation or kind used by a style",
            ));
        }
        let highest = self.resource_generations.get(&resource.id).copied();
        if let Some((kind, generation)) = highest {
            if kind != resource.kind {
                return Err(issue(
                    &format!("{path}.resource.kind"),
                    None,
                    SupportLevel::Candidate,
                    MappingIssueKind::InvalidState,
                    "resource kind changed without a new resource id",
                ));
            }
            if resource.generation < generation {
                return Err(issue(
                    &format!("{path}.resource.generation"),
                    None,
                    SupportLevel::Candidate,
                    MappingIssueKind::InvalidState,
                    "resource generation is stale",
                ));
            }
            if resource.generation > generation && op.admission != ResourceAdmission::Prepare {
                return Err(issue(
                    &format!("{path}.resource.generation"),
                    None,
                    SupportLevel::Candidate,
                    MappingIssueKind::InvalidState,
                    "a newer resource generation must be prepared first",
                ));
            }
        }
        match op.admission {
            ResourceAdmission::Prepare => {
                if self
                    .resources
                    .get(&resource.id)
                    .is_some_and(|state| state.resource == *resource)
                {
                    if self.resources[&resource.id].life == ResourceLife::Prepared {
                        return Ok(());
                    }
                    return Err(issue(
                        path,
                        None,
                        SupportLevel::Candidate,
                        MappingIssueKind::InvalidState,
                        "a committed resource generation cannot be prepared again",
                    ));
                }
                if highest.is_some_and(|(_, generation)| generation == resource.generation) {
                    return Err(issue(
                        path,
                        None,
                        SupportLevel::Candidate,
                        MappingIssueKind::InvalidState,
                        "a released resource generation cannot be reused",
                    ));
                }
                self.resource_generations
                    .insert(resource.id.clone(), (resource.kind, resource.generation));
                self.resources.insert(
                    resource.id.clone(),
                    ResourceState {
                        resource: resource.clone(),
                        life: ResourceLife::Prepared,
                    },
                );
                self.resource_refs
                    .insert(resource.id.clone(), resource.clone());
            }
            ResourceAdmission::Commit => {
                let state = self.resources.get_mut(&resource.id).ok_or_else(|| {
                    issue(
                        path,
                        None,
                        SupportLevel::Candidate,
                        MappingIssueKind::InvalidState,
                        "resource was not prepared",
                    )
                })?;
                if state.resource != *resource || state.life != ResourceLife::Prepared {
                    return Err(issue(
                        path,
                        None,
                        SupportLevel::Candidate,
                        MappingIssueKind::InvalidState,
                        "resource commit does not match the prepared generation",
                    ));
                }
                state.life = ResourceLife::Committed;
            }
            ResourceAdmission::Release => {
                let state = self.resources.get(&resource.id).ok_or_else(|| {
                    issue(
                        path,
                        None,
                        SupportLevel::Candidate,
                        MappingIssueKind::InvalidState,
                        "resource was not live",
                    )
                })?;
                if state.resource != *resource {
                    return Err(issue(
                        path,
                        None,
                        SupportLevel::Candidate,
                        MappingIssueKind::InvalidState,
                        "resource release does not match the live generation",
                    ));
                }
                self.resources.remove(&resource.id);
            }
        }
        Ok(())
    }

    fn update_environment(
        &mut self,
        path: &str,
        env: &UiEnvironment,
    ) -> Result<(), NativeUiFrameError> {
        if env.appearance.is_some()
            || env.density.is_some()
            || env.text_scale.is_some()
            || env.reduce_motion.is_some()
            || env.high_contrast.is_some()
        {
            // Environment semantics are retained by the document. Actual theme resolution stays
            // Framework-owned; reduce-motion has an explicit Native host setter.
            if let Some(appearance) = env.appearance {
                self.environment.appearance = Some(appearance);
            }
            if let Some(density) = env.density {
                self.environment.density = Some(density);
            }
            if let Some(text_scale) = env.text_scale {
                self.environment.text_scale = Some(text_scale);
            }
            if let Some(reduce_motion) = env.reduce_motion {
                self.environment.reduce_motion = Some(reduce_motion);
            }
            if let Some(high_contrast) = env.high_contrast {
                self.environment.high_contrast = Some(high_contrast);
            }
            Ok(())
        } else {
            Err(issue(
                path,
                None,
                SupportLevel::Required,
                MappingIssueKind::InvalidState,
                "environment operation changes no field",
            ))
        }
    }

    fn active_node(&self, path: &str, key: UiKey) -> Result<NodeId, NativeUiFrameError> {
        self.key_to_node.get(&key).copied().ok_or_else(|| {
            issue(
                path,
                None,
                SupportLevel::Required,
                MappingIssueKind::InvalidState,
                format!(
                    "identity {}:{} is not active",
                    key.semantic_id, key.generation
                ),
            )
        })
    }
}

fn native_node_kind(element: &UiElement) -> NodeKind {
    match element.kind {
        ElementKind::Image => NodeKind::Rect,
        ElementKind::Text => NodeKind::Text(element.content.clone().unwrap_or_default()),
        _ => match &element.content {
            Some(content)
                if matches!(
                    element.kind,
                    ElementKind::Button
                        | ElementKind::Link
                        | ElementKind::TextField
                        | ElementKind::TextArea
                        | ElementKind::Checkbox
                        | ElementKind::Radio
                        | ElementKind::Switch
                        | ElementKind::Slider
                        | ElementKind::ListItem
                        | ElementKind::TableCell
                        | ElementKind::MenuItem
                        | ElementKind::Tooltip
                ) =>
            {
                NodeKind::Text(content.clone())
            }
            _ => NodeKind::Container,
        },
    }
}

fn default_style(element: ElementKind) -> NativeStyle {
    let mut style = NativeStyle::default();
    apply_element_defaults(&mut style, element);
    style
}

fn apply_element_defaults(style: &mut NativeStyle, element: ElementKind) {
    match element {
        ElementKind::Table => style.display = NativeDisplay::Table,
        ElementKind::TableRow => style.display = NativeDisplay::TableRow,
        ElementKind::TableCell => style.display = NativeDisplay::TableCell,
        ElementKind::ScrollView => {
            style.overflow = crate::style::Overflow::Scroll;
        }
        _ => {}
    }
}

fn native_length(path: &str, value: Length) -> Result<NativeLength, NativeUiFrameError> {
    match value {
        Length::Auto(_) => Ok(NativeLength::Auto),
        Length::Value(value) => match value.unit {
            LengthUnit::Px => Ok(NativeLength::Px(native_f32(
                &format!("{path}.value"),
                value.value,
                true,
            )?)),
            LengthUnit::Percent => Ok(NativeLength::Percent(native_f32(
                &format!("{path}.value"),
                value.value,
                true,
            )?)),
        },
    }
}

fn native_edges(
    path: &str,
    property: &'static str,
    value: &Edges,
) -> Result<NativeEdges, NativeUiFrameError> {
    let edge = |name: &str, value: Length| match native_length(&format!("{path}.{name}"), value)? {
        NativeLength::Px(value) => Ok(value),
        NativeLength::Percent(_) | NativeLength::Auto => Err(issue(
            &format!("{path}.{name}"),
            Some(property),
            SupportLevel::Required,
            MappingIssueKind::InformationLoss,
            "Native edge spacing stores resolved pixels, not percentages or auto",
        )),
    };
    Ok(NativeEdges::new(
        edge("top", value.top)?,
        edge("right", value.right)?,
        edge("bottom", value.bottom)?,
        edge("left", value.left)?,
    ))
}

fn native_track(path: &str, value: &Track) -> Result<GridTrack, NativeUiFrameError> {
    match value {
        Track::Auto(_) => Ok(GridTrack::Auto),
        Track::Value(value) => {
            let number = native_f32(&format!("{path}.value"), value.value, false)?;
            Ok(match value.unit {
                TrackUnit::Px => GridTrack::Px(number),
                TrackUnit::Percent => GridTrack::Percent(number),
                TrackUnit::Fr => GridTrack::Fr(number),
            })
        }
    }
}

fn native_grid_line(path: &str, value: &GridPlacement) -> Result<GridLine, NativeUiFrameError> {
    match (value.start, value.span) {
        (GridStart::Auto(_), 1) => Ok(GridLine::Auto),
        (GridStart::Auto(_), span) => Ok(GridLine::Span(span)),
        (GridStart::Line(line), 1) => Ok(GridLine::Line(line)),
        (GridStart::Line(_), _) => Err(issue(
            path,
            Some("grid.column"),
            SupportLevel::Candidate,
            MappingIssueKind::InformationLoss,
            "Native GridLine cannot combine an explicit start line with a multi-track span",
        )),
    }
}

fn native_f32(path: &str, value: f64, nonnegative: bool) -> Result<f32, NativeUiFrameError> {
    let value = value as f32;
    if !value.is_finite() || (nonnegative && value < 0.0) {
        return Err(issue(
            path,
            None,
            SupportLevel::Required,
            MappingIssueKind::InvalidState,
            "number cannot be represented as a finite Native f32 value",
        ));
    }
    Ok(value)
}

fn native_color(value: UiColor) -> NativeColor {
    NativeColor::new(value.r, value.g, value.b, value.a)
}

fn usize_index(path: &str, value: u64) -> Result<usize, NativeUiFrameError> {
    usize::try_from(value).map_err(|_| {
        issue(
            path,
            None,
            SupportLevel::Required,
            MappingIssueKind::InvalidState,
            "index cannot be represented on this host",
        )
    })
}

fn count_nodes(node: &UiNode) -> usize {
    1 + node
        .children
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(count_nodes)
        .sum::<usize>()
}

fn find_node(node: &Node, id: NodeId) -> Option<&Node> {
    if node.id == id {
        return Some(node);
    }
    node.children.iter().find_map(|child| find_node(child, id))
}

fn find_node_mut(node: &mut Node, id: NodeId) -> Option<&mut Node> {
    if node.id == id {
        return Some(node);
    }
    node.children
        .iter_mut()
        .find_map(|child| find_node_mut(child, id))
}

fn remove_node(root: &mut Node, id: NodeId) -> Option<Node> {
    let Some(index) = root.children.iter().position(|child| child.id == id) else {
        return root
            .children
            .iter_mut()
            .find_map(|child| remove_node(child, id));
    };
    Some(root.children.remove(index))
}

fn collect_keys(node: &Node, reverse: &BTreeMap<NodeId, UiKey>, output: &mut Vec<UiKey>) {
    if let Some(key) = reverse.get(&node.id) {
        output.push(*key);
    }
    for child in &node.children {
        collect_keys(child, reverse, output);
    }
}

fn stale_mapping(path: &str, key: UiKey) -> NativeUiFrameError {
    issue(
        path,
        None,
        SupportLevel::Required,
        MappingIssueKind::InvalidState,
        format!(
            "Native mapping lost identity {}:{}",
            key.semantic_id, key.generation
        ),
    )
}

fn native_scene_error(error: SceneError) -> NativeUiFrameError {
    NativeUiFrameError::Scene(format!("{error:?}"))
}

fn issue(
    path: &str,
    property: Option<&'static str>,
    support: SupportLevel,
    kind: MappingIssueKind,
    detail: impl Into<String>,
) -> NativeUiFrameError {
    NativeUiFrameError::Mapping(MappingIssue {
        path: path.into(),
        property,
        support,
        kind,
        detail: detail.into(),
    })
}
