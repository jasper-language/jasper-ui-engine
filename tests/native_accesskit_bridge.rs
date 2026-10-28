//! AccessKit bridge through the root entry: full and incremental TreeUpdates built from the
//! accessibility tree are accepted by the reference AccessKit consumer and keep its mirror equal
//! to the full conversion of the newest tree; AccessKit action requests reach focus/activate
//! only for nodes that offer the action.
//! 根入口 AccessKit 桥：由无障碍树生成的完整与增量 TreeUpdate 被参考 AccessKit 消费端接受，且其镜像始终
//! 等于最新树的完整转换；AccessKit 动作请求仅对提供该动作的节点到达 focus/activate。
#![cfg(feature="accesskit-pending")]
use std::collections::HashMap;
use accesskit_consumer::{NodeRef,Tree as Mirror,TreeChangeHandler as ChangeHandler};
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::native::host::{AccessibilityTree,accesskit};
use jasper_ui_engine::native::host::accesskit::{Action,ActionData,ActionRequest,Affine,Node as KitNode,NodeId as KitId,Role as KitRole,TreeId,TreeUpdate};
use jasper_ui_engine::scene::{Node,NodeId,Scene,ScenePatch};
use jasper_ui_engine::semantics::{Role,Semantics};
use jasper_ui_engine::style::{Color,Length,Style};

const W:u32=240;
const H:u32=200;
const OWNER:u64=29;

fn font()->Vec<u8> {
    let path=std::env::var("JASPER_UI_TEST_FONT").expect("JASPER_UI_TEST_FONT must name an injected Latin font");
    std::fs::read(path).expect("injected font readable")
}

fn n(id:u64)->NodeId {NodeId(id)}

fn button(id:u64,label:&str,disabled:bool)->Node {
    Node::container(n(id)).with_style(Style::default().size(Length::Px(80.0),Length::Px(30.0)).background(Color::rgb(40,80,160)))
        .with_semantics(Semantics::default().role(Role::Button).label(label).focusable(true).disabled(disabled))
}

/// 1 root ─ 2 title text, 3 OK button, 4 group (5 Cancel button), 6 status text, 7 disabled button.
fn scene()->Scene {
    Scene::new(Node::container(n(1)).with_style(Style::default().size(Length::Px(W as f32),Length::Px(H as f32)))
        .with_semantics(Semantics::default().label("Window"))
        .child(Node::text(n(2),"Title").with_style(Style::default().text(16.0,Color::WHITE))
            .with_semantics(Semantics::default().role(Role::Text).label("Title")))
        .child(button(3,"OK",false))
        .child(Node::container(n(4)).child(button(5,"Cancel",false)))
        .child(Node::text(n(6),"Ready").with_style(Style::default().text(12.0,Color::WHITE))
            .with_semantics(Semantics::default().role(Role::Text).label("Status").value("Ready")))
        .child(button(7,"Locked",true)))
}

struct Ignore;
impl ChangeHandler for Ignore {
    fn node_added(&mut self,_:&NodeRef) {}
    fn node_updated(&mut self,_:&NodeRef,_:&NodeRef) {}
    fn focus_moved(&mut self,_:Option<&NodeRef>,_:Option<&NodeRef>) {}
    fn node_removed(&mut self,_:&NodeRef) {}
}

/// Nodes reachable from the consumer's root, by local id, plus its root and focus.
fn mirror_state(mirror:&Mirror)->(HashMap<u64,KitNode>,u64,u64) {
    let state=mirror.state();
    let local=|node:&NodeRef|state.locate_node(node.id()).expect("located").0.0;
    let mut nodes=HashMap::new();
    let mut pending=vec![state.root()];
    while let Some(node)=pending.pop() {
        assert!(nodes.insert(local(&node),node.data().clone()).is_none(),"node reachable once");
        pending.extend(node.children());
    }
    (nodes,local(&state.root()),local(&state.focus_in_tree()))
}

fn full_state(update:&TreeUpdate)->(HashMap<u64,KitNode>,u64,u64) {
    (update.nodes.iter().map(|(id,node)|(id.0,node.clone())).collect(),update.tree.as_ref().unwrap().root.0,update.focus.0)
}

fn request(action:Action,id:u64)->ActionRequest {
    ActionRequest {action,target_tree:TreeId::ROOT,target_node:KitId(id),data:None}
}

#[test]
fn full_tree_converts_semantics_geometry_actions_and_focus() {
    let source=scene();let v=source.version();let f=font();
    let engine=Engine::new(OWNER,&source,&f,W,H).expect("native import");
    let tree=engine.accessibility_tree(OWNER,v).unwrap();
    let update=tree.to_accesskit(Affine::IDENTITY).expect("non-empty tree");
    assert_eq!(update.tree_id,TreeId::ROOT);
    let info=update.tree.as_ref().unwrap();
    assert_eq!((info.root,update.focus,info.toolkit_name.as_deref()),(KitId(1),KitId(1),Some("Jasper UI")),"no focus: root");
    assert_eq!(update.nodes.iter().map(|(id,_)|id.0).collect::<Vec<_>>(),vec![1,2,3,4,5,6,7]);
    let kit:HashMap<u64,&KitNode>=update.nodes.iter().map(|(id,node)|(id.0,node)).collect();
    for item in &tree.nodes {
        let node=kit[&item.node.0];
        assert_eq!(node.children().iter().map(|id|id.0).collect::<Vec<_>>(),item.children.iter().map(|id|id.0).collect::<Vec<_>>());
        assert_eq!(node.supports_action(Action::Focus),item.actions.focus,"focus {:?}",item.node);
        assert_eq!(node.supports_action(Action::Click),item.actions.activate,"click {:?}",item.node);
        assert_eq!(node.is_disabled(),item.disabled);
        let bounds=item.bounds.expect("laid out");
        let rect=node.bounds().expect("bounds");
        assert_eq!((rect.x0,rect.y0,rect.x1-rect.x0,rect.y1-rect.y0),
            (bounds.origin.x as f64,bounds.origin.y as f64,bounds.size.width as f64,bounds.size.height as f64));
        assert!(node.transform().is_none(),"identity transform omitted");
    }
    assert_eq!((kit[&1].role(),kit[&1].label()),(KitRole::GenericContainer,Some("Window")));
    assert_eq!((kit[&3].role(),kit[&3].label()),(KitRole::Button,Some("OK")));
    // Text without a value carries its label as AccessKit value; with a value both stay.
    assert_eq!((kit[&2].role(),kit[&2].label(),kit[&2].value()),(KitRole::Label,None,Some("Title")));
    assert_eq!((kit[&6].label(),kit[&6].value()),(Some("Status"),Some("Ready")));
    assert!(kit[&7].is_disabled() && !kit[&7].supports_action(Action::Click) && !kit[&7].supports_action(Action::Focus));

    // A host transform lands on the root only; the consumer composes it into every box.
    let scale=Affine::scale(2.0);
    let scaled=tree.to_accesskit(scale).unwrap();
    assert_eq!(scaled.nodes[0].1.transform(),Some(&scale));
    assert!(scaled.nodes[1..].iter().all(|(_,node)|node.transform().is_none()));
    let mirror=Mirror::new(scaled,true);
    let ok=mirror.state().node_by_tree_local_id(KitId(3),TreeId::ROOT).expect("OK button");
    let raw=ok.raw_bounds().unwrap();
    let boxed=ok.bounding_box().unwrap();
    assert_eq!((boxed.x0,boxed.y0,boxed.x1,boxed.y1),(raw.x0*2.0,raw.y0*2.0,raw.x1*2.0,raw.y1*2.0));

    // An empty tree has no AccessKit form.
    let empty=AccessibilityTree {scene_version:v,nodes:Vec::new(),focused:None};
    assert!(empty.to_accesskit(Affine::IDENTITY).is_none());
}

#[test]
fn incremental_updates_keep_the_consumer_mirror_equal_to_the_newest_tree() {
    let source=scene();let v=source.version();let f=font();
    let mut engine=Engine::new(OWNER,&source,&f,W,H).expect("native import");
    let mut previous=engine.accessibility_tree(OWNER,v).unwrap();
    let mut mirror=Mirror::new(previous.to_accesskit(Affine::IDENTITY).unwrap(),true);
    assert_eq!(mirror_state(&mirror),full_state(&previous.to_accesskit(Affine::IDENTITY).unwrap()));
    let mut version=v;
    // Exactly the added/changed nodes of the engine update are sent; `expected` pins the count
    // where no geometry can move.
    let step=|engine:&mut Engine,mirror:&mut Mirror,previous:&mut AccessibilityTree,version:u64,expected:Option<usize>| {
        let next=engine.accessibility_tree(OWNER,version).unwrap();
        let changes=next.update_since(previous).unwrap();
        let update=changes.to_accesskit(Affine::IDENTITY).expect("non-empty");
        assert_eq!(update.nodes.iter().map(|(id,_)|id.0).collect::<Vec<_>>(),changes.nodes.iter().map(|node|node.node.0).collect::<Vec<_>>());
        assert!(update.nodes.len()<next.nodes.len(),"incremental, not full");
        if let Some(count)=expected {assert_eq!(update.nodes.len(),count,"only changed nodes are sent");}
        mirror.update_and_process_changes(update,&mut Ignore);
        assert_eq!(mirror_state(mirror),full_state(&next.to_accesskit(Affine::IDENTITY).unwrap()),"mirror at {version}");
        *previous=next;
    };
    // Value change: one node.
    let status=Semantics::default().role(Role::Text).label("Status").value("Saving");
    assert!(engine.apply(OWNER,version,version+1,&[ScenePatch::UpdateSemantics {id:n(6),semantics:status}]));version+=1;
    step(&mut engine,&mut mirror,&mut previous,version,Some(1));
    // Focus moves to the OK button: the focused flag flips on one node; AccessKit focus follows.
    assert!(engine.focus(OWNER,version,n(3),&mut |_|{}));
    step(&mut engine,&mut mirror,&mut previous,version,Some(1));
    assert_eq!(mirror_state(&mirror).2,3);
    // Hiding the group drops it and its child through the changed root.
    let hidden=Semantics::default().hidden(true);
    assert!(engine.apply(OWNER,version,version+1,&[ScenePatch::UpdateSemantics {id:n(4),semantics:hidden}]));version+=1;
    step(&mut engine,&mut mirror,&mut previous,version,Some(1));
    assert!(!mirror_state(&mirror).0.contains_key(&5));
    // Insert a node and remove the focused button: focus falls back to the root.
    let added=Node::text(n(8),"New").with_semantics(Semantics::default().role(Role::Text).label("New"));
    assert!(engine.apply(OWNER,version,version+1,&[ScenePatch::Insert {parent:n(1),index:1,node:added},ScenePatch::Remove {id:n(3)}]));version+=1;
    step(&mut engine,&mut mirror,&mut previous,version,None);
    assert_eq!(mirror_state(&mirror).2,1);
    // Reveal the group again: the whole subtree comes back.
    let shown=Semantics::default();
    assert!(engine.apply(OWNER,version,version+1,&[ScenePatch::UpdateSemantics {id:n(4),semantics:shown}]));version+=1;
    step(&mut engine,&mut mirror,&mut previous,version,None);
    // An update to an empty tree has no AccessKit form.
    let gone=Semantics::default().label("Window").hidden(true);
    assert!(engine.apply(OWNER,version,version+1,&[ScenePatch::UpdateSemantics {id:n(1),semantics:gone}]));version+=1;
    let empty=engine.accessibility_tree(OWNER,version).unwrap();
    assert!(empty.update_since(&previous).unwrap().to_accesskit(Affine::IDENTITY).is_none());
}

#[test]
fn action_requests_reach_focus_and_activate_only_when_offered() {
    let source=scene();let v=source.version();let f=font();
    let mut engine=Engine::new(OWNER,&source,&f,W,H).expect("native import");
    let mut focus_events=Vec::new();
    let mut activations=Vec::new();
    // Focus and click on an enabled button.
    assert!(engine.accesskit_action(OWNER,v,&request(Action::Focus,5),&mut |event|focus_events.push(event),&mut |_|{}));
    assert_eq!(engine.focused_node(OWNER,v),Some(n(5)));
    assert_eq!(focus_events.last().map(|event|(event.node,event.focused)),Some((n(5),true)));
    assert!(engine.accesskit_action(OWNER,v,&request(Action::Click,3),&mut |_|{},&mut |event|activations.push(event)));
    assert_eq!(activations.len(),1);
    // Every node: the bridge admits exactly the actions the tree reports, then nothing else moves.
    let tree=engine.accessibility_tree(OWNER,v).unwrap();
    for item in &tree.nodes {
        let before=activations.len();
        assert_eq!(engine.accesskit_action(OWNER,v,&request(Action::Click,item.node.0),&mut |_|{},&mut |event|activations.push(event)),
            item.actions.activate,"click {:?}",item.node);
        assert_eq!(activations.len()-before,usize::from(item.actions.activate));
        assert_eq!(engine.accesskit_action(OWNER,v,&request(Action::Focus,item.node.0),&mut |_|{},&mut |_|{}),
            item.actions.focus,"focus {:?}",item.node);
    }
    assert!(engine.focus(OWNER,v,n(5),&mut |_|{}));
    let count=activations.len();
    let refused=|engine:&mut Engine,version:u64,owner:u64,request:ActionRequest,why:&str| {
        let before=engine.focused_node(OWNER,version);
        assert!(!engine.accesskit_action(owner,version,&request,&mut |_|panic!("focus event: {why}"),&mut |_|panic!("activation: {why}")),"{why}");
        assert_eq!(engine.focused_node(OWNER,version),before,"{why}");
    };
    refused(&mut engine,v,OWNER,request(Action::Click,7),"disabled button");
    refused(&mut engine,v,OWNER,request(Action::Focus,7),"disabled focus");
    refused(&mut engine,v,OWNER,request(Action::Click,2),"text has no default action");
    refused(&mut engine,v,OWNER,request(Action::ScrollIntoView,3),"unsupported action");
    refused(&mut engine,v,OWNER,request(Action::Focus,99),"unknown node");
    refused(&mut engine,v,OWNER+1,request(Action::Focus,3),"foreign owner");
    refused(&mut engine,v+1,OWNER,request(Action::Focus,3),"stale version");
    let mut other=request(Action::Focus,3);other.target_tree=TreeId(accesskit::Uuid::from_u128(1));
    refused(&mut engine,v,OWNER,other,"other tree");
    let mut with_data=request(Action::Click,3);with_data.data=Some(ActionData::Value("x".into()));
    refused(&mut engine,v,OWNER,with_data,"action data");
    // A hidden node is outside the tree and cannot be reached.
    let hidden=Semantics::default().hidden(true);
    assert!(engine.apply(OWNER,v,v+1,&[ScenePatch::UpdateSemantics {id:n(4),semantics:hidden}]));
    refused(&mut engine,v+1,OWNER,request(Action::Focus,5),"hidden subtree");
    assert_eq!(activations.len(),count);
}
