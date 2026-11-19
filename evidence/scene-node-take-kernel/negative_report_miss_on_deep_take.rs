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
            return None;
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

}
