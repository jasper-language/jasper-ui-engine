//! Candidate adapter for the existing Rust scene, using native Blitz mutations only.
//! 现有 Rust 场景适配候选；不序列化 HTML，不执行旧布局/绘制算法。
use blitz_dom::{BaseDocument,DocumentConfig,LocalName,Namespace,QualName};
use blitz_traits::shell::Viewport;
use jasper_ui_engine::{scene::{Scene,Node,NodeKind,NodeId,ScenePatch},style::*,semantics::{Semantics,Role}};
use crate::view_slot::ViewSlot;
use jasper_ui_engine::input_identity::next_ui_update_version;
use std::collections::{BTreeMap,BTreeSet};

enum PreparedSceneMutation {
    Text,
    Semantics(Vec<(&'static str,String)>),
    Scroll(blitz_dom::Point<f64>),
    Style(Vec<(&'static str,String)>),
    Remove(Vec<NodeId>),
    Insert {styles:Vec<Vec<(&'static str,String)>>,index:usize,anchor:Option<NodeId>},
}

/// IEEE-754 positive finite values are ordered by their unsigned bit pattern.
/// Keep host DPI admission integer-only so the exact boundary can be verified
/// without delegating our policy to an unmodelled floating-point comparison.
// BEGIN VERIFIED NATIVE DPI SCALE ADMISSION KERNEL
fn admits_native_dpi_scale_bits(bits:u32)->bool {
    (0x3c80_0000..=0x4280_0000).contains(&bits)
}
// END VERIFIED NATIVE DPI SCALE ADMISSION KERNEL

/// Admit one versioned native CPU render, then submit and present only an exact packed frame.
/// The renderer is an injected third-party boundary; this kernel owns its call eligibility,
/// packed-length check and the transition into the verified backend submission seam.
#[cfg(feature="cpu-pending")]
// BEGIN VERIFIED NATIVE FRAME ORCHESTRATION KERNEL
fn orchestrate_native_frame(
    owner:u64,
    caller:u64,
    current_version:u64,
    requested_version:u64,
    extent:Option<(u32,u32)>,
    budget:usize,
    render:impl FnOnce(usize)->Option<Vec<u8>>,
    backend:&mut impl jasper_ui_engine::backend::RenderBackend,
)->Result<(),jasper_ui_engine::backend::BackendError> {
    use jasper_ui_engine::backend::BackendError;
    if owner!=caller || current_version!=requested_version {
        return Err(BackendError::Invalid("native frame owner or version".into()));
    }
    let (width,height)=extent.ok_or_else(||BackendError::Invalid("retired native view".into()))?;
    let expected=crate::cpu_budget::cpu_frame_bytes(width,height,budget)
        .ok_or_else(||BackendError::Invalid("native frame extent or budget".into()))?;
    let pixels=render(budget)
        .ok_or_else(||BackendError::Invalid("native frame renderer".into()))?;
    if pixels.len()!=expected {
        return Err(BackendError::Invalid("native frame packed length".into()));
    }
    jasper_ui_engine::backend::submit_native_frame(backend,width,height,pixels)
}
// END VERIFIED NATIVE FRAME ORCHESTRATION KERNEL

/// Validate an injected software surface before rendering, then copy one exact
/// packed frame row-by-row without touching host padding.
#[cfg(feature="cpu-pending")]
// BEGIN VERIFIED NATIVE HOST SURFACE ORCHESTRATION KERNEL
fn orchestrate_native_host_surface(
    width:u32,
    height:u32,
    destination:&mut [u8],
    stride:usize,
    max_render_bytes:usize,
    render:impl FnOnce(usize)->Option<Vec<u8>>,
)->bool {
    use jasper_ui_engine::surface::{rgba8_row_bytes,surface_span,write_surface_row};
    let Some(row_bytes)=rgba8_row_bytes(width,destination.len()) else {return false;};
    let height=height as usize;
    if surface_span(row_bytes,height,stride,destination.len()).is_none() {return false;}
    let Some(expected)=row_bytes.checked_mul(height) else {return false;};
    let Some(pixels)=render(max_render_bytes) else {return false;};
    if pixels.len()!=expected {return false;}
    for row in 0..height {
        let start=row*row_bytes;let end=start+row_bytes;
        if !write_surface_row(destination,row_bytes,height,stride,row,&pixels[start..end]) {return false;}
    }
    true
}
// END VERIFIED NATIVE HOST SURFACE ORCHESTRATION KERNEL

// BEGIN VERIFIED NATIVE SCENE TYPE
pub(crate) struct NativeScene {
    document:ViewSlot<BaseDocument>,
    owner:u64,
    scene_version:u64,
    node_count:usize,
    parents:BTreeMap<NodeId,Option<NodeId>>,
    children:BTreeMap<NodeId,Vec<NodeId>>,
    displayed:BTreeMap<NodeId,bool>,
    semantics:BTreeMap<NodeId,Semantics>,
    nodes:BTreeMap<NodeId,(blitz_dom::NodeId,Option<blitz_dom::NodeId>)>,
    frame_dirty:bool,
    frame_cache:Option<(u32,u32,Vec<u8>)>,
}
// END VERIFIED NATIVE SCENE TYPE
impl NativeScene {
    /// Bounded snapshot import; Canvas remains unsupported.
    pub(crate) fn from_scene(owner:u64,scene:&Scene,font:&[u8],width:u32,height:u32)->Option<Self> {
        Self::from_scene_with_context(owner,scene,crate::native_fonts::injected_font_context(font),width,height)
    }
    /// Host-ordered font stack; measurement and painting share the one Parley layout.
    /// 宿主有序字体栈；测量与绘制共用同一份 Parley 布局。
    pub(crate) fn from_scene_with_fonts(owner:u64,scene:&Scene,fonts:&[&[u8]],width:u32,height:u32)->Option<Self> {
        Self::from_scene_with_context(owner,scene,crate::native_fonts::injected_font_stack(fonts)?,width,height)
    }
    fn from_scene_with_context(owner:u64,scene:&Scene,font_ctx:blitz_dom::FontContext,width:u32,height:u32)->Option<Self> {
        let prepared=prepare_subtree(&scene.root,0,&BTreeSet::new())?;
        // Existing identity validation runs only after depth/node admission.
        scene.validate().ok()?;
        let mut document=BaseDocument::new(DocumentConfig {
            font_ctx:Some(font_ctx),
            viewport:Some(Viewport {window_size:(width,height),..Viewport::default()}),
            ..DocumentConfig::default()
        });
        let root=document.root_node().id;
        let body;
        {
            let mut tree=document.mutate();
            let name=|local:&str|QualName::new(None,Namespace::from("http://www.w3.org/1999/xhtml"),LocalName::from(local));
            let html=tree.create_element(name("html"),Vec::new());
            body=tree.create_element(name("body"),Vec::new());
            tree.set_style_property(body,"margin","0px");
            tree.append_children(html,&[body]);tree.append_children(root,&[html]);
        }
        let mut view=Self {document:ViewSlot::new(document),owner,scene_version:scene.version(),node_count:0,
            nodes:BTreeMap::new(),semantics:BTreeMap::new(),parents:BTreeMap::new(),displayed:BTreeMap::new(),children:BTreeMap::new(),
            frame_dirty:true,frame_cache:None};
        if !view.append_prepared_subtree(&scene.root,None,body,prepared) {return None;}
        if !view.sync_native_focus_participation() {return None;}
        view.document.as_mut()?.resolve(0.0);
        if !view.apply_source_scrolls(&scene.root) {return None;}
        Some(view)
    }
    /// Shared native construction for initial snapshots and admitted insertion.
    fn append_prepared_subtree(&mut self,root:&Node,source_parent:Option<NodeId>,parent:blitz_dom::NodeId,
        prepared:Vec<Vec<(&'static str,String)>>)->bool {
        // Preserve Framework's inherited semantic hiding in native tab participation.
        // Native focus ordering stays in Blitz; raw exported semantics remain unchanged.
        let Some(inherited_hidden)=source_hidden_ancestry(&self.parents,&self.semantics,source_parent) else {return false;};
        let Some(document)=self.document.as_mut() else {return false;};
        let mut tree=document.mutate();
        let name=|local:&str|QualName::new(None,Namespace::from("http://www.w3.org/1999/xhtml"),LocalName::from(local));
        let mut stack=vec![(root,parent,source_parent,inherited_hidden)];let mut index=0;
        while let Some((node,parent,source_parent,inherited_hidden))=stack.pop() {
            let hidden=inherited_hidden || node.semantics.hidden;
            let element=tree.create_element(name(if role_uses_native_button(node.semantics.role) {"button"} else {"div"}),Vec::new());
            self.semantics.insert(node.id,node.semantics.clone());self.parents.insert(node.id,source_parent);
            self.displayed.insert(node.id,node.style.display!=Display::None);
            self.children.insert(node.id,node.children.iter().map(|child|child.id).collect());
            for (key,mut value) in semantic_attributes(&node.semantics) {
                if hidden && key=="tabindex" {value="-1".to_owned();}
                tree.set_attribute(element,QualName::new(None,Namespace::from(""),LocalName::from(key)),&value);
            }
            let text=if let NodeKind::Text(text)=&node.kind {
                let text=tree.create_text_node(text);tree.append_children(element,&[text]);Some(text)
            } else {None};
            self.nodes.insert(node.id,(element,text));
            for (property,value) in &prepared[index] {tree.set_style_property(element,property,value);}
            index+=1;tree.append_children(parent,&[element]);
            for child in node.children.iter().rev() {stack.push((child,element,Some(node.id),hidden));}
        }
        self.node_count=self.nodes.len();true
    }
    /// Consume source offsets through native layout data; no scrolling/clamping algorithm.
    fn apply_source_scrolls(&mut self,root:&Node)->bool {
        let mut pending=vec![root];
        while let Some(source)=pending.pop() {
            let Some(offset)=native_scroll_offset(source.scroll) else {return false;};
            let Some((element,_))=self.nodes.get(&source.id) else {return false;};let element=*element;
            let Some(document)=self.document.as_mut() else {return false;};
            let Some(node)=document.get_node_mut(element) else {return false;};
            *node.scroll_offset_mut()=offset;
            for child in source.children.iter().rev() {pending.push(child);}
        }
        true
    }
    /// Execute only a fully prepared insertion; outer frame owns admission and versioning.
    fn insert_prepared(&mut self,parent:NodeId,index:usize,anchor:Option<NodeId>,node:&Node,
        prepared:Vec<Vec<(&'static str,String)>>)->bool {
        let Some((native_parent,_))=self.nodes.get(&parent) else {return false;};let native_parent=*native_parent;
        let anchor=match anchor {Some(id)=>match self.nodes.get(&id) {Some((node,_))=>Some(*node),None=>return false},None=>None};
        if !self.append_prepared_subtree(node,Some(parent),native_parent,prepared) {return false;}
        let Some((inserted,_))=self.nodes.get(&node.id) else {return false;};let inserted=*inserted;
        let Some(document)=self.document.as_mut() else {return false;};
        if let Some(anchor)=anchor {document.mutate().insert_nodes_before(anchor,&[inserted]);}
        document.resolve(0.0);
        if !self.apply_source_scrolls(node) {return false;}
        let Some(children)=self.children.get_mut(&parent) else {return false;};children.insert(index,node.id);
        true
    }
    /// Preflight the complete ordered frame using temporary identity topology, not a Scene copy.
    pub(crate) fn apply_patches(&mut self,caller:u64,expected:u64,next:u64,patches:&[ScenePatch])->bool {
        self.invalidate_frame();
        if !accepts_scene_update(self.owner,caller,self.scene_version,expected,next,
            patches.len(),self.document.is_open()) {return false;}
        let mut prepared=Vec::with_capacity(patches.len());
        let mut live:BTreeSet<_>=self.nodes.keys().copied().collect();
        let mut text_nodes:BTreeSet<_>=self.nodes.iter().filter_map(|(id,(_,text))|text.is_some().then_some(*id)).collect();
        let mut parents=self.parents.clone();let mut children=self.children.clone();
        let mut roles:BTreeMap<_,_>=self.semantics.iter().map(|(id,value)|(*id,value.role)).collect();
        for patch in patches {
            match patch {
                ScenePatch::UpdateText {id,..} => {
                    if !live.contains(id) || !text_nodes.contains(id) {return false;}
                    prepared.push(PreparedSceneMutation::Text);
                },
                ScenePatch::UpdateStyle {id,style} => {
                    if !live.contains(id) {return false;}
                    let Some(properties)=style_properties(style,parents.get(id)==Some(&None)) else {return false;};
                    prepared.push(PreparedSceneMutation::Style(properties));
                },
                ScenePatch::UpdateSemantics {id,semantics}=> {
                    let Some(role)=roles.get(id) else {return false;};
                    // Only Button changes the native element tag; other roles are attributes.
                    if !semantic_role_update_preserves_tag(*role,semantics.role) {return false;}
                    roles.insert(*id,semantics.role);
                    prepared.push(PreparedSceneMutation::Semantics(semantic_attributes(semantics)));
                },
                ScenePatch::SetScroll {id,offset}=> {
                    if !live.contains(id) {return false;}
                    let Some(offset)=native_scroll_offset(*offset) else {return false;};
                    prepared.push(PreparedSceneMutation::Scroll(offset));
                },
                ScenePatch::Remove {id} => {
                    if !live.contains(id) || parents.get(id)==Some(&None) {return false;}
                    let removed:Vec<_>=live.iter().copied().filter(|node|source_subtree_contains(&parents,*node,*id)).collect();
                    if let Some(Some(parent))=parents.get(id) {
                        if let Some(siblings)=children.get_mut(parent) {siblings.retain(|child|child!=id);}
                    }
                    for node in &removed {roles.remove(node);live.remove(node);text_nodes.remove(node);parents.remove(node);children.remove(node);}
                    prepared.push(PreparedSceneMutation::Remove(removed));
                },
                ScenePatch::Insert {parent,index,node} => {
                    let Some(siblings)=children.get(parent) else {return false;};
                    let index=source_insert_index(*index,siblings.len());let anchor=siblings.get(index).copied();
                    let mut depth=1;let mut current=*parent;
                    while let Some(Some(ancestor))=parents.get(&current) {depth+=1;if depth>128 {return false;}current=*ancestor;}
                    let Some(styles)=prepare_subtree(node,depth,&live) else {return false;};
                    let mut stack=vec![(node,Some(*parent))];
                    while let Some((node,source_parent))=stack.pop() {
                        live.insert(node.id);roles.insert(node.id,node.semantics.role);parents.insert(node.id,source_parent);
                        children.insert(node.id,node.children.iter().map(|child|child.id).collect());
                        if matches!(node.kind,NodeKind::Text(_)) {text_nodes.insert(node.id);}
                        for child in node.children.iter().rev() {stack.push((child,Some(node.id)));}
                    }
                    let Some(siblings)=children.get_mut(parent) else {return false;};siblings.insert(index,node.id);
                    prepared.push(PreparedSceneMutation::Insert {styles,index,anchor});
                },
                _=>return false,
            }
        }
        for (patch,plan) in patches.iter().zip(prepared) {
            match (patch,plan) {
                (ScenePatch::Insert {parent,node,..},PreparedSceneMutation::Insert {styles,index,anchor})=> {
                    if !self.insert_prepared(*parent,index,anchor,node,styles) {return false;}
                },
                (ScenePatch::UpdateText {id,text},PreparedSceneMutation::Text)=> {
                    let Some((_,Some(node)))=self.nodes.get(id) else {return false;};let node=*node;
                    let Some(document)=self.document.as_mut() else {return false;};
                    document.mutate().set_node_text(node,text);document.resolve(0.0);
                },
                (ScenePatch::UpdateStyle {id,style},PreparedSceneMutation::Style(properties))=> {
                    let Some((node,_))=self.nodes.get(id) else {return false;};let node=*node;
                    let Some(document)=self.document.as_mut() else {return false;};
                    {let mut tree=document.mutate();for (key,value) in properties {tree.set_style_property(node,key,&value);}}
                    self.displayed.insert(*id,style.display!=Display::None);document.resolve(0.0);
                },
                (ScenePatch::UpdateSemantics {id,semantics},PreparedSceneMutation::Semantics(attributes))=> {
                    let Some((element,_))=self.nodes.get(id) else {return false;};let element=*element;
                    let Some(document)=self.document.as_mut() else {return false;};
                    {
                        let mut tree=document.mutate();
                        for key in ["role","aria-label","aria-description","aria-valuetext","disabled","tabindex"] {
                            tree.clear_attribute(element,QualName::new(None,Namespace::from(""),LocalName::from(key)));
                        }
                        for (key,value) in attributes {
                            tree.set_attribute(element,QualName::new(None,Namespace::from(""),LocalName::from(key)),&value);
                        }
                    }
                    self.semantics.insert(*id,semantics.clone());document.resolve(0.0);
                },
                (ScenePatch::SetScroll {id,..},PreparedSceneMutation::Scroll(offset))=> {
                    let Some((element,_))=self.nodes.get(id) else {return false;};let element=*element;
                    let Some(document)=self.document.as_mut() else {return false;};
                    let Some(node)=document.get_node_mut(element) else {return false;};
                    *node.scroll_offset_mut()=offset;
                },
                (ScenePatch::Remove {id},PreparedSceneMutation::Remove(removed))=> {
                    let Some(node)=self.nodes.get(id).map(|(node,_)|*node) else {return false;};
                    let Some(document)=self.document.as_mut() else {return false;};
                    document.mutate().remove_and_drop_node(node);
                    if let Some(Some(parent))=self.parents.get(id) {
                        if let Some(siblings)=self.children.get_mut(parent) {siblings.retain(|child|child!=id);}
                    }
                    for source in removed {
                        self.nodes.remove(&source);self.parents.remove(&source);self.displayed.remove(&source);
                        self.semantics.remove(&source);self.children.remove(&source);
                    }
                    document.resolve(0.0);
                },
                _=>return false,
            }
        }
        if !self.sync_native_focus_participation() {return false;}
        self.scene_version=next;self.node_count=self.nodes.len();
        if let Some(id)=self.focused_node(caller,next) {
            if !self.focus_eligible(id) {
                if let Some(document)=self.document.as_mut() {document.clear_focus();document.resolve(0.0);}
            }
        }
        true
    }
    /// Publish actual final focus after a successful source patch batch.
    pub(crate) fn apply_patches_with_focus_events(&mut self,caller:u64,expected:u64,next:u64,patches:&[ScenePatch],
        sink:&mut impl FnMut(crate::scene_events::SceneFocusEvent))->bool {
        let before=self.focused_node(caller,expected);
        if !self.apply_patches(caller,expected,next,patches) {return false;}
        let after=self.focused_node(caller,next);
        crate::scene_events::notify_focus_transition(next,before,after,sink);
        true
    }
    /// Native pointer defaults may change focus; report only the final observed transition.
    pub(crate) fn pointer_with_focus_events_from_host(&mut self,caller:u64,version:u64,phase:crate::native_view::PointerAction,
        x:f32,y:f32,pressed:bool,
        pointer_sink:&mut impl FnMut(crate::scene_events::SceneEvent)->crate::native_events::NativeEventReply,
        focus_sink:&mut impl FnMut(crate::scene_events::SceneFocusEvent))->bool {
        let before=self.focused_node(caller,version);
        if !self.pointer_from_host(caller,version,phase,x,y,pressed,pointer_sink) {return false;}
        let after=self.focused_node(caller,version);
        crate::scene_events::notify_focus_transition(version,before,after,focus_sink);
        true
    }
    /// Owner/version-admitted mouse projection; host capture remains a separate obligation.
    pub(crate) fn pointer_from_host(&mut self,caller:u64,version:u64,phase:crate::native_view::PointerAction,
        x:f32,y:f32,pressed:bool,sink:&mut impl FnMut(crate::scene_events::SceneEvent)->crate::native_events::NativeEventReply)->bool {
        self.pointer_input_from_host(caller,version,&crate::scene_events::ScenePointerInput {
            phase,position:(x,y),primary_pressed:pressed,modifiers:Default::default()},sink)
    }
    pub(crate) fn pointer_input_from_host(&mut self,caller:u64,version:u64,input:&crate::scene_events::ScenePointerInput,
        sink:&mut impl FnMut(crate::scene_events::SceneEvent)->crate::native_events::NativeEventReply)->bool {
        self.invalidate_frame();
        use blitz_traits::events::*;
        use crate::native_view::PointerAction;
        let (x,y)=input.position;let phase=input.phase;let pressed=input.primary_pressed;
        if caller!=self.owner || version!=self.scene_version || !x.is_finite() || !y.is_finite() {return false;}
        let Some(document)=self.document.as_mut() else {return false;};
        let scroll=document.viewport_scroll();let page_x=x as f64+scroll.x;let page_y=y as f64+scroll.y;
        if !page_x.is_finite() || !page_y.is_finite() || page_x.abs()>f32::MAX as f64 || page_y.abs()>f32::MAX as f64 {return false;}
        let mut modifiers=keyboard_types::Modifiers::empty();
        if input.modifiers.shift {modifiers.insert(keyboard_types::Modifiers::SHIFT);}
        if input.modifiers.control {modifiers.insert(keyboard_types::Modifiers::CONTROL);}
        if input.modifiers.alt {modifiers.insert(keyboard_types::Modifiers::ALT);}
        if input.modifiers.meta {modifiers.insert(keyboard_types::Modifiers::META);}
        let pointer=BlitzPointerEvent {id:BlitzPointerId::Mouse,is_primary:true,
            coords:PointerCoords {page_x:page_x as f32,page_y:page_y as f32,client_x:x,client_y:y,screen_x:x,screen_y:y},
            button:MouseEventButton::Main,buttons:if pressed {MouseEventButtons::Primary} else {MouseEventButtons::None},
            mods:modifiers,details:PointerDetails::default(),element:Point::default(),active_pointers:Default::default()};
        let event=match phase {PointerAction::Down=>UiEvent::PointerDown(pointer),PointerAction::Move=>UiEvent::PointerMove(pointer),
            PointerAction::Up=>UiEvent::PointerUp(pointer),PointerAction::Cancel=>UiEvent::PointerCancel(pointer),PointerAction::Leave=>UiEvent::PointerLeave(pointer)};
        // Native press identity is authoritative; retired/no press cannot activate a replacement.
        let suppress_unpaired_up=phase==PointerAction::Up && document.get_mousedown_node_id().is_none();
        let handler=crate::scene_events::SceneEventHandler {version,suppress_unpaired_up,nodes:&self.nodes,semantics:&self.semantics,sink};
        blitz_dom::EventDriver::new(document,handler).handle_ui_event(event);
        if matches!(phase,PointerAction::Up|PointerAction::Cancel) {document.set_mousedown_node_id(None);}
        document.resolve(0.0);true
    }
    /// Route only to the admitted currently focused source node; no editor/default text commit.
    pub(crate) fn key_from_host(&mut self,caller:u64,version:u64,
        input:&crate::native_keys::HostKeyInput<'_>,
        sink:&mut impl FnMut(crate::scene_events::SceneKeyEvent)->crate::native_events::NativeEventReply,
        focus:&mut impl FnMut(crate::scene_events::SceneFocusEvent))->bool {
        self.invalidate_frame();
        use blitz_traits::events::UiEvent;
        if caller!=self.owner || version!=self.scene_version {return false;}
        let before=self.focused_node(caller,version);
        let node=match before {
            Some(node) if self.focus_eligible(node)=>node,
            None if input.key=="Tab" && input.pressed && !input.composing=> {
                let Some(root)=self.parents.iter().find_map(|(id,parent)|parent.is_none().then_some(*id)) else {return false;};root
            },
            _=>return false,
        };
        let Some((element,_))=self.nodes.get(&node) else {return false;};let element=*element;
        let Some(key)=crate::native_keys::native_command_key(input) else {return false;};
        let Some(document)=self.document.as_mut() else {return false;};
        let event=if input.pressed {UiEvent::KeyDown(key)} else {UiEvent::KeyUp(key)};
        let handler=crate::scene_events::SceneKeyHandler {version,node,element,sink};
        blitz_dom::EventDriver::new(document,handler).handle_ui_event(event);
        document.resolve(0.0);
        let after=self.focused_node(caller,version);
        crate::scene_events::notify_focus_transition(version,before,after,focus);
        true
    }
    /// Native focus query projects only own stable scene identity.
    pub(crate) fn focused_node(&self,caller:u64,version:u64)->Option<NodeId> {
        if caller!=self.owner || version!=self.scene_version {return None;}
        let focused=self.document.as_ref()?.get_focussed_node_id()?;
        self.nodes.iter().find_map(|(id,(element,_))|(*element==focused).then_some(*id))
    }
    /// Project source participation after native construction/style batches; Blitz orders Tab.
    /// Source semantics stay raw; this does not compute geometry or choose the next focus.
    fn sync_native_focus_participation(&mut self)->bool {
        let entries:Vec<_>=self.nodes.iter().filter_map(|(id,(element,_))| {
            let semantics=self.semantics.get(id)?;
            (semantics.focusable || semantics.role==Role::Button)
                .then(||(*element,self.focus_eligible(*id)))
        }).collect();
        let Some(document)=self.document.as_mut() else {return false;};
        {
            let mut tree=document.mutate();
            for (element,eligible) in entries {
                tree.set_attribute(element,QualName::new(None,Namespace::from(""),LocalName::from("tabindex")),
                    if eligible {"0"} else {"-1"});
            }
        }
        document.resolve(0.0);true
    }
    /// Source ancestry metadata only; native layout and visibility geometry stay upstream.
    fn focus_eligible(&self,id:NodeId)->bool {
        if !self.semantics.get(&id).is_some_and(|s|s.focusable && !s.disabled) {return false;}
        self.source_visible(id)
    }
    fn source_visible(&self,id:NodeId)->bool {
        let mut current=Some(id);let mut remaining=self.parents.len();
        loop {
            let cursor=match current {Some(cursor)=>cursor,None=>return true};
            if remaining==0 {return false;}
            remaining-=1;
            if !match self.displayed.get(&cursor) {Some(value)=>*value,None=>false} {return false;}
            if match self.semantics.get(&cursor) {Some(value)=>value.hidden,None=>true} {return false;}
            current=match self.parents.get(&cursor) {Some(parent)=>*parent,None=>return false};
        }
    }

    /// Host-authenticated semantic action; no fabricated pointer, key or focus request.
    /// Scene version prevents stale routing, not duplicate actions; host owns replay policy.
    pub(crate) fn activate_from_host(&self,caller:u64,version:u64,id:NodeId,
        sink:&mut impl FnMut(crate::scene_events::SceneActivationEvent))->bool {
        if caller!=self.owner || self.document.as_ref().is_none() || !self.nodes.contains_key(&id) {return false;}
        let can_activate=self.semantics.get(&id).is_some_and(|s|s.role==Role::Button && !s.disabled)
            && self.source_visible(id);
        if !crate::native_semantics::accepts_activation(self.scene_version,version,can_activate) {return false;}
        sink(crate::scene_events::SceneActivationEvent {scene_version:version,node:id});true
    }
    /// Framework focus request; does not acquire OS focus or an external input lease.
    pub(crate) fn focus_from_host(&mut self,caller:u64,version:u64,id:NodeId,
        sink:&mut impl FnMut(crate::scene_events::SceneFocusEvent))->bool {
        self.invalidate_frame();
        if caller!=self.owner || version!=self.scene_version
            || !self.focus_eligible(id) {return false;}
        let Some((element,_))=self.nodes.get(&id) else {return false;};let element=*element;
        let before=self.focused_node(caller,version);
        let Some(document)=self.document.as_mut() else {return false;};
        document.set_focus_to(element);document.resolve(0.0);
        let after=self.focused_node(caller,version);
        if before!=after {
            if let Some(node)=before {sink(crate::scene_events::SceneFocusEvent {scene_version:version,node,focused:false});}
            if let Some(node)=after {sink(crate::scene_events::SceneFocusEvent {scene_version:version,node,focused:true});}
        }
        after==Some(id)
    }
    /// Idempotent native focus clear; external input/capture cancellation remains host-owned.
    pub(crate) fn clear_focus_from_host(&mut self,caller:u64,version:u64,
        sink:&mut impl FnMut(crate::scene_events::SceneFocusEvent))->bool {
        self.invalidate_frame();
        if caller!=self.owner || version!=self.scene_version {return false;}
        let before=self.focused_node(caller,version);
        let Some(document)=self.document.as_mut() else {return false;};
        document.clear_focus();document.resolve(0.0);
        if let Some(node)=before {sink(crate::scene_events::SceneFocusEvent {scene_version:version,node,focused:false});}
        true
    }
    /// Host deactivation cleanup only; host still owns capture release and native input protocols.
    pub(crate) fn deactivate_from_host(&mut self,caller:u64,version:u64,
        sink:&mut impl FnMut(crate::scene_events::SceneFocusEvent))->bool {
        self.invalidate_frame();
        if caller!=self.owner || version!=self.scene_version {return false;}
        let before=self.focused_node(caller,version);
        let Some(document)=self.document.as_mut() else {return false;};
        document.unactive_node();document.set_mousedown_node_id(None);document.clear_hover();document.clear_focus();document.resolve(0.0);
        crate::scene_events::notify_focus_transition(version,before,None,sink);true
    }
    /// Own metadata projection, not a computed accessible-name tree or native OS bridge.
    pub(crate) fn semantics_from_host(&self,caller:u64,id:NodeId)->Option<&Semantics> {
        if caller!=self.owner || !self.document.is_open() {return None;}
        self.semantics.get(&id)
    }

    /// Host supplies physical surface extent and DPI; scene version is independent.
    /// Candidate scale bound matches text-view policy; no window/GPU/OS operation is owned here.
    pub(crate) fn set_viewport_from_host(&mut self,caller:u64,width:u32,height:u32,scale:f32)->bool {
        self.invalidate_frame();
        if caller!=self.owner || !admits_native_dpi_scale_bits(scale.to_bits()) {return false;}
        let Some(document)=self.document.as_mut() else {return false;};
        let mut viewport=document.viewport().clone();
        viewport.window_size=(width,height);viewport.hidpi_scale=scale;
        document.set_viewport(viewport);document.resolve(0.0);
        true
    }
    /// Native client-box projection in view-local logical coordinates; no OS screen transform.
    pub(crate) fn node_bounds_from_host(&self,caller:u64,version:u64,id:NodeId)->Option<jasper_ui_engine::geometry::Rect> {
        if caller!=self.owner || version!=self.scene_version || !self.source_visible(id) {return None;}
        let (element,_)=self.nodes.get(&id)?;
        let document=self.document.as_ref()?;
        // Pinned Blitz subtracts the queried box's own scroll from its client origin.
        // Reject that unsupported border-box case; descendants still use upstream geometry.
        let scroll=document.get_node(*element)?.scroll_offset();
        if scroll.x!=0.0 || scroll.y!=0.0 {return None;}
        let rect=document.get_client_bounding_rect(*element)?;
        if [rect.x,rect.y,rect.width,rect.height].iter().any(|value|!value.is_finite() || value.abs()>f32::MAX as f64)
            || rect.width<0.0 || rect.height<0.0 {return None;}
        Some(jasper_ui_engine::geometry::Rect::new(rect.x as f32,rect.y as f32,rect.width as f32,rect.height as f32))
    }
    /// Versioned host-neutral semantic hierarchy, containing no backend identity.
    pub(crate) fn semantics_snapshot_from_host(&self,caller:u64,version:u64)->Option<crate::scene_events::SceneSemanticsSnapshot> {
        if caller!=self.owner || version!=self.scene_version || !self.document.is_open() {return None;}
        let root=self.parents.iter().find_map(|(id,parent)|parent.is_none().then_some(*id))?;
        let mut nodes=Vec::with_capacity(self.semantics.len());
        for (id,semantics) in &self.semantics {
            nodes.push(crate::scene_events::SceneSemanticsNode {node:*id,semantics:semantics.clone(),
                children:self.children.get(id)?.clone()});
        }
        Some(crate::scene_events::SceneSemanticsSnapshot {scene_version:version,root,nodes})
    }
    /// Host viewport scrolling is separate from per-node source SetScroll offsets.
    /// Host supplies valid extent and authenticates routing; Blitz owns scrolling/painting.
    pub(crate) fn set_viewport_scroll_from_host(&mut self,caller:u64,version:u64,x:f64,y:f64)->bool {
        self.invalidate_frame();
        if caller!=self.owner || version!=self.scene_version || !x.is_finite() || !y.is_finite()
            || x<0.0 || y<0.0 || x>u32::MAX as f64 || y>u32::MAX as f64 {return false;}
        let Some(document)=self.document.as_mut() else {return false;};
        document.set_viewport_scroll(blitz_dom::Point {x,y});true
    }
    #[cfg(feature="cpu-pending")]
    pub(crate) fn render_cpu(&mut self,caller:u64,max_bytes:usize)->Option<Vec<u8>> {
        self.render_cpu_with_damage(caller,max_bytes).map(|(pixels,_)|pixels)
    }
    /// Damage is relative to the previous frame this view produced, not to host surface content.
    /// 损伤相对本视图上一次产出的帧，而非宿主表面内容。
    #[cfg(feature="cpu-pending")]
    pub(crate) fn render_cpu_with_damage(&mut self,caller:u64,max_bytes:usize)
        ->Option<(Vec<u8>,crate::frame_schedule::FrameDamage)> {
        use crate::frame_schedule::{frame_damage,FrameDamage};
        if caller!=self.owner {return None;}
        let document=self.document.as_mut()?;
        let (width,height)=document.viewport().window_size;
        let bytes=crate::cpu_budget::cpu_frame_bytes(width,height,max_bytes)?;
        if let Some((cached_width,cached_height,cached))=&self.frame_cache {
            if crate::frame_schedule::frame_reuse_admits(self.frame_dirty,*cached_width,*cached_height,
                cached.len(),width,height,bytes) {return Some((cached.clone(),FrameDamage::Unchanged));}
        }
        let scale=document.viewport().scale_f64();
        let pixels=anyrender::render_to_buffer::<anyrender_vello_cpu::VelloCpuImageRenderer,_>(
            |scene|blitz_paint::paint_scene(scene,document,scale,width,height,0,0),width,height);
        if pixels.len()!=bytes {return None;}
        let previous=self.frame_cache.as_ref().map(|(w,h,cached)|(*w,*h,cached.as_slice()));
        let damage=frame_damage(previous,width,height,&pixels);
        self.frame_cache=Some((width,height,pixels.clone()));self.frame_dirty=false;
        Some((pixels,damage))
    }
    /// Some(true) while a possible visible change is not yet painted; None for a foreign
    /// caller or retired view. Hosts skip render calls while this is Some(false).
    /// 存在尚未绘制的可能可见变化时为 Some(true)；非归属调用方或已退役视图为 None。
    pub(crate) fn needs_frame(&self,caller:u64)->Option<bool> {
        if caller!=self.owner || !self.document.is_open() {return None;}
        Some(self.frame_dirty)
    }
    /// Host-forced repaint (e.g. lost or replaced presentation surface).
    /// 宿主强制重绘（如呈现表面丢失或更换）。
    pub(crate) fn invalidate_frame_from_host(&mut self,caller:u64)->bool {
        if caller!=self.owner || !self.document.is_open() {return false;}
        // The host's copy is gone: the next damage report must be Full, not a pixel diff.
        // 宿主副本已失效：下一次损伤必须为 Full，而非像素差分。
        self.frame_cache=None;
        self.invalidate_frame();true
    }
    /// Conservative damage: every mutating entry marks the view dirty before admission,
    /// so a rejected call can only cause an extra paint, never a stale frame.
    /// 保守失效：每个修改入口在准入前即标脏，被拒调用至多多绘一帧，绝不复用过期帧。
    fn invalidate_frame(&mut self) {self.frame_dirty=true;}
    /// Copy backend RGBA8 bytes into a host-injected software surface, preserving padding.
    /// This is client buffer adaptation, not surface acquisition, IPC import or presentation.
    #[cfg(feature="cpu-pending")]
    pub(crate) fn render_to_host_surface(&mut self,caller:u64,version:u64,destination:&mut [u8],stride:usize,max_render_bytes:usize)->bool {
        if caller!=self.owner || version!=self.scene_version {return false;}
        let Some(document)=self.document.as_ref() else {return false;};
        let (width,height)=document.viewport().window_size;
        orchestrate_native_host_surface(width,height,destination,stride,max_render_bytes,
            |budget|self.render_cpu(caller,budget))
    }
    /// Same as `render_to_host_surface`, also reporting damage relative to the previous frame
    /// this view produced. A failed write drops the cache so the next report is Full.
    /// 同 `render_to_host_surface`，并报告相对本视图上一帧的损伤；写入失败则丢弃缓存，下次为 Full。
    #[cfg(feature="cpu-pending")]
    pub(crate) fn render_to_host_surface_with_damage(&mut self,caller:u64,version:u64,destination:&mut [u8],stride:usize,max_render_bytes:usize)
        ->Option<crate::frame_schedule::FrameDamage> {
        if caller!=self.owner || version!=self.scene_version {return None;}
        let (width,height)=self.document.as_ref()?.viewport().window_size;
        let mut damage=None;
        let written=orchestrate_native_host_surface(width,height,destination,stride,max_render_bytes,
            |budget|{let (pixels,found)=self.render_cpu_with_damage(caller,budget)?;damage=Some(found);Some(pixels)});
        if !written {
            if damage.is_some() {self.frame_cache=None;}
            return None;
        }
        damage
    }
    /// Like `render_to_host_surface_with_damage`, but writes only the damaged pixels: the host
    /// guarantees `destination` holds the frame this view produced last (otherwise it must
    /// invalidate first, making the next damage Full). Surface admission precedes painting;
    /// a failed write drops the cache so the next report is Full.
    /// 同 `render_to_host_surface_with_damage`，但只写受损像素：宿主保证 `destination` 持有本视图
    /// 上一次产出的帧（否则须先失效，使下一次为 Full）。Surface 准入先于绘制；写入失败丢弃缓存。
    #[cfg(feature="cpu-pending")]
    pub(crate) fn render_to_host_surface_damaged(&mut self,caller:u64,version:u64,destination:&mut [u8],stride:usize,max_render_bytes:usize)
        ->Option<crate::frame_schedule::FrameDamage> {
        use jasper_ui_engine::surface::{rgba8_row_bytes,surface_span};
        if caller!=self.owner || version!=self.scene_version {return None;}
        let (width,height)=self.document.as_ref()?.viewport().window_size;
        let row_bytes=rgba8_row_bytes(width,destination.len())?;
        surface_span(row_bytes,height as usize,stride,destination.len())?;
        let (pixels,damage)=self.render_cpu_with_damage(caller,max_render_bytes)?;
        if !crate::frame_schedule::write_damaged_frame(destination,stride,width,height,&pixels,damage) {
            self.frame_cache=None;
            return None;
        }
        Some(damage)
    }
    /// Render through the native painter then the existing backend submission/presentation seam.
    /// 经原生绘制后接入既有后端提交/呈现接缝；不生成或重放旧 PaintOp。
    /// Backend submit/present failures propagate; external presentation rollback is not promised.
    /// 后端提交/呈现失败原样传播，不承诺外部呈现回滚。
    #[cfg(feature="cpu-pending")]
    pub(crate) fn render_to_backend(&mut self,caller:u64,version:u64,
        backend:&mut impl jasper_ui_engine::backend::RenderBackend,budget:usize)
        ->Result<(),jasper_ui_engine::backend::BackendError> {
        let extent=self.document.as_ref().map(|document|document.viewport().window_size);
        orchestrate_native_frame(self.owner,caller,self.scene_version,version,extent,budget,
            |render_budget|self.render_cpu(caller,render_budget),backend)
    }

    /// Report final native focus loss after successful view retirement.
    /// Host still owns pointer release and external input-session cancellation.
    pub(crate) fn close_with_focus_events(&mut self,caller:u64,version:u64,
        sink:&mut impl FnMut(crate::scene_events::SceneFocusEvent))->bool {
        if caller!=self.owner || version!=self.scene_version {return false;}
        let before=self.focused_node(caller,version);
        if !self.close(caller) {return false;}
        crate::scene_events::notify_focus_transition(version,before,None,sink);
        true
    }
}
// BEGIN VERIFIED NATIVE SCENE CLOSE KERNEL
impl NativeScene {
    pub(crate) fn close(&mut self,caller:u64)->bool {
        caller==self.owner && self.document.close()
    }
}
// END VERIFIED NATIVE SCENE CLOSE KERNEL
/// Relation over temporary source-ID topology only; native deletion remains upstream.
fn source_subtree_contains(parents:&BTreeMap<NodeId,Option<NodeId>>,node:NodeId,ancestor:NodeId)->bool {
    let mut current=Some(node);let mut remaining=parents.len();
    loop {
        let cursor=match current {Some(cursor)=>cursor,None=>return false};
        if cursor.0==ancestor.0 {return true;}
        if remaining==0 {return false;}
        remaining-=1;
        current=match parents.get(&cursor) {Some(parent)=>*parent,None=>None};
    }
}

/// Bounded source admission shared by initial import and single-subtree insertion.
fn prepare_subtree(root:&Node,base_depth:usize,existing:&BTreeSet<NodeId>)->Option<Vec<Vec<(&'static str,String)>>> {
    let mut stack=vec![(root,base_depth)];let mut prepared=Vec::new();let mut seen=BTreeSet::new();
    while let Some((node,depth))=stack.pop() {
        if depth>128 || existing.contains(&node.id) || !seen.insert(node.id) || matches!(node.kind,NodeKind::Canvas(_))
            || native_scroll_offset(node.scroll).is_none() {return None;}
        prepared.push(style_properties(&node.style,depth==0)?);
        if !scene_work_fits(existing.len().checked_add(prepared.len())?,stack.len(),node.children.len()) {return None;}
        for child in node.children.iter().rev() {stack.push((child,depth+1));}
    }
    Some(prepared)
}

// BEGIN VERIFIED SOURCE INSERT INDEX KERNEL
/// Match existing ScenePatch insertion semantics: oversized index appends.
pub(crate) fn source_insert_index(index:usize,children:usize)->usize {
    index.min(children)
}
// END VERIFIED SOURCE INSERT INDEX KERNEL

// BEGIN VERIFIED SCENE UPDATE ADMISSION KERNEL
/// Own patch admission only; host authentication and native mutation remain separate.
pub(crate) fn accepts_scene_update(owner:u64,caller:u64,current:u64,expected:u64,next:u64,count:usize,open:bool)->bool {
    if owner!=caller || !open || count==0 || !scene_work_fits(0,0,count) {return false;}
    match next_ui_update_version(current,expected) {
        Some(successor)=>next==successor,
        None=>false,
    }
}
// END VERIFIED SCENE UPDATE ADMISSION KERNEL

// BEGIN VERIFIED SCENE WORK CAPACITY KERNEL
/// Candidate admission count only; not an allocator bound for text, fonts or upstream scratch.
pub(crate) fn scene_work_fits(prepared:usize,pending:usize,incoming:usize)->bool {
    if prepared>4096 || pending>4096-prepared {return false;}
    incoming<=4096-prepared-pending
}
// END VERIFIED SCENE WORK CAPACITY KERNEL

fn semantic_attributes(semantics:&Semantics)->Vec<(&'static str,String)> {
    let mut values=Vec::new();
    let role=match semantics.role {
        Role::Button=>Some("button"),Role::TextField=>Some("textbox"),Role::Checkbox=>Some("checkbox"),
        Role::Radio=>Some("radio"),Role::Switch=>Some("switch"),Role::Slider=>Some("slider"),
        Role::Image=>Some("img"),Role::Link=>Some("link"),Role::List=>Some("list"),
        Role::ListItem=>Some("listitem"),Role::Dialog=>Some("dialog"),Role::Menu=>Some("menu"),
        Role::MenuItem=>Some("menuitem"),Role::ScrollView=>Some("region"),
        Role::Container|Role::Text|Role::Unknown=>None,
    };
    if let Some(role)=role {values.push(("role",role.to_owned()));}
    for (key,value) in [("aria-label",&semantics.label),("aria-description",&semantics.description),("aria-valuetext",&semantics.value)] {
        if !value.is_empty() {values.push((key,value.clone()));}
    }
    values.push(("aria-disabled",semantics.disabled.to_string()));
    values.push(("aria-hidden",semantics.hidden.to_string()));
    if semantics.disabled && semantics.role==Role::Button {values.push(("disabled",String::new()));}
    // Explicitly suppress native default button tab participation when Framework says false.
    if semantics.focusable || semantics.role==Role::Button {
        values.push(("tabindex",if semantics.focusable && !semantics.disabled {"0"} else {"-1"}.to_owned()));
    }
    values
}

fn style_properties(style:&Style,root:bool)->Option<Vec<(&'static str,String)>> {
    let mut values=Vec::new();
    for (key,length) in [("width",style.width),("height",style.height),("min-width",style.min_width),
        ("max-width",style.max_width),("min-height",style.min_height),("max-height",style.max_height)] {
        let value=match length {
            Length::Auto if root && key=="width"=>"100vw".to_owned(),
            Length::Auto if root && key=="height"=>"100vh".to_owned(),
            Length::Auto=>if key.starts_with("max-") {"none".to_owned()} else {"auto".to_owned()},
            Length::Px(v) if v.is_finite() && v>=0.0=>format!("{v}px"),
            Length::Percent(v) if v.is_finite() && v>=0.0=>format!("{v}%"),
            _=>return None,
        };values.push((key,value));
    }
    for (key,value) in [("padding-top",style.padding.top),("padding-right",style.padding.right),
        ("padding-bottom",style.padding.bottom),("padding-left",style.padding.left),
        ("margin-top",style.margin.top),("margin-right",style.margin.right),
        ("margin-bottom",style.margin.bottom),("margin-left",style.margin.left),
        ("gap",style.gap),("border-width",style.border_width),("font-size",style.font_size)] {
        if !value.is_finite() || value<0.0 || (key=="font-size" && value==0.0) {return None;}
        values.push((key,format!("{value}px")));
    }
    if !style.flex_grow.is_finite() || style.flex_grow<0.0 {return None;}
    values.push(("flex-grow",style.flex_grow.to_string()));
    for (key,color) in [("background-color",style.background),("border-color",style.border_color),("color",style.color)] {
        values.push((key,format!("#{:02x}{:02x}{:02x}{:02x}",color.r,color.g,color.b,color.a)));
    }
    for (key,value) in [
        ("display",match style.display {Display::Block=>"block",Display::Flex=>"flex",Display::None=>"none"}),
        ("flex-direction",match style.direction {FlexDirection::Row=>"row",FlexDirection::Column=>"column"}),
        ("align-items",match style.align_items {Align::Start=>"flex-start",Align::Center=>"center",Align::End=>"flex-end",Align::Stretch=>"stretch"}),
        ("justify-content",match style.justify_content {Justify::Start=>"flex-start",Justify::Center=>"center",Justify::End=>"flex-end",Justify::SpaceBetween=>"space-between"}),
        ("overflow",match style.overflow {Overflow::Visible=>"visible",Overflow::Hidden=>"hidden",Overflow::Scroll=>"scroll"}),
        ("border-style","solid"),
        ("box-sizing","border-box")
    ] {values.push((key,value.to_owned()));}
    Some(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jasper_ui_engine::scene::{Node,NodeId};
    #[test]
    fn visibility_rejects_cyclic_or_incomplete_source_metadata_without_native_focus() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
        let button=Node::text(NodeId(2),"Action").with_semantics(Semantics::default().role(Role::Button).focusable(true));
        let source=Scene::new(Node::container(NodeId(1)).child(button));let version=source.version();
        let mut view=NativeScene::from_scene(10,&source,&bytes,160,80).unwrap();assert!(view.source_visible(NodeId(2)));
        view.parents.insert(NodeId(1),Some(NodeId(2)));
        assert!(!view.source_visible(NodeId(2)));assert!(!view.focus_from_host(10,version,NodeId(2),&mut |_|panic!("cyclic focus")));
        assert!(!view.activate_from_host(10,version,NodeId(2),&mut |_|panic!("cyclic activation")));
        view.parents.insert(NodeId(1),None);assert!(view.source_visible(NodeId(2)));
        view.semantics.get_mut(&NodeId(1)).unwrap().hidden=true;assert!(!view.source_visible(NodeId(2)));
        view.semantics.get_mut(&NodeId(1)).unwrap().hidden=false;view.displayed.remove(&NodeId(2));
        assert!(!view.source_visible(NodeId(2)));assert_eq!(view.focused_node(10,version),None);
    }

    #[test]
    fn source_ancestry_is_bounded_even_for_cyclic_metadata() {
        let parents=BTreeMap::from([(NodeId(1),None),(NodeId(2),Some(NodeId(1))),
            (NodeId(3),Some(NodeId(2)))]);
        assert!(source_subtree_contains(&parents,NodeId(3),NodeId(1)));
        assert!(source_subtree_contains(&parents,NodeId(2),NodeId(2)));
        assert!(!source_subtree_contains(&parents,NodeId(1),NodeId(3)));
        assert!(!source_subtree_contains(&parents,NodeId(99),NodeId(1)));
        let cycle=BTreeMap::from([(NodeId(1),Some(NodeId(2))),(NodeId(2),Some(NodeId(1)))]);
        assert!(!source_subtree_contains(&cycle,NodeId(1),NodeId(3)));
        assert!(source_subtree_contains(&cycle,NodeId(1),NodeId(2)));
        assert!(!source_subtree_contains(&BTreeMap::new(),NodeId(1),NodeId(2)));
        assert!(source_subtree_contains(&BTreeMap::new(),NodeId(1),NodeId(1)));
    }

    #[test]
    fn host_semantic_hierarchy_tracks_source_frame_order_and_retirement() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
        let mut scene=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Display only")));
        let mut native=NativeScene::from_scene(10,&scene,&bytes,160,80).unwrap();let version=scene.version();
        let before=native.semantics_snapshot_from_host(10,version).unwrap();
        assert_eq!(before.root,NodeId(1));assert_eq!(before.nodes[1].semantics.label,"");
        assert!(native.semantics_snapshot_from_host(11,version).is_none());
        let metadata=Semantics {role:Role::Button,label:"Explicit name".into(),description:"Host metadata".into(),..Default::default()};
        let patches=[ScenePatch::Insert {parent:NodeId(1),index:0,node:Node::text(NodeId(3),"Caption").with_semantics(metadata.clone())},
            ScenePatch::Remove {id:NodeId(2)}];
        scene.apply_batch_at(version,&patches).unwrap();assert!(native.apply_patches(10,version,scene.version(),&patches));
        assert!(native.semantics_snapshot_from_host(10,version).is_none());
        let after=native.semantics_snapshot_from_host(10,scene.version()).unwrap();
        assert_eq!(after.scene_version,scene.version());assert_eq!(after.nodes.len(),2);
        assert_eq!(after.nodes[0].children,vec![NodeId(3)]);assert_eq!(after.nodes[1].semantics,metadata);
        assert!(native.close(10));assert!(native.semantics_snapshot_from_host(10,scene.version()).is_none());
    }

    #[test]
    #[cfg(feature="cpu-pending")]
    fn host_viewport_scroll_changes_pixels_and_rejects_stale_routing() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
        let scene=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Viewport 中文"))
            .child(Node::rect(NodeId(3)).with_style(Style::default().size(Length::Px(80.0),Length::Px(120.0)).background(Color::RED))));
        let mut native=NativeScene::from_scene(10,&scene,&bytes,160,80).unwrap();let version=scene.version();
        let before=native.render_cpu(10,1024*1024).unwrap();
        assert!(native.set_viewport_scroll_from_host(10,version,0.0,24.0));
        let scrolled=native.render_cpu(10,1024*1024).unwrap();assert!(before!=scrolled);
        for (caller,v,x,y) in [(11,version,0.0,0.0),(10,version+1,0.0,0.0),
            (10,version,f64::NAN,0.0),(10,version,0.0,f64::INFINITY),
            (10,version,-1.0,0.0),(10,version,0.0,u32::MAX as f64+1.0)] {
            assert!(!native.set_viewport_scroll_from_host(caller,v,x,y));
            assert!(scrolled==native.render_cpu(10,1024*1024).unwrap());
        }
        assert_eq!(native.snapshot_identity(10),Some((version,3)));
        assert!(native.set_viewport_scroll_from_host(10,version,0.0,0.0));
        assert!(before==native.render_cpu(10,1024*1024).unwrap());
        assert!(native.close(10));assert!(!native.set_viewport_scroll_from_host(10,version,0.0,0.0));
    }

    #[test]
    fn semantic_activation_routes_own_node_without_requiring_keyboard_focus() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
        let mut button=Node::text(NodeId(2),"Save");button.semantics=Semantics {role:Role::Button,..Default::default()};
        let mut disabled=button.clone();disabled.id=NodeId(3);disabled.semantics.disabled=true;
        let mut hidden=Node::container(NodeId(4)).child(button.clone());hidden.children[0].id=NodeId(5);hidden.semantics.hidden=true;
        let mut absent=Node::container(NodeId(6)).child(button.clone());absent.children[0].id=NodeId(7);absent.style.display=Display::None;
        let scene=Scene::new(Node::container(NodeId(1)).child(button).child(disabled).child(hidden).child(absent));
        let mut native=NativeScene::from_scene(10,&scene,&bytes,160,80).unwrap();let version=scene.version();
        let mut events=Vec::new();
        for (caller,v,id) in [(11,version,2),(10,version+1,2),(10,version,1),(10,version,3),
            (10,version,5),(10,version,7),(10,version,999)] {
            assert!(!native.activate_from_host(caller,v,NodeId(id),&mut |e|events.push(e)));
        }
        assert!(events.is_empty());assert_eq!(native.focused_node(10,version),None);
        assert!(native.activate_from_host(10,version,NodeId(2),&mut |e|events.push(e)));
        assert_eq!(events.len(),1);assert_eq!(events[0].node,NodeId(2));assert_eq!(events[0].scene_version,version);
        assert_eq!(native.focused_node(10,version),None);
        assert!(native.close(10));
        assert!(!native.activate_from_host(10,version,NodeId(2),&mut |e|events.push(e)));assert_eq!(events.len(),1);
    }

    #[test]
    #[cfg(feature="cpu-pending")]
    fn native_scene_copies_into_injected_host_stride_without_touching_padding() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
        let scene=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Software host 中文")));
        let mut native=NativeScene::from_scene(10,&scene,&bytes,160,80).unwrap();let version=scene.version();
        let packed=native.render_cpu(10,1024*1024).unwrap();let row=160*4;let stride=row+16;
        let mut destination=vec![0xa5;79*stride+row];
        for (caller,v,s,budget) in [(11,version,stride,1024*1024),(10,version+1,stride,1024*1024),
            (10,version,row-1,1024*1024),(10,version,stride,1)] {
            assert!(!native.render_to_host_surface(caller,v,&mut destination,s,budget));
            assert!(destination.iter().all(|byte|*byte==0xa5));
        }
        assert!(native.render_to_host_surface(10,version,&mut destination,stride,1024*1024));
        let view=jasper_ui_engine::surface::Rgba8SurfaceView::new(&destination,160,80,stride).unwrap();
        for y in 0..80 {assert_eq!(view.row(y).unwrap(),&packed[y*row..(y+1)*row]);}
        for y in 0..79 {assert!(destination[y*stride+row..(y+1)*stride].iter().all(|byte|*byte==0xa5));}
        let mut short=vec![0xa5;destination.len()-1];
        assert!(!native.render_to_host_surface(10,version,&mut short,stride,1024*1024));
        assert!(short.iter().all(|byte|*byte==0xa5));
    }

    #[test]
    #[cfg(feature="cpu-pending")]
    fn mixed_source_frame_preflights_new_targets_then_matches_authoritative_scene() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
        let mut scene=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Original")));
        let mut native=NativeScene::from_scene(10,&scene,&bytes,320,240).unwrap();let version=scene.version();
        let before=native.render_cpu(10,1024*1024).unwrap();
        let invalid=[ScenePatch::Insert {parent:NodeId(1),index:0,node:Node::text(NodeId(3),"Must not appear")},
            ScenePatch::UpdateText {id:NodeId(99),text:"No target".into()}];
        assert!(!native.apply_patches(10,version,version+1,&invalid));
        assert!(before==native.render_cpu(10,1024*1024).unwrap());assert!(!native.nodes.contains_key(&NodeId(3)));
        let patches=[ScenePatch::Insert {parent:NodeId(1),index:0,node:Node::container(NodeId(3)).child(Node::text(NodeId(4),"Old"))},
            ScenePatch::UpdateText {id:NodeId(4),text:"New 中文".into()},
            ScenePatch::Insert {parent:NodeId(3),index:usize::MAX,node:Node::text(NodeId(5),"Temporary")},
            ScenePatch::UpdateStyle {id:NodeId(3),style:Style::default().background(Color::BLUE)},
            ScenePatch::Remove {id:NodeId(2)},ScenePatch::Remove {id:NodeId(5)}];
        scene.apply_batch_at(version,&patches).unwrap();assert!(native.apply_patches(10,version,scene.version(),&patches));
        assert_eq!(native.snapshot_identity(10),Some((scene.version(),3)));
        assert_eq!(native.children.get(&NodeId(1)),Some(&vec![NodeId(3)]));
        assert_eq!(native.children.get(&NodeId(3)),Some(&vec![NodeId(4)]));
        let mut reference=NativeScene::from_scene(10,&scene,&bytes,320,240).unwrap();
        assert!(native.render_cpu(10,1024*1024).unwrap()==reference.render_cpu(10,1024*1024).unwrap());
        let version=scene.version();let patches=[ScenePatch::Remove {id:NodeId(3)},
            ScenePatch::Insert {parent:NodeId(1),index:0,node:Node::text(NodeId(3),"Replaced kind")}];
        scene.apply_batch_at(version,&patches).unwrap();assert!(native.apply_patches(10,version,scene.version(),&patches));
        assert_eq!(native.snapshot_identity(10),Some((scene.version(),2)));assert!(!native.nodes.contains_key(&NodeId(4)));
        let mut reference=NativeScene::from_scene(10,&scene,&bytes,320,240).unwrap();
        assert!(native.render_cpu(10,1024*1024).unwrap()==reference.render_cpu(10,1024*1024).unwrap());
    }

    #[test]
    #[cfg(feature="cpu-pending")]
    fn source_scene_clamps_out_of_range_insert_index_to_append() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
        let mut scene=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"First")));
        let mut native=NativeScene::from_scene(10,&scene,&bytes,320,200).unwrap();let version=scene.version();
        let patches=[ScenePatch::Insert {parent:NodeId(1),index:usize::MAX,node:Node::text(NodeId(3),"Append")}];
        scene.apply_batch_at(version,&patches).unwrap();
        assert!(native.apply_patches(10,version,scene.version(),&patches));
        assert_eq!(native.children.get(&NodeId(1)),Some(&vec![NodeId(2),NodeId(3)]));
        let mut reference=NativeScene::from_scene(10,&scene,&bytes,320,200).unwrap();
        assert!(native.render_cpu(10,1024*1024).unwrap()==reference.render_cpu(10,1024*1024).unwrap());
    }

    #[test]
    #[cfg(feature="cpu-pending")]
    fn source_insert_subtree_preserves_order_identity_and_snapshot_pixels() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
        let mut scene=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"First"))
            .child(Node::text(NodeId(3),"Last")));
        let mut native=NativeScene::from_scene(10,&scene,&bytes,320,240).unwrap();
        let version=scene.version();let before=native.render_cpu(10,1024*1024).unwrap();
        for patch in [ScenePatch::Insert {parent:NodeId(1),index:0,node:Node::text(NodeId(2),"Duplicate")},
            ScenePatch::Insert {parent:NodeId(99),index:0,node:Node::text(NodeId(9),"Bad parent")}] {
            assert!(!native.apply_patches(10,version,version+1,&[patch]));
        }
        assert!(before==native.render_cpu(10,1024*1024).unwrap());
        let mut button=Node::text(NodeId(5),"Inserted button");button.semantics=Semantics {role:Role::Button,focusable:true,..Default::default()};
        let incoming=Node::container(NodeId(4)).child(button).child(Node::text(NodeId(6),"Inserted 中文"));
        let patches=[ScenePatch::Insert {parent:NodeId(1),index:1,node:incoming}];
        scene.apply_batch_at(version,&patches).unwrap();
        assert!(native.apply_patches(10,version,scene.version(),&patches));
        assert_eq!(native.children.get(&NodeId(1)),Some(&vec![NodeId(2),NodeId(4),NodeId(3)]));
        let mut reference=NativeScene::from_scene(10,&scene,&bytes,320,240).unwrap();
        assert!(native.render_cpu(10,1024*1024).unwrap()==reference.render_cpu(10,1024*1024).unwrap(),"inserted native subtree must match source order");
        assert!(native.focus_from_host(10,scene.version(),NodeId(5),&mut |_|{}));
        for (id,index) in [(NodeId(7),0),(NodeId(8),4)] {
            let version=scene.version();let patches=[ScenePatch::Insert {parent:NodeId(1),index,node:Node::text(id,"Edge insertion")}];
            scene.apply_batch_at(version,&patches).unwrap();assert!(native.apply_patches(10,version,scene.version(),&patches));
        }
        assert_eq!(native.snapshot_identity(10),Some((scene.version(),8)));
        let mut reference=NativeScene::from_scene(10,&scene,&bytes,320,240).unwrap();
        // Focus styling is transient; compare with equal native focus state.
        assert!(reference.focus_from_host(10,scene.version(),NodeId(5),&mut |_|{}));
        assert!(native.render_cpu(10,1024*1024).unwrap()==reference.render_cpu(10,1024*1024).unwrap());
    }

    #[test]
    #[cfg(feature="cpu-pending")]
    fn source_subtree_removal_cleans_descendant_identity_semantics_and_focus() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
        let mut button=Node::text(NodeId(3),"Nested");button.semantics=Semantics {role:Role::Button,focusable:true,..Default::default()};
        let mut scene=Scene::new(Node::container(NodeId(1)).child(Node::container(NodeId(2)).child(button)
            .child(Node::text(NodeId(4),"Other nested"))).child(Node::rect(NodeId(5))));
        let mut native=NativeScene::from_scene(10,&scene,&bytes,320,200).unwrap();let version=scene.version();
        assert!(native.focus_from_host(10,version,NodeId(3),&mut |_|{}));
        let before=native.render_cpu(10,1024*1024).unwrap();
        let invalid=[ScenePatch::Remove {id:NodeId(2)},ScenePatch::UpdateText {id:NodeId(4),text:"late".into()}];
        assert!(!native.apply_patches(10,version,version+1,&invalid));
        assert!(before==native.render_cpu(10,1024*1024).unwrap());
        let patches=[ScenePatch::Remove {id:NodeId(2)}];scene.apply_batch_at(version,&patches).unwrap();let mut focus=Vec::new();
        assert!(native.apply_patches_with_focus_events(10,version,scene.version(),&patches,&mut |e|focus.push((e.node,e.focused))));
        assert_eq!(focus,vec![(NodeId(3),false)]);assert_eq!(native.snapshot_identity(10),Some((scene.version(),2)));
        for id in [NodeId(2),NodeId(3),NodeId(4)] {
            assert!(native.semantics_from_host(10,id).is_none());assert!(!native.nodes.contains_key(&id));
        }
        let mut reference=NativeScene::from_scene(10,&scene,&bytes,320,200).unwrap();
        assert!(native.render_cpu(10,1024*1024).unwrap()==reference.render_cpu(10,1024*1024).unwrap());
    }

    #[test]
    #[cfg(feature="cpu-pending")]
    fn source_leaf_removal_preflights_order_and_retires_node_focus_metadata() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
        let mut button=Node::text(NodeId(2),"Remove");button.semantics=Semantics {role:Role::Button,focusable:true,..Default::default()};
        let mut scene=Scene::new(Node::container(NodeId(1)).child(button).child(Node::rect(NodeId(3))));
        let mut native=NativeScene::from_scene(10,&scene,&bytes,320,200).unwrap();let version=scene.version();
        assert!(native.focus_from_host(10,version,NodeId(2),&mut |_|{}));
        let before=native.render_cpu(10,1024*1024).unwrap();
        let invalid=[ScenePatch::Remove {id:NodeId(2)},ScenePatch::UpdateText {id:NodeId(2),text:"Late".into()}];
        assert!(!native.apply_patches(10,version,version+1,&invalid));
        assert!(before==native.render_cpu(10,1024*1024).unwrap());
        assert_eq!(native.focused_node(10,version),Some(NodeId(2)));
        assert!(!native.apply_patches(10,version,version+1,&[ScenePatch::Remove {id:NodeId(1)}]));
        let patches=[ScenePatch::UpdateText {id:NodeId(2),text:"Before remove".into()},ScenePatch::Remove {id:NodeId(2)},
            ScenePatch::UpdateStyle {id:NodeId(3),style:Style::default().size(Length::Px(80.0),Length::Px(30.0)).background(Color::GREEN)}];
        scene.apply_batch_at(version,&patches).unwrap();let mut focus=Vec::new();
        assert!(native.apply_patches_with_focus_events(10,version,scene.version(),&patches,
            &mut |event|focus.push((event.node,event.focused))));
        assert_eq!(focus,vec![(NodeId(2),false)]);
        assert_eq!(native.snapshot_identity(10),Some((scene.version(),2)));
        assert!(native.semantics_from_host(10,NodeId(2)).is_none());
        let mut reference=NativeScene::from_scene(10,&scene,&bytes,320,200).unwrap();
        assert!(native.render_cpu(10,1024*1024).unwrap()==reference.render_cpu(10,1024*1024).unwrap());
    }

    #[test]
    fn scene_retirement_notifies_focus_loss_once_after_owner_version_admission() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
        let mut button=Node::text(NodeId(2),"Save");button.semantics=Semantics {role:Role::Button,focusable:true,..Default::default()};
        let scene=Scene::new(Node::container(NodeId(1)).child(button));
        let mut native=NativeScene::from_scene(10,&scene,&bytes,320,200).unwrap();let version=scene.version();
        assert!(native.focus_from_host(10,version,NodeId(2),&mut |_|{}));
        assert!(!native.close_with_focus_events(11,version,&mut |_|panic!("foreign close")));
        assert!(!native.close_with_focus_events(10,version+1,&mut |_|panic!("stale close")));
        assert_eq!(native.focused_node(10,version),Some(NodeId(2)));
        let mut lost=Vec::new();
        assert!(native.close_with_focus_events(10,version,&mut |e|lost.push((e.scene_version,e.node,e.focused))));
        assert_eq!(lost,vec![(version,NodeId(2),false)]);
        assert!(!native.close_with_focus_events(10,version,&mut |_|panic!("duplicate close")));
        assert!(native.snapshot_identity(10).is_none());
    }

    #[test]
    fn pointer_and_style_batch_publish_actual_scene_focus_transitions_once() {
        use crate::{native_view::PointerAction,native_events::NativeEventReply};
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
        let mut button=Node::text(NodeId(2),"Save");button.semantics=Semantics {role:Role::Button,focusable:true,..Default::default()};
        let mut scene=Scene::new(Node::container(NodeId(1)).child(button));
        let mut native=NativeScene::from_scene(10,&scene,&bytes,320,200).unwrap();
        let mut notifications=Vec::new();let version=scene.version();
        // Pinned Blitz does not focus a generic button on Down; Framework requests focus explicitly.
        assert!(native.focus_from_host(10,version,NodeId(2),
            &mut |e|notifications.push((e.scene_version,e.node,e.focused))));
        assert!(native.pointer_with_focus_events_from_host(10,version,PointerAction::Down,12.0,12.0,true,
            &mut |_|NativeEventReply::default(),&mut |_|panic!("unchanged pointer focus")));
        assert_eq!(notifications,vec![(version,NodeId(2),true)]);
        assert!(native.pointer_with_focus_events_from_host(10,version,PointerAction::Up,12.0,12.0,false,
            &mut |_|NativeEventReply::default(),&mut |e|notifications.push((e.scene_version,e.node,e.focused))));
        assert_eq!(notifications,vec![(version,NodeId(2),true),(version,NodeId(2),false)]);
        assert!(native.focus_from_host(10,version,NodeId(2),
            &mut |e|notifications.push((e.scene_version,e.node,e.focused))));
        let patches=[ScenePatch::UpdateStyle {id:NodeId(1),style:Style::default().display(Display::None)}];
        assert!(!native.apply_patches_with_focus_events(11,version,version+1,&patches,&mut |_|panic!("foreign patch focus")));
        scene.apply_batch_at(version,&patches).unwrap();
        assert!(native.apply_patches_with_focus_events(10,version,scene.version(),&patches,
            &mut |e|notifications.push((e.scene_version,e.node,e.focused))));
        assert_eq!(notifications,vec![(version,NodeId(2),true),(version,NodeId(2),false),
            (version,NodeId(2),true),(scene.version(),NodeId(2),false)]);
        assert!(!native.apply_patches_with_focus_events(10,version,scene.version(),&patches,&mut |_|panic!("stale patch focus")));
    }

    #[test]
    fn scene_focus_rejects_display_none_and_hidden_ancestors() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
        let mut child=Node::text(NodeId(2),"Focus");child.semantics=Semantics {role:Role::Button,focusable:true,..Default::default()};
        let mut scene=Scene::new(Node::container(NodeId(1)).child(child));
        scene.root.style.display=Display::None;
        let mut native=NativeScene::from_scene(10,&scene,&bytes,320,200).unwrap();
        assert!(!native.focus_from_host(10,scene.version(),NodeId(2),&mut |_|{}));
        scene.root.style.display=Display::Flex;scene.root.semantics.hidden=true;
        let mut native=NativeScene::from_scene(10,&scene,&bytes,320,200).unwrap();
        assert!(!native.focus_from_host(10,scene.version(),NodeId(2),&mut |_|{}));
        scene.root.semantics.hidden=false;
        let mut native=NativeScene::from_scene(10,&scene,&bytes,320,200).unwrap();
        assert!(native.focus_from_host(10,scene.version(),NodeId(2),&mut |_|{}));
        let expected=scene.version();
        let patches=[ScenePatch::UpdateStyle {id:NodeId(1),style:Style::default().display(Display::None)}];
        scene.apply_batch_at(expected,&patches).unwrap();
        assert!(native.apply_patches(10,expected,scene.version(),&patches));
        assert!(native.focused_node(10,scene.version()).is_none());
        assert!(!native.focus_from_host(10,scene.version(),NodeId(2),&mut |_|panic!("hidden after patch")));
    }

    #[test]
    fn scene_native_focus_checks_semantics_and_notifies_actual_transitions() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
        let mut first=Node::text(NodeId(2),"First");first.semantics=Semantics {role:Role::Button,focusable:true,..Default::default()};
        let mut second=Node::text(NodeId(3),"Second");second.semantics=first.semantics.clone();
        let mut disabled=Node::text(NodeId(4),"Disabled");disabled.semantics=Semantics {disabled:true,..first.semantics.clone()};
        let mut hidden=Node::text(NodeId(5),"Hidden");hidden.semantics=Semantics {hidden:true,..first.semantics.clone()};
        let scene=Scene::new(Node::container(NodeId(1)).with_children(vec![first,second,disabled,hidden]));
        let mut native=NativeScene::from_scene(10,&scene,&bytes,320,200).unwrap();let version=scene.version();
        let mut events=Vec::new();
        for (caller,v,id) in [(11,version,NodeId(2)),(10,version+1,NodeId(2)),(10,version,NodeId(4)),
            (10,version,NodeId(5)),(10,version,NodeId(1))] {
            assert!(!native.focus_from_host(caller,v,id,&mut |_|panic!("rejected focus")));
        }
        assert!(native.focus_from_host(10,version,NodeId(2),&mut |e|events.push((e.scene_version,e.node,e.focused))));
        assert!(native.focus_from_host(10,version,NodeId(2),&mut |_|panic!("duplicate focus")));
        assert!(native.focus_from_host(10,version,NodeId(3),&mut |e|events.push((e.scene_version,e.node,e.focused))));
        assert_eq!(native.focused_node(10,version),Some(NodeId(3)));
        assert!(!native.clear_focus_from_host(11,version,&mut |_|panic!("foreign clear")));
        assert!(native.clear_focus_from_host(10,version,&mut |e|events.push((e.scene_version,e.node,e.focused))));
        assert!(native.clear_focus_from_host(10,version,&mut |_|panic!("duplicate clear")));
        assert_eq!(events,vec![(version,NodeId(2),true),(version,NodeId(2),false),(version,NodeId(3),true),(version,NodeId(3),false)]);
        assert!(native.close(10));assert!(native.focused_node(10,version).is_none());
        assert!(!native.focus_from_host(10,version,NodeId(2),&mut |_|panic!("closed focus")));
    }

    #[test]
    fn nested_button_content_keeps_pointer_target_but_activates_button_owner() {
        use crate::{native_view::PointerAction,native_events::NativeEventReply,scene_events::SceneEventKind};
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
        let mut button=Node::container(NodeId(2)).child(Node::text(NodeId(3),"Nested save"));
        button.semantics=Semantics::default().role(Role::Button);
        let scene=Scene::new(Node::container(NodeId(1)).child(button));
        let mut native=NativeScene::from_scene(10,&scene,&bytes,320,200).unwrap();
        let mut events=Vec::new();
        for (phase,pressed) in [(PointerAction::Down,true),(PointerAction::Up,false)] {
            assert!(native.pointer_from_host(10,scene.version(),phase,12.0,12.0,pressed,
                &mut |e| {events.push(e);NativeEventReply::default()}));
        }
        assert!(events.iter().any(|e|e.kind==SceneEventKind::Pointer(PointerAction::Down) && e.node==NodeId(3)));
        assert!(events.iter().any(|e|e.kind==SceneEventKind::Activate && e.node==NodeId(2)));
    }

    #[test]
    fn scene_pointer_routes_own_node_identity_and_native_activation() {
        use crate::{native_view::PointerAction,native_events::NativeEventReply,scene_events::SceneEventKind};
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
        let mut button=Node::text(NodeId(2),"Save");button.semantics=Semantics::default().role(Role::Button);
        let scene=Scene::new(Node::container(NodeId(1)).child(button));
        let mut native=NativeScene::from_scene(10,&scene,&bytes,320,200).unwrap();let version=scene.version();
        let mut events=Vec::new();
        assert!(!native.pointer_from_host(11,version,PointerAction::Down,12.0,12.0,true,&mut |_|panic!("foreign")));
        assert!(!native.pointer_from_host(10,version+1,PointerAction::Down,12.0,12.0,true,&mut |_|panic!("stale")));
        assert!(native.pointer_from_host(10,version,PointerAction::Down,12.0,12.0,true,&mut |e| {events.push(e);NativeEventReply::default()}));
        assert!(native.pointer_from_host(10,version,PointerAction::Up,12.0,12.0,false,&mut |e| {events.push(e);NativeEventReply::default()}));
        assert!(events.iter().any(|e|e.kind==SceneEventKind::Activate && e.node==NodeId(2)));
        assert!(events.iter().all(|e|e.scene_version==version));
        assert!(events.iter().filter(|e|e.kind==SceneEventKind::Pointer(PointerAction::Down)).all(|e|e.node==NodeId(2) && e.position==(12.0,12.0)));
        assert!(native.close(10));
        assert!(!native.pointer_from_host(10,version,PointerAction::Down,12.0,12.0,true,&mut |_|panic!("closed")));
    }

    #[test]
    #[cfg(feature="cpu-pending")]
    fn scene_consumes_host_physical_extent_dpi_and_zero_surface_lifecycle() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
        let scene=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Scale 中文")));
        let mut native=NativeScene::from_scene(10,&scene,&bytes,320,200).unwrap();
        let baseline=native.render_cpu(10,1024*1024).unwrap();
        for (caller,scale) in [(11,2.0),(10,0.0),(10,f32::NAN),(10,f32::INFINITY)] {
            assert!(!native.set_viewport_from_host(caller,640,400,scale));
            assert!(baseline==native.render_cpu(10,1024*1024).unwrap());
        }
        assert!(native.set_viewport_from_host(10,640,400,2.0));
        assert_eq!(native.render_cpu(10,2*1024*1024).unwrap().len(),640*400*4);
        assert_eq!(native.snapshot_identity(10),Some((scene.version(),2)));
        assert!(native.set_viewport_from_host(10,0,0,2.0));
        assert!(native.render_cpu(10,1024*1024).is_none());
        assert!(native.set_viewport_from_host(10,320,200,1.0));
        assert!(baseline==native.render_cpu(10,1024*1024).unwrap());
        assert!(native.close(10));assert!(!native.set_viewport_from_host(10,320,200,1.0));
    }

    #[test]
    fn scene_semantics_preserve_all_fields_without_claiming_os_actions() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
        let supplied=Semantics {role:Role::Button,label:"Save 中文".into(),description:"Save this file".into(),
            value:"pending".into(),disabled:true,focusable:true,hidden:false};
        let mut node=Node::text(NodeId(2),"Save");node.semantics=supplied.clone();
        let scene=Scene::new(Node::container(NodeId(1)).child(node));
        let mut native=NativeScene::from_scene(10,&scene,&bytes,320,200).unwrap();
        assert_eq!(native.semantics_from_host(10,NodeId(2)),Some(&supplied));
        let (id,_)=native.nodes.get(&NodeId(2)).unwrap();
        let element=native.document.as_ref().unwrap().get_node(*id).unwrap().element_data().unwrap();
        assert_eq!(element.name.local,LocalName::from("button"));
        assert_eq!(element.attr(LocalName::from("aria-label")),Some("Save 中文"));
        assert_eq!(element.attr(LocalName::from("tabindex")),Some("-1"));
        assert!(element.attr(LocalName::from("disabled")).is_some());
        assert!(native.close(10));assert!(native.semantics_from_host(10,NodeId(2)).is_none());
    }

    #[test]
    #[cfg(feature="cpu-pending")]
    fn source_patch_batch_updates_same_native_tree_and_preflights_rejection() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
        let mut scene=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Before")));
        let mut native=NativeScene::from_scene(10,&scene,&bytes,320,200).unwrap();
        let before=native.render_cpu(10,1024*1024).unwrap();let version=scene.version();
        let invalid=[ScenePatch::UpdateText {id:NodeId(2),text:"Must not appear".into()},
            ScenePatch::UpdateText {id:NodeId(1),text:"Container is not text".into()}];
        assert!(!native.apply_patches(10,version,version+1,&invalid));
        assert!(before==native.render_cpu(10,1024*1024).unwrap());
        let oversized=vec![ScenePatch::UpdateText {id:NodeId(2),text:"x".into()};4097];
        assert!(!native.apply_patches(10,version,version+1,&oversized));
        assert!(before==native.render_cpu(10,1024*1024).unwrap());
        let patches=[ScenePatch::UpdateText {id:NodeId(2),text:"After 中文".into()},
            ScenePatch::UpdateStyle {id:NodeId(2),style:Style::default().background(Color::BLUE)}];
        assert!(!native.apply_patches(11,version,version+1,&patches));
        assert!(!native.apply_patches(10,version,version+2,&patches));
        scene.apply_batch_at(version,&patches).unwrap();
        assert!(native.apply_patches(10,version,scene.version(),&patches));
        assert_eq!(native.snapshot_identity(10),Some((scene.version(),2)));
        let after=native.render_cpu(10,1024*1024).unwrap();assert!(after!=before);
        let mut reference=NativeScene::from_scene(10,&scene,&bytes,320,200).unwrap();
        assert!(after==reference.render_cpu(10,1024*1024).unwrap(),"patched tree must match source snapshot import");
        assert!(!native.apply_patches(10,version,scene.version(),&patches));
        assert!(native.close(10));assert!(!native.apply_patches(10,scene.version(),scene.version()+1,&patches));
    }

    #[test]
    #[cfg(feature="cpu-pending")]
    fn existing_scene_imports_native_tree_and_rejects_unsupported_inputs() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("injected font required")).unwrap();
        let scene=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Existing 中文"))
            .child(Node::rect(NodeId(3)).with_style(Style::default().size(Length::Px(80.0),Length::Px(30.0)).background(Color::RED))));
        let mut native=NativeScene::from_scene(10,&scene,&bytes,320,200).unwrap();
        assert_eq!(native.snapshot_identity(10),Some((scene.version(),3)));
        assert_eq!(native.snapshot_identity(11),None);assert!(native.render_cpu(11,1024*1024).is_none());
        let pixels=native.render_cpu(10,1024*1024).unwrap();assert!(pixels.chunks_exact(4).any(|p|p[0]>150 && p[1]<100));
        let mut duplicate=scene.clone();duplicate.root.children[1].id=NodeId(2);
        assert!(NativeScene::from_scene(10,&duplicate,&bytes,320,200).is_none());
        let mut invalid=scene.clone();invalid.root.style.gap=f32::NAN;
        assert!(NativeScene::from_scene(10,&invalid,&bytes,320,200).is_none());
        invalid=scene.clone();invalid.root.semantics.label="Preserved semantics".into();
        let semantic_view=NativeScene::from_scene(10,&invalid,&bytes,320,200).unwrap();
        assert_eq!(semantic_view.semantics_from_host(10,NodeId(1)).unwrap().label,"Preserved semantics");
        assert!(semantic_view.semantics_from_host(11,NodeId(1)).is_none());
        invalid=scene.clone();invalid.root.scroll.x=f32::NAN;
        assert!(NativeScene::from_scene(10,&invalid,&bytes,320,200).is_none());
        let wide=Scene::new(Node::container(NodeId(0)).with_children(
            (1..4097).map(|id|Node::text(NodeId(id),"x")).collect()));
        assert!(NativeScene::from_scene(10,&wide,&bytes,320,200).is_none());
        let mut deep=Node::text(NodeId(0),"x");
        for id in 1..131 {deep=Node::container(NodeId(id)).child(deep);}
        assert!(NativeScene::from_scene(10,&Scene::new(deep),&bytes,320,200).is_none());
        assert!(!native.close(11));assert!(native.close(10));assert!(native.render_cpu(10,1024*1024).is_none());
    }
}

// BEGIN VERIFIED SOURCE HIDDEN ANCESTRY KERNEL
fn source_hidden_ancestry(parents:&BTreeMap<NodeId,Option<NodeId>>,semantics:&BTreeMap<NodeId,Semantics>,ancestor:Option<NodeId>)->Option<bool> {
    let mut current=ancestor;let mut hidden=false;let mut remaining=parents.len();
    loop {
        let id=match current {Some(id)=>id,None=>return Some(hidden)};
        if remaining==0 {return None;}remaining-=1;
        let Some(semantic)=semantics.get(&id) else {return None;};
        hidden |= semantic.hidden;
        current=match parents.get(&id) {Some(parent)=>*parent,None=>return None};
    }
}
// END VERIFIED SOURCE HIDDEN ANCESTRY KERNEL

/// Candidate finite nonnegative source-offset admission; native owns scroll geometry.
fn native_scroll_offset(offset:jasper_ui_engine::geometry::Point)->Option<blitz_dom::Point<f64>> {
    if !offset.x.is_finite() || !offset.y.is_finite() || offset.x<0.0 || offset.y<0.0
        || offset.x as f64>u32::MAX as f64 || offset.y as f64>u32::MAX as f64 {return None;}
    Some(blitz_dom::Point {x:offset.x as f64,y:offset.y as f64})
}

#[cfg(test)]
mod semantic_attribute_update_tests {
    use super::*;
    #[test]
    fn native_semantic_update_clears_stale_attributes_on_same_element() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
        let initial=Semantics {role:Role::Link,label:"Open".into(),description:"Resource".into(),value:"old".into(),
            disabled:true,hidden:true,focusable:true};
        let mut source=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Text").with_semantics(initial)));
        let mut native=NativeScene::from_scene(10,&source,&bytes,160,80).unwrap();
        let element_id=native.nodes[&NodeId(2)].0;
        let old=source.version();let patches=[ScenePatch::UpdateSemantics {id:NodeId(2),semantics:Semantics::default().role(Role::Text)}];
        source.apply_batch_at(old,&patches).unwrap();assert!(native.apply_patches(10,old,source.version(),&patches));
        assert_eq!(native.nodes[&NodeId(2)].0,element_id);
        let element=native.document.as_ref().unwrap().get_node(element_id).unwrap().element_data().unwrap();
        for key in ["role","aria-label","aria-description","aria-valuetext","disabled","tabindex"] {
            assert_eq!(element.attr(LocalName::from(key)),None,"stale native attribute {key}");
        }
        assert_eq!(element.attr(LocalName::from("aria-hidden")),Some("false"));
        assert_eq!(element.attr(LocalName::from("aria-disabled")),Some("false"));
    }
}

// BEGIN VERIFIED SCENE SNAPSHOT IDENTITY KERNEL
impl NativeScene {
    pub(crate) fn snapshot_identity(&self,caller:u64)->Option<(u64,usize)> {
        if caller!=self.owner || !self.document.is_open() {return None;}
        Some((self.scene_version,self.node_count))
    }
}
// END VERIFIED SCENE SNAPSHOT IDENTITY KERNEL

// BEGIN VERIFIED SEMANTIC ROLE TAG KERNEL
/// Only Button selects a different native element tag in append_prepared_subtree.
pub(crate) fn semantic_role_update_preserves_tag(current:Role,next:Role)->bool {
    role_uses_native_button(current)==role_uses_native_button(next)
}
// END VERIFIED SEMANTIC ROLE TAG KERNEL

#[cfg(test)]
mod source_box_sizing_tests {
    use super::*;
    use jasper_ui_engine::geometry::Edges;
    #[test]
    fn source_declared_size_includes_padding_and_border() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
        let style=Style {display:Display::Block,width:Length::Px(80.0),height:Length::Px(40.0),
            padding:Edges::all(5.0),border_width:2.0,..Default::default()};
        let source=Scene::new(Node::container(NodeId(1)).child(Node::rect(NodeId(2)).with_style(style)));
        let view=NativeScene::from_scene(10,&source,&bytes,320,200).unwrap();
        let node=view.document.as_ref().unwrap().get_node(view.nodes[&NodeId(2)].0).unwrap();
        assert_eq!(node.final_layout().size.width,80.0);
        assert_eq!(node.final_layout().size.height,40.0);
    }
}

#[cfg(test)]
mod source_root_viewport_tests {
    use super::*;
    #[test]
    fn auto_source_root_fills_native_logical_viewport() {
        let bytes=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
        let mut scene=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Small content")));
        let mut view=NativeScene::from_scene(10,&scene,&bytes,320,200).unwrap();
        let root=view.document.as_ref().unwrap().get_node(view.nodes[&NodeId(1)].0).unwrap();
        assert_eq!(root.final_layout().size.width,320.0);assert_eq!(root.final_layout().size.height,200.0);
        assert!(view.set_viewport_from_host(10,320,200,2.0));
        let size=|view:&NativeScene|view.document.as_ref().unwrap().get_node(view.nodes[&NodeId(1)].0).unwrap().final_layout().size;
        assert_eq!((size(&view).width,size(&view).height),(160.0,100.0));
        for (style,expected) in [(Style::default().size(Length::Px(80.0),Length::Px(40.0)),(80.0,40.0)),
            (Style::default(),(160.0,100.0))] {
            let old=scene.version();let patches=[ScenePatch::UpdateStyle {id:NodeId(1),style}];
            scene.apply_batch_at(old,&patches).unwrap();assert!(view.apply_patches(10,old,scene.version(),&patches));
            assert_eq!((size(&view).width,size(&view).height),expected);
        }
        assert!(view.set_viewport_from_host(10,640,400,2.0));
        assert_eq!((size(&view).width,size(&view).height),(320.0,200.0));

    }
}

// BEGIN VERIFIED NATIVE BUTTON ROLE KERNEL
pub(crate) fn role_uses_native_button(role:Role)->bool {
    matches!(role,Role::Button)
}
// END VERIFIED NATIVE BUTTON ROLE KERNEL

#[cfg(test)]
mod deactivation_hover_tests {
    use super::*;
    #[test]
    fn deactivation_clears_actual_native_hover_and_press() {
        let font=std::fs::read(std::env::var("JASPER_UI_TEST_FONT").unwrap()).unwrap();
        let source=Scene::new(Node::container(NodeId(1)).child(Node::text(NodeId(2),"Save")
            .with_semantics(Semantics::default().role(Role::Button).focusable(true))));
        let mut view=NativeScene::from_scene(10,&source,&font,160,80).unwrap();
        assert!(view.pointer_from_host(10,source.version(),crate::native_view::PointerAction::Down,12.0,12.0,true,&mut |_|Default::default()));
        assert!(view.document.as_ref().unwrap().get_hover_node_id().is_some());
        assert!(view.document.as_ref().unwrap().get_mousedown_node_id().is_some());
        assert!(!view.deactivate_from_host(11,source.version(),&mut |_|panic!("foreign cleanup")));
        assert!(view.document.as_ref().unwrap().get_hover_node_id().is_some());
        assert!(view.deactivate_from_host(10,source.version(),&mut |_|{}));
        let doc=view.document.as_ref().unwrap();
        assert!(doc.get_hover_node_id().is_none());assert!(doc.get_mousedown_node_id().is_none());
        assert_ne!(doc.get_focussed_node_id(),Some(view.nodes[&NodeId(2)].0));
        assert_eq!(view.focused_node(10,source.version()),None);
    }
}
