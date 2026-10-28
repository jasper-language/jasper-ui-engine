//! Incremental accessibility updates through the root entry: the difference between two tree
//! snapshots of one view replays to the newer tree, and carries only added/changed nodes,
//! removed nodes, the root and the focus.
//! 根入口无障碍增量更新：同一视图两份树快照之差可回放出较新的树，且只携带新增/变更节点、移除节点、根与焦点。
#![cfg(feature="native-blitz-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::native::host::{AccessibilityTree,AccessibilityUpdate};
use jasper_ui_engine::scene::{Node,NodeId,Scene,ScenePatch};
use jasper_ui_engine::semantics::{Role,Semantics};
use jasper_ui_engine::style::{Color,Length,Style};

const W:u32=240;
const H:u32=200;
const OWNER:u64=23;

fn font()->Vec<u8> {
    let path=std::env::var("JASPER_UI_TEST_FONT").expect("JASPER_UI_TEST_FONT must name an injected Latin font");
    std::fs::read(path).expect("injected font readable")
}

fn n(id:u64)->NodeId {NodeId(id)}

fn button(id:u64,label:&str)->Node {
    Node::container(n(id)).with_style(Style::default().size(Length::Px(80.0),Length::Px(30.0)).background(Color::rgb(40,80,160)))
        .with_semantics(Semantics::default().role(Role::Button).label(label).focusable(true))
}

/// 1 root ─ 2 title text, 3 OK button, 4 group (5 Cancel button), 6 status text.
fn scene()->Scene {
    Scene::new(Node::container(n(1)).with_style(Style::default().size(Length::Px(W as f32),Length::Px(H as f32)))
        .with_semantics(Semantics::default().label("Window"))
        .child(Node::text(n(2),"Title").with_style(Style::default().text(16.0,Color::WHITE))
            .with_semantics(Semantics::default().role(Role::Text).label("Title")))
        .child(button(3,"OK"))
        .child(Node::container(n(4)).child(button(5,"Cancel")))
        .child(Node::text(n(6),"Ready").with_style(Style::default().text(12.0,Color::WHITE))
            .with_semantics(Semantics::default().role(Role::Text).label("Status").value("Ready"))))
}

fn ids(nodes:&[jasper_ui_engine::native::host::AccessibilityNode])->Vec<u64> {nodes.iter().map(|node|node.node.0).collect()}

/// The update replays exactly and contains exactly the nodes that differ.
fn checked(previous:&AccessibilityTree,next:&AccessibilityTree)->AccessibilityUpdate {
    let update=next.update_since(previous).expect("newer tree");
    assert_eq!(previous.apply(&update).as_ref(),Some(next),"replay");
    for node in &next.nodes {
        let before=previous.nodes.iter().find(|old|old.node==node.node);
        assert_eq!(update.nodes.iter().any(|item|item.node==node.node),before!=Some(node),"node {:?}",node.node);
    }
    for node in &previous.nodes {
        assert_eq!(update.removed.contains(&node.node),!next.nodes.iter().any(|item|item.node==node.node));
    }
    assert_eq!((update.from_version,update.scene_version,update.root),(previous.scene_version,next.scene_version,next.root()));
    update
}

#[test]
fn same_version_is_an_empty_update() {
    let source=scene();let v=source.version();let f=font();
    let engine=Engine::new(OWNER,&source,&f,W,H).expect("native import");
    let tree=engine.accessibility_tree(OWNER,v).unwrap();
    let update=checked(&tree,&tree);
    assert!(update.is_empty());
    assert_eq!(update.root,Some(n(1)));
    // From an empty mirror the update carries the whole tree.
    let empty=AccessibilityTree {scene_version:v,nodes:Vec::new(),focused:None};
    let full=checked(&empty,&tree);
    assert_eq!(ids(&full.nodes),vec![1,2,3,4,5,6]);
    assert!(full.removed.is_empty());
}

#[test]
fn engine_updates_replay_through_incremental_updates() {
    let source=scene();let v=source.version();let f=font();
    let mut engine=Engine::new(OWNER,&source,&f,W,H).expect("native import");
    let t0=engine.accessibility_tree(OWNER,v).unwrap();

    // A value change touches only the status node.
    let status=Semantics::default().role(Role::Text).label("Status").value("Saving");
    assert!(engine.apply(OWNER,v,v+1,&[ScenePatch::UpdateSemantics {id:n(6),semantics:status}]));
    let t1=engine.accessibility_tree(OWNER,v+1).unwrap();
    let u1=checked(&t0,&t1);
    assert_eq!((ids(&u1.nodes),u1.removed.clone(),u1.focus_changed),(vec![6],vec![],false));

    // Focus moves: the two nodes whose focused flag flips, and the focus itself.
    assert!(engine.focus(OWNER,v+1,n(3),&mut |_|{}));
    let t1f=engine.accessibility_tree(OWNER,v+1).unwrap();
    let uf=checked(&t1,&t1f);
    assert_eq!((ids(&uf.nodes),uf.focused,uf.focus_changed),(vec![3],Some(n(3)),true));

    // Hiding the group removes it with its subtree; the root's child list changes.
    let hidden=Semantics::default().hidden(true);
    assert!(engine.apply(OWNER,v+1,v+2,&[ScenePatch::UpdateSemantics {id:n(4),semantics:hidden}]));
    let t2=engine.accessibility_tree(OWNER,v+2).unwrap();
    let u2=checked(&t1f,&t2);
    assert_eq!((ids(&u2.nodes),u2.removed.clone()),(vec![1],vec![n(4),n(5)]));

    // Inserting a node adds it and changes its parent; removing the OK button drops the focus.
    let added=Node::text(n(7),"New").with_semantics(Semantics::default().role(Role::Text).label("New"));
    assert!(engine.apply(OWNER,v+2,v+3,&[ScenePatch::Insert {parent:n(1),index:1,node:added},ScenePatch::Remove {id:n(3)}]));
    let t3=engine.accessibility_tree(OWNER,v+3).unwrap();
    let u3=checked(&t2,&t3);
    assert!(u3.nodes.iter().any(|node|node.node==n(7)) && u3.nodes.iter().any(|node|node.node==n(1)));
    assert_eq!(u3.removed,vec![n(3)]);
    assert_eq!((u3.focused,u3.focus_changed),(None,true));

    // Updates compose: the old mirror replays the whole sequence to the latest tree.
    let mut mirror=t0.clone();
    for (prev,next) in [(&t0,&t1),(&t1,&t1f),(&t1f,&t2),(&t2,&t3)] {
        mirror=mirror.apply(&next.update_since(prev).unwrap()).expect("chain");
    }
    assert_eq!(mirror,t3);
    // A single update across all versions is the same as the chain.
    assert_eq!(t0.apply(&checked(&t0,&t3)).as_ref(),Some(&t3));

    // Hiding the root empties the tree: everything removed, no root.
    let gone=Semantics::default().label("Window").hidden(true);
    assert!(engine.apply(OWNER,v+3,v+4,&[ScenePatch::UpdateSemantics {id:n(1),semantics:gone}]));
    let t4=engine.accessibility_tree(OWNER,v+4).unwrap();
    let u4=checked(&t3,&t4);
    assert_eq!((u4.root,u4.nodes.len(),u4.removed.len()),(None,0,t3.nodes.len()));
}

#[test]
fn reversed_mismatched_and_inconsistent_updates_are_rejected() {
    let source=scene();let v=source.version();let f=font();
    let mut engine=Engine::new(OWNER,&source,&f,W,H).expect("native import");
    let t0=engine.accessibility_tree(OWNER,v).unwrap();
    let status=Semantics::default().role(Role::Text).label("Status").value("Done");
    assert!(engine.apply(OWNER,v,v+1,&[ScenePatch::UpdateSemantics {id:n(6),semantics:status}]));
    let t1=engine.accessibility_tree(OWNER,v+1).unwrap();
    assert!(t0.update_since(&t1).is_none(),"older than previous");
    let update=t1.update_since(&t0).unwrap();
    // Applied to a mirror of another version.
    assert!(t1.apply(&update).is_none(),"mirror already newer");
    // Removing a node that the mirror does not hold.
    let mut bad=update.clone();bad.removed.push(n(99));
    assert!(t0.apply(&bad).is_none(),"absent removal");
    // A parent naming a child that exists nowhere.
    let mut bad=update.clone();
    let mut root=t1.nodes[0].clone();root.children.push(n(99));bad.nodes.push(root);
    assert!(t0.apply(&bad).is_none(),"missing child");
    // A node reachable twice.
    let mut bad=update.clone();
    let mut root=t1.nodes[0].clone();root.children.push(n(5));bad.nodes.push(root);
    assert!(t0.apply(&bad).is_none(),"duplicate child");
    // Removing a node still named as a child leaves it missing.
    let mut bad=update.clone();bad.removed.push(n(5));
    assert!(t0.apply(&bad).is_none(),"dangling child");
    // A node that no parent names is unreachable from the root.
    let mut bad=update.clone();
    let mut orphan=t1.nodes[2].clone();orphan.node=n(98);orphan.children.clear();bad.nodes.push(orphan);
    assert!(t0.apply(&bad).is_none(),"unreachable node");
    // Focus outside the tree.
    let mut bad=update.clone();bad.focused=Some(n(99));bad.focus_changed=true;
    assert!(t0.apply(&bad).is_none(),"foreign focus");
    // The mirror is untouched by failed applications.
    assert_eq!(t0.apply(&update).as_ref(),Some(&t1));
}
