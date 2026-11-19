use vstd::prelude::*;
extern crate alloc;
verus! {

pub struct Canvas(u64);

pub struct Style(u64);
pub struct Semantics(u64);
pub struct Point(u64);
broadcast use vstd::std_specs::vec::axiom_vec_index_decreases;



pub open spec fn node_holds(n:Node,id:NodeId)->bool decreases n, {
    n.id==id || exists|i:int| #![trigger n.children@[i]] 0<=i<n.children@.len() && node_holds(n.children@[i],id)
}
pub open spec fn node_holds_value(n:Node,id:NodeId,t:Node)->bool decreases n, {
    if n.id==id { t==n }
    else { exists|i:int| #![trigger n.children@[i]] 0<=i<n.children@.len() && node_holds_value(n.children@[i],id,t) }
}
proof fn lemma_child_holds_value(n:Node,p:int,id:NodeId,t:Node)
    requires n.id!=id, 0<=p<n.children@.len(), node_holds_value(n.children@[p],id,t),
    ensures node_holds_value(n,id,t),
{
    assert(node_holds_value(n,id,t)) by {
        assert(node_holds_value(n.children@[p],id,t));
    }
}
// f is o with the first preorder match of id, namely t, replaced by ft; nothing else differs.
// f 是把 o 中前序首个匹配 id 的节点（即 t）换成 ft 的结果，其余皆同。
pub open spec fn node_relocated(o:Node,f:Node,id:NodeId,t:Node,ft:Node)->bool decreases o, {
    if o.id==id { t==o && f==ft }
    else {
        o.id==f.id && o.kind==f.kind && o.style==f.style && o.semantics==f.semantics && o.scroll==f.scroll
        && o.children@.len()==f.children@.len()
        && exists|p:int| #![trigger o.children@[p]] 0<=p<o.children@.len()
            && (forall|j:int| #![trigger o.children@[j]] 0<=j<p ==> !node_holds(o.children@[j],id))
            && (forall|j:int| #![trigger o.children@[j]] 0<=j<o.children@.len() && j!=p ==> o.children@[j]==f.children@[j])
            && node_relocated(o.children@[p],f.children@[p],id,t,ft)
    }
}

pub open spec fn node_below(node:Node,id:NodeId)->bool decreases node, {
    exists|i:int| #![trigger node.children@[i]] 0<=i<node.children@.len() && (node.children@[i].id==id || node_below(node.children@[i],id))
}
pub open spec fn node_fields_same(a:Node,b:Node)->bool {
    a.id==b.id && a.kind==b.kind && a.style==b.style && a.semantics==b.semantics && a.scroll==b.scroll
}
pub open spec fn node_same(a:Node,b:Node)->bool decreases a, {
    node_fields_same(a,b) && a.children@.len()==b.children@.len()
        && forall|i:int| #![trigger a.children@[i]] 0<=i<a.children@.len() ==> node_same(a.children@[i],b.children@[i])
}
pub proof fn node_same_refl(a:Node) ensures node_same(a,a), decreases a, {
    assert forall|i:int| #![trigger a.children@[i]] 0<=i<a.children@.len() implies node_same(a.children@[i],a.children@[i]) by {
        node_same_refl(a.children@[i]);
    };
}
pub open spec fn node_removed(o:Node,n:Node,id:NodeId)->bool decreases o, {
    node_fields_same(o,n) && (
        (exists|p:int| #![trigger o.children@[p]] 0<=p<o.children@.len() && o.children@[p].id==id
            && (forall|j:int| #![trigger o.children@[j]] 0<=j<p ==> o.children@[j].id!=id)
            && n.children@=~=o.children@.remove(p))
        || ((forall|j:int| #![trigger o.children@[j]] 0<=j<o.children@.len() ==> o.children@[j].id!=id)
            && n.children@.len()==o.children@.len()
            && exists|p:int| #![trigger o.children@[p]] 0<=p<o.children@.len()
                && (forall|j:int| #![trigger o.children@[j]] 0<=j<p ==> !node_below(o.children@[j],id))
                && (forall|j:int| #![trigger o.children@[j]] 0<=j<o.children@.len() && j!=p ==> node_same(o.children@[j],n.children@[j]))
                && node_removed(o.children@[p],n.children@[p],id)))
}


proof fn lemma_child_holds(n:Node,p:int,id:NodeId)
    requires n.id!=id, 0<=p<n.children@.len(), node_holds(n.children@[p],id),
    ensures node_holds(n,id),
{
    assert(node_holds(n,id)) by {
        assert(node_holds(n.children@[p],id));
    }
}
proof fn lemma_node_below_holds(n:Node,id:NodeId)
    requires node_below(n,id),
    ensures node_holds(n,id),
    decreases n,
{
    if n.id==id {
        assert(node_holds(n,id));
    } else {
        let p=choose|p:int| #![trigger n.children@[p]] 0<=p<n.children@.len()
            && (n.children@[p].id==id || node_below(n.children@[p],id));
        if n.children@[p].id==id {
            assert(node_holds(n.children@[p],id));
            lemma_child_holds(n,p,id);
        } else {
            lemma_node_below_holds(n.children@[p],id);
            lemma_child_holds(n,p,id);
        }
    }
}
proof fn lemma_holds_below(n:Node,id:NodeId)
    requires n.id!=id, node_holds(n,id),
    ensures node_below(n,id),
    decreases n,
{
    let p=choose|p:int| #![trigger n.children@[p]] 0<=p<n.children@.len()
        && node_holds(n.children@[p],id);
    if n.children@[p].id!=id { lemma_holds_below(n.children@[p],id); }
}
// Functional extraction result: `taken` is the exact subtree removed. Unlike the existential
// node_holds_value relation, this also identifies which duplicate wins.
pub open spec fn node_removed_value(o:Node,n:Node,id:NodeId,taken:Node)->bool decreases o, {
    node_fields_same(o,n) && (
        (exists|p:int| #![trigger o.children@[p]]
            0<=p<o.children@.len() && o.children@[p].id==id
            && (forall|j:int| #![trigger o.children@[j]] 0<=j<p ==> o.children@[j].id!=id)
            && n.children@=~=o.children@.remove(p)
            && taken==o.children@[p])
        || ((forall|j:int| #![trigger o.children@[j]]
                0<=j<o.children@.len() ==> o.children@[j].id!=id)
            && n.children@.len()==o.children@.len()
            && exists|p:int| #![trigger o.children@[p]]
                0<=p<o.children@.len()
                && (forall|j:int| #![trigger o.children@[j]] 0<=j<p ==> !node_below(o.children@[j],id))
                && (forall|j:int| #![trigger o.children@[j]]
                    0<=j<o.children@.len() && j!=p ==> node_same(o.children@[j],n.children@[j]))
                && node_removed_value(o.children@[p],n.children@[p],id,taken)))
}


pub open spec fn move_succeeded(o:Node,n:Node,id:NodeId,parent:NodeId,index:usize)->bool {
    exists|moved:Node,removed:Node,target:Node,inserted:Node|
        #![trigger node_removed_value(o,removed,id,moved), node_relocated(removed,n,parent,target,inserted)]
        node_removed(o,removed,id)
        && node_removed_value(o,removed,id,moved)
        && node_relocated(removed,n,parent,target,inserted)
        && node_fields_same(target,inserted)
        && inserted.children@==target.children@.insert(
            if index<target.children@.len() {index as int} else {target.children@.len() as int},moved)
}
pub open spec fn move_applied(
    o:Node,n:Node,id:NodeId,parent:NodeId,index:usize,
    result:Result<(),SceneError>)->bool {
    match result {
        Ok(_) => move_succeeded(o,n,id,parent,index),
        Err(SceneError::CannotMoveRoot) => o.id==id && n==o,
        Err(SceneError::NodeNotFound(missing)) =>
            (missing==id && !node_holds(o,id) && n==o)
            || (missing==parent && !node_holds(o,parent) && n==o)
            || (missing==parent && exists|moved:Node,removed:Node|
                #![trigger node_removed_value(o,removed,id,moved)]
                node_removed_value(o,removed,id,moved) && n==removed),
        Err(SceneError::MoveIntoDescendant(moved)) => moved==id && n==o,
        Err(_) => false,
    }
}

pub open spec fn patch_ok()->Result<(),SceneError> { Ok(()) }
pub open spec fn patch_err(e:SceneError)->Result<(),SceneError> { Err(e) }
// The identity is absent: this error, and the tree is the old tree.
// 身份不存在：报此错误，树仍是旧树。
pub open spec fn patch_missed(o:Node,n:Node,id:NodeId,r:Result<(),SceneError>)->bool {
    !node_holds(o,id) && n==o && r==patch_err(SceneError::NodeNotFound(id))
}
pub open spec fn patch_updated(o:Node,n:Node,id:NodeId,r:Result<(),SceneError>,written:spec_fn(Node,Node)->bool)->bool {
    patch_missed(o,n,id,r) || (node_holds(o,id) && r==patch_ok()
        && exists|t:Node,f:Node| #![trigger node_relocated(o,n,id,t,f)] node_relocated(o,n,id,t,f) && written(t,f))
}
pub open spec fn patch_kind_set(o:Node,n:Node,id:NodeId,r:Result<(),SceneError>,admits:spec_fn(NodeKind)->bool,kind:NodeKind,refusal:SceneError)->bool {
    patch_missed(o,n,id,r) || (node_holds(o,id)
        && exists|t:Node,f:Node| #![trigger node_relocated(o,n,id,t,f)] node_relocated(o,n,id,t,f)
            && if admits(t.kind) { r==patch_ok() && f==(Node{kind:kind,..t}) }
               else { r==patch_err(refusal) && f==t })
}
pub open spec fn patch_applied(o:Node,n:Node,patch:ScenePatch,r:Result<(),SceneError>)->bool {
    match patch {
        ScenePatch::Insert{parent,index,node} => patch_updated(o,n,parent,r,|t:Node,f:Node| node_fields_same(t,f)
            && f.children@==t.children@.insert(if index<t.children@.len() {index as int} else {t.children@.len() as int},node)),
        ScenePatch::UpdateSemantics{id,semantics} => patch_updated(o,n,id,r,|t:Node,f:Node| f==(Node{semantics:semantics,..t})),
        ScenePatch::UpdateStyle{id,style} => patch_updated(o,n,id,r,|t:Node,f:Node| f==(Node{style:style,..t})),
        ScenePatch::SetScroll{id,offset} => patch_updated(o,n,id,r,|t:Node,f:Node| f==(Node{scroll:offset,..t})),
        ScenePatch::UpdateText{id,text} => patch_kind_set(o,n,id,r,|k:NodeKind| k is Text,NodeKind::Text(text),SceneError::NotText(id)),
        ScenePatch::SetCanvas{id,canvas} => patch_kind_set(o,n,id,r,|k:NodeKind| k is Canvas,NodeKind::Canvas(canvas),SceneError::NotCanvas(id)),
        ScenePatch::Move{id,parent,index} => move_applied(o,n,id,parent,index,r),
        ScenePatch::Remove{id} => if o.id==id { n==o && r==patch_err(SceneError::CannotRemoveRoot) }
            else if node_below(o,id) { r==patch_ok() && node_removed(o,n,id) }
            else { node_same(o,n) && r==patch_err(SceneError::NodeNotFound(id)) },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u64);

pub enum NodeKind {
    Container,
    Text(String),
    Rect,
    Canvas(Canvas),
}

pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub style: Style,
    pub semantics: Semantics,
    /// Child offset applied after layout; scroll containers update this.
    pub scroll: Point,
    pub children: Vec<Node>,
}

pub enum ScenePatch {
    Insert {
        parent: NodeId,
        index: usize,
        node: Node,
    },
    UpdateStyle {
        id: NodeId,
        style: Style,
    },
    /// Framework-owned semantic state; native backends may require stable component role.
    UpdateSemantics {
        id: NodeId,
        semantics: Semantics,
    },
    UpdateText {
        id: NodeId,
        text: String,
    },
    SetCanvas {
        id: NodeId,
        canvas: Canvas,
    },
    SetScroll {
        id: NodeId,
        offset: Point,
    },
    /// Moves an existing node without changing its identity or interaction state.
    Move {
        id: NodeId,
        parent: NodeId,
        index: usize,
    },
    Remove {
        id: NodeId,
    },
}

pub enum SceneError {
    NodeNotFound(NodeId),
    DuplicateNode(NodeId),
    CannotRemoveRoot,
    CannotMoveRoot,
    MoveIntoDescendant(NodeId),
    NotText(NodeId),
    NotCanvas(NodeId),
    StaleVersion {
        expected: u64,
        actual: u64,
    },
    /// No fresh version is available; the scene remains unchanged.
    /// 无可用的新版本；场景保持不变。
    VersionExhausted,
}

/// Finds the first node with the identity in preorder, the node itself before its children.
/// 按前序查找首个身份匹配的节点，节点本身先于其子节点。
fn find_node(node: &Node, id: NodeId) -> (result:Option<&Node>)
        ensures (result is None) == !node_holds(*node,id),
            result matches Some(found) ==> node_relocated(*node,*node,id,*found,*found)
                && node_holds_value(*node,id,*found),
        decreases *node,
    {
    if node.id.0 == id.0 {
        return Some(node);
    }
    let mut index = 0usize;
    while index < node.children.len()
        invariant index<=node.children@.len(), node.id!=id,
            forall|j:int| #![trigger node.children@[j]] 0<=j<index ==> !node_holds(node.children@[j],id),
        decreases node.children.len()-index,
    {
        proof {
            assert(decreases_to!(node.children => node.children@[index as int]));
            assert(decreases_to!(*node => node.children@[index as int]));
        }
        if let Some(found) = find_node(&node.children[index], id) {
            proof {
                let p=index as int;
                assert(node_relocated(node.children@[p],node.children@[p],id,*found,*found));
                assert(node_holds_value(node.children@[p],id,*found));
                lemma_child_holds_value(*node,p,id,*found);
            }
            return Some(found);
        }
        index += 1;
    }
    None
}

/// Lends the node `find_node` finds. Each level asks the read path which child holds the identity,
/// because a mutable borrow cannot be retried after a miss; a level rescans, so the worst case is
/// nodes times depth. Nothing is written here: the only change to the tree is what the caller
/// does through the returned reference.
/// 借出 `find_node` 找到的节点。每层先用只读路径问清哪个子节点含该身份，因为可变借用在未命中后无法重试；
/// 每层都重新扫描，最坏为节点数乘深度。这里不写任何东西：树的唯一变化是调用者经返回引用所做的修改。
fn find_node_mut(node: &mut Node, id: NodeId) -> (result:Option<&mut Node>)
        ensures (result is None) == !node_holds(*old(node),id),
            result is None ==> *final(node)==*old(node),
            result matches Some(found) ==> node_relocated(*old(node),*final(node),id,*found,*final(found)),
        decreases *old(node),
    {
    let ghost start=*node;
    if node.id.0 == id.0 {
        return Some(node);
    }
    let mut index = 0usize;
    while index < node.children.len()
        invariant index<=node.children@.len(), *node==start, *old(node)==start, start.id!=id,
            forall|j:int| #![trigger start.children@[j]] 0<=j<index ==> !node_holds(start.children@[j],id),
        decreases node.children.len()-index,
    {
        if find_node(&node.children[index], id).is_some() {
            proof {
                assert(decreases_to!(start.children => start.children@[index as int]));
                assert(decreases_to!(start => start.children@[index as int]));
            }
            return find_node_mut(&mut node.children[index], id);
        }
        index += 1;
    }
    None
}

/// Removes the first direct child with the identity; failing that, removes it from the first child
/// subtree that holds it, leaving every other node alone. A miss changes nothing.
/// 优先移除首个身份匹配的直接子节点；否则只在首个含该身份的子树中移除，其余节点不动；未命中不做任何改变。
fn remove_node(parent: &mut Node, id: NodeId) -> (result:bool)
        ensures result==node_below(*old(parent),id),
            !result ==> node_same(*old(parent),*final(parent)),
            result ==> node_removed(*old(parent),*final(parent),id),
        decreases *old(parent),
    {
    let ghost start=*parent;
    let mut index = 0usize;
    while index < parent.children.len()
        invariant index<=parent.children@.len(), *parent==start, *old(parent)==start,
            forall|j:int| #![trigger start.children@[j]] 0<=j<index ==> start.children@[j].id!=id,
        decreases parent.children.len()-index,
    {
        if parent.children[index].id.0 == id.0 {
            parent.children.remove(index);
            proof {
                let p=index as int;
                assert(start.children@[p].id==id);
                assert(parent.children@=~=start.children@.remove(p));
            }
            return true;
        }
        index += 1;
    }
    proof { node_same_refl(start); }
    index = 0;
    while index < parent.children.len()
        invariant index<=parent.children@.len(), *old(parent)==start,
            node_fields_same(start,*parent),
            parent.children@.len()==start.children@.len(),
            forall|j:int| #![trigger start.children@[j]] 0<=j<start.children@.len() ==> start.children@[j].id!=id,
            forall|j:int| #![trigger start.children@[j]] 0<=j<index ==> !node_below(start.children@[j],id),
            forall|j:int| #![trigger start.children@[j]] 0<=j<index ==> node_same(start.children@[j],parent.children@[j]),
            forall|j:int| #![trigger start.children@[j]] index<=j<start.children@.len() ==> parent.children@[j]==start.children@[j],
        decreases parent.children.len()-index,
    {
        let ghost before=parent.children@;
        proof {
            assert(before[index as int]==start.children@[index as int]);
            assert(decreases_to!(start.children => start.children@[index as int]));
            assert(decreases_to!(start => start.children@[index as int]));
        }
        if remove_node(&mut parent.children[index], id) {
            proof {
                let p=index as int;
                assert forall|j:int| #![trigger start.children@[j]] 0<=j<start.children@.len() && j!=p implies node_same(start.children@[j],parent.children@[j]) by {
                    assert(parent.children@[j]==before[j]);
                    if j>p { node_same_refl(start.children@[j]); }
                };
                assert(start.children@[p]==before[p]);
                assert(node_removed(start.children@[p],parent.children@[p],id));
            }
            return true;
        }
        proof {
            let p=index as int;
            assert forall|j:int| #![trigger start.children@[j]] 0<=j<start.children@.len() && j!=p implies parent.children@[j]==before[j] by {};
            assert(start.children@[p]==before[p]);
        }
        index += 1;
    }
    false
}

fn scene_move_failure(
    root_matches: bool,
    id_present: bool,
    parent_present: bool,
    parent_in_moved: bool,
    id: NodeId,
    parent: NodeId,
) -> (result: Option<SceneError>)
    ensures match result {
        Some(SceneError::CannotMoveRoot) => root_matches,
        Some(SceneError::NodeNotFound(missing)) => !root_matches && (
            (!id_present && missing == id)
            || (id_present && !parent_present && missing == parent)),
        Some(SceneError::MoveIntoDescendant(moved)) =>
            !root_matches && id_present && parent_present && parent_in_moved && moved == id,
        None => !root_matches && id_present && parent_present && !parent_in_moved,
        Some(_) => false,
    },
{
    if root_matches {
        Some(SceneError::CannotMoveRoot)
    } else if !id_present {
        Some(SceneError::NodeNotFound(id))
    } else if !parent_present {
        Some(SceneError::NodeNotFound(parent))
    } else if parent_in_moved {
        Some(SceneError::MoveIntoDescendant(id))
    } else {
        None
    }
}

/// Takes the first direct child with the identity; failing that, takes it from the first child
/// subtree that holds it. A miss changes nothing.
fn take_node(parent: &mut Node, id: NodeId) -> (result:Option<Node>)
        requires (*old(parent)).id != id,
        ensures match result {
            Some(taken) => node_removed(*old(parent),*final(parent),id)
                && node_removed_value(*old(parent),*final(parent),id,taken)
                && node_holds_value(*old(parent),id,taken),
            None => node_same(*old(parent),*final(parent)) && !node_below(*old(parent),id),
        },
        decreases *old(parent),
    {
    let ghost start=*parent;
    let mut index = 0usize;
    proof { assert(start.id != id); }
    while index < parent.children.len()
        invariant index<=parent.children@.len(), *parent==start, *old(parent)==start, start.id!=id,
            forall|j:int| #![trigger start.children@[j]] 0<=j<index ==> start.children@[j].id!=id,
        decreases parent.children.len()-index,
    {
        if parent.children[index].id.0 == id.0 {
            let taken=parent.children.remove(index);
            proof {
                let p=index as int;
                assert(start.children@[p].id==id);
                assert(start.children@[p]==taken);
                assert(parent.children@=~=start.children@.remove(p));
                assert(node_holds_value(start.children@[p],id,taken));
                lemma_child_holds_value(start,p,id,taken);
                assert(node_removed_value(start,*parent,id,taken)) by {
                    assert(parent.children@=~=start.children@.remove(p));
                    assert(taken==start.children@[p]);
                }
            }
            return Some(taken);
        }
        index += 1;
    }
    proof { node_same_refl(start); }
    index = 0;
    while index < parent.children.len()
        invariant index<=parent.children@.len(), *old(parent)==start,
            node_fields_same(start,*parent),
            parent.children@.len()==start.children@.len(),
            start.id!=id,
            forall|j:int| #![trigger start.children@[j]] 0<=j<start.children@.len() ==> start.children@[j].id!=id,
            forall|j:int| #![trigger start.children@[j]] 0<=j<index ==> !node_below(start.children@[j],id),
            forall|j:int| #![trigger start.children@[j]] 0<=j<index ==> node_same(start.children@[j],parent.children@[j]),
            forall|j:int| #![trigger start.children@[j]] index<=j<start.children@.len() ==> parent.children@[j]==start.children@[j],
        decreases parent.children.len()-index,
    {
        let ghost before=parent.children@;
        proof {
            assert(before[index as int]==start.children@[index as int]);
            assert(decreases_to!(start.children => start.children@[index as int]));
            assert(decreases_to!(start => start.children@[index as int]));
        }
        let found = find_node(&parent.children[index], id);
        if found.is_some() {
            let taken=take_node(&mut parent.children[index], id);
            proof {
                let p=index as int;
                assert forall|j:int| #![trigger start.children@[j]] 0<=j<start.children@.len() && j!=p implies node_same(start.children@[j],parent.children@[j]) by {
                    assert(parent.children@[j]==before[j]);
                    if j>p { node_same_refl(start.children@[j]); }
                };
                assert(start.children@[p]==before[p]);
                if taken is Some {
                    let moved=taken->Some_0;
                    assert(node_removed(start.children@[p],parent.children@[p],id));
                    assert(node_holds_value(start.children@[p],id,moved));
                    assert(start.id != id);
                    lemma_child_holds_value(start,p,id,moved);
                    assert(node_removed(start,*parent,id));
                    assert(node_removed_value(start,*parent,id,moved)) by {
                        assert(parent.children@.len()==start.children@.len());
                        assert(node_removed_value(start.children@[p],parent.children@[p],id,moved));
                    }
                } else {
                    assert(!node_below(start.children@[p],id));
                    assert(node_holds(start.children@[p],id));
                    assert(start.children@[p].id!=id);
                    lemma_holds_below(start.children@[p],id);
                    assert(false);
                }
            }
            return taken;
        }
        proof {
            let p=index as int;
            if node_below(start.children@[p],id) {
                lemma_node_below_holds(start.children@[p],id);
                assert(found.is_some());
                assert(false);
            }
            node_same_refl(start.children@[p]);
            assert forall|j:int| #![trigger start.children@[j]] 0<=j<start.children@.len() && j!=p implies parent.children@[j]==before[j] by {};
            assert(start.children@[p]==before[p]);
        }
        index += 1;
    }
    None
}

/// Inserts a node under the first matching parent, clamping the index to the child count.
fn insert_node_at(root: &mut Node, parent: NodeId, index: usize, node: Node) -> (result:bool)
        ensures result == node_holds(*old(root),parent),
            result ==> exists|t:Node,f:Node| #![trigger node_relocated(*old(root),*final(root),parent,t,f)]
                node_relocated(*old(root),*final(root),parent,t,f)
                && node_fields_same(t,f)
                && f.children@==t.children@.insert(
                    if index<t.children@.len() {index as int} else {t.children@.len() as int},node),
            !result ==> *final(root)==*old(root),
    {
    let Some(parent_node) = find_node_mut(root, parent) else {
        return false;
    };
    let len = parent_node.children.len();
    let index = if index < len { index } else { len };
    parent_node.children.insert(index, node);
    true
}

/// Admits, extracts and reinserts one existing subtree. The extracted value is moved into the
/// destination, so the successful result is exactly removal followed by insertion.
fn move_node_at(
    root: &mut Node,
    id: NodeId,
    parent: NodeId,
    index: usize,
) -> (result: Result<(), SceneError>)
    ensures move_applied(*old(root),*final(root),id,parent,index,result),
{
    let ghost start=*root;
    let moved = find_node(&*root, id);
    let parent_present = find_node(&*root, parent).is_some();
    let parent_in_moved = match moved {
        Some(moved) => find_node(moved, parent).is_some(),
        None => false,
    };
    let ghost source_match=moved;
    let _source_match = moved;
    match scene_move_failure(
        root.id.0 == id.0,
        moved.is_some(),
        parent_present,
        parent_in_moved,
        id,
        parent,
    ) {
        Some(failure) => {
            proof {
                assert(*root==start);
                assert(move_applied(start,start,id,parent,index,Err(failure)));
            }
            return Err(failure);
        }
        None => {
            proof { assert(moved.is_some()); }
        }
    }
    let Some(moved) = take_node(root, id) else {
        proof {
            assert(start.id!=id);
            lemma_holds_below(start,id);
        }
        return Err(SceneError::NodeNotFound(id));
    };
    let ghost extracted=moved;
    let ghost removed_tree=*root;
    let inserted = insert_node_at(root, parent, index, moved);
    if inserted {
        proof {
            assert(move_succeeded(start,*root,id,parent,index));
            assert(move_applied(start,*root,id,parent,index,Ok(())));
        }
        Ok(())
    } else {
        proof {
            assert(!node_holds(removed_tree,parent));
            assert(move_applied(start,*root,id,parent,index,Err(SceneError::NodeNotFound(parent))));
        }
        Err(SceneError::NodeNotFound(parent))
    }
}

/// Applies one patch to the tree. The tree is the only state reached; a failed patch may be left
/// half-way by nothing: each branch writes only after its checks pass.
/// 把一个补丁应用到树。只触及树本身；每个分支都先通过检查再写入。
fn apply_tree_patch(root: &mut Node, patch: ScenePatch) -> (result:Result<(), SceneError>)
        ensures patch_applied(*old(root),*final(root),patch,result),
    {
    match patch {
        ScenePatch::Insert {
            parent,
            index,
            node,
        } => {
            if insert_node_at(root, parent, index, node) {
                Ok(())
            } else {
                Err(SceneError::NodeNotFound(parent))
            }
        }
        ScenePatch::UpdateSemantics { id, semantics } => match find_node_mut(root, id) {
            Some(node) => {
                let ghost t=*node;
                node.semantics = semantics;
                proof { assert(node_relocated(*old(root),*final(root),id,t,*node)); }
                Ok(())
            }
            None => Err(SceneError::NodeNotFound(id)),
        },
        ScenePatch::UpdateStyle { id, style } => match find_node_mut(root, id) {
            Some(node) => {
                let ghost t=*node;
                node.style = style;
                proof { assert(node_relocated(*old(root),*final(root),id,t,*node)); }
                Ok(())
            }
            None => Err(SceneError::NodeNotFound(id)),
        },
        ScenePatch::SetScroll { id, offset } => match find_node_mut(root, id) {
            Some(node) => {
                let ghost t=*node;
                node.scroll = offset;
                proof { assert(node_relocated(*old(root),*final(root),id,t,*node)); }
                Ok(())
            }
            None => Err(SceneError::NodeNotFound(id)),
        },
        ScenePatch::Move { id, parent, index } => {
            move_node_at(root, id, parent, index)
        },
        ScenePatch::UpdateText { id, text } => match find_node_mut(root, id) {
            Some(node) => match &mut node.kind {
                NodeKind::Text(existing) => {
                    *existing = text;
                    Ok(())
                }
                _ => Err(SceneError::NotText(id)),
            },
            None => Err(SceneError::NodeNotFound(id)),
        },
        ScenePatch::SetCanvas { id, canvas } => match find_node_mut(root, id) {
            Some(node) => match &mut node.kind {
                NodeKind::Canvas(existing) => {
                    *existing = canvas;
                    Ok(())
                }
                _ => Err(SceneError::NotCanvas(id)),
            },
            None => Err(SceneError::NodeNotFound(id)),
        },
        ScenePatch::Remove { id } => {
                        if remove_node(root, id) {
                Ok(())
            } else {
                Err(SceneError::NodeNotFound(id))
            }
        }
    }
}

}
