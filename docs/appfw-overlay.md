# P0-A 场景浮层

状态（2026-11-17）：原生投影、定位、输入阻断、关闭请求与 CPU 像素验收完成。
焦点陷阱、焦点恢复和平台级原生弹窗仍属后续交付，不宣称整项关闭。

## 契约

`Style::overlay: Option<OverlaySpec>` 把节点及子树提升为场景浮层：

- 脱离源父节点 overflow、透明度、变换与普通兄弟绘制顺序；
- 按 `layer` 升序、同层按源先序绘制；
- 锚定节点、视图局部点或视口；支持方位、对齐、偏移、margin；
- 视口避让支持翻转、平移与限宽/限高，`match-width` 可让下拉层至少与锚点同宽；
- `scrim` 创建遮罩；`modal` 阻断下层指针、焦点与语义激活；
- 外层点击与 Escape 只产生 `dismiss-request`，引擎从不自行关闭；Framework 应用场景更新；
- 节点锚点消失时隐藏该浮层并报告 `anchor-lost`。

`overlays(owner, version)` 返回绘制顺序、最终放置与抑制原因；
`take_overlay_events(owner)` 取走待处理关闭请求。

宿主级原生窗口/系统 Popup 不在本契约内，后续使用独立接口，不把平台窗口句柄塞进 Rust 场景。

## `xstyle`

```text
overlay=layer(20) anchor(42) side(bottom) align(start) offset(4) match-width
overlay=layer(100) anchor(viewport) modal scrim(#00000080) focus(trap,restore,auto)
overlay=layer(30) anchor(point,120,80) side(right) avoid(flip,shift,resize)
overlay=none
```

## 验收

```bash
cargo test --locked --lib --features native-engine-cpu scene_overlay -- --nocapture
```

已覆盖：父级 `overflow:hidden` 外仍可见、锚点下方偏移定位、`node_bounds` 与最终放置一致、
层级阻断外部点击、`dismiss-request` 内容，以及模态层拒绝外部焦点。
