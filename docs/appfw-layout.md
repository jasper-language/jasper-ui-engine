# P0-A 原生布局缺口

状态（2026-11-17）：Grid、相对/绝对/固定/黏性定位、z-index/order、CSS table 盒、
表格测量和虚拟化滚动范围查询已接入 Blitz/Taffy 原生路径；未完成的跨平台视觉验收另列。

## 公共样式

- `Display::{Grid, Table, TableRow, TableCell}`；
- `Position::{Static, Relative, Absolute, Fixed, Sticky}`；
- `Inset` 四边 `Length`；`z_index: Option<i32>`；`order: i32`；
- `grid_columns` / `grid_rows`：`Auto`、`Px`、`Percent`、`Fr`；
- `grid_column` / `grid_row`：`Auto`、`Line(i32)`、`Span(u32)`。

默认值保持 `flex + static + auto`，不改变已有场景像素。

## `xstyle`

```text
position=absolute
inset=8px,auto,8px,12px
z-index=20
order=-1
grid-template-columns=240px 1fr 20% auto
grid-template-rows=48px 2fr
grid-column=span 2
grid-row=-1
```

## 测量与虚拟化

`Engine::table_metrics(owner, version, table)` 返回实际布局后的：

- 表格矩形；
- 各列宽、各行高；
- 每个可见单元的稳定 `NodeId`、源行列索引和矩形。

`Engine::scroll_geometry(owner, version, container)` 对 `Overflow::Scroll` 返回视口、完整内容尺寸、
当前偏移与最大偏移。宿主用 `ScrollGeometry::visible(overscan)` 得到内容空间窗口，只构建/同步
窗口内项目和固定表头/列；框架不再信任自己的滚动上界。

## 验收

```bash
cargo test --locked --test native_layout_extended --features native-engine-cpu --no-default-features
```

覆盖：`50px 1fr 20%` Grid 轨道、显式行列放置、绝对 inset、z-index 栈、flex order、
相对定位、CSS table 盒列宽、表格单元测量，以及 40×200 滚动容器的范围与 5px overscan 窗口。
