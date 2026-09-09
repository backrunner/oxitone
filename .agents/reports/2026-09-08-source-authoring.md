# 首批 source/DAW 实施验证

范围：生成式 Pattern source DAG、局部 edit 与初始 AST 表达式 writer。接口与剩余边界见
[15-source-authoring.md](../docs/15-source-authoring.md)。这不是完整 DAW/ABI 2 的验收报告。

## 已验证

- `pnpm lint`、`pnpm typecheck`、`cargo fmt --all --check`、`cargo test --workspace` 通过。
- core 全量 160 项、CLI 全量 16 项、protocol 全量 38 项通过。新增 15 项 source 语义测试
  与 6 项 writer 测试；协议错误码清单同步更新。
- 关键性质：删除不重排节奏/力度、倒位前音级与声部分离、截顶同音消歧、共享 DAG 保留、
  repeat/concat 的单轮修改、source window 裁剪、来源继承、seed/JSON 往返、无显式 ID、
  预算/循环/错误选择拒绝、expect 原值核对、degree/voice 别名覆盖顺序、反复 set 归约。
- writer 跨文件测试保存普通 TS 后删 bundle 再重建，输出/来源与直接 Apply(edit) 一致，
  原共享引用和定义文件字节不变；旧锚点拒绝、注释/CRLF 保留、无操作文本不变。
- `examples/offline/src/source-edit.ts` 经 CLI 实际打包并由 Rust 离线导出：原始 32 音、
  变体 31 音；WAV 8 秒、peak −15.635 dBFS，MIDI 675 bytes/2 tracks。产物仅在
  `target/examples/source-edit.*`，不提交构建输出。整个验证不打开系统音频输出。

## 聚焦 benchmark

命令：`node packages/core/bench/source-authoring.mjs`，先构建 core。
Apple M4、darwin 27.0.0、Node v26.5.0，3 次预热、20 次测量。以下为本次最终执行结果，
不是跨硬件保证；20 次样本的 p99 实际接近最大值。

| 输出数 | 操作 | p50 ms | p95 ms | p99 ms |
| --- | --- | --- | --- | --- |
| 1,000 | 单音 edit | 0.426 | 0.596 | 0.943 |
| 1,000 | 128 音 edit | 1.042 | 1.226 | 1.232 |
| 1,000 | JSON source 重建 | 1.538 | 1.761 | 1.766 |
| 100,000 | 单音 edit | 37.313 | 39.635 | 50.993 |
| 100,000 | 128 音 edit | 38.735 | 53.237 | 58.224 |
| 100,000 | JSON source 重建 | 74.807 | 91.521 | 96.344 |

选择器按所需坐标一次索引，集合编辑不再对每个选择器扫描整个音符数组。
所有测量均为 Node authoring，未包含 AST 求值、Rust prepare 或 GUI 绘制。设备、
sample rate、block size、CPU utilization、callback p95/p99、xruns 均未测，记 null。
脚本另输出包含 GC 的 heap net delta，不把它当作峰值内存。

## 未完成的发布门槛

来源插桩和符号/实例索引、完整 Document Service/Undo/save journal、Arrangement/automation
range、Rust origin/random/typed targets、插件 ABI 2、GPUI 编辑和插件管理器仍待实施。
当前 writer 要求文档所有者提供正确的 accepted expression/source 对；测试中的文件保存
不能替代生产事务/崩溃恢复。旧 Project.save 仍保存 JSON snapshot。
