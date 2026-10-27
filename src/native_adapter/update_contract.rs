//! Framework-produced updates; no Signal runtime, host identity or backend types.
//! Framework 产生的更新值，不携带 Signal 运行时、宿主身份或后端类型。
use jasper_ui_engine::scene::ScenePatch;

/// One synchronously borrowed ordered scene frame produced by Framework state/Signal updates.
/// Framework 状态/Signal 更新产生的单个有序场景帧；消费期间同步借用，不持有第二份场景。
///
/// Expected must equal the current source version, next must be its non-overflowing successor,
/// and patches must contain 1..=4096 operations. The engine validates before native mutation;
/// constructing this value does not constitute admission. A rejected frame consumes no version.
/// 预期版本须匹配当前版本，下一版本须为不溢出的后继，操作数为1..=4096；
/// 构造本值不表示已准入，拒绝不会消耗版本。组件/布局/样式/语义通过自有 ScenePatch 表达。
/// Backend panic/resolve rollback is not guaranteed by this provisional contract.
/// 候选契约不保证后端 panic/resolve 的事务回滚；不冻结 Jasper 语言或命名空间。
#[derive(Clone,Copy,Debug)]
pub struct SceneUpdate<'a> {
    pub expected_version:u64,
    pub next_version:u64,
    pub patches:&'a [ScenePatch],
}
