//! Exact native getter/field declaration with opaque unused third-party payloads.
//! 真实查询方法和字段声明；未读取的第三方载荷仅作不透明占位，不声称验证后端。
#![allow(dead_code)]
use jasper_ui_input_identity_verification::{UiCompositionSlot, UiInputContext, NodeId, matches_ui_view_owner, UiInputBinding};
#[path = "../../../src/native_adapter/view_slot.rs"]
mod view_slot;
use view_slot::ViewSlot;
// This is an explicitly abstract backend payload, not a mock renderer or ABI model.
struct BaseDocument;
mod blitz_dom { pub struct NodeId; }
mod native_view {pub(crate) use super::NativeTextView;}
include!(concat!(env!("OUT_DIR"), "/actual.rs"));

pub fn check_projection(version: u64, close: bool) {
    let mut view = NativeTextView {
        document: ViewSlot::new(BaseDocument), preedit: UiCompositionSlot::new(),
        component_version: version, session: 1,
        context: UiInputContext { target: NodeId(2), target_generation: 3, document: 4, revision: 5 },
        content_node: blitz_dom::NodeId, text_node: blitz_dom::NodeId,
        text_element: blitz_dom::NodeId, preedit_node: blitz_dom::NodeId,
        preedit_element: blitz_dom::NodeId,
    };
    assert_eq!(view.component_version(), Some(version));
    assert_eq!(view.component_version(), Some(version));
    if close {
        assert!(view.document.close());
        assert_eq!(view.component_version(), None);
        assert!(!view.document.close());
        assert_eq!(view.component_version(), None);
    } else {
        assert_eq!(view.component_version(), Some(version));
    }
    assert_eq!(view.component_version, version);
}
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn stored_version_and_retirement(version in any::<u64>(), close in any::<bool>()) {
            check_projection(version, close);
        }
    }
    #[test]
    fn miri_projection_boundaries() {
        for version in [0, 1, u64::MAX-1, u64::MAX] {
            check_projection(version, false); check_projection(version, true);
        }
    }
}
#[cfg(kani)]
mod proofs {
    #[kani::proof]
    fn native_version_projection() { super::check_projection(kani::any(), kani::any()); }
}

pub fn check_public_projection(version:u64,ids:[u64;10],close:bool) {
    let current=UiInputContext {target:NodeId(ids[0]),target_generation:ids[1],document:ids[2],revision:ids[3]};
    let incoming=UiInputContext {target:NodeId(ids[6]),target_generation:ids[7],document:ids[8],revision:ids[9]};
    let mut view=TextDisplayView {inner:NativeTextView {
        document:ViewSlot::new(BaseDocument),preedit:UiCompositionSlot::new(),component_version:version,
        session:ids[4],context:current,content_node:blitz_dom::NodeId,text_node:blitz_dom::NodeId,
        text_element:blitz_dom::NodeId,preedit_node:blitz_dom::NodeId,preedit_element:blitz_dom::NodeId,
    }};
    if close {assert!(view.inner.document.close());}
    let expected=if !close && ids[4]==ids[5] && ids[0]==ids[6] && ids[1]==ids[7] && ids[2]==ids[8] && ids[3]==ids[9] {Some(version)} else {None};
    assert_eq!(view.update_version(ids[5],incoming),expected);
    assert_eq!(view.update_version(ids[5],incoming),expected);
    assert_eq!(view.inner.component_version,version);
}
pub fn public_fuzz(data:&[u8;89]) {
    let word=|i:usize|u64::from_le_bytes(data[i..i+8].try_into().unwrap());
    let mut ids=[0;10];for (i,id) in ids.iter_mut().enumerate() {*id=word(8+i*8);}
    check_public_projection(word(0),ids,data[88]&1!=0);
}
#[cfg(test)]
mod public_tests {
    use super::*;use proptest::prelude::*;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn actual_public_projection(version in any::<u64>(),mut ids in any::<[u64;10]>(),close in any::<bool>()) {
            check_public_projection(version,ids,close);
            ids[5]=ids[4];ids[6]=ids[0];ids[7]=ids[1];ids[8]=ids[2];ids[9]=ids[3];
            check_public_projection(version,ids,close);
            for i in 5..10 {let mut mismatch=ids;mismatch[i]^=1;check_public_projection(version,mismatch,close);}
        }
    }
    #[test]
    fn miri_public_projection() {
        for version in [0,1,u64::MAX] {for close in [false,true] {check_public_projection(version,[u64::MAX;10],close);}}
    }
}
#[cfg(kani)]
mod public_proofs {
    #[kani::proof]
    fn public_version_projection() {super::check_public_projection(kani::any(),kani::any(),kani::any());}
}

pub fn check_public_close(version:u64,ids:[u64;10],closed:bool,active:bool) {
    let current=UiInputContext {target:NodeId(ids[0]),target_generation:ids[1],document:ids[2],revision:ids[3]};
    let incoming=UiInputContext {target:NodeId(ids[6]),target_generation:ids[7],document:ids[8],revision:ids[9]};
    let mut binding=UiInputBinding::new(ids[4],1);let ticket=binding.bind(ids[4],current).unwrap();
    let mut preedit=UiCompositionSlot::new();if active {assert!(preedit.begin(&binding,ids[4],ticket).is_some());}
    let mut view=TextDisplayView {inner:NativeTextView {document:ViewSlot::new(BaseDocument),preedit,
        component_version:version,session:ids[4],context:current,content_node:blitz_dom::NodeId,text_node:blitz_dom::NodeId,
        text_element:blitz_dom::NodeId,preedit_node:blitz_dom::NodeId,preedit_element:blitz_dom::NodeId}};
    if closed {assert!(view.inner.document.close());}
    let allowed=ids[4]==ids[5] && ids[0]==ids[6] && ids[1]==ids[7] && ids[2]==ids[8] && ids[3]==ids[9];
    assert_eq!(view.close(ids[5],incoming),allowed && !closed);
    assert_eq!(view.inner.document.is_open(),!allowed && !closed);
    assert_eq!(view.inner.preedit.current_token().is_some(),!allowed && active);
    assert_eq!(view.inner.component_version,version);assert_eq!(view.inner.session,ids[4]);
    if allowed {assert!(view.inner.preedit.begin(&binding,ids[4],ticket).is_none());assert!(!view.close(ids[5],incoming));}
    assert!(binding.accepts(ids[4],ticket)); // View retirement is not external input closure.
}
pub fn close_fuzz(data:&[u8;89]) {
    let word=|i:usize|u64::from_le_bytes(data[i..i+8].try_into().unwrap());
    let mut ids=[0;10];for (i,id) in ids.iter_mut().enumerate() {*id=word(8+i*8);}
    check_public_close(word(0),ids,data[88]&1!=0,data[88]&2!=0);
}
#[cfg(test)]
mod close_tests {
    use super::*;use proptest::prelude::*;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn actual_public_close(version in any::<u64>(),mut ids in any::<[u64;10]>(),closed in any::<bool>(),active in any::<bool>()) {
            check_public_close(version,ids,closed,active);
            ids[5]=ids[4];ids[6]=ids[0];ids[7]=ids[1];ids[8]=ids[2];ids[9]=ids[3];check_public_close(version,ids,closed,active);
            for i in 5..10 {let mut mismatch=ids;mismatch[i]^=1;check_public_close(version,mismatch,closed,active);}
        }
    }
    #[test]
    fn miri_public_close() {for closed in [false,true] {for active in [false,true] {check_public_close(u64::MAX,[u64::MAX;10],closed,active);}}}
}
#[cfg(kani)]
mod close_proofs {
    #[kani::proof]
    fn public_close_projection() {super::check_public_close(kani::any(),kani::any(),kani::any(),kani::any());}
}

pub mod scene_close;

pub mod capture_policy;

pub fn check_host_context(ids:[u64;10]) {
    let context=UiInputContext {target:NodeId(ids[0]),target_generation:ids[1],document:ids[2],revision:ids[3]};
    let expected=UiInputContext {target:NodeId(ids[6]),target_generation:ids[7],document:ids[8],revision:ids[9]};
    let view=NativeTextView {document:ViewSlot::new(BaseDocument),preedit:UiCompositionSlot::new(),component_version:0,
        session:ids[4],context,content_node:blitz_dom::NodeId,text_node:blitz_dom::NodeId,text_element:blitz_dom::NodeId,
        preedit_node:blitz_dom::NodeId,preedit_element:blitz_dom::NodeId};
    assert_eq!(view.host_context_matches(ids[5],expected),
        (ids[4],ids[0],ids[1],ids[2],ids[3])==(ids[5],ids[6],ids[7],ids[8],ids[9]));
}
pub fn host_context_fuzz(data:&[u8;80]) {
    let mut ids=[0;10];for (i,id) in ids.iter_mut().enumerate() {*id=u64::from_le_bytes(data[i*8..i*8+8].try_into().unwrap());}
    check_host_context(ids);ids[5]=ids[4];ids[6]=ids[0];ids[7]=ids[1];ids[8]=ids[2];ids[9]=ids[3];check_host_context(ids);
}
#[cfg(test)]
mod host_context_tests {
    use super::*;use proptest::prelude::*;
    proptest! {#![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn actual_host_context(data in any::<[u8;80]>()) {host_context_fuzz(&data);}
    }
    #[test]
    fn miri_host_context() {for word in [0,1,u64::MAX] {
        let ids=[word;10];check_host_context(ids);
        for i in 0..10 {let mut ids=ids;ids[i]^=1;check_host_context(ids);}
    }}
}
#[cfg(kani)]
mod host_context_proofs {
    #[kani::proof]
    fn native_host_context() {super::check_host_context(kani::any());}
}
