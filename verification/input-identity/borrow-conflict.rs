use jasper_ui_input_identity_verification::{UiInputBinding,UiInputContext,NodeId};
fn main() {
    let mut binding=UiInputBinding::new(1,2);
    let current=UiInputContext{target:NodeId(3),target_generation:1,document:4,revision:5};
    let ticket=binding.bind(1,current).unwrap();
    let payload=7;
    let admitted=binding.admit(1,ticket,&payload).unwrap();
    binding.retire();
    assert_eq!(*admitted.payload(),7);
}
