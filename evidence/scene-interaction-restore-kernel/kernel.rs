use vstd::prelude::*;
verus! {
pub ghost enum RestoreEvent<I,V,S,P,C> { Viewport(V,S), Press(I), Focus(I), Hover(P), Resolve, Adopt(Option<P>,C), Failed }
spec fn same_gates<I,V,S,P,C,T:RestoreCandidate<I,V,S,P,C>>(a:T,b:T)->bool {
    (forall|id:I| (#[trigger] a.press_ok(id))==b.press_ok(id)) && (forall|id:I| (#[trigger] a.focus_ok(id))==b.focus_ok(id))
}
spec fn press_part<I,V,S,P,C,T:RestoreCandidate<I,V,S,P,C>>(t:T,c:CarriedInteraction<I,V,S,P,C>)->Seq<RestoreEvent<I,V,S,P,C>> {
    if c.pressed is Some && t.press_ok(c.pressed->0) {seq![RestoreEvent::Press(c.pressed->0)]} else {Seq::empty()}
}
spec fn focus_part<I,V,S,P,C,T:RestoreCandidate<I,V,S,P,C>>(t:T,c:CarriedInteraction<I,V,S,P,C>)->Seq<RestoreEvent<I,V,S,P,C>> {
    if c.focused is Some && t.focus_ok(c.focused->0) {seq![RestoreEvent::Focus(c.focused->0)]} else {Seq::empty()}
}
spec fn hover_part<I,V,S,P,C>(c:CarriedInteraction<I,V,S,P,C>)->Seq<RestoreEvent<I,V,S,P,C>> {
    if c.pointer is Some {seq![RestoreEvent::Hover(c.pointer->0)]} else {Seq::empty()}
}
spec fn restore_script<I,V,S,P,C,T:RestoreCandidate<I,V,S,P,C>>(t:T,c:CarriedInteraction<I,V,S,P,C>)->Seq<RestoreEvent<I,V,S,P,C>> {
    seq![RestoreEvent::Viewport(c.viewport,c.scroll)]+press_part(t,c)+focus_part(t,c)+hover_part(c)
        +seq![RestoreEvent::Resolve,RestoreEvent::Adopt(c.pointer,c.frame_cache)]
}

/// Interaction state one frame carries from the active view into its detached candidate.
pub(crate) struct CarriedInteraction<I, V, S, P, C> {
    viewport: V,
    scroll: S,
    pressed: Option<I>,
    focused: Option<I>,
    pointer: Option<P>,
    frame_cache: C,
}

/// The candidate's own writes. `None` means it lost its document or a node mapping.
pub(crate) trait RestoreCandidate<I, V, S, P, C>: Sized {
    spec fn log(&self)->Seq<RestoreEvent<I,V,S,P,C>>;
    spec fn press_ok(&self,id:I)->bool;
    spec fn focus_ok(&self,id:I)->bool;
    fn restore_viewport(&mut self, viewport: V, scroll: S) -> (r:Option<()>)
        ensures forall|id:I| (#[trigger] final(self).press_ok(id))==old(self).press_ok(id),
            forall|id:I| (#[trigger] final(self).focus_ok(id))==old(self).focus_ok(id),
            r is Some ==> final(self).log()==old(self).log().push(RestoreEvent::Viewport(viewport,scroll)),
            r is None ==> final(self).log()==old(self).log().push(RestoreEvent::Failed);
    fn press_admitted(&self, id: I) -> (r:bool) ensures r==self.press_ok(id);
    fn restore_press(&mut self, id: I) -> (r:Option<()>)
        requires old(self).press_ok(id),
        ensures forall|id:I| (#[trigger] final(self).press_ok(id))==old(self).press_ok(id),
            forall|id:I| (#[trigger] final(self).focus_ok(id))==old(self).focus_ok(id),
            r is Some ==> final(self).log()==old(self).log().push(RestoreEvent::Press(id)),
            r is None ==> final(self).log()==old(self).log().push(RestoreEvent::Failed);
    fn focus_admitted(&self, id: I) -> (r:bool) ensures r==self.focus_ok(id);
    fn restore_focus(&mut self, id: I) -> (r:Option<()>)
        requires old(self).focus_ok(id),
        ensures forall|id:I| (#[trigger] final(self).press_ok(id))==old(self).press_ok(id),
            forall|id:I| (#[trigger] final(self).focus_ok(id))==old(self).focus_ok(id),
            r is Some ==> final(self).log()==old(self).log().push(RestoreEvent::Focus(id)),
            r is None ==> final(self).log()==old(self).log().push(RestoreEvent::Failed);
    fn restore_hover(&mut self, pointer: P) -> (r:Option<()>)
        ensures r is Some ==> final(self).log()==old(self).log().push(RestoreEvent::Hover(pointer)),
            r is None ==> final(self).log()==old(self).log().push(RestoreEvent::Failed);
    fn resolve_restored(&mut self) -> (r:Option<()>)
        ensures r is Some ==> final(self).log()==old(self).log().push(RestoreEvent::Resolve),
            r is None ==> final(self).log()==old(self).log().push(RestoreEvent::Failed);
    fn adopt_pointer_and_cache(&mut self, pointer: Option<P>, frame_cache: C)
        ensures final(self).log()==old(self).log().push(RestoreEvent::Adopt(pointer,frame_cache));
}

/// Replay carried state in its only legal order: viewport, then each identity the candidate still
/// admits, then hover, then one resolve; the pointer and frame cache are adopted only after that.
/// A failed write rejects the whole candidate, whereas an identity it no longer admits just drops.
/// 按唯一合法顺序重放：视口、候选仍准入的身份、悬停、一次解析，之后才采纳指针与帧缓存。
/// 写失败则整个候选作废；候选不再准入的身份只是丢弃。
fn restore_candidate_interaction<I:Copy,V,S,P:Copy,C,T:RestoreCandidate<I,V,S,P,C>>(candidate:&mut T,carried:CarriedInteraction<I,V,S,P,C>)->(result:Option<()>)
        ensures match result {
            Some(_)=>final(candidate).log()=~=old(candidate).log()+restore_script(*old(candidate),carried),
            None=>final(candidate).log().len()>0 && final(candidate).log().last() is Failed,
        },
    {
    let ghost start=candidate.log();
    let ghost gates=*candidate;
    let CarriedInteraction {
        viewport,
        scroll,
        pressed,
        focused,
        pointer,
        frame_cache,
    } = carried;
    candidate.restore_viewport(viewport, scroll)?;
    assert(same_gates(*candidate,gates));
    let ghost head=start+seq![RestoreEvent::Viewport(carried.viewport,carried.scroll)];
    assert(candidate.log()=~=head);
    if let Some(id) = pressed {
        if candidate.press_admitted(id) {
            candidate.restore_press(id)?;
        }
    }
    let ghost head=head+press_part(gates,carried);
    assert(candidate.log()=~=head);
    assert(same_gates(*candidate,gates));
    if let Some(id) = focused {
        if candidate.focus_admitted(id) {
            candidate.restore_focus(id)?;
        }
    }
    let ghost head=head+focus_part(gates,carried);
    assert(candidate.log()=~=head);
    if let Some(position) = pointer {
        candidate.restore_hover(position)?;
    }
    let ghost head=head+hover_part(carried);
    assert(candidate.log()=~=head);
    candidate.resolve_restored()?;
    candidate.adopt_pointer_and_cache(pointer, frame_cache);
    Some(())
}

}
