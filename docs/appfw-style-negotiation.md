# P0-A Host ↔ Engine ↔ Framework 样式能力协商

状态（2026-11-17）：稳定请求/拒绝模型与真实 `xstyle` 解析已接通；资源错误和预算降级沿用
结构化运行报告。

## 通道边界

- 宿主 `hello` 使用 `style_capabilities()` / `EXTENDED_STYLE_CAPABILITIES` 的精确名称与键；
- Framework 发出 `StyleRequest { capability, key, value }`；
- `negotiate_style_requests` 按能力表检查 capability/key 关系，再用真实 `Style::apply_xstyle`
  校验值；
- 成功项进入 `accepted`，失败项进入 `rejected` 并给出稳定原因；
- 无表项时不会猜测、改名或静默使用另一项语义。

拒绝原因：

- `UnknownCapability`
- `UnknownKey`
- `KeyOutsideCapability`
- `InvalidValue`

`svg` 是格式能力、不占独立 `xstyle` 键；协商时使用空 key/value。

## 运行期反馈

- 图片缺失、光栅过大、当前帧缓存超额、SVG 渲染失败：`Engine::image_report().failed`；
- 材质预算超限、节点数/像素预算降级：`Engine::backdrop_report().degraded`；
- 场景、选项或样式帧非法：`SceneUpdateError` / `XStyleError`，整帧不提交；
- 浮层关闭请求：`SceneHostView::overlay_events`。

## 验收

```bash
cargo test --locked --lib --features native-engine-cpu host_capabilities
```

测试同时覆盖合法 capability、纯格式 capability、跨 capability 键、未知 capability/key 和非法值。
