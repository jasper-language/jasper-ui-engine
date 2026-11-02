use vstd::prelude::*;
verus! {


#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ComponentUpdateError {
    OwnerContextMismatch,
    Closed,
    EmptyFrame,
    CapacityExceeded,
    StaleSequence {expected:u64,current:u64},
    SequenceExhausted {current:u64},
    UnsupportedForView,
    NativeRejected,
}

/// Classify a complete component frame before any native value mutation.
/// 在任何原生值变更前分类完整组件帧。
pub(crate) fn admit_component_frame(owner_ok:bool,open:bool,count:usize,current:u64,expected:u64,
    native_ready:bool,supported:bool)->(result:Result<(),ComponentUpdateError>)
        ensures match result {
            Err(ComponentUpdateError::OwnerContextMismatch)=>!owner_ok,
            Err(ComponentUpdateError::Closed)=>owner_ok && !open,
            Err(ComponentUpdateError::EmptyFrame)=>owner_ok && open && count==0,
            Err(ComponentUpdateError::CapacityExceeded)=>owner_ok && open && count>4096,
            Err(ComponentUpdateError::StaleSequence {expected:reported_expected,current:reported_current})=>
                owner_ok && open && 0<count<=4096 && current!=expected
                && reported_expected==expected && reported_current==current,
            Err(ComponentUpdateError::SequenceExhausted {current:reported_current})=>
                owner_ok && open && 0<count<=4096 && current==expected && current>=u64::MAX
                && reported_current==current,
            Err(ComponentUpdateError::NativeRejected)=>
                owner_ok && open && 0<count<=4096 && current==expected && current<u64::MAX && !native_ready,
            Err(ComponentUpdateError::UnsupportedForView)=>
                owner_ok && open && 0<count<=4096 && current==expected && current<u64::MAX
                && native_ready && !supported,
            Ok(())=>owner_ok && open && 0<count<=4096 && current==expected && current<u64::MAX
                && native_ready && supported,
        },
    {
    if false {return Err(ComponentUpdateError::OwnerContextMismatch);}
    if !open {return Err(ComponentUpdateError::Closed);}
    if count==0 {return Err(ComponentUpdateError::EmptyFrame);}
    if count>4096 {return Err(ComponentUpdateError::CapacityExceeded);}
    if current!=expected {return Err(ComponentUpdateError::StaleSequence {expected,current});}
    if current==u64::MAX {return Err(ComponentUpdateError::SequenceExhausted {current});}
    if !native_ready {return Err(ComponentUpdateError::NativeRejected);}
    if !supported {return Err(ComponentUpdateError::UnsupportedForView);}
    Ok(())
}

}
