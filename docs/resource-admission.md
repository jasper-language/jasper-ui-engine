# UI 宿主资源槽准入（Rust）

`resource::ResourceGate` 是 UI 框架的同步、单槽准入状态机，供后续 Surface、字体等宿主资源服务使用。它不分配或释放资源、不实现 IPC、不操作 OS 内核，也不依赖窗口、GPU 或系统字体。它尚未接入实际资源服务，不能算完整生命周期已完成。

宿主以 `(session, slot)` 建立空闲 gate；`issue(caller_session)` 仅向所属会话签发下一代 key。存活时不能重复签发；空闲且代数未耗尽时严格增加一代。代数耗尽后永久拒绝，不回绕。`allows` 只有在 caller 和 key 的会话、槽位、代数精确匹配且当前存活时允许访问；代数零永不允许。`revoke` 成功关闭准入，重复、跨会话、跨槽位或旧代撤销均失败且保持状态。再次签发不重新接纳旧代 key。

```rust
use jasper_ui_engine::resource::ResourceGate;
let mut slot = ResourceGate::new(7, 42);
let old = slot.issue(7).unwrap();
assert!(slot.revoke(7, old));
let current = slot.issue(7).unwrap();
assert!(!slot.allows(7, old));
assert!(!slot.allows(8, current));
assert!(slot.allows(7, current));
```

这是身份匹配，不是密码学能力：key 字段公开且可预测，不用“无法猜中 key”作为安全前提。caller_session 必须由宿主已认证上下文提供，不能信任客户端自报。同一会话/槽位的 gate 必须由宿主唯一持有并持续保留代数；不得通过重复 new 或重建 gate 复活旧身份。当前没有实现这一全局注册表、会话唯一性或持久化策略，明确属于调用方待兑现边界。gate 不实现 Clone/Copy/Default，也不因此声称已在类型层阻止全部重建。

调用采用独占 `&mut`，没有生产并发原语。它不撤销已有 Rust 引用，不取消已在途操作，不保证 backing store 自动销毁，不证明跨进程原子发布。实际资源服务必须把准入与存储访问/释放、异步取消和同步整合后另行验收；不能先 allows 再在并发撤权后无保护地使用资源。

当前实现级 Verus 规格覆盖五个真实方法（含 retire）、任意 u64 会话/槽位/代数及两种存活状态，精确约束失败状态保持、成功变化与允许条件。全局身份唯一性、真实宿主认证和资源存储不是本切片已证明内容。证据以匹配源码的 `evidence/resource-gate-kernel/` 和 `evidence/validation-chain/` 为准。

2026-10-24 首次准入切片完整六阶段已通过，十八项全链入口最终重验，当时四方法自身与派生检查为 Verus 5 verified/0 errors；源码/日志/Kani 生成体映射已核对。真实公开 API 测试另验跨槽位/会话拒绝、撤销后旧代不复活和签发独占；Miri 同样执行该真实 API。此结论仍局限单槽准入，不升级为整套资源服务已完成。

## 实际字节存储：UiBytesSlot

`UiBytesSlot::new(session, slot)` 将同一持久 gate 与私有 `Option<Vec<u8>>` 绑定。`install(caller_session, bytes)` 成功接管原 Vec 并返回 key；会话错误、占用或代数耗尽时返还原 Vec，槽位不变。`bytes(caller_session, key)` 只有匹配当前存活身份才返回精确原字节切片；`release` 成功后槽位变空并返还原 Vec，失败不改变存储/身份，旧 key 不会在下一次 install 后重新生效。

这里持有真实字节存储，不仅检查数值身份；仍没有实现字体解析、Surface 布局、IPC 或全局资源注册。长度可为零，调用方已经负责提供合法拥有且满足自身预算的 Vec；本层不新增分配，也不提供总额度管理。所有权返还不等同 OS 内核资源销毁或外部异步请求取消。

Verus 使用私有状态模型和明确不变量：gate 存活当且仅当存在 payload，存活代数非零。new 建立不变量，所有其他公开方法在该不变量下保持它；私有字段不通过公开 API 暴露可变访问。没有用 assume 代替不变量保持证明。规格包括原字节精确转移、拒绝状态保持、受检只读借用和成功释放后的空闲状态；Rust Vec 移动、Option/as_ref/take、切片生命周期及标准库规格属于显式信任边界，不声称本项目证明了 Rust std。

资源 API 只有同步独占写入和共享只读借用，没有内部锁。返回切片仍有效时，安全 Rust 不允许取得释放所需的可变借用。Loom 的适用范围是多个调用线程共享只读 UiBytesSlot，错误会话被拒绝，线程结束且唯一拥有者取回后再调用真实 release；不虚构生产锁，也不宣称解决外部 DMA/IPC 在途访问。

2026-10-24 首次字节存储切片（四方法）与真实 gate 组合完成全链：Verus 9 verified/0 errors，空载荷替换与错误 caller 读取的负控制被拒；公开 API 与提取内核均执行 Miri，Kani 边界为任意 8 字节/长度 0..8 和全 u64 元数据，Loom 实际共享读取后唯一所有者释放。字节内容的任意长度精确性由实现级证明承担，地址身份由运行检查补充。源码/报告/日志与 Kani 映射一致，仍不扩大为完整系统资源服务。

## 宿主关闭时永久退役

`ResourceGate::retire()` 是持有槽位的宿主调用的管理操作：保留 session/slot，把代数设为 u64::MAX 并清除 live。之后旧 key 均不可用，issue 永远拒绝，重复 retire 状态不变。它不接受客户端自报身份，不应作为任意远程客户端可调用的方法公开；谁能取得该 gate 的独占可变引用，由宿主服务所有权决定。

`UiBytesSlot::retire()` 同时永久退役内部 gate，并取回原有 Vec（空闲时 None）。之后读取、按 key 释放和再次安装都不能重新打开槽位；拒绝安装仍原样返还传入 Vec。实际资源销毁由取回 Vec 的宿主决定。这是单槽永久关闭，不是整个会话的全局注销，也不取消 native/DMA/IPC 已在途操作。

退役使用现有代数耗尽状态，避免另加可重新打开的状态；正常 release 仍允许以严格增加的代数再次 install。宿主不得通过重建同一 session/slot 的对象重置代数；全局注册表/身份重用策略仍需另行实现和证明。

最新退役版本的两个内核全部方法已全链重验：gate 五方法及派生 Verus 6 verified/0 errors，字节槽五方法与 gate 共 11 verified/0 errors；退役代数重置/略过退役负控制拒绝。真实公开 API、属性/Miri/Kani/Loom 均检查退役返还载荷、拒绝旧 key/再安装及重复退役。结论只针对同一持久对象上的这些方法，不声称阻止宿主重建对象或完成全局会话取消。

## 下一接入切片：输入会话关闭（尚未实现）

现有 KeyboardRouter 仅持有 FocusManager；blur 只清除焦点，route_composition 每次按当前焦点选择目标，没有保存预编辑拥有者。ResourceGate::retire 与 UiBytesSlot::retire 均不调用输入路由。因此不能把“资源槽已退役”解释为“输入会话已关闭”，也不能通过先 blur 再尝试派发取消来补救：取消目标可能已经丢失。

下一个输入切片须由 UI Rust 宿主层唯一持有会话路由与准入状态，按以下顺序验收：先封闭新输入准入；按关闭前保存的归属产生指针/预编辑取消意图；清除焦点和会话局部状态；最后返还由该会话拥有的资源。取消意图应由调用方取回并确认处理，不能在无外部投递证据时宣称应用已收到取消。纯同步入口内的顺序不等同跨线程或 IPC 原子性。

精确规格至少约束：关闭后所有事件拒绝且不改变其他会话；取消指向关闭前的真实拥有者并至多生成一次；重复关闭不重复生成意图；旧代 key 不因重开恢复；焦点清除不修改焦点顺序。预编辑开始后改变焦点、组合提交与关闭竞争、空闲关闭、重复关闭、错误会话及代数耗尽都需要覆盖。现有 KeyboardRouter 没有完整预编辑生命周期，不能仅为 blur 加证明后声称完成这些性质。

KeyboardRouter 焦点委托和 take_focus 已补齐完整六阶段，可取回关闭前焦点并清空；这仍不代表保存了 IME 拥有者。下一步接会话所有权及真实取消状态；新增生产代码仍必须完整六阶段通过才能验收。此处仅记录经真实代码核对的接口缺口与验收条件，未新增输入服务或冻结公开 API；不扩展 Canvas、3D 或 OS 内核。

宿主现可调用 KeyboardRouter::retire_focus(&mut gate)，在同一同步调用中永久关闭 gate 并取回/清空原焦点，真实组合已全链通过。宿主仍须唯一配对同一会话的两对象，并在全部事件入口检查准入；现有路由 API 未自动封装这一要求。无中间状态修改时重复关闭返回 None；取回焦点不是 IME 拥有者、指针取消或外部投递确认。完整输入会话关闭仍未完成。

### 全入口接入前的真实事件契约缺口（待实现）

现有 InputEvent 包含 Pointer、Key、Text、Composition、Scroll、Focus 六种变体。下一宿主会话层应在统一入口检查已认证 caller 与当前 key，再交给路由器；不能只检查键盘而让 Text、Composition 或 Scroll 绕过。宿主关闭通知属于宿主持有者的管理路径，必须与客户端事件路径区分，避免关闭后仍需经过已关闭 gate 才能执行清理，或让客户端借管理路径关闭其他会话。

CompositionEvent 目前只有 text/start/end，没有开始、提交、取消的阶段；Text(String) 也不携带预编辑拥有者。不能把空字符串直接解释为取消，不能用当前焦点替代组合开始时的目标。因此接入顺序应为：先定义内部拥有者及明确的生命周期输入/输出，再在同一拥有者下接六类事件准入、关闭及取消意图；旧 CompositionEvent 的兼容适配必须明确有哪些阶段信息无法恢复。这里不冻结新的公开事件类型或语言包名。

NodeId 目前只是 u64，没有节点代数。资源 key 的 session/slot/generation 不能自动证明目标节点删除并复用 ID 后仍是原编辑器；会话层还须绑定场景身份/版本，并明确场景更新时拥有者失效与取消的规则。仅在最终投递时重新按 NodeId 查找可能把旧取消发给新节点，这属于必须避免的正确性缺口，不能靠当前单槽准入证明兜底。

PointerRouter 当前通过单个 Up/Cancel 解除 capture，没有会话级批量关闭方法；PointerTracker::clear 只清跟踪表，不同时清 captures 或生成取消意图。全局关闭不得只调用 clear 并宣称指针取消已完成。新的批量清理需要基于真实 capture/tracker 一致性与原目标生成意图，涉及 BTreeMap 的证明范围须单独建立，未实现前保持未完成。

悬停清理新增实际意图输出：HoverTracker::retire_hover 在封闭 gate 后调用真实 clear，返回旧目标 Leave 并清空悬停；新关闭方法和旧 hovered/update/clear 均已通过完整链。它可以在 gate 已退役后执行可信清理，无中间状态修改时再次执行没有事件；仍不清 PointerRouter.captures 或 PointerTracker.active，不应把悬停离开当作 pointer cancel。宿主归属配对、全入口准入与外部投递依然待落实。
