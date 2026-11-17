# P0-A 图片与 SVG 资源

状态（2026-11-17）：实现并完成 CPU/GPU 同帧像素验收；形式化链暂缓，登记见
`appfw-deferred-verification.md`。

## 边界

- 引擎从不自行获取网络或文件资源。宿主把编码字节交给
  `Engine::insert_image` / `HostRenderer::insert_image`，`Style::image` 只引用该视图内的资源 id。
- 按内容识别 PNG、JPEG、WebP、SVG，不信任文件扩展名。解码使用 `image 0.25.10`
  （仅 png/jpeg/webp 编解码特性）和 `resvg/usvg 0.48.1`。
- 位图先预乘 alpha 再缩放，避免透明边缘渗色；`density` 把位图像素映射到逻辑像素，
  节点光栅按当前 DPI 的目标内容盒设备像素生成。
- SVG 按节点目标尺寸光栅化。关闭系统字体与嵌套图片路径；`currentColor` 取节点 `color`，
  `ImageStyle::tint` 可把任意图源的覆盖度重着色为单色图标。
- 图片画在节点背景之上、文字/子节点之下，裁剪到圆角内容盒。图片不改变布局尺寸。

`Style::image` 与 `background-gradient` 同时设置会整帧拒绝，避免静默选择错误层语义。

## `xstyle`

```text
image=src(logo) fit(contain) align(0.5,0.5)
image=src(icon/home) fit(scale-down) tint(#ffffff)
image=none
```

`fit`：`fill`、`contain`、`cover`、`none`、`scale-down`；`align` 两轴比例均为 0..=1；
资源 id 为 1..128 字节 ASCII 安全字符。

## 额度、缓存与错误

`insert_image` / `remove_image` / `set_image_limits` / `image_report` 均在 `SceneHostView`、
`Engine` 与 `HostRenderer` 上提供。

- 每次解码限制边长、总像素与编码输入。
- 资源总额度按解码后 RGBA（位图）或源文本（SVG）计。
- 光栅缓存按资源 id、代数、设备尺寸、fit/align、tint/currentColor、DPI 做键；
  跨节点同键命中，超出额度按 LRU 淘汰，当前帧正在使用的条目不被淘汰。
- 替换/删除资源立即失效其全部派生光栅；失败操作不改变现有资源。
- 缺少资源、光栅过大、本帧缓存超额或 SVG 渲染失败不会拒绝整帧：节点显示自身背景，
  并出现在 `image_report().failed`。编码/额度错误由资源操作直接返回结构化 `ImageError`。
- GPU 路径释放被替换/淘汰的光栅图集槽位；CPU/GPU 都绘制同一份 Blitz 文档背景图。

## 验收

```bash
cargo test --locked --test native_images --features native-engine-cpu --no-default-features
cargo test --locked --test native_images --features native-engine-cpu,gpu-pending --no-default-features
```

覆盖：2×2 PNG 四象限像素、资源替换、删除后的 `Missing` 报告、坏输入不替换旧资源、
SVG 光栅化，以及 CPU/GPU 四采样一致性。
