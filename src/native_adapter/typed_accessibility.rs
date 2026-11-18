//! Lossless projection of Native accessibility data onto public `UiKey` identities.
use crate::access_tree::{AccessActions, AccessibilityTree, AccessibilityUpdate};
use crate::geometry::Rect;
use crate::semantics::Role;
use crate::ui_ir::{NativeUiDocument, UiKey};
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub struct TypedAccessibilityNode {
    pub key: UiKey,
    pub role: Role,
    pub label: String,
    pub description: String,
    pub value: String,
    pub text_selection: Option<(usize, usize)>,
    pub disabled: bool,
    pub selected: bool,
    pub modal: bool,
    pub bounds: Option<Rect>,
    pub focused: bool,
    pub actions: AccessActions,
    pub children: Vec<UiKey>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypedAccessibilityTree {
    pub scene_version: u64,
    pub nodes: Vec<TypedAccessibilityNode>,
    pub root: Option<UiKey>,
    pub focused: Option<UiKey>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypedAccessibilityUpdate {
    pub from_version: u64,
    pub scene_version: u64,
    pub nodes: Vec<TypedAccessibilityNode>,
    pub removed: Vec<UiKey>,
    pub root: Option<UiKey>,
    pub focused: Option<UiKey>,
    pub focus_changed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypedAccessibilityError {
    VersionMismatch {
        tree: u64,
        document: u64,
    },
    UpdateVersionMismatch {
        from: u64,
        from_document: u64,
        scene: u64,
        scene_document: u64,
    },
    UnmappedNode,
    DuplicateKey(UiKey),
}

impl fmt::Display for TypedAccessibilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VersionMismatch { tree, document } => write!(
                f,
                "accessibility tree version {tree} does not match document revision {document}"
            ),
            Self::UpdateVersionMismatch {
                from,
                from_document,
                scene,
                scene_document,
            } => write!(
                f,
                "accessibility update versions {from}->{scene} do not match documents {from_document}->{scene_document}"
            ),
            Self::UnmappedNode => write!(f, "accessibility tree contains an unmapped Native node"),
            Self::DuplicateKey(key) => write!(
                f,
                "accessibility tree maps more than one Native node to {}:{}",
                key.semantic_id, key.generation
            ),
        }
    }
}

impl std::error::Error for TypedAccessibilityError {}

pub fn typed_accessibility_tree(
    tree: &AccessibilityTree,
    document: &NativeUiDocument,
) -> Result<TypedAccessibilityTree, TypedAccessibilityError> {
    if tree.scene_version != document.revision() {
        return Err(TypedAccessibilityError::VersionMismatch {
            tree: tree.scene_version,
            document: document.revision(),
        });
    }
    let mut nodes = Vec::with_capacity(tree.nodes.len());
    let mut seen = std::collections::BTreeSet::new();
    for node in &tree.nodes {
        let key = document
            .key_for_node(node.node)
            .ok_or(TypedAccessibilityError::UnmappedNode)?;
        if !seen.insert(key) {
            return Err(TypedAccessibilityError::DuplicateKey(key));
        }
        nodes.push(map_node(node, document)?);
    }
    Ok(TypedAccessibilityTree {
        scene_version: tree.scene_version,
        nodes,
        root: tree
            .root()
            .map(|node| {
                document
                    .key_for_node(node)
                    .ok_or(TypedAccessibilityError::UnmappedNode)
            })
            .transpose()?,
        focused: tree
            .focused
            .map(|node| {
                document
                    .key_for_node(node)
                    .ok_or(TypedAccessibilityError::UnmappedNode)
            })
            .transpose()?,
    })
}

pub fn typed_accessibility_update(
    update: &AccessibilityUpdate,
    previous: &NativeUiDocument,
    next: &NativeUiDocument,
) -> Result<TypedAccessibilityUpdate, TypedAccessibilityError> {
    if update.from_version != previous.revision() || update.scene_version != next.revision() {
        return Err(TypedAccessibilityError::UpdateVersionMismatch {
            from: update.from_version,
            from_document: previous.revision(),
            scene: update.scene_version,
            scene_document: next.revision(),
        });
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut nodes = Vec::with_capacity(update.nodes.len());
    for node in &update.nodes {
        let mapped = map_node(node, next)?;
        if !seen.insert(mapped.key) {
            return Err(TypedAccessibilityError::DuplicateKey(mapped.key));
        }
        nodes.push(mapped);
    }
    let removed = update
        .removed
        .iter()
        .map(|node| {
            previous
                .key_for_node(*node)
                .ok_or(TypedAccessibilityError::UnmappedNode)
        })
        .collect::<Result<_, _>>()?;
    let root = update
        .root
        .map(|node| {
            next.key_for_node(node)
                .ok_or(TypedAccessibilityError::UnmappedNode)
        })
        .transpose()?;
    let focused = update
        .focused
        .map(|node| {
            next.key_for_node(node)
                .ok_or(TypedAccessibilityError::UnmappedNode)
        })
        .transpose()?;
    Ok(TypedAccessibilityUpdate {
        from_version: update.from_version,
        scene_version: update.scene_version,
        nodes,
        removed,
        root,
        focused,
        focus_changed: update.focus_changed,
    })
}

fn map_node(
    node: &crate::access_tree::AccessibilityNode,
    document: &NativeUiDocument,
) -> Result<TypedAccessibilityNode, TypedAccessibilityError> {
    Ok(TypedAccessibilityNode {
        key: document
            .key_for_node(node.node)
            .ok_or(TypedAccessibilityError::UnmappedNode)?,
        role: node.role,
        label: node.label.clone(),
        description: node.description.clone(),
        value: node.value.clone(),
        text_selection: node.text_selection,
        disabled: node.disabled,
        selected: node.selected,
        modal: node.modal,
        bounds: node.bounds,
        focused: node.focused,
        actions: node.actions,
        children: node
            .children
            .iter()
            .map(|child| {
                document
                    .key_for_node(*child)
                    .ok_or(TypedAccessibilityError::UnmappedNode)
            })
            .collect::<Result<_, _>>()?,
    })
}
