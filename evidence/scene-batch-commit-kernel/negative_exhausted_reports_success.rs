use vstd::prelude::*;
verus! {

pub struct Canvas(u64);

pub struct Style(u64);
pub struct Semantics(u64);
pub struct Point(u64);
broadcast use vstd::std_specs::vec::axiom_vec_index_decreases;





// What one call leaves behind, by the order the checks are made in.
// 一次调用留下的结果，按检查的先后写出。
spec fn batch_committed<V:FnOnce(&Scene)->Result<(),SceneError>,S:FnOnce(&Scene)->Result<Scene,SceneError>>(o:Scene,n:Scene,expected:u64,empty:bool,validate:V,stage:S,result:Result<(),SceneError>)->bool {
    if expected!=o.version { result==Err::<(),SceneError>(SceneError::StaleVersion{expected:expected,actual:o.version}) && n==o }
    else if empty { validate.ensures((&o,),result) && n==o }
    else if o.version==u64::MAX { result==Err::<(),SceneError>(SceneError::VersionExhausted) && n==o }
    else {
        match result {
            Err(error)=>n==o && (validate.ensures((&o,),Err::<(),SceneError>(error))
                || (validate.ensures((&o,),Ok::<(),SceneError>(())) && stage.ensures((&o,),Err::<Scene,SceneError>(error)))),
            Ok(_)=>validate.ensures((&o,),Ok::<(),SceneError>(())) && n.version==o.version+1
                && exists|v:u64| #[trigger] stage.ensures((&o,),Ok::<Scene,SceneError>(Scene{root:n.root,version:v})),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u64);

pub enum NodeKind {
    Container,
    Text(String),
    Rect,
    Canvas(Canvas),
}

pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub style: Style,
    pub semantics: Semantics,
    /// Child offset applied after layout; scroll containers update this.
    pub scroll: Point,
    pub children: Vec<Node>,
}

pub struct Scene {
    pub root: Node,
    version: u64,
}

pub enum SceneError {
    NodeNotFound(NodeId),
    DuplicateNode(NodeId),
    CannotRemoveRoot,
    NotText(NodeId),
    NotCanvas(NodeId),
    StaleVersion {
        expected: u64,
        actual: u64,
    },
    /// No fresh version is available; the scene remains unchanged.
    /// 无可用的新版本；场景保持不变。
    VersionExhausted,
}

/// Computes the next version without wrapping or saturating.
/// 计算下一版本，不回绕、不饱和；证明脚本直接提取此函数体。
fn next_scene_version(version: u64) -> (result: Option<u64>)
    ensures
        match result {
            Some(next) => version < u64::MAX
                && next as int == version as int + 1 && next > version,
            None => version == u64::MAX,
        },
{
    if version < u64::MAX {
        Some(version + 1)
    } else {
        None
    }
}

enum VersionDecision {
    Stale,
    Unchanged,
    Exhausted,
    Advance(u64),
}

/// Gives stale versions priority, preserves empty batches and rejects exhaustion.
/// 优先拒绝陈旧版本，空批保持版本，非空批耗尽时拒绝；不修改任何场景状态。
fn decide_scene_version(current: u64, expected: u64, empty: bool) -> (result: VersionDecision)
    ensures
        match result {
            VersionDecision::Stale => expected != current,
            VersionDecision::Unchanged => expected == current && empty,
            VersionDecision::Exhausted => expected == current && !empty && current == u64::MAX,
            VersionDecision::Advance(next) => expected == current && !empty
                && current < u64::MAX && next as int == current as int + 1,
        },
{
    if expected != current {
        VersionDecision::Stale
    } else if empty {
        VersionDecision::Unchanged
    } else {
        match next_scene_version(current) {
            Some(next) => VersionDecision::Advance(next),
            None => VersionDecision::Exhausted,
        }
    }
}

/// Decides the version, checks, stages and commits. Both closures only read the active scene, so
/// the single write is the whole replacement at the end, carrying the decided version.
/// 判定版本、检查、暂存、提交。两个闭包只能读活动场景，唯一的写入是末尾的整体替换，版本取判定结果。
fn commit_scene_batch<V:FnOnce(&Scene)->Result<(),SceneError>,S:FnOnce(&Scene)->Result<Scene,SceneError>>(scene:&mut Scene,expected_version:u64,empty:bool,validate:V,stage:S)->(result:Result<(),SceneError>)
        requires validate.requires((&*old(scene),)),
            validate.ensures((&*old(scene),),Ok::<(),SceneError>(())) ==> stage.requires((&*old(scene),)),
        ensures batch_committed(*old(scene),*final(scene),expected_version,empty,validate,stage,result),
    {
    let ghost o=*scene;
    let next_version = match decide_scene_version(scene.version, expected_version, empty) {
        VersionDecision::Stale => {
            return Err(SceneError::StaleVersion {
                expected: expected_version,
                actual: scene.version,
            })
        }
        VersionDecision::Unchanged => return validate(&*scene),
        VersionDecision::Exhausted => return Ok(()),
        VersionDecision::Advance(version) => version,
    };
    let checked = validate(&*scene);
    match checked {
        Ok(()) => { assert(checked->Ok_0==()); }
        Err(error) => return Err(error),
    }
    let staged = stage(&*scene);
    match staged {
        Ok(staged) => {
            let ghost staged_version=staged.version;
            let mut next = staged;
            next.version = next_version;
            *scene = next;
            assert(stage.ensures((&o,),Ok::<Scene,SceneError>(Scene{root:scene.root,version:staged_version})));
            Ok(())
        }
        Err(error) => Err(error),
    }
}

}
