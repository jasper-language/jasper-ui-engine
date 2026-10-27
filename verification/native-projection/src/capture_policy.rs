use jasper_ui_input_identity_verification::{UiInputContext,NodeId,accepts_ui_display_snapshot,matches_ui_view_owner};
include!(concat!(env!("OUT_DIR"),"/actual_capture_policy.rs"));
pub fn check(ids:[u64;10],deliver:bool,open:bool) {
    let current=UiInputContext {target:NodeId(ids[0]),target_generation:ids[1],document:ids[2],revision:ids[3]};
    let origin=UiInputContext {target:NodeId(ids[6]),target_generation:ids[7],document:ids[8],revision:ids[9]};
    let actual=capture_cancel_route(ids[4],current,ids[5],origin,deliver,open);
    let expected=if ids[4]!=ids[5] || ids[0]!=ids[6] || ids[1]!=ids[7] || ids[2]!=ids[8] {CaptureCancelRoute::Reject}
        else if !deliver || !open {CaptureCancelRoute::HostOnly}
        else if ids[3]==ids[9] {CaptureCancelRoute::Current} else {CaptureCancelRoute::Successor};
    assert_eq!(actual,expected);
}
pub fn fuzz(data:&[u8;82]) {
    let mut ids=[0;10];for (i,id) in ids.iter_mut().enumerate() {*id=u64::from_le_bytes(data[i*8..i*8+8].try_into().unwrap());}
    let deliver=data[80]&1!=0;let open=data[81]&1!=0;check(ids,deliver,open);
    ids[5]=ids[4];ids[6]=ids[0];ids[7]=ids[1];ids[8]=ids[2];check(ids,deliver,open);
    ids[9]=ids[3];check(ids,deliver,open);
}
#[cfg(test)]
mod tests {
    use super::*;use proptest::prelude::*;
    proptest! {#![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn route_fields(ids in any::<[u64;10]>(),deliver in any::<bool>(),open in any::<bool>()) {check(ids,deliver,open);}
        #[test]
        fn route_dense(data in any::<[u8;82]>()) {fuzz(&data);}
    }
    #[test]
    fn miri_capture_policy() {for word in [0,1,u64::MAX] {for deliver in [false,true] {for open in [false,true] {
        let ids=[word;10];check(ids,deliver,open);
        for i in 0..10 {let mut ids=ids;ids[i]^=1;check(ids,deliver,open);}
    }}}}
}
#[cfg(kani)]
mod proofs {
    #[kani::proof]
    fn capture_cancel_policy() {super::check(kani::any(),kani::any(),kani::any());}
}
