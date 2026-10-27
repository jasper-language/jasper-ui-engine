//! Actual production admission body and resource gate; no alternate implementation.
include!(concat!(env!("OUT_DIR"),"/actual.rs"));

/// Exercise each independent stale identity dimension and close/reopen/retire flow.
/// 逐维检查过期身份，以及关闭/重开/退役；输入数据完整 u64，不模拟 gate。
pub fn check_identity(data: [u64;8], mismatch: u8) {
    let [session,slot,node,node_generation,document,revision,_,_]=data;
    let current = data[6];
    assert_eq!(next_ui_update_version(current,current), current.checked_add(1));
    match next_ui_update_version(current,data[7]) {
        Some(next) => { assert_eq!(current,data[7]); assert!(next > current); assert_eq!(next-current,1); }
        None => assert!(current != data[7] || current == u64::MAX),
    }

    let mut gate=ResourceGate::new(session,slot);
    let key=gate.issue(session).unwrap();
    let current=UiInputContext {target:NodeId(node),target_generation:node_generation,document,revision};
    let mut ticket=UiInputTicket {key,context:current};
    let mut incoming = current;
    let owner_mask = mismatch & 31;
    let owner_caller = if owner_mask & 1 != 0 {session ^ 1} else {session};
    if owner_mask & 2 != 0 {incoming.target.0 ^= 1;}
    if owner_mask & 4 != 0 {incoming.target_generation ^= 1;}
    if owner_mask & 8 != 0 {incoming.document ^= 1;}
    if owner_mask & 16 != 0 {incoming.revision ^= 1;}
    assert_eq!(matches_ui_view_owner(session,current,owner_caller,incoming), owner_mask == 0);
    let mut next = current;
    next.revision = data[6];
    if mismatch & 32 != 0 {next.target.0 ^= 1;}
    if mismatch & 64 != 0 {next.target_generation ^= 1;}
    if mismatch & 128 != 0 {next.document ^= 1;}
    assert_eq!(accepts_ui_display_snapshot(session,current,owner_caller,incoming,next), mismatch == 0);


    let caller=if mismatch & 1 !=0 {session ^ 1} else {session};
    if mismatch & 2 !=0 {ticket.key.session ^=1;}
    if mismatch & 4 !=0 {ticket.key.slot ^=1;}
    if mismatch & 8 !=0 {ticket.key.generation ^=1;}
    if mismatch & 16 !=0 {ticket.context.target.0 ^=1;}
    if mismatch & 32 !=0 {ticket.context.target_generation ^=1;}
    if mismatch & 64 !=0 {ticket.context.document ^=1;}
    if mismatch & 128 !=0 {ticket.context.revision ^=1;}
    assert_eq!(accepts_ui_input(&gate,caller,current,ticket),mismatch==0);
    // Authenticated closure invalidates even a perfect old ticket.
    assert!(gate.revoke(session,key));
    assert!(!accepts_ui_input(&gate,session,current,UiInputTicket{key,context:current}));
    let fresh=gate.issue(session).unwrap();
    assert!(!accepts_ui_input(&gate,session,current,UiInputTicket{key,context:current}));
    assert!(accepts_ui_input(&gate,session,current,UiInputTicket{key:fresh,context:current}));
    gate.retire();
    assert!(!accepts_ui_input(&gate,session,current,UiInputTicket{key:fresh,context:current}));
    assert!(gate.issue(session).is_none());
}
/// Actual persistent binding transitions; old tickets cannot mutate a successor.
pub fn check_binding(data: [u64;8], mismatch: u8) {
    let [session,slot,node,target_generation,document,revision,_,_]=data;
    let current=UiInputContext{target:NodeId(node),target_generation,document,revision};
    let mut binding=UiInputBinding::new(session,slot);
    assert!(binding.bind(session^1,current).is_none());
    let first=binding.bind(session,current).unwrap();
    assert!(binding.accepts(session,first));
    assert!(binding.bind(session,current).is_none());
    assert!(binding.accepts(session,first));
    let mut stale=first;
    let caller=if mismatch&1!=0 {session^1} else {session};
    if mismatch&2!=0 {stale.key.session^=1;}
    if mismatch&4!=0 {stale.key.slot^=1;}
    if mismatch&8!=0 {stale.key.generation^=1;}
    if mismatch&16!=0 {stale.context.target.0^=1;}
    if mismatch&32!=0 {stale.context.target_generation^=1;}
    if mismatch&64!=0 {stale.context.document^=1;}
    if mismatch&128!=0 {stale.context.revision^=1;}
    assert_eq!(binding.accepts(caller,stale),mismatch==0);
    if mismatch!=0 {
        assert!(!binding.close(caller,stale));
        assert!(binding.accepts(session,first));
    }
    assert!(binding.close(session,first));
    assert!(!binding.close(session,first));
    assert!(!binding.accepts(session,first));
    let mut next=current;next.revision^=1;
    let second=binding.bind(session,next).unwrap();
    assert!(binding.accepts(session,second));
    assert!(!binding.close(session,first));
    assert!(binding.accepts(session,second));
    let stale_revision=UiInputTicket{key:second.key,context:current};
    assert!(!binding.close(session,stale_revision));
    assert!(binding.accepts(session,second));
    binding.retire();
    assert!(!binding.accepts(session,second));
    assert!(!binding.close(session,second));
    assert!(binding.bind(session,next).is_none());
}
pub fn check_payload(data: [u64;8], mismatch: u8) {
    let current=UiInputContext{target:NodeId(data[2]),target_generation:data[3],document:data[4],revision:data[5]};
    let mut binding=UiInputBinding::new(data[0],data[1]);
    let ticket=binding.bind(data[0],current).unwrap();
    let payload=data[6];
    let caller=if mismatch&1!=0 {data[0]^1} else {data[0]};
    match binding.admit(caller,ticket,&payload) {
        Some(admitted)=>{
            assert_eq!(mismatch&1,0);
            assert_eq!(admitted.target(),current.target);
            assert_eq!(*admitted.payload(),payload);
            assert!(std::ptr::eq(admitted.payload(),&payload));
        },
        None=>assert_ne!(mismatch&1,0),
    }
    assert!(binding.close(data[0],ticket));
    assert!(binding.admit(data[0],ticket,&payload).is_none());
    let fresh=binding.bind(data[0],current).unwrap();
    assert!(binding.admit(data[0],ticket,&payload).is_none());
    assert_eq!(*binding.admit(data[0],fresh,&payload).unwrap().payload(),payload);
    binding.retire();
    assert!(binding.admit(data[0],fresh,&payload).is_none());
}
pub fn check_keyboard(data: [u64;8], mode: u8) {
    let current=UiInputContext{target:NodeId(data[2]),target_generation:data[3],document:data[4],revision:data[5]};
    let mut binding=UiInputBinding::new(data[0],data[1]);
    let ticket=binding.bind(data[0],current).unwrap();
    let focused=if mode&1==0 {None} else {Some(NodeId(if mode&2==0 {data[2]} else {data[7]}))};
    let mut router=KeyboardRouter{focus:FocusManager{order:Vec::new(),focused}};
    let payload=data[6];
    {
        let admitted=binding.admit(data[0],ticket,&payload).unwrap();
        let routed=router.route_admitted(&admitted);
        assert_eq!(routed.is_some(),focused==Some(current.target));
        if let Some(value)=routed {assert_eq!(*value,payload);assert!(std::ptr::eq(value,&payload));}
    }
    assert_eq!(router.retire_input_binding(&mut binding),focused);
    assert_eq!(router.focused(),None);
    assert!(!binding.accepts(data[0],ticket));
    assert!(binding.bind(data[0],current).is_none());
    assert_eq!(router.retire_input_binding(&mut binding),None);
}
/// Seed the actual private state to cover every generation without looping 2^64 times.
/// 直接构造真实私有状态覆盖全部代数；不替换实现，不迭代2^64次。
pub fn check_rotation(data: [u64;8], mismatch: u8) {
    let current=UiInputContext{target:NodeId(data[2]),target_generation:data[3],document:data[4],revision:data[5]};
    let key=ResourceKey{session:data[0],slot:data[1],generation:data[6]};
    let mut binding=UiInputBinding{gate:ResourceGate{session:data[0],slot:data[1],generation:data[6],live:true},current:Some(current)};
    let old=UiInputTicket{key,context:current};let mut ticket=old;
    let caller=if mismatch&1!=0 {data[0]^1} else {data[0]};
    if mismatch&2!=0 {ticket.key.session^=1;}
    if mismatch&4!=0 {ticket.key.slot^=1;}
    if mismatch&8!=0 {ticket.key.generation^=1;}
    if mismatch&16!=0 {ticket.context.target.0^=1;}
    if mismatch&32!=0 {ticket.context.target_generation^=1;}
    if mismatch&64!=0 {ticket.context.document^=1;}
    if mismatch&128!=0 {ticket.context.revision^=1;}
    let next=UiInputContext{target:NodeId(data[7]),target_generation:data[3]^1,document:data[4]^1,revision:data[5]^1};
    let result=binding.replace(caller,ticket,next);
    let expected=mismatch==0 && data[6]!=0 && data[6]!=u64::MAX;
    assert_eq!(result.is_some(),expected);
    match result {
        Some(fresh)=>{
            assert_eq!(fresh.key.generation,data[6]+1);
            assert!(binding.accepts(data[0],fresh));
            assert_eq!((fresh.context.target.0,fresh.context.target_generation,fresh.context.document,fresh.context.revision),(next.target.0,next.target_generation,next.document,next.revision));
            assert!(!binding.accepts(data[0],old));
            assert!(!binding.close(data[0],old));
            assert!(binding.accepts(data[0],fresh));
        },
        None=>{
            assert_eq!((binding.gate.session,binding.gate.slot,binding.gate.generation,binding.gate.live),(data[0],data[1],data[6],true));
            assert_eq!(binding.current.map(|c|(c.target.0,c.target_generation,c.document,c.revision)),Some((current.target.0,current.target_generation,current.document,current.revision)));
            assert_eq!(binding.accepts(data[0],old),data[6]!=0);
        },
    }
}
pub fn check_transfer(data: [u64;8], mode: u8) {
    let current=UiInputContext{target:NodeId(data[2]),target_generation:data[3],document:data[4],revision:data[5]};
    let key=ResourceKey{session:data[0],slot:data[1],generation:data[6]};
    let mut binding=UiInputBinding{gate:ResourceGate{session:data[0],slot:data[1],generation:data[6],live:true},current:Some(current)};
    let original=UiInputTicket{key,context:current};let mut request=original;
    if mode&16!=0 {request.context.revision^=1;}
    let caller=if mode&8!=0 {data[0]^1} else {data[0]};
    let focused=if mode&1==0 {None} else {Some(NodeId(if mode&2==0 {data[2]} else {data[7]}))};
    let mut router=KeyboardRouter{focus:FocusManager{order:vec![NodeId(data[2]),NodeId(data[7])],focused}};
    let next=UiInputContext{target:NodeId(if mode&4==0 {data[7]} else {data[2]^1}),target_generation:data[3]^1,document:data[4]^1,revision:data[5]^1};
    let eligible=next.target.0==data[2] || next.target.0==data[7];
    assert_eq!(router.focus.can_focus(next.target),eligible);
    let expected=focused==Some(current.target) && eligible && mode&24==0 && data[6]!=0 && data[6]!=u64::MAX;
    let result=router.transfer_input_focus(&mut binding,caller,request,next);
    assert_eq!(result.is_some(),expected);
    assert_eq!(router.focus.order.len(),2);
    assert_eq!(router.focus.order[0],NodeId(data[2]));
    assert_eq!(router.focus.order[1],NodeId(data[7]));
    match result {
        Some(fresh)=>{
            assert_eq!(router.focused(),Some(next.target));
            assert_eq!(fresh.key.generation,data[6]+1);
            assert!(binding.accepts(data[0],fresh));
            assert!(!binding.accepts(data[0],original));
            assert!(!binding.close(data[0],original));
            let payload=data[5];
            let admitted=binding.admit(data[0],fresh,&payload).unwrap();
            assert_eq!(*router.route_admitted(&admitted).unwrap(),payload);
        },
        None=>{
            assert_eq!(router.focused(),focused);
            assert_eq!((binding.gate.session,binding.gate.slot,binding.gate.generation,binding.gate.live),(data[0],data[1],data[6],true));
            assert_eq!(binding.current.map(|c|(c.target.0,c.target_generation,c.document,c.revision)),Some((current.target.0,current.target_generation,current.document,current.revision)));
        },
    }
}
pub fn check_open_close(data: [u64;8], mode: u8) {
    let context=UiInputContext{target:NodeId(if mode&2==0 {data[7]} else {data[2]^1}),target_generation:data[3],document:data[4],revision:data[5]};
    let live=mode&4!=0;
    let mut binding=UiInputBinding{gate:ResourceGate{session:data[0],slot:data[1],generation:data[6],live},current:if live {Some(context)} else {None}};
    let focused=if mode&1!=0 {Some(NodeId(data[2]))} else {None};
    let mut router=KeyboardRouter{focus:FocusManager{order:vec![NodeId(data[2]),NodeId(data[7])],focused}};
    let caller=if mode&8!=0 {data[0]^1} else {data[0]};
    let eligible=context.target.0==data[2] || context.target.0==data[7];
    let expected=focused.is_none() && eligible && !live && mode&8==0 && data[6]<u64::MAX;
    let opened=router.begin_input_focus(&mut binding,caller,context);
    assert_eq!(opened.is_some(),expected);
    assert_eq!(router.focus.order.len(),2);assert_eq!(router.focus.order[0],NodeId(data[2]));assert_eq!(router.focus.order[1],NodeId(data[7]));
    match opened {
        Some(ticket)=>{
            assert_eq!(router.focused(),Some(context.target));
            assert_eq!(ticket.key.generation,data[6]+1);assert!(binding.accepts(data[0],ticket));
            let mut stale=ticket;stale.context.revision^=1;
            assert!(router.close_input_focus(&mut binding,data[0],stale).is_none());
            assert_eq!(router.focused(),Some(context.target));assert!(binding.accepts(data[0],ticket));
            let owner=router.close_input_focus(&mut binding,data[0],ticket).unwrap();
            assert_eq!(owner.key,ticket.key);
            assert_eq!((owner.context.target.0,owner.context.target_generation,owner.context.document,owner.context.revision),(context.target.0,context.target_generation,context.document,context.revision));
            assert_eq!(router.focused(),None);assert!(!binding.accepts(data[0],ticket));
            let reopened=router.begin_input_focus(&mut binding,data[0],context);
            assert_eq!(reopened.is_some(),ticket.key.generation<u64::MAX);
            if let Some(fresh)=reopened {
                assert!(router.close_input_focus(&mut binding,data[0],owner).is_none());
                assert!(binding.accepts(data[0],fresh));assert_eq!(router.focused(),Some(context.target));
            }
        },
        None=>{
            assert_eq!(router.focused(),focused);
            assert_eq!((binding.gate.session,binding.gate.slot,binding.gate.generation,binding.gate.live),(data[0],data[1],data[6],live));
            assert_eq!(binding.current.map(|c|(c.target.0,c.target_generation,c.document,c.revision)),if live {Some((context.target.0,context.target_generation,context.document,context.revision))} else {None});
        },
    }
}
pub fn check_composition(data: [u64;8], mismatch: u8) {
    let context=UiInputContext{target:NodeId(data[2]),target_generation:data[3],document:data[4],revision:data[5]};
    let mut binding=UiInputBinding::new(data[0],data[1]);let owner=binding.bind(data[0],context).unwrap();
    let empty=UiCompositionSlot::new();assert_eq!(empty.generation,0);assert!(empty.active.is_none());
    let mut slot=UiCompositionSlot{generation:data[6],active:None};
    let opened=slot.begin(&binding,data[0],owner);
    assert_eq!(opened.is_some(),data[6]<u64::MAX);
    match opened {
        Some(token)=>{
            assert_eq!(token.serial,data[6]+1);assert!(slot.accepts(&binding,data[0],token));
            assert!(slot.begin(&binding,data[0],owner).is_none());assert!(slot.accepts(&binding,data[0],token));
            let mut stale=token;stale.serial=data[7];
            let caller=if mismatch&1!=0 {data[0]^1} else {data[0]};
            if mismatch&2!=0 {stale.owner.key.session^=1;}
            if mismatch&4!=0 {stale.owner.key.slot^=1;}
            if mismatch&8!=0 {stale.owner.key.generation^=1;}
            if mismatch&16!=0 {stale.owner.context.target.0^=1;}
            if mismatch&32!=0 {stale.owner.context.target_generation^=1;}
            if mismatch&64!=0 {stale.owner.context.document^=1;}
            if mismatch&128!=0 {stale.owner.context.revision^=1;}
            let expected=mismatch==0 && data[7]==token.serial;
            assert_eq!(slot.accepts(&binding,caller,stale),expected);
            if !expected {assert!(!slot.cancel(caller,stale));assert!(slot.accepts(&binding,data[0],token));}
            assert!(slot.cancel(data[0],token));assert!(!slot.cancel(data[0],token));
            assert!(!slot.accepts(&binding,data[0],token));
            let second=slot.begin(&binding,data[0],owner);
            assert_eq!(second.is_some(),token.serial<u64::MAX);
            if let Some(next)=second {
                assert!(!slot.cancel(data[0],token));assert!(slot.accepts(&binding,data[0],next));
                assert!(binding.close(data[0],owner));assert!(!slot.accepts(&binding,data[0],next));
                assert!(slot.cancel(data[0],next));
            }
        },
        None=>{assert_eq!(slot.generation,data[6]);assert!(slot.active.is_none());}
    }
    let previous=slot.active;let retired=slot.retire();
    assert_eq!(retired.map(|t|t.serial),previous.map(|t|t.serial));
    assert!(slot.begin(&binding,data[0],owner).is_none());assert_eq!(slot.generation,u64::MAX);
}
// Exact production implementation against Rust safe slicing; no document edits.
pub fn check_composition_range(document: &str, start: usize, end: usize) {
    let expected=document.get(start..end).is_some();
    assert_eq!(valid_composition_range(document,start,end),expected);
    let mut binding=UiInputBinding::new(1,2);
    let owner=binding.bind(1,UiInputContext{target:NodeId(3),target_generation:4,document:5,revision:6}).unwrap();
    let mut slot=UiCompositionSlot::new();let token=slot.begin(&binding,1,owner).unwrap();
    let event=CompositionEvent{text:String::new(),start,end};
    assert_eq!(slot.admit_composition(&binding,1,token,document,&event).is_some(),expected);
    assert!(slot.admit_composition(&binding,2,token,document,&event).is_none());
    assert!(slot.cancel(1,token));
    assert!(slot.admit_composition(&binding,1,token,document,&event).is_none());
    let next=slot.begin(&binding,1,owner).unwrap();
    assert!(slot.admit_composition(&binding,1,token,document,&event).is_none());
    assert_eq!(slot.admit_composition(&binding,1,next,document,&event).is_some(),expected);
}
// Preedit-local selection remains independent of document replacement coordinates.
pub fn check_preedit(text: &str, cursor: Option<(usize,usize)>) {
    let expected=match cursor {None=>true,Some((start,end))=>text.get(start..end).is_some()};
    assert_eq!(valid_preedit_cursor(text,cursor),expected);
    let event=PreeditEvent{text:text.to_owned(),cursor};
    let mut binding=UiInputBinding::new(1,2);
    let owner=binding.bind(1,UiInputContext{target:NodeId(3),target_generation:4,document:5,revision:6}).unwrap();
    let mut slot=UiCompositionSlot::new();let token=slot.begin(&binding,1,owner).unwrap();
    assert_eq!(slot.admit_preedit(&binding,1,token,&event).is_some(),expected);
    assert_eq!(event.cursor,cursor);
    assert!(slot.admit_preedit(&binding,2,token,&event).is_none());
    assert!(slot.cancel(1,token));assert!(slot.admit_preedit(&binding,1,token,&event).is_none());
    let next=slot.begin(&binding,1,owner).unwrap();
    assert!(slot.admit_preedit(&binding,1,token,&event).is_none());
    assert_eq!(slot.admit_preedit(&binding,1,next,&event).is_some(),expected);
}
// Exercise the exact desktop mapper on the exact pinned dependency enum declaration.
pub fn check_desktop_ime(text:&str,cursor:Option<(usize,usize)>) {
    let mut desktop=DesktopInput{pointer_id:PointerId(0),position:Point{x:0.0,y:0.0},pressed:std::collections::BTreeSet::new(),modifiers:Modifiers::default(),start:std::time::Instant::now()};
    match desktop.ime(&Ime::Preedit(text.to_owned(),cursor)) {
        Some(InputEvent::Preedit{text:copy,cursor:position})=>{assert_eq!(copy,text);assert_eq!(position,cursor);},
        _=>panic!("delegate mapped preedit to document coordinates"),
    }
    assert_eq!(desktop.pointer_id.0,0);assert_eq!(desktop.position.x,0.0);assert!(desktop.pressed.is_empty());
    match normalize_ime(&Ime::Preedit(text.to_owned(),cursor)) {
        Some(InputEvent::Preedit{text:copy,cursor:position})=>{assert_eq!(copy,text);assert_eq!(position,cursor);},
        _=>panic!("preedit mapped to wrong coordinate domain"),
    }
    match normalize_ime(&Ime::Commit(text.to_owned())) {
        Some(InputEvent::Text(copy))=>assert_eq!(copy,text),_=>panic!("missing commit"),
    }
    assert!(matches!(normalize_ime(&Ime::Enabled),Some(InputEvent::ImeEnabled(true))));assert!(matches!(normalize_ime(&Ime::Disabled),Some(InputEvent::ImeEnabled(false))));
}
// Pure production host normalizer -> current composition -> focus-owner admission.
// Exact old stateful clock wrapper remains a separate pending verification path.
pub fn check_normalized_preedit_pipeline(text:&str,cursor:Option<(usize,usize)>) {
    let event=normalize_ime(&Ime::Preedit(text.to_owned(),cursor)).unwrap();
    let expected=match cursor {None=>true,Some((start,end))=>text.get(start..end).is_some()};
    let mut binding=UiInputBinding::new(1,2);
    let owner=binding.bind(1,UiInputContext{target:NodeId(3),target_generation:4,document:5,revision:6}).unwrap();
    let mut slot=UiCompositionSlot::new();let first=slot.begin(&binding,1,owner).unwrap();
    assert_eq!(slot.admit_normalized_preedit(&binding,1,first,&event).is_some(),expected);
    assert!(slot.admit_normalized_preedit(&binding,2,first,&event).is_none());
    let commit=normalize_ime(&Ime::Commit(text.to_owned())).unwrap();
    assert!(slot.admit_normalized_preedit(&binding,1,first,&commit).is_none());
    assert!(slot.cancel(1,first));let second=slot.begin(&binding,1,owner).unwrap();
    assert!(slot.admit_normalized_preedit(&binding,1,first,&event).is_none());
    assert_eq!(slot.admit_normalized_preedit(&binding,1,second,&event).is_some(),expected);
    assert!(binding.close(1,owner));assert!(slot.admit_normalized_preedit(&binding,1,second,&event).is_none());
}
// Actual owned UI presentation updates, rejection frame, and qualified stale clear.
pub fn check_preedit_view(text:&str,cursor:Option<(usize,usize)>) {
    let mut binding=UiInputBinding::new(1,2);
    let owner=binding.bind(1,UiInputContext{target:NodeId(3),target_generation:4,document:5,revision:6}).unwrap();
    let mut slot=UiCompositionSlot::new();let first=slot.begin(&binding,1,owner).unwrap();
    let mut view=UiPreeditView::new();assert!(view.current().is_none());
    let expected=match cursor {None=>true,Some((start,end))=>text.get(start..end).is_some()};
    let event=normalize_ime(&Ime::Preedit(text.to_owned(),cursor)).unwrap();
    assert_eq!(view.apply(&slot,&binding,1,first,event),expected);
    if expected {let frame=view.current().unwrap();assert_eq!(frame.text,text);assert_eq!(frame.cursor,cursor);assert_eq!(frame.token.serial,first.serial);}
    else {assert!(view.current().is_none());}
    let commit=normalize_ime(&Ime::Commit("x".into())).unwrap();
    assert!(!view.apply(&slot,&binding,1,first,commit));assert_eq!(view.current().is_some(),expected);
    if expected {assert_eq!(view.current().unwrap().text,text);assert_eq!(view.current().unwrap().cursor,cursor);}
    assert!(!view.clear(2,first));assert_eq!(view.current().is_some(),expected);
    assert!(slot.cancel(1,first));let second=slot.begin(&binding,1,owner).unwrap();
    let fresh=normalize_ime(&Ime::Preedit("新".into(),None)).unwrap();
    assert!(view.apply(&slot,&binding,1,second,fresh));
    assert!(!view.clear(1,first));assert_eq!(view.current().unwrap().text,"新");
    assert!(binding.close(1,owner));
    let late=normalize_ime(&Ime::Preedit("old".into(),None)).unwrap();
    assert!(!view.apply(&slot,&binding,1,second,late));assert_eq!(view.current().unwrap().text,"新");
    assert!(view.clear(1,second));assert!(view.current().is_none());assert!(!view.clear(1,second));
}
// Actual two-object cancellation, exact old-frame matching, and successor protection.
pub fn check_paired_preedit_cancel(text:&str, mismatched_view:bool) {
    let mut binding=UiInputBinding::new(1,2);
    let owner=binding.bind(1,UiInputContext{target:NodeId(3),target_generation:4,document:5,revision:6}).unwrap();
    let mut slot=UiCompositionSlot::new();let first=slot.begin(&binding,1,owner).unwrap();
    let mut view=UiPreeditView::new();
    assert!(view.apply(&slot,&binding,1,first,normalize_ime(&Ime::Preedit(text.to_owned(),None)).unwrap()));
    if mismatched_view {view.frame.as_mut().unwrap().token.serial^=1;}
    assert!(!slot.cancel_preedit(&mut view,2,first));
    assert!(slot.accepts(&binding,1,first));assert_eq!(view.current().unwrap().text,text);
    assert!(slot.cancel_preedit(&mut view,1,first));assert!(!slot.accepts(&binding,1,first));
    assert_eq!(view.current().is_some(),mismatched_view);
    if mismatched_view {assert_eq!(view.current().unwrap().text,text);}
    let second=slot.begin(&binding,1,owner).unwrap();
    assert!(view.apply(&slot,&binding,1,second,normalize_ime(&Ime::Preedit("新".into(),None)).unwrap()));
    assert!(!slot.cancel_preedit(&mut view,1,first));assert!(slot.accepts(&binding,1,second));
    assert_eq!(view.current().unwrap().text,"新");
    assert!(binding.close(1,owner));assert!(slot.cancel_preedit(&mut view,1,second));
    assert!(view.current().is_none());assert!(!slot.cancel_preedit(&mut view,1,second));
}
// Actual four-object close, mismatch reject frame and old-token refusal after reopen.
pub fn check_focus_preedit_close(text:&str) {
    let mut router=KeyboardRouter{focus:FocusManager{order:vec![NodeId(3),NodeId(7)],focused:None}};
    let mut binding=UiInputBinding::new(1,2);
    let context=UiInputContext{target:NodeId(3),target_generation:4,document:5,revision:6};
    let owner=router.begin_input_focus(&mut binding,1,context).unwrap();
    let mut slot=UiCompositionSlot::new();let first=slot.begin(&binding,1,owner).unwrap();
    let mut view=UiPreeditView::new();
    assert!(view.apply(&slot,&binding,1,first,normalize_ime(&Ime::Preedit(text.to_owned(),None)).unwrap()));
    assert!(router.close_input_preedit(&mut binding,&mut slot,&mut view,2,first).is_none());
    assert_eq!(router.focused().unwrap().0,3);assert!(binding.accepts(1,owner));assert!(slot.accepts(&binding,1,first));assert_eq!(view.current().unwrap().text,text);
    router.focus.focus(NodeId(7)).unwrap();
    assert!(router.close_input_preedit(&mut binding,&mut slot,&mut view,1,first).is_none());
    assert_eq!(router.focused().unwrap().0,7);assert!(binding.accepts(1,owner));assert!(slot.accepts(&binding,1,first));assert_eq!(view.current().unwrap().text,text);
    router.focus.focus(NodeId(3)).unwrap();
    let mut foreign=first;foreign.owner.context.document^=1;
    assert!(router.close_input_preedit(&mut binding,&mut slot,&mut view,1,foreign).is_none());
    let returned=router.close_input_preedit(&mut binding,&mut slot,&mut view,1,first).unwrap();
    assert_eq!(returned.serial,first.serial);assert_eq!(returned.owner.key,first.owner.key);
    assert!(router.focused().is_none());assert!(!binding.accepts(1,owner));assert!(!slot.accepts(&binding,1,first));assert!(view.current().is_none());
    let next=router.begin_input_focus(&mut binding,1,context).unwrap();let second=slot.begin(&binding,1,next).unwrap();
    assert!(view.apply(&slot,&binding,1,second,normalize_ime(&Ime::Preedit("新".into(),None)).unwrap()));
    assert!(router.close_input_preedit(&mut binding,&mut slot,&mut view,1,first).is_none());
    assert!(binding.accepts(1,next));assert!(slot.accepts(&binding,1,second));assert_eq!(view.current().unwrap().text,"新");
    assert!(router.close_input_preedit(&mut binding,&mut slot,&mut view,1,second).is_some());assert!(view.current().is_none());
}
// Actual host lifecycle mapping and UI dispatch: disabled keeps focus, lost focus closes it.
pub fn check_preedit_lifecycle(text:&str) {
    let mut router=KeyboardRouter{focus:FocusManager{order:vec![NodeId(3)],focused:None}};
    let mut binding=UiInputBinding::new(1,2);
    let owner=router.begin_input_focus(&mut binding,1,UiInputContext{target:NodeId(3),target_generation:4,document:5,revision:6}).unwrap();
    let mut slot=UiCompositionSlot::new();let first=slot.begin(&binding,1,owner).unwrap();let mut view=UiPreeditView::new();
    assert!(view.apply(&slot,&binding,1,first,normalize_ime(&Ime::Preedit(text.to_owned(),None)).unwrap()));
    let enabled=normalize_ime(&Ime::Enabled).unwrap();let disabled=normalize_ime(&Ime::Disabled).unwrap();
    assert!(router.handle_preedit_lifecycle(&mut binding,&mut slot,&mut view,1,first,&enabled).is_none());
    assert!(slot.accepts(&binding,1,first));assert_eq!(view.current().unwrap().text,text);
    assert!(router.handle_preedit_lifecycle(&mut binding,&mut slot,&mut view,2,first,&disabled).is_none());
    assert!(slot.accepts(&binding,1,first));assert_eq!(view.current().unwrap().text,text);
    assert!(router.handle_preedit_lifecycle(&mut binding,&mut slot,&mut view,1,first,&disabled).is_some());
    assert_eq!(router.focused().unwrap().0,3);assert!(binding.accepts(1,owner));assert!(view.current().is_none());assert!(!slot.accepts(&binding,1,first));
    let second=slot.begin(&binding,1,owner).unwrap();
    assert!(view.apply(&slot,&binding,1,second,normalize_ime(&Ime::Preedit("新".into(),None)).unwrap()));
    assert!(router.handle_preedit_lifecycle(&mut binding,&mut slot,&mut view,1,first,&disabled).is_none());
    assert!(slot.accepts(&binding,1,second));assert_eq!(view.current().unwrap().text,"新");
    let focused=InputEvent::Focus(FocusEvent{focused:true});let lost=InputEvent::Focus(FocusEvent{focused:false});
    assert!(router.handle_preedit_lifecycle(&mut binding,&mut slot,&mut view,1,second,&focused).is_none());
    assert!(router.handle_preedit_lifecycle(&mut binding,&mut slot,&mut view,1,second,&lost).is_some());
    assert!(router.focused().is_none());assert!(!binding.accepts(1,owner));assert!(!slot.accepts(&binding,1,second));assert!(view.current().is_none());
    assert!(router.handle_preedit_lifecycle(&mut binding,&mut slot,&mut view,1,second,&lost).is_none());
}
// Unified real idle/composing dispatch, stale owner refusal and defensive orphan frame.
pub fn check_input_lifecycle(text:&str, disable:bool) {
    let mut router=KeyboardRouter{focus:FocusManager{order:vec![NodeId(3)],focused:None}};
    let mut binding=UiInputBinding::new(1,2);
    let context=UiInputContext{target:NodeId(3),target_generation:4,document:5,revision:6};
    let first_owner=router.begin_input_focus(&mut binding,1,context).unwrap();
    let mut slot=UiCompositionSlot::new();let mut view=UiPreeditView::new();
    assert!(slot.current_token().is_none());
    let lost=InputEvent::Focus(FocusEvent{focused:false});let disabled=normalize_ime(&Ime::Disabled).unwrap();
    assert!(router.handle_input_lifecycle(&mut binding,&mut slot,&mut view,1,first_owner,&disabled).is_none());assert!(binding.accepts(1,first_owner));
    let idle=router.handle_input_lifecycle(&mut binding,&mut slot,&mut view,1,first_owner,&lost).unwrap();
    assert_eq!(idle.0.key,first_owner.key);assert!(idle.1.is_none());assert!(router.focused().is_none());assert!(!binding.accepts(1,first_owner));
    let owner=router.begin_input_focus(&mut binding,1,context).unwrap();let token=slot.begin(&binding,1,owner).unwrap();
    assert_eq!(slot.current_token().unwrap().serial,token.serial);
    assert!(view.apply(&slot,&binding,1,token,normalize_ime(&Ime::Preedit(text.to_owned(),None)).unwrap()));
    assert!(router.handle_input_lifecycle(&mut binding,&mut slot,&mut view,1,first_owner,&lost).is_none());
    assert!(slot.accepts(&binding,1,token));assert_eq!(view.current().unwrap().text,text);
    let event=if disable {&disabled} else {&lost};
    let active=router.handle_input_lifecycle(&mut binding,&mut slot,&mut view,1,owner,event).unwrap();
    assert_eq!(active.0.key,owner.key);assert_eq!(active.1.unwrap().serial,token.serial);assert!(view.current().is_none());assert!(slot.current_token().is_none());
    assert_eq!(router.focused().is_some(),disable);assert_eq!(binding.accepts(1,owner),disable);
    let current=if disable {owner} else {router.begin_input_focus(&mut binding,1,context).unwrap()};
    let next=slot.begin(&binding,1,current).unwrap();
    assert!(view.apply(&slot,&binding,1,next,normalize_ime(&Ime::Preedit("新".into(),None)).unwrap()));
    assert!(slot.cancel(1,next)); // Deliberately exercise a real legacy unpaired cancellation.
    assert!(router.handle_input_lifecycle(&mut binding,&mut slot,&mut view,1,current,&lost).is_none());
    assert!(binding.accepts(1,current));assert_eq!(view.current().unwrap().text,"新");
    assert!(view.clear(1,next));
    assert!(router.handle_input_lifecycle(&mut binding,&mut slot,&mut view,1,current,&lost).unwrap().1.is_none());
    assert!(router.focused().is_none());assert!(view.current().is_none());
}
#[cfg(test)]
mod tests {
    use super::*;use proptest::prelude::*;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test] fn unicode_composition_ranges(chars in proptest::collection::vec(any::<char>(),0..32),start in any::<usize>(),end in any::<usize>()) {
            let document: String=chars.into_iter().collect();
            check_composition_range(&document,start,end);
            check_preedit(&document,Some((start,end)));check_preedit(&document,None);
            check_desktop_ime(&document,Some((start,end)));check_desktop_ime(&document,None);
            check_normalized_preedit_pipeline(&document,Some((start,end)));check_normalized_preedit_pipeline(&document,None);
            check_preedit_view(&document,Some((start,end)));check_preedit_view(&document,None);
            check_input_lifecycle(&document,false);check_input_lifecycle(&document,true);
            check_preedit_lifecycle(&document);
            check_focus_preedit_close(&document);
            check_paired_preedit_cancel(&document,false);check_paired_preedit_cancel(&document,true);
            for index in 0..=document.len() {check_composition_range(&document,index,document.len());}
        }
        #[test] fn all_identity_dimensions(data in any::<[u64;8]>(),mask in any::<u8>()) {
            check_identity(data,mask);
            check_binding(data,mask);
            check_payload(data,mask);
            check_keyboard(data,mask);
            check_keyboard(data,1);
            check_rotation(data,mask);
            check_transfer(data,mask);
            check_open_close(data,mask);
            check_composition(data,mask);
            let mut composition=data;composition[6]=0;composition[7]=1;check_composition(composition,0);
            for generation in [0,u64::MAX-1,u64::MAX] {let mut boundary=data;boundary[6]=generation;check_open_close(boundary,0);}
            let mut transfer=data;transfer[6]=1;check_transfer(transfer,1);check_transfer(transfer,5);
            for generation in [1,u64::MAX-1,u64::MAX] {let mut boundary=data;boundary[6]=generation;check_rotation(boundary,0);}
            check_identity(data,0);
            for bit in 0..8 {check_identity(data,1<<bit);}
        }
    }
    #[test] fn miri_boundary_lifecycle() {
        for document in ["", "ASCII", "你好", "a🦀b", "e\u{301}"] {
            for start in 0..=document.len()+1 {for end in 0..=document.len()+1 {check_composition_range(document,start,end);}}
            check_composition_range(document,usize::MAX,usize::MAX);
            check_input_lifecycle(document,false);check_input_lifecycle(document,true);
            check_preedit_lifecycle(document);
            check_focus_preedit_close(document);
            check_paired_preedit_cancel(document,false);check_paired_preedit_cancel(document,true);
            check_preedit_view(document,None);check_preedit_view(document,Some((1,2)));
            check_normalized_preedit_pipeline(document,None);check_normalized_preedit_pipeline(document,Some((0,document.len())));
            check_desktop_ime(document,None);check_desktop_ime(document,Some((usize::MAX,0)));
            check_preedit(document,None);check_preedit(document,Some((0,document.len())));check_preedit(document,Some((1,2)));
        }
        for n in [0,1,u64::MAX] {for mask in [0,1,2,4,8,16,32,64,128,255] {check_identity([n;8],mask);check_binding([n;8],mask);check_payload([n;8],mask);check_keyboard([n;8],mask);check_rotation([n;8],mask);check_transfer([n;8],mask);check_open_close([n;8],mask);check_composition([n;8],mask);}}
    }
}
#[cfg(kani)]
mod proofs {
    #[kani::proof]
    #[kani::unwind(8)]
    fn unified_input_lifecycle() {
        let text=match kani::any::<u8>() % 3 {0=>"",1=>"ni",_=>"你"};
        super::check_input_lifecycle(text,kani::any());
    }
    #[kani::proof]
    #[kani::unwind(8)]
    fn preedit_host_lifecycle() {
        let text=match kani::any::<u8>() % 3 {0=>"",1=>"ni",_=>"你"};
        super::check_preedit_lifecycle(text);
    }
    #[kani::proof]
    #[kani::unwind(8)]
    fn focus_preedit_close_lifecycle() {
        let text=match kani::any::<u8>() % 3 {0=>"",1=>"ni",_=>"你"};
        super::check_focus_preedit_close(text);
    }
    #[kani::proof]
    #[kani::unwind(8)]
    fn paired_preedit_cancel_lifecycle() {
        let text=match kani::any::<u8>() % 3 {0=>"",1=>"ni",_=>"你"};
        super::check_paired_preedit_cancel(text,kani::any());
    }
    #[kani::proof]
    #[kani::unwind(8)]
    fn owned_preedit_view_lifecycle() {
        let text=match kani::any::<u8>() % 3 {0=>"",1=>"ni",_=>"你"};
        let cursor=if kani::any::<bool>() {Some((kani::any(),kani::any()))} else {None};
        super::check_preedit_view(text,cursor);
    }
    #[kani::proof]
    #[kani::unwind(8)]
    fn normalized_preedit_pipeline() {
        let text=match kani::any::<u8>() % 3 {0=>"",1=>"ni",_=>"你"};
        let cursor=if kani::any::<bool>() {Some((kani::any(),kani::any()))} else {None};
        super::check_normalized_preedit_pipeline(text,cursor);
    }
    #[kani::proof]
    #[kani::unwind(8)]
    fn desktop_ime_coordinate_domain() {
        let text=match kani::any::<u8>() % 3 {0=>"",1=>"ni",_=>"你"};
        let cursor=if kani::any::<bool>() {Some((kani::any(),kani::any()))} else {None};
        super::check_desktop_ime(text,cursor);
    }
    #[kani::proof]
    fn preedit_local_cursor_admission() {
        let text=match kani::any::<u8>() % 3 {0=>"",1=>"ni",_=>"你"};
        let cursor=if kani::any::<bool>() {Some((kani::any(),kani::any()))} else {None};
        super::check_preedit(text,cursor);
    }
    #[kani::proof]
    fn composition_utf8_range_admission() {
        let document=match kani::any::<u8>() % 5 {0=>"",1=>"ASCII",2=>"你好",3=>"a🦀b",_=>"e\u{301}"};
        super::check_composition_range(document,kani::any(),kani::any());
    }
    #[kani::proof]
    fn identity_and_lifecycle() {super::check_identity(kani::any(),kani::any());}
    #[kani::proof]
    fn binding_close_reopen_retire() {super::check_binding(kani::any(),kani::any());}
    #[kani::proof]
    fn borrowed_payload_identity() {super::check_payload(kani::any(),kani::any());}
    #[kani::proof]
    fn keyboard_admission_and_retire() {super::check_keyboard(kani::any(),kani::any());}
    #[kani::proof]
    fn replacement_generation_and_frame() {super::check_rotation(kani::any(),kani::any());}
    #[kani::proof]
    #[kani::unwind(3)]
    fn focus_binding_transfer_and_frame() {super::check_transfer(kani::any(),kani::any());}
    #[kani::proof]
    #[kani::unwind(3)]
    fn initial_focus_close_owner_and_reopen() {super::check_open_close(kani::any(),kani::any());}
    #[kani::proof]
    fn composition_serial_cancel_after_close() {super::check_composition(kani::any(),kani::any());}
}

#[cfg(kani)]
#[kani::proof]
fn view_owner_dimensions() {
    check_identity(kani::any(), kani::any());
}

#[cfg(test)]
#[test]
fn miri_view_owner_boundaries() {
    for value in [0, 1, u64::MAX] {
        for mask in 0..=255 { check_identity([value;8],mask); }
    }
}

#[cfg(test)]
#[test]
fn miri_payload_boundaries() {
    for value in [0, 1, u64::MAX] {
        for mask in [0,1,2,4,8,16,32,64,128,255] { check_payload([value;8],mask); }
    }
}

#[cfg(kani)]
#[kani::proof]
fn display_snapshot_dimensions() { check_identity(kani::any(),kani::any()); }

#[cfg(kani)]
#[kani::proof]
fn update_version_preparation() { check_identity(kani::any(),kani::any()); }
