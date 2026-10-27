// Generated from production markers by generate-types.py.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u64);

#[derive(Clone, Debug)]
struct Capture {
    target: Option<NodeId>,
    path: Vec<NodeId>,
}

