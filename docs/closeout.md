# 第 6 项收尾与交付审计

本文件定义第 6 项“全量收尾与交付”的可重复执行入口。它不提升第 1–5 项状态，也不把测试、候选切片或工具审计写成清单完成。

## 事实分层

- 静态总审计：检查第 1–5 项的源码入口、现有证据、证据记录的源码哈希、前序分支集成状态、工具版本/配置和 Claude 复杂证明遗留清单。
- 基础回归：在项目构建 VM 内检查依赖归档、格式、默认/原生 CPU/原生 GPU 三组全 targets 编译和默认测试；字体与 GPU 集成测试按显式环境开关追加。
- 最终交付门：只有第 1–5 项均有明确完成记录、前序分支已合入、证据绑定当前源码、工具配置一致、VM 回归通过且工作树干净时，`--require-ready` 才成功。

默认静态审计可以返回 0，即使清单仍未完成；这表示“审计已执行”，不表示“交付已通过”。只有 `--require-ready` 的退出码 0 可作为最终交付门通过条件。

## 执行

在当前分支先执行静态审计：

```sh
python3 scripts/audit-closeout.py --print-summary
```

第 2–5 项合并后，重新生成源码快照与证据，再执行 VM 基础回归：

```sh
JASPER_UI_VM_RUN=/path/to/vm-entry.sh \
  python3 scripts/audit-closeout.py --run-baseline
```

最终候选提交必须重新运行并启用交付门：

```sh
JASPER_UI_VM_RUN=/path/to/vm-entry.sh \
  python3 scripts/audit-closeout.py --run-baseline --require-ready
```

基础回归脚本不会在桌面宿主直接启动 Cargo。它通过 `scripts/verify-in-vm.sh` 进入构建 VM，默认仍使用 3 GiB 内存、零 swap、Cargo jobs=1 和 180 秒单项工作上限。可选变量：

- `JASPER_UI_TEST_FONT`：设置后追加原生 CPU 库测试。
- `CLOSEOUT_RUN_NATIVE_INTEGRATION=1`：追加原生 CPU 关键集成测试，必须同时设置字体。
- `CLOSEOUT_RUN_GPU=1`：追加软件 Vulkan GPU 集成测试；不冒充硬件 GPU 或真实 Surface 生命周期验收。

## 报告

默认输出：

- `evidence/final-closeout-audit/report.json`
- `evidence/final-closeout-audit/summary.md`
- 使用 `--run-baseline` 时还会有 `evidence/final-closeout-audit/baseline.log`

审计会明确保留以下失败/待办形态：

- `not_complete`：现有最终审计或条目证据记录为未完成。
- `stale`：证据记录的源码 SHA-256 与当前文件不一致。
- `missing`：证据引用缺失，不能按“历史存在”推理通过。
- `pending`：前序分支尚未合入当前 HEAD。
- `not_run` / `unavailable`：基础回归未执行或 VM 入口不可用，不能计为通过。

复杂证明不在此流程中现场攻克。按 `docs/claude-closeout-proof-backlog.md` 登记范围、现存证据、禁止的降级方式和后续复验入口，避免用超时、缩域、替代容器或有限样例填坑。
