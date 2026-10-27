# Rime 原生输入闭环实验

只运行已固定的上游 rime_api_console，不是 Jasper Rust 输入法适配或 UI 编辑框。夹具为本项目自有 Apache-2.0 YAML 与两个常见短语的最小测试表，不使用上游完整拼音词库、用户词库或下载的语言模型。

```sh
# 构建 VM 内，依次执行；本地 target 安装不接管系统输入法：
python3 scripts/ime-deps.py --component rime --jobs 2
python3 scripts/rime-smoke.py
```

实验在 evidence/rime-smoke/session 独立目录中部署唯一方案，向上游控制台输入 nihao、空格、shijie、空格，要求真实输出“commit: 你好”和“commit: 世界”。首次输入还展示候选“你好”。真实原生引擎负责匹配、候选和提交，脚本只驱动并检查输出；没有用预设 stdout 或 Rust 假引擎代替。

存在旧 session 时拒绝覆盖，重跑前先保存旧证据并移出该目录。原生字节/索引、候选、撤销/取消、线程、资源释放和最终 UI 集成仍需各自验证。测试不代表现代完整中文词库体验，也不代表 Keyman 的多语言框架已经接通；框架/自有 Rust/形式化证明完成状态保持 false。

上游默认源包包含不同来源的字典/数据，本轮不将其作为已完成授权核验的正式语料。Keyman、Rime 及其库、插件、键盘方案和词库继续逐项登记许可，排除已撤销的 Fcitx 5 / fcitx5-rime 依赖。
