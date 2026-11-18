//! Lossless projection of Native accessibility data onto public `UiKey` identities.
use crate::access_tree::{AccessActions, AccessibilityTree};
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypedAccessibilityError {
    VersionMismatch { tree: u64, document: u64 },
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
    let map = |node| {
        document
            .key_for_node(node)
            .ok_or(TypedAccessibilityError::UnmappedNode)
    };
    let mut nodes = Vec::with_capacity(tree.nodes.len());
    let mut seen = std::collections::BTreeSet::new();
    for node in &tree.nodes {
        let key = map(node.node)?;
        if !seen.insert(key) {
            return Err(TypedAccessibilityError::DuplicateKey(key));
        }
        nodes.push(TypedAccessibilityNode {
            key,
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
                .map(|child| map(*child))
                .collect::<Result<_, _>>()?,
        });
    }
    Ok(TypedAccessibilityTree {
        scene_version: tree.scene_version,
        nodes,
        root: tree.root().map(map).transpose()?,
        focused: tree.focused.map(map).transpose()?,
    })
}
