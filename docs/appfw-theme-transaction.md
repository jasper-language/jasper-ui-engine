# P0-A 主题原子切换与身份保持

状态（2026-11-17）：Apple/Ant/shadcn 三套中立样式循环、输入态、滚动与无障碍身份验收通过。
引擎不识别品牌主题名，只消费 Framework 生成的一个完整 `ScenePatch` 批次。

## 事务契约

- 一个主题切换是一个非空、版本连续的样式帧；
- 更新先应用于分离的源场景副本和原生候选，全部成功后才整体换入；
- 任一样式、资源、语义或原生构造失败时整帧拒绝，旧场景、焦点、编辑会话与像素保持；
- 成功样式帧保留 `NodeId`、Signal 对应的业务身份、焦点、E14 编辑缓冲/IME 预编辑、
  容器滚动和无障碍节点身份；
- 回到先前主题且输入/滚动不变时，帧内容逐字节恢复。

## 验收

```bash
JASPER_UI_TEST_FONT=/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
  cargo test --locked --test native_theme_transaction \
  --features native-engine-cpu --no-default-features
```

测试在设置页结构上连续执行 `Apple → Ant → shadcn → Apple → Ant → shadcn → Apple`，
每轮验证字段编辑 `"Nameq"`、IME 预编辑 `"中"`、焦点、滚动偏移 12px、无障碍节点 ID 顺序；
重复 Apple 帧逐字节一致，NaN 样式更新被拒且旧帧不变。
