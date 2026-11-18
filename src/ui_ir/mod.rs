//! Strict Rust reader and validator for Jasper Typed UI IR v0.1.
//!
//! Jasper Language/UI owns the canonical Schema. This module is a backend-side compatibility
//! profile: it rejects malformed or ambiguous input before any Native mutation. JSON is used only
//! as the v0.1 interchange representation; CSS text is not an input path here.
//!
//! Jasper Language/UI 拥有规范 Schema；本模块只是后端兼容读取器，在任何 Native 修改前拒绝
//! 非法或有歧义的输入。JSON 仅是 v0.1 交换表示，CSS 文本不进入这条路径。

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeSet;
use std::fmt;

mod events;
mod identity;
mod native;

pub use events::{
    AdmittedEvent, UiEventAdmissionError, UiEventContext, UiPermission, UiPermissionContext,
};
pub use identity::{IdentityError, UiIdentityLedger};
pub use native::{
    MappedProperty, MappingIssue, MappingIssueKind, NativeSceneUpdate, NativeUiDocument,
    NativeActivationSource, NativeUiFrameError, SupportLevel,
};

/// The only IR version this profile reads.
pub const IR_VERSION: &str = "0.1.0";

/// Public node identity within one identity domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiKey {
    pub semantic_id: u64,
    pub generation: u64,
}

impl UiKey {
    pub const fn new(semantic_id: u64, generation: u64) -> Self {
        Self {
            semantic_id,
            generation,
        }
    }

    /// Next generation without wrap; `None` at `u64::MAX`.
    pub const fn next_generation(self) -> Option<Self> {
        match self.generation.checked_add(1) {
            Some(generation) => Some(Self::new(self.semantic_id, generation)),
            None => None,
        }
    }
}

/// One atomic Framework-produced update frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiFrame {
    pub ir_version: String,
    pub identity_domain: String,
    pub owner: String,
    pub session: String,
    pub expected_revision: u64,
    pub next_revision: u64,
    pub ops: Vec<UiOp>,
}

impl UiFrame {
    /// Parse strictly. Unknown fields at all structural levels are rejected.
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, UiIrError> {
        let frame: Self = serde_json::from_slice(bytes).map_err(|error| UiIrError::Json {
            path: "$".into(),
            detail: error.to_string(),
        })?;
        frame.validate()?;
        Ok(frame)
    }

    pub fn to_json_vec(&self) -> Result<Vec<u8>, UiIrError> {
        self.validate()?;
        serde_json::to_vec(self).map_err(|error| UiIrError::Json {
            path: "$".into(),
            detail: error.to_string(),
        })
    }

    pub fn expected_next_revision(&self) -> Option<u64> {
        self.expected_revision.checked_add(1)
    }

    pub fn validate(&self) -> Result<(), UiIrError> {
        if self.ir_version != IR_VERSION {
            return invalid(
                "$.ir_version",
                format!("unsupported IR version {}", self.ir_version),
            );
        }
        nonempty("$.identity_domain", &self.identity_domain)?;
        nonempty("$.owner", &self.owner)?;
        nonempty("$.session", &self.session)?;
        if self.ops.is_empty() {
            return invalid("$.ops", "a frame must contain at least one operation");
        }
        let Some(next) = self.expected_next_revision() else {
            return invalid(
                "$.expected_revision",
                "revision has no successor; refusing to wrap u64",
            );
        };
        if self.next_revision != next {
            return invalid(
                "$.next_revision",
                format!(
                    "must equal expected_revision + 1 ({next}), got {}",
                    self.next_revision
                ),
            );
        }

        let mut inserted = BTreeSet::new();
        for (index, op) in self.ops.iter().enumerate() {
            op.validate(&format!("$.ops[{index}]"), &mut inserted)?;
        }
        Ok(())
    }

    pub fn node(&self, key: UiKey) -> Option<&UiNode> {
        self.ops.iter().find_map(|op| match op {
            UiOp::Insert(op) => op.node.find(key),
            _ => None,
        })
    }
}

/// Strongly typed operation variants. Unknown fields are rejected by each payload struct.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UiOp {
    Insert(InsertOp),
    Update(UpdateOp),
    Move(MoveOp),
    Remove(RemoveOp),
    Resource(ResourceOp),
    Env(EnvOp),
}

impl UiOp {
    pub fn insert(parent: Option<UiKey>, index: u64, node: UiNode) -> Self {
        Self::Insert(InsertOp {
            op: InsertTag::Tag,
            parent,
            index,
            node,
        })
    }

    pub fn update(key: UiKey, fields: UiFields) -> Self {
        Self::Update(UpdateOp {
            op: UpdateTag::Tag,
            key,
            fields,
        })
    }

    pub fn move_node(key: UiKey, parent: UiKey, index: u64) -> Self {
        Self::Move(MoveOp {
            op: MoveTag::Tag,
            key,
            parent,
            index,
        })
    }

    pub fn remove(key: UiKey) -> Self {
        Self::Remove(RemoveOp {
            op: RemoveTag::Tag,
            key,
        })
    }

    pub fn resource(resource: ResourceRef, admission: ResourceAdmission) -> Self {
        Self::Resource(ResourceOp {
            op: ResourceTag::Tag,
            resource,
            admission,
        })
    }

    pub fn environment(env: UiEnvironment) -> Self {
        Self::Env(EnvOp {
            op: EnvTag::Tag,
            env,
        })
    }

    fn validate(&self, path: &str, inserted: &mut BTreeSet<UiKey>) -> Result<(), UiIrError> {
        match self {
            Self::Insert(op) => {
                if let Some(parent) = op.parent {
                    key_valid(path, parent)?;
                }
                op.node.validate(&format!("{path}.node"), inserted)
            }
            Self::Update(op) => {
                key_valid(path, op.key)?;
                op.fields.validate(&format!("{path}.fields"))
            }
            Self::Move(op) => {
                key_valid(path, op.key)?;
                key_valid(path, op.parent)?;
                Ok(())
            }
            Self::Remove(op) => key_valid(path, op.key),
            Self::Resource(op) => {
                op.resource.validate(&format!("{path}.resource"))?;
                Ok(())
            }
            Self::Env(op) => op.env.validate(&format!("{path}.env")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InsertOp {
    #[serde(rename = "op")]
    op: InsertTag,
    pub parent: Option<UiKey>,
    pub index: u64,
    pub node: UiNode,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateOp {
    #[serde(rename = "op")]
    op: UpdateTag,
    pub key: UiKey,
    pub fields: UiFields,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveOp {
    #[serde(rename = "op")]
    op: MoveTag,
    pub key: UiKey,
    pub parent: UiKey,
    pub index: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoveOp {
    #[serde(rename = "op")]
    op: RemoveTag,
    pub key: UiKey,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceOp {
    #[serde(rename = "op")]
    op: ResourceTag,
    pub resource: ResourceRef,
    pub admission: ResourceAdmission,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvOp {
    #[serde(rename = "op")]
    op: EnvTag,
    pub env: UiEnvironment,
}

macro_rules! operation_tag {
    ($name:ident, $wire:literal) => {
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        enum $name {
            #[serde(rename = $wire)]
            Tag,
        }
    };
}

operation_tag!(InsertTag, "insert");
operation_tag!(UpdateTag, "update");
operation_tag!(MoveTag, "move");
operation_tag!(RemoveTag, "remove");
operation_tag!(ResourceTag, "resource");
operation_tag!(EnvTag, "env");

/// One node and its source-order children.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiNode {
    pub key: UiKey,
    pub element: UiElement,
    pub semantics: UiSemantics,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<TypedStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<EventBinding>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<UiNode>>,
}

impl UiNode {
    pub fn find(&self, key: UiKey) -> Option<&Self> {
        if self.key == key {
            return Some(self);
        }
        self.children
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find_map(|child| child.find(key))
    }

    fn validate(&self, path: &str, inserted: &mut BTreeSet<UiKey>) -> Result<(), UiIrError> {
        key_valid(path, self.key)?;
        if !inserted.insert(self.key) {
            return invalid(
                &format!("{path}.key"),
                format!(
                    "duplicate inserted identity {}:{}",
                    self.key.semantic_id, self.key.generation
                ),
            );
        }
        self.element.validate(&format!("{path}.element"))?;
        self.semantics.validate(&format!("{path}.semantics"))?;
        if let Some(style) = &self.style {
            style.validate(&format!("{path}.style"))?;
        }
        let mut events = BTreeSet::new();
        for (index, event) in self
            .events
            .as_deref()
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            let event_path = format!("{path}.events[{index}]");
            nonempty(&format!("{event_path}.action"), &event.action)?;
            if !events.insert(event.event) {
                return invalid(
                    &format!("{event_path}.event"),
                    "a node may bind each event kind at most once",
                );
            }
        }
        for (index, child) in self
            .children
            .as_deref()
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            child.validate(&format!("{path}.children[{index}]"), inserted)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiFields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<UiElement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics: Option<UiSemantics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<TypedStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<EventBinding>>,
}

impl UiFields {
    fn validate(&self, path: &str) -> Result<(), UiIrError> {
        let changed = self.element.is_some()
            || self.semantics.is_some()
            || self.style.is_some()
            || self.events.is_some();
        if !changed {
            return invalid(path, "an update must change at least one field");
        }
        if let Some(element) = &self.element {
            element.validate(&format!("{path}.element"))?;
        }
        if let Some(semantics) = &self.semantics {
            semantics.validate(&format!("{path}.semantics"))?;
        }
        if let Some(style) = &self.style {
            style.validate(&format!("{path}.style"))?;
        }
        if let Some(events) = &self.events {
            let mut kinds = BTreeSet::new();
            for (index, event) in events.iter().enumerate() {
                nonempty(&format!("{path}.events[{index}].action"), &event.action)?;
                if !kinds.insert(event.event) {
                    return invalid(
                        &format!("{path}.events[{index}].event"),
                        "a node may bind each event kind at most once",
                    );
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiElement {
    pub kind: ElementKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<std::collections::BTreeMap<String, AttributeValue>>,
}

impl UiElement {
    fn validate(&self, path: &str) -> Result<(), UiIrError> {
        if let Some(attributes) = &self.attributes {
            for key in attributes.keys() {
                nonempty(&format!("{path}.attributes"), key)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ElementKind {
    Container,
    Text,
    Button,
    Link,
    TextField,
    TextArea,
    Checkbox,
    Radio,
    Switch,
    Slider,
    List,
    ListItem,
    Table,
    TableRow,
    TableCell,
    Image,
    Dialog,
    Popover,
    Menu,
    MenuItem,
    Tooltip,
    ScrollView,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttributeValue {
    String(String),
    Number(f64),
    Boolean(bool),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiSemantics {
    pub role: SemanticRole,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub focusable: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub selected: bool,
    #[serde(default)]
    pub level: Option<u8>,
}

impl UiSemantics {
    fn validate(&self, path: &str) -> Result<(), UiIrError> {
        if self.role == SemanticRole::Heading {
            if let Some(level) = self.level {
                if !(1..=6).contains(&level) {
                    return invalid(
                        &format!("{path}.level"),
                        format!("heading level must be 1..=6, got {level}"),
                    );
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticRole {
    Container,
    Text,
    Button,
    Link,
    TextField,
    TextArea,
    Checkbox,
    Radio,
    Switch,
    Slider,
    List,
    ListItem,
    Table,
    TableRow,
    TableCell,
    Image,
    Dialog,
    Popover,
    Menu,
    MenuItem,
    Tooltip,
    ScrollView,
    Heading,
    Status,
    Live,
    Alert,
    Form,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventBinding {
    pub event: UiEventKind,
    pub action: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UiEventKind {
    Click,
    Activate,
    PointerDown,
    PointerUp,
    Hover,
    Focus,
    Key,
    TextCommit,
    ImeCommit,
    ImeCancel,
    Scroll,
    Dismiss,
    AccessibilityAction,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedStyle {
    #[serde(default, rename = "box", skip_serializing_if = "Option::is_none")]
    pub box_: Option<BoxStyle>,
    #[serde(default, rename = "flex", skip_serializing_if = "Option::is_none")]
    pub flex: Option<FlexStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid: Option<GridStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<PositionStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paint: Option<PaintStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typography: Option<TypographyStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay: Option<OverlayStyle>,
}

impl TypedStyle {
    #[allow(non_snake_case)]
    pub fn box_(box_: BoxStyle) -> Self {
        Self {
            box_: Some(box_),
            ..Self::default()
        }
    }

    fn validate(&self, path: &str) -> Result<(), UiIrError> {
        if let Some(value) = &self.box_ {
            value.validate(&format!("{path}.box"))?;
        }
        if let Some(value) = &self.flex {
            value.validate(&format!("{path}.flex"))?;
        }
        if let Some(value) = &self.grid {
            value.validate(&format!("{path}.grid"))?;
        }
        if let Some(value) = &self.position {
            value.validate(&format!("{path}.position"))?;
        }
        if let Some(value) = &self.paint {
            value.validate(&format!("{path}.paint"))?;
        }
        if let Some(value) = &self.typography {
            value.validate(&format!("{path}.typography"))?;
        }
        if let Some(value) = &self.image {
            value.validate(&format!("{path}.image"))?;
        }
        if let Some(value) = &self.overlay {
            value.validate(&format!("{path}.overlay"))?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoxStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<Length>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<Length>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_width: Option<Length>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_width: Option<Length>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_height: Option<Length>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_height: Option<Length>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding: Option<Edges>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin: Option<Edges>,
}

impl BoxStyle {
    fn validate(&self, path: &str) -> Result<(), UiIrError> {
        for (name, value) in [
            ("width", self.width),
            ("height", self.height),
            ("min_width", self.min_width),
            ("max_width", self.max_width),
            ("min_height", self.min_height),
            ("max_height", self.max_height),
        ] {
            if let Some(value) = value {
                value.validate(&format!("{path}.{name}"))?;
            }
        }
        if let Some(value) = &self.padding {
            value.validate(&format!("{path}.padding"))?;
        }
        if let Some(value) = &self.margin {
            value.validate(&format!("{path}.margin"))?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlexStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<FlexDirection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<FlexWrap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grow: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shrink: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align_items: Option<AlignItems>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub justify_content: Option<JustifyContent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<f64>,
}

impl FlexStyle {
    fn validate(&self, path: &str) -> Result<(), UiIrError> {
        for (name, value) in [
            ("grow", self.grow),
            ("shrink", self.shrink),
            ("gap", self.gap),
        ] {
            if let Some(value) = value {
                nonnegative(&format!("{path}.{name}"), value)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GridStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<Track>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<Vec<Track>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<GridPlacement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row: Option<GridPlacement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<f64>,
}

impl GridStyle {
    fn validate(&self, path: &str) -> Result<(), UiIrError> {
        for (name, tracks) in [("columns", &self.columns), ("rows", &self.rows)] {
            if let Some(tracks) = tracks {
                if tracks.len() > 256 {
                    return invalid(
                        &format!("{path}.{name}"),
                        "at most 256 explicit tracks are allowed",
                    );
                }
                for (index, track) in tracks.iter().enumerate() {
                    track.validate(&format!("{path}.{name}[{index}]"))?;
                }
            }
        }
        if let Some(value) = &self.column {
            value.validate(&format!("{path}.column"))?;
        }
        if let Some(value) = &self.row {
            value.validate(&format!("{path}.row"))?;
        }
        if let Some(value) = self.gap {
            nonnegative(&format!("{path}.gap"), value)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PositionStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<PositionKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inset: Option<Edges>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z_index: Option<ZIndex>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<i32>,
}

impl PositionStyle {
    fn validate(&self, path: &str) -> Result<(), UiIrError> {
        if let Some(value) = &self.inset {
            value.validate(&format!("{path}.inset"))?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaintStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<UiColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<UiColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<BorderStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radii: Option<Radii>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
}

impl PaintStyle {
    fn validate(&self, path: &str) -> Result<(), UiIrError> {
        if let Some(value) = &self.border {
            value.validate(&format!("{path}.border"))?;
        }
        if let Some(value) = &self.radii {
            value.validate(&format!("{path}.radii"))?;
        }
        if let Some(value) = self.opacity {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return invalid(&format!("{path}.opacity"), "must be finite and in 0..=1");
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypographyStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<FontStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_height: Option<Length>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<TextAlign>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertical_align: Option<VerticalAlign>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<TextDirection>,
}

impl TypographyStyle {
    fn validate(&self, path: &str) -> Result<(), UiIrError> {
        if let Some(families) = &self.family {
            if families.is_empty() {
                return invalid(
                    &format!("{path}.family"),
                    "must contain at least one family",
                );
            }
            for (index, family) in families.iter().enumerate() {
                nonempty(&format!("{path}.family[{index}]"), family)?;
            }
        }
        if let Some(value) = self.size {
            if !value.is_finite() || value <= 0.0 {
                return invalid(&format!("{path}.size"), "must be finite and positive");
            }
        }
        if let Some(value) = self.weight {
            if !(1..=1000).contains(&value) {
                return invalid(&format!("{path}.weight"), "must be in 1..=1000");
            }
        }
        if let Some(value) = self.line_height {
            value.validate(&format!("{path}.line_height"))?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageStyle {
    pub resource: ResourceRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<ImageFit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align_x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align_y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<UiColor>,
}

impl ImageStyle {
    fn validate(&self, path: &str) -> Result<(), UiIrError> {
        self.resource.validate(&format!("{path}.resource"))?;
        for (name, value) in [("align_x", self.align_x), ("align_y", self.align_y)] {
            if let Some(value) = value {
                if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                    return invalid(&format!("{path}.{name}"), "must be finite and in 0..=1");
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverlayStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<OverlayAnchor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<OverlaySide>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<OverlayAlign>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cross_offset: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modal: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scrim: Option<UiColor>,
}

impl OverlayStyle {
    fn validate(&self, path: &str) -> Result<(), UiIrError> {
        if let Some(layer) = self.layer {
            if !(-1_000_000..=1_000_000).contains(&layer) {
                return invalid(&format!("{path}.layer"), "must be in -1000000..=1000000");
            }
        }
        if let Some(anchor) = &self.anchor {
            anchor.validate(&format!("{path}.anchor"))?;
        }
        for (name, value) in [("offset", self.offset), ("cross_offset", self.cross_offset)] {
            if let Some(value) = value {
                finite(&format!("{path}.{name}"), value)?;
            }
        }
        if let Some(value) = self.margin {
            nonnegative(&format!("{path}.margin"), value)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Length {
    Auto(AutoLength),
    Value(LengthValue),
}

impl Length {
    pub const AUTO: Self = Self::Auto(AutoLength { auto: true });

    pub const fn px(value: f64) -> Self {
        Self::Value(LengthValue {
            unit: LengthUnit::Px,
            value,
        })
    }

    pub const fn percent(value: f64) -> Self {
        Self::Value(LengthValue {
            unit: LengthUnit::Percent,
            value,
        })
    }

    fn validate(&self, path: &str) -> Result<(), UiIrError> {
        match self {
            Self::Auto(auto) if !auto.auto => invalid(
                &format!("{path}.auto"),
                "only the literal true is a valid auto length",
            ),
            Self::Auto(_) => Ok(()),
            Self::Value(value) => value.validate(path),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoLength {
    pub auto: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LengthValue {
    pub unit: LengthUnit,
    pub value: f64,
}

impl LengthValue {
    fn validate(self, path: &str) -> Result<(), UiIrError> {
        nonnegative(&format!("{path}.value"), self.value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LengthUnit {
    Px,
    Percent,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edges {
    pub top: Length,
    pub right: Length,
    pub bottom: Length,
    pub left: Length,
}

impl Edges {
    fn validate(&self, path: &str) -> Result<(), UiIrError> {
        self.top.validate(&format!("{path}.top"))?;
        self.right.validate(&format!("{path}.right"))?;
        self.bottom.validate(&format!("{path}.bottom"))?;
        self.left.validate(&format!("{path}.left"))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Track {
    Auto(AutoTrack),
    Value(TrackValue),
}

impl Track {
    fn validate(&self, path: &str) -> Result<(), UiIrError> {
        match self {
            Self::Auto(auto) if !auto.auto => invalid(
                &format!("{path}.auto"),
                "only the literal true is a valid auto track",
            ),
            Self::Auto(_) => Ok(()),
            Self::Value(value) => {
                if !value.value.is_finite() || value.value <= 0.0 {
                    return invalid(
                        &format!("{path}.value"),
                        "must be finite and greater than zero",
                    );
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoTrack {
    pub auto: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrackValue {
    pub unit: TrackUnit,
    pub value: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrackUnit {
    Px,
    Percent,
    Fr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GridPlacement {
    pub start: GridStart,
    pub span: u32,
}

impl GridPlacement {
    fn validate(&self, path: &str) -> Result<(), UiIrError> {
        if self.span == 0 {
            return invalid(&format!("{path}.span"), "must be at least one");
        }
        if matches!(self.start, GridStart::Line(0)) {
            return invalid(
                &format!("{path}.start"),
                "grid line zero is not addressable",
            );
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GridStart {
    Auto(GridAuto),
    Line(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GridAuto {
    #[serde(rename = "auto")]
    Auto,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BorderStyle {
    pub width: f32,
    pub color: UiColor,
}

impl BorderStyle {
    fn validate(&self, path: &str) -> Result<(), UiIrError> {
        if !self.width.is_finite() || self.width < 0.0 {
            return invalid(&format!("{path}.width"), "must be finite and nonnegative");
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Radii {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl Radii {
    fn validate(&self, path: &str) -> Result<(), UiIrError> {
        for (name, value) in [
            ("top_left", self.top_left),
            ("top_right", self.top_right),
            ("bottom_right", self.bottom_right),
            ("bottom_left", self.bottom_left),
        ] {
            if !value.is_finite() || value < 0.0 {
                return invalid(&format!("{path}.{name}"), "must be finite and nonnegative");
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FlexDirection {
    Row,
    Column,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FlexWrap {
    Nowrap,
    Wrap,
    WrapReverse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AlignItems {
    Start,
    Center,
    End,
    Stretch,
    Baseline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JustifyContent {
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PositionKind {
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ZIndex {
    Auto(ZAuto),
    Value(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ZAuto {
    #[serde(rename = "auto")]
    Auto,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FontStyle {
    Normal,
    Italic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TextAlign {
    Start,
    Center,
    End,
    Justify,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VerticalAlign {
    Top,
    Center,
    Baseline,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TextDirection {
    Auto,
    Ltr,
    Rtl,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImageFit {
    Fill,
    Contain,
    Cover,
    None,
    ScaleDown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OverlayAnchor {
    Viewport(ViewportAnchor),
    Node(UiKey),
    Point(PointAnchor),
}

impl OverlayAnchor {
    fn validate(&self, path: &str) -> Result<(), UiIrError> {
        match self {
            Self::Viewport(_) => Ok(()),
            Self::Node(key) => key_valid(path, *key),
            Self::Point(point) => {
                finite(&format!("{path}.x"), point.x)?;
                finite(&format!("{path}.y"), point.y)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewportAnchor {
    #[serde(rename = "viewport")]
    Viewport,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PointAnchor {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverlaySide {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverlayAlign {
    Start,
    Center,
    End,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceRef {
    pub id: String,
    pub generation: u64,
    pub kind: ResourceKind,
}

impl ResourceRef {
    fn validate(&self, path: &str) -> Result<(), UiIrError> {
        if self.id.is_empty() || self.id.len() > 128 {
            return invalid(&format!("{path}.id"), "must contain 1..=128 bytes");
        }
        if self.generation == 0 {
            return invalid(&format!("{path}.generation"), "must be at least one");
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceKind {
    Image,
    Svg,
    Font,
    Icon,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceAdmission {
    Prepare,
    Commit,
    Release,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiEnvironment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance: Option<Appearance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density: Option<Density>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_scale: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reduce_motion: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high_contrast: Option<bool>,
}

impl UiEnvironment {
    fn validate(&self, path: &str) -> Result<(), UiIrError> {
        if let Some(value) = self.text_scale {
            if !value.is_finite() || value <= 0.0 {
                return invalid(&format!("{path}.text_scale"), "must be finite and positive");
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Appearance {
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Density {
    Compact,
    Standard,
    Touch,
}

/// Exact straight-alpha RGBA color. Wire form is `#RRGGBB` or `#RRGGBBAA`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UiColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl UiColor {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

impl fmt::Display for UiColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.a == 255 {
            write!(f, "#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            write!(
                f,
                "#{:02X}{:02X}{:02X}{:02X}",
                self.r, self.g, self.b, self.a
            )
        }
    }
}

impl Serialize for UiColor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for UiColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let text = String::deserialize(deserializer)?;
        parse_color(&text).ok_or_else(|| serde::de::Error::custom("invalid #RRGGBB[AA] color"))
    }
}

fn parse_color(text: &str) -> Option<UiColor> {
    if !matches!(text.len(), 7 | 9) || !text.starts_with('#') {
        return None;
    }
    let byte = |range: std::ops::Range<usize>| u8::from_str_radix(&text[range], 16).ok();
    Some(UiColor {
        r: byte(1..3)?,
        g: byte(3..5)?,
        b: byte(5..7)?,
        a: if text.len() == 9 { byte(7..9)? } else { 255 },
    })
}

/// Structured read/validation failure. `path` is a JSON Pointer-like field path, not a backend id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiIrError {
    Json { path: String, detail: String },
    Invalid { path: String, detail: String },
}

impl fmt::Display for UiIrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json { path, detail } => write!(f, "{path}: invalid UI IR JSON: {detail}"),
            Self::Invalid { path, detail } => write!(f, "{path}: invalid UI IR: {detail}"),
        }
    }
}

impl std::error::Error for UiIrError {}

fn invalid<T>(path: &str, detail: impl Into<String>) -> Result<T, UiIrError> {
    Err(UiIrError::Invalid {
        path: path.into(),
        detail: detail.into(),
    })
}

fn nonempty(path: &str, value: &str) -> Result<(), UiIrError> {
    if value.is_empty() {
        invalid(path, "must not be empty")
    } else {
        Ok(())
    }
}

fn finite(path: &str, value: f64) -> Result<(), UiIrError> {
    if value.is_finite() {
        Ok(())
    } else {
        invalid(path, "must be finite")
    }
}

fn nonnegative(path: &str, value: f64) -> Result<(), UiIrError> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        invalid(path, "must be finite and nonnegative")
    }
}

fn key_valid(path: &str, key: UiKey) -> Result<(), UiIrError> {
    if key.generation == 0 {
        invalid(
            &format!("{path}.generation"),
            "generation must be at least one",
        )
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_wire_is_exact() {
        let color: UiColor = serde_json::from_str("\"#0A84FFFF\"").unwrap();
        assert_eq!(color, UiColor::new(10, 132, 255, 255));
        assert_eq!(serde_json::to_string(&color).unwrap(), "\"#0A84FF\"");
        assert!(serde_json::from_str::<UiColor>("\"#fff\"").is_err());
        assert!(serde_json::from_str::<UiColor>("\"#GG0000\"").is_err());
    }

    #[test]
    fn overflow_never_wraps() {
        assert_eq!(UiKey::new(7, u64::MAX).next_generation(), None);
    }
}
