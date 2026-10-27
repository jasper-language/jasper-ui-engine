use jasper_ui_input_identity_verification::{AdmittedUiInput,KeyboardRouter};
fn bad<'a>(router: &'a mut KeyboardRouter, admitted: &'a AdmittedUiInput<'a,u64>) {
    let payload=router.route_admitted(admitted).unwrap();
    router.take_focus();
    assert_eq!(*payload,7);
}
fn main() {}
