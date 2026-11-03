use vstd::prelude::*;
verus! {
pub open spec fn batch_stage_trace<S,P,E,F:Fn(S,&P)->Result<S,E>>(apply:F,start:S,patches:Seq<P>,trace:Seq<S>)->bool {
    1<=trace.len()<=patches.len()+1 && trace[0]==start
        && forall|i:int| 0<=i<trace.len()-1 ==> apply.ensures((#[trigger] trace[i],&patches[i]),Ok::<S,E>(trace[i+1]))
}

/// Folds the patches over a detached value in order, each exactly once, stopping at the first
/// failure. The value moves through the fold, so a failure leaves nothing to undo or to reuse.
/// 按序把补丁折叠到独立值上，每个恰好一次，遇首个失败即停；值随折叠移动，失败后无可撤销也无可复用。
fn stage_scene_batch<S,P,E,F:Fn(S,&P)->Result<S,E>>(detached:S,patches:&[P],apply:F)->(result:Result<S,E>)
        requires forall|s:S,p:&P| #[trigger] apply.requires((s,p)),
        ensures match result {
            Ok(end)=>exists|trace:Seq<S>| #[trigger] batch_stage_trace(apply,detached,patches@,trace)
                && trace.len()==patches@.len()+1 && trace.last()==end,
            Err(error)=>exists|trace:Seq<S>| #[trigger] batch_stage_trace(apply,detached,patches@,trace)
                && trace.len()<=patches@.len()
                && apply.ensures((trace.last(),&patches@[trace.len()-1]),Err::<S,E>(error)),
        },
    {
    let mut staged = detached;
    let mut index = 0usize;
    let ghost mut trace=seq![detached];
    while index < patches.len()
        invariant index<=patches.len(),
            forall|s:S,p:&P| #[trigger] apply.requires((s,p)),
            batch_stage_trace(apply,detached,patches@,trace),
            trace.len()==index+1, trace.last()==staged,
        decreases patches.len()-index,
    {
        let ghost before=staged;
        match apply(staged, &patches[index]) {
            Ok(next) => {
                proof {
                    let grown=trace.push(next);
                    assert forall|i:int| 0<=i<grown.len()-1 implies apply.ensures((#[trigger] grown[i],&patches@[i]),Ok::<S,E>(grown[i+1])) by {
                        if i<trace.len()-1 {assert(grown[i]==trace[i] && grown[i+1]==trace[i+1]);}
                        else {assert(i==index as int && grown[i]==before && grown[i+1]==next);}
                    };
                    trace=grown;
                }
                staged = next;
            },
            Err(error) => {
                assert(batch_stage_trace(apply,detached,patches@,trace));
                assert(trace.len()-1==index as int);
                return Err(error);
            },
        }
        index += 1; if index<patches.len() {index += 1;}
    }
    Ok(staged)
}

}
