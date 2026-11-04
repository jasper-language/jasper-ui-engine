use vstd::prelude::*;
verus! {

pub struct Canvas(u64);

pub struct Style(u64);
pub struct Semantics(u64);
pub struct Point(u64);
broadcast use vstd::std_specs::vec::axiom_vec_index_decreases;


// The identities of a tree in preorder, the node itself before its children, children in order.
// 树的前序身份序列：节点本身先于子节点，子节点按序。
pub open spec fn preorder_ids(n:Node)->Seq<NodeId> decreases n, 1int, 0int {
    seq![n.id]+children_ids(n,0)
}
pub open spec fn children_ids(n:Node,i:int)->Seq<NodeId> decreases n, 0int, n.children@.len()-i {
    if i<0 || i>=n.children@.len() { Seq::empty() }
    else { preorder_ids(n.children@[i])+children_ids(n,i+1) }
}
// What the stack still owes: the top of the stack first.
// 栈中尚欠的身份：栈顶优先。
pub open spec fn pending_ids(p:Seq<&Node>)->Seq<NodeId> decreases p.len() {
    if p.len()==0 { Seq::empty() } else { preorder_ids(*p.last())+pending_ids(p.drop_last()) }
}
// No identity repeats within s, and none was already in start.
// s 内无重复，且没有一个已在 start 中。
pub open spec fn unrepeated(start:Set<NodeId>,s:Seq<NodeId>)->bool {
    (forall|i:int,j:int| #![trigger s[i],s[j]] 0<=i<j<s.len() ==> s[i]!=s[j])
    && forall|i:int| #![trigger s[i]] 0<=i<s.len() ==> !start.contains(s[i])
}
// after is exactly start plus the identities of s.
// after 恰为 start 加上 s 的身份。
pub open spec fn records(after:Set<NodeId>,start:Set<NodeId>,s:Seq<NodeId>)->bool {
    forall|id:NodeId| #![trigger after.contains(id)] after.contains(id) <==> (start.contains(id) || s.contains(id))
}
// The traversal's verdict on the sequence order against the set it began with and left behind.
// 遍历对序列 order 的判定，相对于起始集合与留下的集合。
pub open spec fn identity_verdict(result:Result<(),SceneError>,start:Set<NodeId>,after:Set<NodeId>,order:Seq<NodeId>)->bool {
    match result {
        Ok(_)=>unrepeated(start,order) && records(after,start,order),
        Err(SceneError::DuplicateNode(d))=>exists|k:int| #![trigger order[k]] 0<=k<order.len() && order[k]==d
            && unrepeated(start,order.take(k))
            && (start.contains(d) || order.take(k).contains(d))
            && records(after,start,order.take(k)),
        Err(_)=>false,
    }
}
proof fn lemma_concat_contains(a:Seq<NodeId>,b:Seq<NodeId>,x:NodeId)
    ensures (a+b).contains(x) <==> (a.contains(x) || b.contains(x)),
{
    if a.contains(x) { let i=choose|i:int| 0<=i<a.len() && a[i]==x; assert((a+b)[i]==x); }
    if b.contains(x) { let i=choose|i:int| 0<=i<b.len() && b[i]==x; assert((a+b)[a.len()+i]==x); }
    if (a+b).contains(x) {
        let i=choose|i:int| 0<=i<(a+b).len() && (a+b)[i]==x;
        if i<a.len() { assert(a[i]==x); } else { assert(b[i-a.len()]==x); }
    }
}
proof fn lemma_records_chain(after:Set<NodeId>,mid:Set<NodeId>,start:Set<NodeId>,a:Seq<NodeId>,b:Seq<NodeId>)
    requires records(mid,start,a), records(after,mid,b),
    ensures records(after,start,a+b),
{
    assert forall|x:NodeId| #![trigger after.contains(x)] after.contains(x) <==> (start.contains(x) || (a+b).contains(x)) by {
        lemma_concat_contains(a,b,x);
        assert(mid.contains(x) <==> (start.contains(x) || a.contains(x)));
    }
}
proof fn lemma_unrepeated_chain(mid:Set<NodeId>,start:Set<NodeId>,a:Seq<NodeId>,b:Seq<NodeId>)
    requires unrepeated(start,a), records(mid,start,a), unrepeated(mid,b),
    ensures unrepeated(start,a+b),
{
    let s=a+b;
    assert forall|i:int| 0<=i<a.len() implies mid.contains(#[trigger] a[i]) by { assert(a.contains(a[i])); }
    assert forall|i:int| 0<=i<b.len() implies !start.contains(#[trigger] b[i]) by { assert(!mid.contains(b[i])); }
    assert forall|i:int,j:int| #![trigger s[i],s[j]] 0<=i<j<s.len() implies s[i]!=s[j] by {
        if i<a.len() && j<a.len() { assert(s[i]==a[i] && s[j]==a[j]); }
        else if i<a.len() { assert(s[i]==a[i] && s[j]==b[j-a.len()]); assert(mid.contains(a[i])); assert(!mid.contains(b[j-a.len()])); }
        else { assert(s[i]==b[i-a.len()] && s[j]==b[j-a.len()]); }
    }
    assert forall|i:int| #![trigger s[i]] 0<=i<s.len() implies !start.contains(s[i]) by {
        if i<a.len() { assert(s[i]==a[i]); } else { assert(s[i]==b[i-a.len()]); }
    }
}
proof fn lemma_visit_fresh(after:Set<NodeId>,pre:Set<NodeId>,start:Set<NodeId>,visited:Seq<NodeId>,id:NodeId)
    requires unrepeated(start,visited), records(pre,start,visited), !pre.contains(id), after==pre.insert(id),
    ensures unrepeated(start,visited.push(id)), records(after,start,visited.push(id)),
{
    let v=visited.push(id);
    assert forall|i:int| 0<=i<visited.len() implies #[trigger] visited[i]!=id by {
        if visited[i]==id { assert(visited.contains(id)); }
    }
    assert forall|i:int,j:int| #![trigger v[i],v[j]] 0<=i<j<v.len() implies v[i]!=v[j] by {
        assert(v[i]==visited[i]);
        if j<visited.len() { assert(v[j]==visited[j]); }
    }
    assert forall|i:int| #![trigger v[i]] 0<=i<v.len() implies !start.contains(v[i]) by {
        if i<visited.len() { assert(v[i]==visited[i]); }
    }
    assert forall|x:NodeId| #![trigger after.contains(x)] after.contains(x) <==> (start.contains(x) || v.contains(x)) by {
        if x==id { assert(v[visited.len() as int]==x); }
        if visited.contains(x) { let i=choose|i:int| 0<=i<visited.len() && visited[i]==x; assert(v[i]==x); }
        if v.contains(x) { let i=choose|i:int| 0<=i<v.len() && v[i]==x; if i<visited.len() { assert(visited[i]==x); } }
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

pub enum SceneError {
    NodeNotFound(NodeId),
    DuplicateNode(NodeId),
    CannotRemoveRoot,
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

/// What the identity traversal needs from its set: records an identity and reports whether it was new.
/// 身份遍历对集合的全部要求：记录一个身份，并报告它此前是否未出现。
trait IdentitySet: Sized {
    spec fn seen(&self)->Set<NodeId>;
    fn admit(&mut self, id: NodeId) -> (fresh:bool)
        ensures fresh==!old(self).seen().contains(id), final(self).seen()==old(self).seen().insert(id);
}

/// Traverses iteratively so identity validation itself does not recurse.
/// 迭代遍历，身份校验本身不递归；不代表后续布局/克隆已具备深度资源限制。
fn collect_unique_ids<T: IdentitySet>(root: &Node, ids: &mut T) -> (result:Result<(), SceneError>)
        ensures identity_verdict(result,old(ids).seen(),final(ids).seen(),preorder_ids(*root)),
    {
    let ghost start=ids.seen();
    let ghost order=preorder_ids(*root);
    let ghost mut visited:Seq<NodeId>=Seq::empty();
    let mut pending = Vec::new();
    pending.push(root);
    proof {
        assert(pending@.drop_last()=~=Seq::<&Node>::empty());
        assert(pending@.last()==root);
        assert(pending_ids(pending@.drop_last())=~=Seq::<NodeId>::empty());
        assert(pending_ids(pending@)==preorder_ids(*root)+pending_ids(pending@.drop_last()));
        assert(pending_ids(pending@)=~=order);
        assert(visited+pending_ids(pending@)=~=order);
    }
    loop
        invariant visited+pending_ids(pending@)==order, unrepeated(start,visited), records(ids.seen(),start,visited),
            start==old(ids).seen(), order==preorder_ids(*root),
        decreases order.len()-visited.len(),
    {
        let ghost before=pending@;
        let node = match pending.pop() {
            Some(node) => node,
            None => {
                proof {
                    assert(pending_ids(pending@)=~=Seq::<NodeId>::empty());
                    assert(visited=~=order);
                }
                return Ok(());
            }
        };
        let ghost pre=ids.seen();
        let ghost rest_ids=pending_ids(pending@);
        proof {
            assert(before.drop_last()=~=pending@);
            assert(pending_ids(before)==preorder_ids(*node)+rest_ids);
            assert(preorder_ids(*node)==seq![node.id]+children_ids(*node,0));
            assert(order=~=visited+seq![node.id]+children_ids(*node,0)+rest_ids);
            assert(order[visited.len() as int]==node.id);
            assert(order.take(visited.len() as int)=~=visited);
        }
        if !ids.admit(node.id) {
            proof {
                let k=visited.len() as int;
                assert(ids.seen()=~=pre);
                assert(order[k]==node.id);
                assert(start.contains(node.id) || order.take(k).contains(node.id));
                assert(records(ids.seen(),start,order.take(k)));
                assert(identity_verdict(Err(SceneError::DuplicateNode(node.id)),start,ids.seen(),order));
            }
            return Err(SceneError::DuplicateNode(node.id));
        }
        proof {
            lemma_visit_fresh(ids.seen(),pre,start,visited,node.id);
            assert(visited.push(node.id)=~=visited+seq![node.id]);
            visited=visited.push(node.id);
        }
        let mut index = node.children.len();
        proof { assert(children_ids(*node,index as int)=~=Seq::<NodeId>::empty()); }
        while index > 0
            invariant index<=node.children@.len(),
                pending_ids(pending@)==children_ids(*node,index as int)+rest_ids,
                visited+children_ids(*node,0)+rest_ids==order,
                unrepeated(start,visited), records(ids.seen(),start,visited),
                start==old(ids).seen(), order==preorder_ids(*root),
            decreases index,
        {
            let ghost prior=pending@;
            index -= 1;
            pending.push(&node.children[index]);
            proof {
                assert(pending@.drop_last()=~=prior);
                assert(children_ids(*node,index as int)==preorder_ids(node.children@[index as int])+children_ids(*node,index as int+1));
                assert(pending_ids(pending@)=~=children_ids(*node,index as int)+rest_ids);
            }
        }
        proof {
            assert(pending_ids(pending@)==children_ids(*node,0)+rest_ids);
            assert(visited+pending_ids(pending@)=~=order);
        }
    }
}

/// The identity check for an insertion: the tree first, then the inserted subtree, in one set.
/// 插入的身份检查：先查树，再查待插入子树，共用一个集合。
fn check_insert_identities<T: IdentitySet>(root:&Node,inserted:&Node,ids:&mut T)->(result:Result<(), SceneError>)
        ensures identity_verdict(result,old(ids).seen(),final(ids).seen(),preorder_ids(*root)+preorder_ids(*inserted)),
    {
    let ghost start=ids.seen();
    let ghost a=preorder_ids(*root);
    let ghost b=preorder_ids(*inserted);
    let ghost s=a+b;
    match collect_unique_ids(root, ids) {
        Ok(()) => {
            let ghost mid=ids.seen();
            let second=collect_unique_ids(inserted, ids);
            proof {
                match second {
                    Ok(_)=>{
                        lemma_unrepeated_chain(mid,start,a,b);
                        lemma_records_chain(ids.seen(),mid,start,a,b);
                    }
                    Err(SceneError::DuplicateNode(d))=>{
                        let k=choose|k:int| #![trigger b[k]] 0<=k<b.len() && b[k]==d && unrepeated(mid,b.take(k))
                            && (mid.contains(d) || b.take(k).contains(d)) && records(ids.seen(),mid,b.take(k));
                        let w=a.len()+k;
                        assert(s[w]==d);
                        assert(s.take(w)=~=a+b.take(k));
                        lemma_unrepeated_chain(mid,start,a,b.take(k));
                        lemma_records_chain(ids.seen(),mid,start,a,b.take(k));
                        lemma_concat_contains(a,b.take(k),d);
                        assert(mid.contains(d) <==> (start.contains(d) || a.contains(d)));
                        assert(identity_verdict(second,start,ids.seen(),s));
                    }
                    Err(_)=>{}
                }
            }
            second
        }
        Err(error) => {
            return Ok(());
            proof {
                if let SceneError::DuplicateNode(d)=error {
                    let k=choose|k:int| #![trigger a[k]] 0<=k<a.len() && a[k]==d && unrepeated(start,a.take(k))
                        && (start.contains(d) || a.take(k).contains(d)) && records(ids.seen(),start,a.take(k));
                    assert(s[k]==d);
                    assert(s.take(k)=~=a.take(k));
                    assert(identity_verdict(Err(error),start,ids.seen(),s));
                }
            }
            Err(error)
        }
    }
}

}
