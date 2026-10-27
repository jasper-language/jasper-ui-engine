//! Exact production eligibility predicate; no backend or callback model.
include!(concat!(env!("OUT_DIR"),"/actual.rs"));
pub fn check_activation(current:u64,expected:u64,enabled:bool) {
    let result=accepts_activation(current,expected,enabled);
    assert_eq!(result,current==expected && enabled);
    if result { assert!(enabled); assert_eq!(current,expected); }
}
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn activation_admission(current in any::<u64>(),expected in any::<u64>(),enabled in any::<bool>()) {
            check_activation(current,expected,enabled);
            check_activation(current,current,enabled);
        }
    }
    #[test]
    fn miri_activation_boundaries() {
        for current in [0,1,u64::MAX-1,u64::MAX] {
            for expected in [0,1,u64::MAX-1,u64::MAX] {
                check_activation(current,expected,false);check_activation(current,expected,true);
            }
        }
    }
}
#[cfg(kani)]
mod proofs {
    #[kani::proof]
    fn activation_admission() { super::check_activation(kani::any(),kani::any(),kani::any()); }
}

#[path="../../../src/native_adapter/component_contract.rs"]
mod component_contract;
pub fn check_component_support(button:bool,font_size:u16) {
    use component_contract::*;
    let appearance=TextAppearance {font_size_px:font_size,padding_px:u16::MAX,
        foreground:[0,127,255],background:[255,127,0]};
    for update in [ComponentUpdate::AccessibleLabel(None),ComponentUpdate::AccessibleLabel(Some("")),
        ComponentUpdate::AccessibleLabel(Some("中文😀")),ComponentUpdate::ButtonCaption("中文😀"),
        ComponentUpdate::ButtonDisabled(false),ComponentUpdate::ButtonDisabled(true),
        ComponentUpdate::Appearance(appearance),ComponentUpdate::Flow {direction:ContentFlow::Row,gap_px:u16::MAX},
        ComponentUpdate::Flow {direction:ContentFlow::Column,gap_px:0}] {
        let expected=match update {
            ComponentUpdate::ButtonCaption(_) | ComponentUpdate::ButtonDisabled(_) => button,
            ComponentUpdate::Appearance(_) => font_size!=0,
            _ => true,
        };
        assert_eq!(supports_component_update(button,update),expected);
    }
}
#[cfg(test)]
mod component_tests {
    use super::*;
    use proptest::prelude::*;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn component_support(button in any::<bool>(),font_size in any::<u16>()) {
            check_component_support(button,font_size);
        }
    }
    #[test]
    fn miri_component_boundaries() {
        for button in [false,true] { for size in [0,1,u16::MAX] { check_component_support(button,size); } }
    }
}
#[cfg(kani)]
mod component_proofs {
    #[kani::proof]
    #[kani::unwind(10)]
    fn component_support() { super::check_component_support(kani::any(),kani::any()); }
}

mod capacity { include!(concat!(env!("OUT_DIR"),"/capacity.rs")); }
pub fn check_scene_capacity(prepared:usize,pending:usize,incoming:usize) {
    let expected=(prepared as u128)+(pending as u128)+(incoming as u128)<=4096;
    assert_eq!(capacity::scene_work_fits(prepared,pending,incoming),expected);
}
#[cfg(test)]
mod capacity_tests {
    use super::*;use proptest::prelude::*;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn scene_capacity(a in any::<usize>(),b in any::<usize>(),c in any::<usize>()) {
            check_scene_capacity(a,b,c);check_scene_capacity(a%4097,b%4097,c%4097);
        }
    }
    #[test]
    fn miri_capacity_boundaries() {
        for a in [0,1,4095,4096,4097,usize::MAX] {
            for b in [0,1,4096,usize::MAX] {for c in [0,1,4096,usize::MAX] {check_scene_capacity(a,b,c);}}
        }
    }
}
#[cfg(kani)]
mod capacity_proofs {
    #[kani::proof]
    fn scene_capacity() {super::check_scene_capacity(kani::any(),kani::any(),kani::any());}
}

mod scene_gate {include!(concat!(env!("OUT_DIR"),"/scene_gate.rs"));}
pub fn check_scene_update(owner:u64,caller:u64,current:u64,expected:u64,next:u64,count:usize,open:bool) {
    let accepted=scene_gate::accepts_scene_update(owner,caller,current,expected,next,count,open);
    let model=owner==caller && open && current==expected && current.checked_add(1)==Some(next) && count>0 && count<=4096;
    assert_eq!(accepted,model);
}
#[cfg(test)]
mod scene_gate_tests {
    use super::*;use proptest::prelude::*;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn scene_update(owner in any::<u64>(),caller in any::<u64>(),current in any::<u64>(),expected in any::<u64>(),next in any::<u64>(),count in any::<usize>(),open in any::<bool>()) {
            check_scene_update(owner,caller,current,expected,next,count,open);
            check_scene_update(owner,owner,current,current,current.wrapping_add(1),count%4097,open);
        }
    }
    #[test]
    fn miri_scene_update_boundaries() {
        for version in [0,1,u64::MAX-1,u64::MAX] {for count in [0,1,4096,4097,usize::MAX] {for open in [false,true] {
            check_scene_update(10,10,version,version,version.wrapping_add(1),count,open);
            check_scene_update(10,11,version,version,version.wrapping_add(1),count,open);
        }}}
    }
}
#[cfg(kani)]
mod scene_gate_proofs {
    #[kani::proof]
    fn scene_update() {super::check_scene_update(kani::any(),kani::any(),kani::any(),kani::any(),kani::any(),kani::any(),kani::any());}
}

mod insert_index {include!(concat!(env!("OUT_DIR"),"/insert_index.rs"));}
pub fn check_insert_index(index:usize,children:usize) {
    let actual=insert_index::source_insert_index(index,children);
    assert!(actual<=children);assert_eq!(actual,if index<=children {index} else {children});
}
#[cfg(test)]
mod insert_index_tests {
    use super::*;use proptest::prelude::*;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn insert_index(index in any::<usize>(),children in any::<usize>()) {check_insert_index(index,children);}
    }
    #[test]
    fn miri_insert_index_boundaries() {
        for index in [0,1,4096,usize::MAX] {for children in [0,1,4096,usize::MAX] {check_insert_index(index,children);}}
    }
}
#[cfg(kani)]
mod insert_index_proofs {
    #[kani::proof]
    fn insert_index() {super::check_insert_index(kani::any(),kani::any());}
}

pub mod ancestry;

#[path="../../../src/native_adapter/cpu_budget.rs"]
mod cpu_budget;
pub fn check_cpu_frame(width:u32,height:u32,budget:usize) {
    let area=width as u128*height as u128*4;
    let expected=if width>0 && height>0 && width<=65535 && height<=65535
        && area<=budget as u128 && area<=u32::MAX as u128 {Some(area as usize)} else {None};
    assert_eq!(cpu_budget::cpu_frame_bytes(width,height,budget),expected);
}
#[cfg(test)]
mod cpu_frame_tests {
    use super::*;use proptest::prelude::*;
    proptest! {#![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn packed_cpu_admission(width in any::<u32>(),height in any::<u32>(),budget in any::<usize>()) {check_cpu_frame(width,height,budget);}
        #[test]
        fn dense_cpu_admission(width in 0u32..65537,height in 0u32..65537,budget in any::<u32>()) {check_cpu_frame(width,height,budget as usize);}
    }
    #[test]
    fn miri_cpu_frame_boundaries() {
        for width in [0,1,32767,32768,65535,65536,u32::MAX] {
            for height in [0,1,32767,32768,65535,65536,u32::MAX] {
                for budget in [0,4,u32::MAX as usize,usize::MAX] {check_cpu_frame(width,height,budget);}
            }
        }
        assert_eq!(cpu_budget::cpu_frame_bytes(1,1,4),Some(4));
        assert_eq!(cpu_budget::cpu_frame_bytes(32768,32768,usize::MAX),None);
    }
}

#[cfg(kani)]
mod cpu_frame_proofs {
    #[kani::proof]
    fn cpu_frame_admission() {super::check_cpu_frame(kani::any(),kani::any(),kani::any());}
}

pub mod semantic_tag {
    include!(concat!(env!("OUT_DIR"),"/semantic_tag.rs"));
    pub fn check(a:u8,b:u8) {
        let roles=[Role::Container,Role::Text,Role::Button,Role::TextField,Role::Checkbox,Role::Radio,
            Role::Switch,Role::Slider,Role::Image,Role::Link,Role::List,Role::ListItem,Role::Dialog,
            Role::Menu,Role::MenuItem,Role::ScrollView,Role::Unknown];
        let a=(a as usize)%roles.len();let b=(b as usize)%roles.len();
        assert_eq!(semantic_role_update_preserves_tag(roles[a],roles[b]),(a==2)==(b==2));
    }
    #[cfg(test)] mod tests {
        use super::*;use proptest::prelude::*;
        proptest! {#[test] fn role_tag_property(a in any::<u8>(),b in any::<u8>()) {check(a,b);}}
        #[test] fn miri_role_tags() {for a in 0..17 {for b in 0..17 {check(a,b);}}}
    }
    #[cfg(kani)] mod proofs {#[kani::proof] fn semantic_role_tags() {super::check(kani::any(),kani::any());}}
}
