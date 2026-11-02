//! Public structured component-frame rejection contract.
//! 公共结构化组件帧拒绝契约。
#![cfg(feature="native-blitz-pending")]
use jasper_ui_engine::native::host::{TextDisplayView,UiInputContext};
use jasper_ui_engine::native::{ComponentUpdate,ComponentUpdateError,TextAppearance};
use jasper_ui_engine::scene::NodeId;

const OWNER:u64=37;
const CONTEXT:UiInputContext=UiInputContext {target:NodeId(9),target_generation:1,document:2,revision:3};

fn font()->Vec<u8> {
    let path=std::env::var("JASPER_UI_TEST_FONT").expect("injected Latin font");
    std::fs::read(path).expect("font readable")
}

#[test]
fn public_component_update_errors_distinguish_rejection_stages() {
    let mut view=TextDisplayView::new(OWNER,CONTEXT,&font(),"component",160,80);
    assert_eq!(view.try_update(OWNER+1,CONTEXT,0,&[ComponentUpdate::AccessibleLabel(None)]),
        Err(ComponentUpdateError::OwnerContextMismatch));
    assert_eq!(view.try_update(OWNER,CONTEXT,0,&[]),Err(ComponentUpdateError::EmptyFrame));
    let oversized=vec![ComponentUpdate::AccessibleLabel(None);4097];
    assert_eq!(view.try_update(OWNER,CONTEXT,0,&oversized),Err(ComponentUpdateError::CapacityExceeded));
    assert_eq!(view.try_update(OWNER,CONTEXT,3,&[ComponentUpdate::AccessibleLabel(None)]),
        Err(ComponentUpdateError::StaleSequence {expected:3,current:0}));
    assert_eq!(view.try_update(OWNER,CONTEXT,0,&[ComponentUpdate::ButtonCaption("not a button")]),
        Err(ComponentUpdateError::UnsupportedForView));
    assert_eq!(view.update_version(OWNER,CONTEXT),Some(0));

    let appearance=TextAppearance {font_size_px:18,padding_px:4,foreground:[255,255,255],background:[0,0,0]};
    assert_eq!(view.try_update(OWNER,CONTEXT,0,&[ComponentUpdate::Appearance(appearance),
        ComponentUpdate::AccessibleLabel(Some("Updated"))]),Ok(()));
    assert_eq!(view.update_version(OWNER,CONTEXT),Some(1));
    assert_eq!(view.semantics(OWNER,CONTEXT).unwrap().name,"Updated");

    assert!(view.close(OWNER,CONTEXT));
    assert_eq!(view.try_update(OWNER,CONTEXT,1,&[ComponentUpdate::AccessibleLabel(None)]),
        Err(ComponentUpdateError::Closed));
}
