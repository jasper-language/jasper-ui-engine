use super::{BaseDocument,blitz_dom,ViewSlot,NodeId};
use std::collections::{BTreeMap,BTreeSet};
mod native_scene {pub(crate) use super::NativeScene;}
include!(concat!(env!("OUT_DIR"),"/actual_scene.rs"));
pub fn check(version:u64,owner:u64,caller:u64,closed:bool) {
    // Unread map contents are universally framed by Verus; runtime fixture keeps them empty.
    let mut view=SceneHostView {inner:NativeScene {document:ViewSlot::new(BaseDocument),owner,scene_version:version,node_count:7,
        parents:BTreeMap::new(),children:BTreeMap::new(),displayed:BTreeMap::new(),semantics:BTreeMap::new(),nodes:BTreeMap::new(),frame_dirty:true,frame_cache:None,families:BTreeSet::new()}};
    if closed {assert!(view.inner.document.close());}
    assert_eq!(view.close(caller),owner==caller && !closed);
    assert_eq!(view.inner.document.is_open(),owner!=caller && !closed);
    assert_eq!(view.inner.owner,owner);assert_eq!(view.inner.scene_version,version);assert_eq!(view.inner.node_count,7);
    assert_eq!(view.inner.parents.len(),0);assert_eq!(view.inner.children.len(),0);assert_eq!(view.inner.displayed.len(),0);
    assert_eq!(view.inner.semantics.len(),0);assert_eq!(view.inner.nodes.len(),0);
    if owner==caller {assert!(!view.close(caller));}
    // Wrapper/map destruction is outside the borrowed close method's logical scope.
    std::mem::forget(view);
}
pub fn fuzz(data:&[u8;25]) {
    let word=|i:usize|u64::from_le_bytes(data[i..i+8].try_into().unwrap());check(word(0),word(8),word(16),data[24]&1!=0);
}
#[cfg(test)]
mod tests {
    use super::*;use proptest::prelude::*;
    proptest! {#![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn actual_scene_close(version in any::<u64>(),owner in any::<u64>(),caller in any::<u64>(),closed in any::<bool>()) {
            check(version,owner,caller,closed);check(version,owner,owner,closed);
        }
    }
    #[test]
    fn miri_scene_close() {for owner in [0,1,u64::MAX] {for closed in [false,true] {check(u64::MAX,owner,owner,closed);check(0,owner,owner^1,closed);}}}
}
#[cfg(kani)]
mod proofs {
    #[kani::proof]
    fn scene_host_close() {super::check(kani::any(),kani::any(),kani::any(),kani::any());}
}

pub fn check_visibility(entries:&[(u64,Option<u64>,Option<bool>,Option<bool>)],start:u64) {
    let mut view=SceneHostView {inner:NativeScene {document:ViewSlot::new(BaseDocument),owner:1,scene_version:0,node_count:entries.len(),
        parents:BTreeMap::new(),children:BTreeMap::new(),displayed:BTreeMap::new(),semantics:BTreeMap::new(),nodes:BTreeMap::new(),frame_dirty:true,frame_cache:None,families:BTreeSet::new()}};
    for (id,parent,displayed,hidden) in entries {
        view.inner.parents.insert(NodeId(*id),parent.map(NodeId));
        match displayed {Some(value)=>{view.inner.displayed.insert(NodeId(*id),*value);},None=>{view.inner.displayed.remove(&NodeId(*id));}}
        match hidden {Some(value)=>{view.inner.semantics.insert(NodeId(*id),Semantics {hidden:*value,..Default::default()});},None=>{view.inner.semantics.remove(&NodeId(*id));}}
    }
    let actual=view.inner.source_visible(NodeId(start));let mut seen=Vec::new();let mut current=Some(start);
    let expected=loop {
        let node=match current {Some(node)=>node,None=>break true};
        if seen.contains(&node) {break false;}seen.push(node);
        let Some((_,parent,displayed,hidden))=entries.iter().rev().find(|entry|entry.0==node) else {break false;};
        if *displayed!=Some(true) || *hidden!=Some(false) {break false;}
        current=*parent;
    };
    assert_eq!(actual,expected);
}
pub fn visibility_fuzz(data:&[u8;33]) {
    let flag=|v:u8|match v%3 {0=>None,1=>Some(false),_=>Some(true)};
    let entries:Vec<_>=(0..8).filter(|i|data[*i]&1!=0).map(|i|(i as u64,
        if data[i+8]%11==10 {None} else {Some((data[i+8]%10) as u64)},flag(data[i+16]),flag(data[i+24]))).collect();
    check_visibility(&entries,(data[32]%10) as u64);
}
#[cfg(test)]
mod visibility_tests {
    use super::*;use proptest::prelude::*;
    proptest! {#![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn actual_visibility(entries in prop::collection::vec((0u64..10,prop::option::of(0u64..10),prop::option::of(any::<bool>()),prop::option::of(any::<bool>())),0..16),start in 0u64..10) {
            check_visibility(&entries,start);
        }
        #[test]
        fn dense_visibility(data in any::<[u8;33]>()) {visibility_fuzz(&data);}
    }
    #[test]
    fn miri_visibility() {
        check_visibility(&[(0,None,Some(true),Some(false))],0);
        check_visibility(&[(0,Some(1),Some(true),Some(false)),(1,Some(0),Some(true),Some(false))],0);
        check_visibility(&[(0,None,Some(true),Some(true))],0);check_visibility(&[(0,None,None,Some(false))],0);
        for x in 0..16 {visibility_fuzz(&[x;33]);}
    }
}

#[cfg(kani)]
mod visibility_proofs {
    #[kani::proof]
    #[kani::unwind(3)]
    fn one_node_visibility() {
        use super::*;
        let parent_present:bool=kani::any();let display_present:bool=kani::any();let semantic_present:bool=kani::any();
        let shown:bool=kani::any();let hidden:bool=kani::any();let cycle:bool=kani::any();
        let mut view=SceneHostView {inner:NativeScene {document:ViewSlot::new(BaseDocument),owner:1,scene_version:0,node_count:1,
            parents:BTreeMap::new(),children:BTreeMap::new(),displayed:BTreeMap::new(),semantics:BTreeMap::new(),nodes:BTreeMap::new(),frame_dirty:true,frame_cache:None,families:BTreeSet::new()}};
        if parent_present {view.inner.parents.insert(NodeId(0),if cycle {Some(NodeId(0))} else {None});}
        if display_present {view.inner.displayed.insert(NodeId(0),shown);}
        if semantic_present {view.inner.semantics.insert(NodeId(0),Semantics {hidden,..Default::default()});}
        assert_eq!(view.inner.source_visible(NodeId(0)),parent_present && display_present && semantic_present && shown && !hidden && !cycle);
        std::mem::forget(view); // Borrowed predicate does not own map destruction.
    }
}

// The executable helper is extracted from production, using real standard maps.
pub fn check_hidden_ancestry(entries:&[(u64,Option<u64>,Option<bool>)],start:Option<u64>) {
    let mut parents=BTreeMap::new();let mut semantics=BTreeMap::new();
    for (id,parent,hidden) in entries {
        parents.insert(NodeId(*id),parent.map(NodeId));
        match hidden {Some(hidden)=>{semantics.insert(NodeId(*id),Semantics {hidden:*hidden,..Default::default()});},
            None=>{semantics.remove(&NodeId(*id));}}
    }
    let actual=source_hidden_ancestry(&parents,&semantics,start.map(NodeId));
    // Independent producer-vector oracle: newest occurrence wins, visited list detects cycles.
    let mut current=start;let mut seen=Vec::new();let mut any_hidden=false;
    let expected=loop {
        let Some(id)=current else {break Some(any_hidden);};
        if seen.contains(&id) {break None;}seen.push(id);
        let Some((_,parent,Some(hidden)))=entries.iter().rev().find(|entry|entry.0==id) else {break None;};
        any_hidden |= *hidden;current=*parent;
    };
    assert_eq!(actual,expected);
}
pub fn hidden_fuzz(data:&[u8;25]) {
    let entries:Vec<_>=(0..8).filter(|i|data[*i]&1!=0).map(|i|(i as u64,
        if data[i+8]%11==10 {None} else {Some((data[i+8]%10) as u64)},
        match data[i+16]%3 {0=>None,1=>Some(false),_=>Some(true)})).collect();
    check_hidden_ancestry(&entries,if data[24]%11==10 {None} else {Some((data[24]%10) as u64)});
}
#[cfg(test)]
mod hidden_tests {
    use super::*;use proptest::prelude::*;
    proptest! {#![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn actual_hidden_ancestry(entries in prop::collection::vec((0u64..10,prop::option::of(0u64..10),prop::option::of(any::<bool>())),0..16),start in prop::option::of(0u64..10)) {
            check_hidden_ancestry(&entries,start);
        }
        #[test]
        fn dense_hidden_ancestry(data in any::<[u8;25]>()) {hidden_fuzz(&data);}
    }
    #[test]
    fn miri_hidden_ancestry() {
        check_hidden_ancestry(&[],None);check_hidden_ancestry(&[],Some(0));
        check_hidden_ancestry(&[(0,Some(1),Some(false)),(1,None,Some(true))],Some(0));
        check_hidden_ancestry(&[(0,Some(1),Some(true)),(1,None,Some(false))],Some(0));
        check_hidden_ancestry(&[(0,Some(1),Some(false)),(1,Some(0),Some(false))],Some(0));
        check_hidden_ancestry(&[(0,None,None)],Some(0));
        for x in 0..16 {hidden_fuzz(&[x;25]);}
    }
}

#[cfg(kani)]
mod hidden_proofs {
    #[kani::proof]
    #[kani::unwind(3)]
    fn one_node_hidden_ancestry() {
        use super::*;
        let start:bool=kani::any();let present:bool=kani::any();let semantic:bool=kani::any();
        let cycle:bool=kani::any();let hidden:bool=kani::any();
        let mut parents=BTreeMap::new();let mut semantics=BTreeMap::new();
        if present {parents.insert(NodeId(0),if cycle {Some(NodeId(0))} else {None});}
        if semantic {semantics.insert(NodeId(0),Semantics {hidden,..Default::default()});}
        let expected=if !start {Some(false)} else if present && semantic && !cycle {Some(hidden)} else {None};
        assert_eq!(source_hidden_ancestry(&parents,&semantics,if start {Some(NodeId(0))} else {None}),expected);
        // Borrowed helper does not own/destruct these maps; no safety checks are disabled.
        std::mem::forget(parents);std::mem::forget(semantics);
    }
}

pub fn check_snapshot_identity(version:u64,count:usize,owner:u64,caller:u64,closed:bool) {
    // Maps are never accessed by this method; Verus frames arbitrary map payloads.
    let mut view=NativeScene {document:ViewSlot::new(BaseDocument),owner,scene_version:version,node_count:count,
        parents:BTreeMap::new(),children:BTreeMap::new(),displayed:BTreeMap::new(),semantics:BTreeMap::new(),nodes:BTreeMap::new(),frame_dirty:true,frame_cache:None,families:BTreeSet::new()};
    if closed {assert!(view.document.close());}
    let expected=if owner==caller && !closed {Some((version,count))} else {None};
    assert_eq!(view.snapshot_identity(caller),expected);assert_eq!(view.snapshot_identity(caller),expected);
    assert_eq!((view.owner,view.scene_version,view.node_count),(owner,version,count));
    assert_eq!(view.document.is_open(),!closed);
    let public=SceneHostView {inner:view};
    assert_eq!(public.snapshot_identity(caller),expected);
    assert_eq!(public.snapshot_identity(caller),expected);
    std::mem::forget(public);
}
pub fn snapshot_identity_fuzz(data:&[u8;33]) {
    let word=|i|u64::from_le_bytes(data[i..i+8].try_into().unwrap());
    check_snapshot_identity(word(0),word(8) as usize,word(16),word(24),data[32]&1!=0);
    check_snapshot_identity(word(0),word(8) as usize,word(16),word(16),data[32]&1!=0);
}
#[cfg(test)]
mod snapshot_identity_tests {
    use super::*;use proptest::prelude::*;
    proptest! {#[test] fn actual_snapshot_identity(data in any::<[u8;33]>()) {snapshot_identity_fuzz(&data);}}
    #[test] fn miri_snapshot_identity() {for v in [0,1,u64::MAX] {for closed in [false,true] {
        check_snapshot_identity(v,usize::MAX,v,v,closed);check_snapshot_identity(v,0,v,v^1,closed);
    }}}
}
#[cfg(kani)]
mod snapshot_identity_proofs {
    #[kani::proof] fn scene_snapshot_identity() {
        super::check_snapshot_identity(kani::any(),kani::any(),kani::any(),kani::any(),kani::any());
    }
}
