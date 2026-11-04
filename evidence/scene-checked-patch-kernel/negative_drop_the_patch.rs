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
        ScenePatch::Remove{id} => if o.id==id { n==o && r==patch_err(SceneError::CannotRemoveRoot) }
            else if node_below(o,id) { r==patch_ok() && node_removed(o,n,id) }
            else { node_same(o,n) && r==patch_err(SceneError::NodeNotFound(id)) },
    }
}

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

pub open spec fn ids_distinct(s:Seq<NodeId>)->bool { unrepeated(Set::<NodeId>::empty(),s) }
// whole is rest with part put in at one position.
// whole 是在 rest 的某个位置放入 part 的结果。
pub open spec fn spliced_ids(whole:Seq<NodeId>,part:Seq<NodeId>,rest:Seq<NodeId>)->bool {
    exists|k:int| #![trigger rest.take(k)] 0<=k<=rest.len() && whole==rest.take(k)+part+rest.skip(k)
}
// A successful removal takes exactly one subtree with the identity out of the sequence; everything else keeps the sequence.
// 成功的移除从序列中恰好取走一棵带该身份的子树；其余情况序列不变。
pub open spec fn patch_keeps_ids(o:Node,n:Node,patch:ScenePatch,r:Result<(),SceneError>)->bool {
    match patch {
        ScenePatch::Remove{id} => if r is Ok {
                exists|t:Node| #![trigger preorder_ids(t)] t.id==id && spliced_ids(preorder_ids(o),preorder_ids(t),preorder_ids(n))
            } else { preorder_ids(n)==preorder_ids(o) },
        _ => preorder_ids(n)==preorder_ids(o),
    }
}
// Insertion is admitted only when the tree followed by the subtree repeats nothing; its result then repeats nothing either.
// Any other patch on a tree that repeats nothing leaves a tree that repeats nothing, without being checked.
// 只有树后接子树无重复时才准入插入，其结果也无重复。其他补丁不做检查，无重复的树仍无重复。
pub open spec fn patch_checked(o:Node,n:Node,patch:ScenePatch,r:Result<(),SceneError>)->bool {
    match patch {
        ScenePatch::Insert{node,..} => {
            let order=preorder_ids(o)+preorder_ids(node);
            if ids_distinct(order) {
                patch_applied(o,n,patch,r) && (r is Ok ==> ids_distinct(preorder_ids(n)) && spliced_ids(preorder_ids(n),preorder_ids(node),preorder_ids(o)))
                    && (r is Err ==> n==o)
            } else {
                n==o && exists|d:NodeId| #![trigger order.contains(d)] order.contains(d) && r==patch_err(SceneError::DuplicateNode(d))
            }
        }
        _ => patch_applied(o,n,patch,r) && patch_keeps_ids(o,n,patch,r) && (ids_distinct(preorder_ids(o)) ==> ids_distinct(preorder_ids(n))),
    }
}
pub open spec fn ids_span(n:Node,i:int,p:int)->Seq<NodeId> decreases p-i {
    if i>=p { Seq::empty() } else { preorder_ids(n.children@[i])+ids_span(n,i+1,p) }
}
proof fn lemma_children_split(n:Node,i:int,p:int)
    requires 0<=i<=p<=n.children@.len(),
    ensures children_ids(n,i)==ids_span(n,i,p)+children_ids(n,p),
    decreases p-i,
{
    if i==p { assert(ids_span(n,i,p)=~=Seq::<NodeId>::empty()); assert(children_ids(n,i)=~=ids_span(n,i,p)+children_ids(n,p)); }
    else {
        lemma_children_split(n,i+1,p);
        assert(children_ids(n,i)==preorder_ids(n.children@[i])+children_ids(n,i+1));
        assert(ids_span(n,i,p)==preorder_ids(n.children@[i])+ids_span(n,i+1,p));
        assert(children_ids(n,i)=~=ids_span(n,i,p)+children_ids(n,p));
    }
}
proof fn lemma_span_same(a:Node,b:Node,i:int,p:int)
    requires 0<=i<=p, forall|j:int| #![trigger b.children@[j]] i<=j<p ==> preorder_ids(a.children@[j])==preorder_ids(b.children@[j]),
    ensures ids_span(a,i,p)==ids_span(b,i,p),
    decreases p-i,
{
    if i<p { lemma_span_same(a,b,i+1,p); assert(preorder_ids(a.children@[i])==preorder_ids(b.children@[i])); }
}
proof fn lemma_suffix_same(a:Node,b:Node,i:int)
    requires 0<=i, a.children@.len()==b.children@.len(),
        forall|j:int| #![trigger b.children@[j]] i<=j<b.children@.len() ==> preorder_ids(a.children@[j])==preorder_ids(b.children@[j]),
    ensures children_ids(a,i)==children_ids(b,i),
    decreases a.children@.len()-i,
{
    if i<a.children@.len() { lemma_suffix_same(a,b,i+1); assert(preorder_ids(a.children@[i])==preorder_ids(b.children@[i])); }
}
// long has one more child than short before position i; from there on they line up one apart.
// long 在位置 i 之前比 short 多一个子节点；从那以后二者错开一位对齐。
proof fn lemma_suffix_shift(long:Node,short:Node,i:int)
    requires 0<=i<=short.children@.len(), long.children@.len()==short.children@.len()+1,
        forall|j:int| #![trigger short.children@[j]] i<=j<short.children@.len() ==> long.children@[j+1]==short.children@[j],
    ensures children_ids(long,i+1)==children_ids(short,i),
    decreases short.children@.len()-i,
{
    if i<short.children@.len() {
        lemma_suffix_shift(long,short,i+1);
        assert(long.children@[i+1]==short.children@[i]);
        assert(children_ids(long,i+1)==preorder_ids(long.children@[i+1])+children_ids(long,i+2));
    }
}
proof fn lemma_same_ids(a:Node,b:Node)
    requires node_same(a,b),
    ensures preorder_ids(a)==preorder_ids(b),
    decreases a,
{
    assert forall|j:int| #![trigger b.children@[j]] 0<=j<b.children@.len() implies preorder_ids(a.children@[j])==preorder_ids(b.children@[j]) by {
        assert(node_same(a.children@[j],b.children@[j]));
        lemma_same_ids(a.children@[j],b.children@[j]);
    };
    lemma_suffix_same(a,b,0);
}
proof fn lemma_same_children_ids(a:Node,b:Node)
    requires a.id==b.id, a.children==b.children,
    ensures preorder_ids(a)==preorder_ids(b),
{
    lemma_suffix_same(a,b,0);
}
// Equal children outside p and any pair of sequences at p give two sequences that differ only at p.
// p 之外子节点身份序列相同时，两侧序列只在 p 处不同。
proof fn lemma_child_at(a:Node,b:Node,p:int)
    requires 0<=p<a.children@.len(), a.id==b.id, a.children@.len()==b.children@.len(),
        forall|j:int| #![trigger b.children@[j]] 0<=j<b.children@.len() && j!=p ==> preorder_ids(a.children@[j])==preorder_ids(b.children@[j]),
    ensures
        preorder_ids(a)==seq![a.id]+ids_span(a,0,p)+preorder_ids(a.children@[p])+children_ids(a,p+1),
        preorder_ids(b)==seq![a.id]+ids_span(a,0,p)+preorder_ids(b.children@[p])+children_ids(a,p+1),
{
    lemma_children_split(a,0,p);
    lemma_children_split(b,0,p);
    lemma_span_same(a,b,0,p);
    lemma_suffix_same(a,b,p+1);
    assert(children_ids(a,p)==preorder_ids(a.children@[p])+children_ids(a,p+1));
    assert(children_ids(b,p)==preorder_ids(b.children@[p])+children_ids(b,p+1));
    assert(preorder_ids(a)=~=seq![a.id]+ids_span(a,0,p)+preorder_ids(a.children@[p])+children_ids(a,p+1));
    assert(preorder_ids(b)=~=seq![a.id]+ids_span(a,0,p)+preorder_ids(b.children@[p])+children_ids(a,p+1));
}
proof fn lemma_relocated_ids(o:Node,f:Node,id:NodeId,t:Node,ft:Node)->(w:(Seq<NodeId>,Seq<NodeId>))
    requires node_relocated(o,f,id,t,ft),
    ensures preorder_ids(o)==w.0+preorder_ids(t)+w.1, preorder_ids(f)==w.0+preorder_ids(ft)+w.1,
    decreases o,
{
    if o.id==id {
        assert(preorder_ids(o)=~=Seq::<NodeId>::empty()+preorder_ids(t)+Seq::<NodeId>::empty());
        assert(preorder_ids(f)=~=Seq::<NodeId>::empty()+preorder_ids(ft)+Seq::<NodeId>::empty());
        (Seq::empty(),Seq::empty())
    } else {
        let p=choose|p:int| #![trigger o.children@[p]] 0<=p<o.children@.len()
            && (forall|j:int| #![trigger o.children@[j]] 0<=j<p ==> !node_holds(o.children@[j],id))
            && (forall|j:int| #![trigger o.children@[j]] 0<=j<o.children@.len() && j!=p ==> o.children@[j]==f.children@[j])
            && node_relocated(o.children@[p],f.children@[p],id,t,ft);
        let inner=lemma_relocated_ids(o.children@[p],f.children@[p],id,t,ft);
        assert forall|j:int| #![trigger f.children@[j]] 0<=j<f.children@.len() && j!=p implies preorder_ids(o.children@[j])==preorder_ids(f.children@[j]) by {
            assert(o.children@[j]==f.children@[j]);
        };
        lemma_child_at(o,f,p);
        let a=seq![o.id]+ids_span(o,0,p)+inner.0;
        let b=inner.1+children_ids(o,p+1);
        assert(preorder_ids(o)=~=a+preorder_ids(t)+b);
        assert(preorder_ids(f)=~=a+preorder_ids(ft)+b);
        (a,b)
    }
}
proof fn lemma_removed_direct(o:Node,n:Node,p:int)->(w:(Seq<NodeId>,Seq<NodeId>))
    requires 0<=p<o.children@.len(), o.id==n.id, n.children@==o.children@.remove(p),
    ensures preorder_ids(o)==w.0+preorder_ids(o.children@[p])+w.1, preorder_ids(n)==w.0+w.1,
{
    lemma_children_split(o,0,p);
    lemma_children_split(n,0,p);
    assert forall|j:int| #![trigger n.children@[j]] 0<=j<p implies preorder_ids(o.children@[j])==preorder_ids(n.children@[j]) by {
        assert(n.children@[j]==o.children@[j]);
    };
    lemma_span_same(o,n,0,p);
    assert forall|j:int| #![trigger n.children@[j]] p<=j<n.children@.len() implies o.children@[j+1]==n.children@[j] by {
        assert(n.children@[j]==o.children@[j+1]);
    };
    lemma_suffix_shift(o,n,p);
    assert(children_ids(o,p)==preorder_ids(o.children@[p])+children_ids(o,p+1));
    let a=seq![o.id]+ids_span(o,0,p);
    let b=children_ids(o,p+1);
    assert(preorder_ids(o)=~=a+preorder_ids(o.children@[p])+b);
    assert(preorder_ids(n)=~=a+b);
    (a,b)
}
proof fn lemma_removed_nested(o:Node,n:Node,p:int,x:Seq<NodeId>,a:Seq<NodeId>,b:Seq<NodeId>)->(w:(Seq<NodeId>,Seq<NodeId>))
    requires 0<=p<o.children@.len(), o.id==n.id, o.children@.len()==n.children@.len(),
        forall|j:int| #![trigger o.children@[j]] 0<=j<o.children@.len() && j!=p ==> node_same(o.children@[j],n.children@[j]),
        preorder_ids(o.children@[p])==a+x+b, preorder_ids(n.children@[p])==a+b,
    ensures preorder_ids(o)==w.0+x+w.1, preorder_ids(n)==w.0+w.1,
{
    assert forall|j:int| #![trigger n.children@[j]] 0<=j<n.children@.len() && j!=p implies preorder_ids(o.children@[j])==preorder_ids(n.children@[j]) by {
        assert(node_same(o.children@[j],n.children@[j]));
        lemma_same_ids(o.children@[j],n.children@[j]);
    };
    lemma_child_at(o,n,p);
    let front=seq![o.id]+ids_span(o,0,p)+a;
    let back=b+children_ids(o,p+1);
    assert(preorder_ids(o)=~=front+x+back);
    assert(preorder_ids(n)=~=front+back);
    (front,back)
}
proof fn lemma_removed_ids(o:Node,n:Node,id:NodeId)->(w:(Node,Seq<NodeId>,Seq<NodeId>))
    requires node_removed(o,n,id),
    ensures w.0.id==id, preorder_ids(o)==w.1+preorder_ids(w.0)+w.2, preorder_ids(n)==w.1+w.2,
    decreases o,
{
    if exists|p:int| #![trigger o.children@[p]] 0<=p<o.children@.len() && o.children@[p].id==id
            && (forall|j:int| #![trigger o.children@[j]] 0<=j<p ==> o.children@[j].id!=id)
            && n.children@=~=o.children@.remove(p) {
        let p=choose|p:int| #![trigger o.children@[p]] 0<=p<o.children@.len() && o.children@[p].id==id
            && (forall|j:int| #![trigger o.children@[j]] 0<=j<p ==> o.children@[j].id!=id)
            && n.children@=~=o.children@.remove(p);
        let w=lemma_removed_direct(o,n,p);
        (o.children@[p],w.0,w.1)
    } else {
        let p=choose|p:int| #![trigger o.children@[p]] 0<=p<o.children@.len()
            && (forall|j:int| #![trigger o.children@[j]] 0<=j<p ==> !node_below(o.children@[j],id))
            && (forall|j:int| #![trigger o.children@[j]] 0<=j<o.children@.len() && j!=p ==> node_same(o.children@[j],n.children@[j]))
            && node_removed(o.children@[p],n.children@[p],id);
        let inner=lemma_removed_ids(o.children@[p],n.children@[p],id);
        let w=lemma_removed_nested(o,n,p,preorder_ids(inner.0),inner.1,inner.2);
        (inner.0,w.0,w.1)
    }
}
// The sequence of a parent with one child put in at k, read against the parent before it.
// 在 k 处放入一个子节点后的父节点序列，对照放入之前。
proof fn lemma_inserted_child(t:Node,f:Node,k:int,node:Node)
    requires 0<=k<=t.children@.len(), t.id==f.id, f.children@==t.children@.insert(k,node),
    ensures
        preorder_ids(t)==seq![t.id]+ids_span(t,0,k)+children_ids(t,k),
        preorder_ids(f)==seq![t.id]+ids_span(t,0,k)+preorder_ids(node)+children_ids(t,k),
{
    lemma_children_split(t,0,k);
    lemma_children_split(f,0,k);
    assert forall|j:int| #![trigger t.children@[j]] 0<=j<k implies preorder_ids(f.children@[j])==preorder_ids(t.children@[j]) by {
        assert(f.children@[j]==t.children@[j]);
    };
    lemma_span_same(f,t,0,k);
    assert forall|j:int| #![trigger t.children@[j]] k<=j<t.children@.len() implies f.children@[j+1]==t.children@[j] by {
        assert(f.children@[j+1]==t.children@[j]);
    };
    lemma_suffix_shift(f,t,k);
    assert(f.children@[k]==node);
    assert(children_ids(f,k)==preorder_ids(node)+children_ids(f,k+1));
    assert(preorder_ids(t)=~=seq![t.id]+ids_span(t,0,k)+children_ids(t,k));
    assert(preorder_ids(f)=~=seq![t.id]+ids_span(t,0,k)+preorder_ids(node)+children_ids(t,k));
}
proof fn lemma_spliced_witness(a:Seq<NodeId>,x:Seq<NodeId>,b:Seq<NodeId>)
    ensures spliced_ids(a+x+b,x,a+b),
{
    let rest=a+b;
    let k=a.len() as int;
    assert(rest.take(k)=~=a);
    assert(rest.skip(k)=~=b);
}
proof fn lemma_distinct_any(s:Seq<NodeId>,i:int,j:int)
    requires ids_distinct(s), 0<=i<s.len(), 0<=j<s.len(), i!=j,
    ensures s[i]!=s[j],
{
    if i>j { assert(s[j]!=s[i]); } else { assert(s[i]!=s[j]); }
}
proof fn lemma_distinct_removed(a:Seq<NodeId>,x:Seq<NodeId>,b:Seq<NodeId>)
    requires ids_distinct(a+x+b),
    ensures ids_distinct(a+b),
{
    let w=a+x+b;
    let s=a+b;
    assert forall|i:int,j:int| #![trigger s[i],s[j]] 0<=i<j<s.len() implies s[i]!=s[j] by {
        let wi=if i<a.len() { i } else { i+x.len() };
        let wj=if j<a.len() { j } else { j+x.len() };
        assert(s[i]==w[wi]);
        assert(s[j]==w[wj]);
        lemma_distinct_any(w,wi,wj);
    };
}
proof fn lemma_distinct_inserted(a:Seq<NodeId>,x:Seq<NodeId>,b:Seq<NodeId>)
    requires ids_distinct(a+b+x),
    ensures ids_distinct(a+x+b),
{
    let p=a+b+x;
    let s=a+x+b;
    assert forall|i:int,j:int| #![trigger s[i],s[j]] 0<=i<j<s.len() implies s[i]!=s[j] by {
        let pi=if i<a.len() { i } else if i<a.len()+x.len() { i+b.len() } else { i-x.len() };
        let pj=if j<a.len() { j } else if j<a.len()+x.len() { j+b.len() } else { j-x.len() };
        assert(s[i]==p[pi]);
        assert(s[j]==p[pj]);
        lemma_distinct_any(p,pi,pj);
    };
}
// The written node keeps its identity and its children, so the whole sequence stays.
// 被写入的节点保留身份与子节点，整个序列不变。
proof fn lemma_written_keeps_ids(o:Node,n:Node,id:NodeId,t:Node,f:Node)
    requires node_relocated(o,n,id,t,f), t.id==f.id, t.children==f.children,
    ensures preorder_ids(n)==preorder_ids(o),
{
    let w=lemma_relocated_ids(o,n,id,t,f);
    lemma_same_children_ids(t,f);
}
proof fn lemma_patch_keeps_ids(o:Node,n:Node,patch:ScenePatch,r:Result<(),SceneError>)
    requires patch_applied(o,n,patch,r), !(patch is Insert),
    ensures patch_keeps_ids(o,n,patch,r), ids_distinct(preorder_ids(o)) ==> ids_distinct(preorder_ids(n)),
{
    match patch {
        ScenePatch::Insert{..} => {}
        ScenePatch::UpdateSemantics{id,semantics} => {
            if node_holds(o,id) && r==patch_ok() {
                let (t,f)=choose|t:Node,f:Node| #![trigger node_relocated(o,n,id,t,f)] node_relocated(o,n,id,t,f) && (|t:Node,f:Node| f==(Node{semantics:semantics,..t}))(t,f);
                lemma_written_keeps_ids(o,n,id,t,f);
            }
        }
        ScenePatch::UpdateStyle{id,style} => {
            if node_holds(o,id) && r==patch_ok() {
                let (t,f)=choose|t:Node,f:Node| #![trigger node_relocated(o,n,id,t,f)] node_relocated(o,n,id,t,f) && (|t:Node,f:Node| f==(Node{style:style,..t}))(t,f);
                lemma_written_keeps_ids(o,n,id,t,f);
            }
        }
        ScenePatch::SetScroll{id,offset} => {
            if node_holds(o,id) && r==patch_ok() {
                let (t,f)=choose|t:Node,f:Node| #![trigger node_relocated(o,n,id,t,f)] node_relocated(o,n,id,t,f) && (|t:Node,f:Node| f==(Node{scroll:offset,..t}))(t,f);
                lemma_written_keeps_ids(o,n,id,t,f);
            }
        }
        ScenePatch::UpdateText{id,text} => {
            if node_holds(o,id) {
                let (t,f)=choose|t:Node,f:Node| #![trigger node_relocated(o,n,id,t,f)] node_relocated(o,n,id,t,f)
                    && if (|k:NodeKind| k is Text)(t.kind) { r==patch_ok() && f==(Node{kind:NodeKind::Text(text),..t}) } else { r==patch_err(SceneError::NotText(id)) && f==t };
                lemma_written_keeps_ids(o,n,id,t,f);
            }
        }
        ScenePatch::SetCanvas{id,canvas} => {
            if node_holds(o,id) {
                let (t,f)=choose|t:Node,f:Node| #![trigger node_relocated(o,n,id,t,f)] node_relocated(o,n,id,t,f)
                    && if (|k:NodeKind| k is Canvas)(t.kind) { r==patch_ok() && f==(Node{kind:NodeKind::Canvas(canvas),..t}) } else { r==patch_err(SceneError::NotCanvas(id)) && f==t };
                lemma_written_keeps_ids(o,n,id,t,f);
            }
        }
        ScenePatch::Remove{id} => {
            if o.id!=id {
                if node_below(o,id) {
                    let w=lemma_removed_ids(o,n,id);
                    lemma_spliced_witness(w.1,preorder_ids(w.0),w.2);
                    assert(spliced_ids(preorder_ids(o),preorder_ids(w.0),preorder_ids(n)));
                    if ids_distinct(preorder_ids(o)) { lemma_distinct_removed(w.1,preorder_ids(w.0),w.2); }
                } else {
                    lemma_same_ids(o,n);
                }
            }
        }
    }
}
proof fn lemma_insert_ok_ids(o:Node,n:Node,parent:NodeId,index:usize,node:Node)
    requires ids_distinct(preorder_ids(o)+preorder_ids(node)),
        patch_applied(o,n,ScenePatch::Insert{parent:parent,index:index,node:node},patch_ok()),
    ensures ids_distinct(preorder_ids(n)), spliced_ids(preorder_ids(n),preorder_ids(node),preorder_ids(o)),
{
    let written=|t:Node,f:Node| node_fields_same(t,f)
        && f.children@==t.children@.insert(if index<t.children@.len() {index as int} else {t.children@.len() as int},node);
    let (t,f)=choose|t:Node,f:Node| #![trigger node_relocated(o,n,parent,t,f)] node_relocated(o,n,parent,t,f) && written(t,f);
    let w=lemma_relocated_ids(o,n,parent,t,f);
    let k=if index<t.children@.len() {index as int} else {t.children@.len() as int};
    lemma_inserted_child(t,f,k,node);
    let x=preorder_ids(node);
    let a=w.0+seq![t.id]+ids_span(t,0,k);
    let b=children_ids(t,k)+w.1;
    assert(preorder_ids(o)=~=a+b);
    assert(preorder_ids(n)=~=a+x+b);
    lemma_spliced_witness(a,x,b);
    lemma_distinct_inserted(a,x,b);
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
    Remove {
        id: NodeId,
    },
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
        } => match find_node_mut(root, parent) {
            Some(parent_node) => {
                let ghost t=*parent_node;
                let len = parent_node.children.len();
                let index = if index < len { index } else { len };
                parent_node.children.insert(index, node);
                proof { assert(node_relocated(*old(root),*final(root),parent,t,*parent_node)); }
                Ok(())
            }
            None => Err(SceneError::NodeNotFound(parent)),
        },
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
            if root.id.0 == id.0 {
                return Err(SceneError::CannotRemoveRoot);
            }
            if remove_node(root, id) {
                Ok(())
            } else {
                Err(SceneError::NodeNotFound(id))
            }
        }
    }
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

/// Validates all descendants of an insertion before any follow-up patch can observe ambiguous
/// identities, then applies the patch. The set starts empty and is local to staging.
/// 插入前检查全部后代，避免后续补丁观察到歧义身份，再应用补丁；集合从空开始，仅在暂存阶段使用。
fn apply_checked_patch<T: IdentitySet>(root:&mut Node,patch:ScenePatch,ids:&mut T)->(result:Result<(), SceneError>)
        requires old(ids).seen()==Set::<NodeId>::empty(),
        ensures patch_checked(*old(root),*final(root),patch,result),
    {
    let ghost o=*root;
    let ghost original=patch;
    if let ScenePatch::Insert { node, .. } = &patch {
        let checked=check_insert_identities(&*root, node, ids);
        match checked {
            Ok(()) => { assert(ids_distinct(preorder_ids(o)+preorder_ids(*node))); }
            Err(error) => {
                proof {
                    let s=preorder_ids(o)+preorder_ids(*node);
                    if let SceneError::DuplicateNode(d)=error {
                        let k=choose|k:int| #![trigger s[k]] 0<=k<s.len() && s[k]==d && unrepeated(Set::<NodeId>::empty(),s.take(k))
                            && (Set::<NodeId>::empty().contains(d) || s.take(k).contains(d));
                        assert(s.contains(d));
                        let i=choose|i:int| 0<=i<s.take(k).len() && s.take(k)[i]==d;
                        assert(s[i]==d);
                        assert(!ids_distinct(s));
                        assert(s.contains(d) && error==SceneError::DuplicateNode(d));
                    }
                }
                return Err(error);
            }
        }
    }
    let result=Ok::<(),SceneError>(());
    proof {
        if let ScenePatch::Insert{parent,index,node}=original {
            if result is Ok { lemma_insert_ok_ids(o,*root,parent,index,node); }
        } else {
            lemma_patch_keeps_ids(o,*root,original,result);
        }
    }
    result
}

}
