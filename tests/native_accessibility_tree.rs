//! Native accessibility tree through the root entry: visible nodes in source order with
//! Framework semantics, node_bounds geometry, scene focus and the actions the engine admits.
//! 根入口原生无障碍树：可见节点按源顺序，含 Framework 语义、node_bounds 几何、场景焦点与引擎准入的动作。
#![cfg(feature="native-blitz-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::native::host::{AccessActions,AccessibilityTree};
use jasper_ui_engine::scene::{Node,NodeId,Scene,ScenePatch};
use jasper_ui_engine::semantics::{Role,Semantics};
use jasper_ui_engine::style::{Color,Display,Length,Style};

const W:u32=240;
const H:u32=200;
const OWNER:u64=21;

fn font()->Vec<u8> {
    let path=std::env::var("JASPER_UI_TEST_FONT").expect("JASPER_UI_TEST_FONT must name an injected Latin font");
    std::fs::read(path).expect("injected font readable")
}

fn n(id:u64)->NodeId {NodeId(id)}

fn sized(width:f32,height:f32)->Style {
    Style::default().size(Length::Px(width),Length::Px(height))
}

fn button(id:u64,label:&str,disabled:bool)->Node {
    Node::container(n(id)).with_style(sized(80.0,30.0).background(Color::rgb(40,80,160)))
        .with_semantics(Semantics::default().role(Role::Button).label(label).focusable(true).disabled(disabled))
}

/// 1 root ─ 2 title text, 3 OK button (4 its caption), 5 hidden group (6 button),
/// 7 undisplayed group (8 text), 9 disabled button, 10 focusable text field.
fn scene()->Scene {
    Scene::new(Node::container(n(1)).with_style(sized(W as f32,H as f32).background(Color::rgb(18,22,30)))
        .with_semantics(Semantics::default().label("Window"))
        .child(Node::text(n(2),"Title").with_style(Style::default().text(16.0,Color::WHITE))
            .with_semantics(Semantics::default().role(Role::Text).label("Title")))
        .child(button(3,"OK",false).child(Node::text(n(4),"OK").with_style(Style::default().text(14.0,Color::WHITE))))
        .child(Node::container(n(5)).with_semantics(Semantics::default().hidden(true)).child(button(6,"Hidden",false)))
        .child(Node::container(n(7)).with_style(Style::default().display(Display::None))
            .child(Node::text(n(8),"Gone")))
        .child(button(9,"Off",true))
        .child(Node::container(n(10)).with_style(sized(120.0,24.0))
            .with_semantics(Semantics::default().role(Role::TextField).label("Name").description("Your name")
                .value("abc").focusable(true))))
}

fn ids(tree:&AccessibilityTree)->Vec<u64> {tree.nodes.iter().map(|node|node.node.0).collect()}
fn node(tree:&AccessibilityTree,id:u64)->&jasper_ui_engine::native::host::AccessibilityNode {
    tree.nodes.iter().find(|node|node.node==n(id)).unwrap_or_else(||panic!("node {id} missing from {:?}",ids(tree)))
}

#[test]
fn visible_nodes_carry_semantics_geometry_and_order() {
    let source=scene();let v=source.version();let f=font();
    let engine=Engine::new(OWNER,&source,&f,W,H).expect("native import");
    let tree=engine.accessibility_tree(OWNER,v).expect("tree");
    assert_eq!(tree.scene_version,v);
    // Depth-first source order; the hidden group (5, 6) and undisplayed group (7, 8) are pruned.
    assert_eq!(ids(&tree),vec![1,2,3,4,9,10]);
    assert_eq!(node(&tree,1).children,vec![n(2),n(3),n(9),n(10)]);
    assert_eq!(node(&tree,3).children,vec![n(4)]);
    assert!(node(&tree,4).children.is_empty());
    // Framework semantics are passed through unchanged.
    let field=node(&tree,10);
    assert_eq!((field.role,field.label.as_str(),field.description.as_str(),field.value.as_str(),field.disabled),
        (Role::TextField,"Name","Your name","abc",false));
    assert_eq!((node(&tree,1).role,node(&tree,1).label.as_str()),(Role::Container,"Window"));
    assert_eq!((node(&tree,3).role,node(&tree,3).label.as_str()),(Role::Button,"OK"));
    assert!(node(&tree,9).disabled);
    // Geometry is node_bounds for every node, and laid out (root fills the view, buttons 80×30).
    for item in &tree.nodes {
        assert_eq!(item.bounds,engine.node_bounds(OWNER,v,item.node),"node {:?}",item.node);
    }
    let root=node(&tree,1).bounds.unwrap();
    assert_eq!((root.x(),root.y(),root.size.width,root.size.height),(0.0,0.0,W as f32,H as f32));
    let ok=node(&tree,3).bounds.unwrap();
    assert_eq!((ok.size.width,ok.size.height),(80.0,30.0));
    let off=node(&tree,9).bounds.unwrap();
    assert!(off.y()>=ok.y()+ok.size.height,"column order: {ok:?} then {off:?}");
    // Actions follow the focus/activate admission rules.
    let none=AccessActions::default();
    assert_eq!(node(&tree,3).actions,AccessActions {focus:true,activate:true});
    assert_eq!(node(&tree,9).actions,none);
    assert_eq!(node(&tree,10).actions,AccessActions {focus:true,activate:false});
    for id in [1,2,4] {assert_eq!(node(&tree,id).actions,none,"node {id}");}
    assert_eq!(tree.focused,None);
    assert!(tree.nodes.iter().all(|item|!item.focused));
}

#[test]
fn reported_actions_match_engine_admission() {
    let source=scene();let v=source.version();let f=font();
    let mut engine=Engine::new(OWNER,&source,&f,W,H).expect("native import");
    let tree=engine.accessibility_tree(OWNER,v).unwrap();
    for item in &tree.nodes {
        let mut activated=0;
        assert_eq!(engine.activate(OWNER,v,item.node,&mut |_|activated+=1),item.actions.activate,"activate {:?}",item.node);
        assert_eq!(activated,item.actions.activate as usize);
        assert_eq!(engine.focus(OWNER,v,item.node,&mut |_|{}),item.actions.focus,"focus {:?}",item.node);
        if item.actions.focus {
            let after=engine.accessibility_tree(OWNER,v).unwrap();
            assert_eq!(after.focused,Some(item.node));
            assert_eq!(after.nodes.iter().filter(|other|other.focused).map(|other|other.node).collect::<Vec<_>>(),vec![item.node]);
        }
    }
    // Pruned nodes admit no action either.
    for id in [5,6,7,8] {
        assert!(!engine.activate(OWNER,v,n(id),&mut |_|{}),"activate {id}");
        assert!(!engine.focus(OWNER,v,n(id),&mut |_|{}),"focus {id}");
    }
}

#[test]
fn updates_reshape_the_tree_and_hiding_drops_focus() {
    let source=scene();let v=source.version();let f=font();
    let mut engine=Engine::new(OWNER,&source,&f,W,H).expect("native import");
    assert!(engine.focus(OWNER,v,n(3),&mut |_|{}));
    // Reveal the hidden group and relabel the OK button in one update.
    let shown=Semantics::default();
    let relabelled=Semantics::default().role(Role::Button).label("Accept").focusable(true);
    assert!(engine.apply(OWNER,v,v+1,&[ScenePatch::UpdateSemantics {id:n(5),semantics:shown},
        ScenePatch::UpdateSemantics {id:n(3),semantics:relabelled}]));
    assert!(engine.accessibility_tree(OWNER,v).is_none(),"stale version");
    let tree=engine.accessibility_tree(OWNER,v+1).unwrap();
    assert_eq!(ids(&tree),vec![1,2,3,4,5,6,9,10]);
    assert_eq!(node(&tree,1).children,vec![n(2),n(3),n(5),n(9),n(10)]);
    assert_eq!(node(&tree,3).label,"Accept");
    assert_eq!(node(&tree,6).actions,AccessActions {focus:true,activate:true});
    assert_eq!(tree.focused,Some(n(3)));
    // Hiding the focused button removes it and leaves no focused node in the tree.
    let hidden=Semantics::default().role(Role::Button).label("Accept").focusable(true).hidden(true);
    assert!(engine.apply(OWNER,v+1,v+2,&[ScenePatch::UpdateSemantics {id:n(3),semantics:hidden}]));
    let tree=engine.accessibility_tree(OWNER,v+2).unwrap();
    assert_eq!(ids(&tree),vec![1,2,5,6,9,10]);
    assert_eq!(tree.focused,None);
    assert!(tree.nodes.iter().all(|item|!item.focused));
}

#[test]
fn hidden_root_foreign_and_closed_are_explicit() {
    let source=scene();let v=source.version();let f=font();
    let mut engine=Engine::new(OWNER,&source,&f,W,H).expect("native import");
    assert!(engine.accessibility_tree(OWNER+1,v).is_none(),"foreign owner");
    assert!(engine.accessibility_tree(OWNER,v+1).is_none(),"future version");
    let hidden=Semantics::default().label("Window").hidden(true);
    assert!(engine.apply(OWNER,v,v+1,&[ScenePatch::UpdateSemantics {id:n(1),semantics:hidden}]));
    let tree=engine.accessibility_tree(OWNER,v+1).unwrap();
    assert!(tree.nodes.is_empty() && tree.focused.is_none());
    assert!(engine.close(OWNER));
    assert!(engine.accessibility_tree(OWNER,v+1).is_none(),"closed");
}
