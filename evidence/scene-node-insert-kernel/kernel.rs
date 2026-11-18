use vstd::prelude::*;
verus! {

pub struct Canvas(u64);

pub struct Style(u64);
pub struct Semantics(u64);
pub struct Point(u64);
broadcast use vstd::std_specs::vec::axiom_vec_index_decreases;

pub open spec fn node_holds(n:Node,id:NodeId)->bool decreases n, {
    n.id==id || exists|i:int| #![trigger n.children@[i]] 0<=i<n.children@.len() && node_holds(n.children@[i],id)
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

/// Finds the first node with the identity in preorder, the node itself before its children.
/// 按前序查找首个身份匹配的节点，节点本身先于其子节点。
fn find_node(node: &Node, id: NodeId) -> (result:Option<&Node>)
        ensures (result is None) == !node_holds(*node,id),
            result matches Some(found) ==> node_relocated(*node,*node,id,*found,*found),
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

}
