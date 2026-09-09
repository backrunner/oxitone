# 发布前源码编辑清理

按用户确认，未发布接口不维护 deprecated/shim 分支；迁移调用、测试和文档后删除被取代的
实现。该原则已写入 AGENTS.md 与 oxitone-guard，后续修改继续适用。

## 删除与统一

- 删除独立 PatternDocument、document-types、project-pattern-document、project-evaluator、
  evaluation-boundary 和 evaluation-worker 六个模块及旧公共 exports。代码/DAW 只保留
  ProjectDocument 一份草稿、Undo/Redo、求值和 Save 状态。
- 删除旧 source-session 基准，完整工程基准统一为 source-daw。共享子进程与 watcher 改为
  runEvaluationProcess、watchProjectDocument；worker 类型明确限于 project/plugin。
- journal 只接受 version 2，删除 version 1 转换分支。旧日志明确拒绝，保留源码及恢复镜像；
  拒绝旧格式不是自动丢弃未完成事务。
- request history 删除任意旧式 ID 的 retired 集合及 4096 上限兼容路径，只接受有界客户端
  的单调 stream/sequence。测试和 GPUI 请求 fixture 已同步；重复提交幂等和过期重放保护保留。
- DocumentView.projectRoot 改为必填；旧服务缺字段事件在 TS 协议入口拒绝，不保留猜测根目录
  或按旧事件降级创建的逻辑。
- Rust 删除 ParameterSmoothing.onePole 解码别名，统一 one-pole 公开拼写；descriptor fixture
  同步迁移，增加旧拼写拒绝断言。
- CLI build 在 tsc 前清理 dist，确保已删除的模块不会残留到 npm 包；生成 schema 已重建。

## 回归迁移

原单 Pattern 测试不再维护一份自制 AST evaluator/文档行为。唯一有效覆盖迁至真实工程：
project-session 测试 async factory 的 this、模块 URL、调用次数、重复边界不可编辑、失败
subscriber 隔离、迟到源码/DAW 求值、close、超时取消，以及 atomic save/dirty conflict/watch。
project-localization 增加 type-only import/共享 export 保留与作者移除原依赖后的真实重开；
已有 project-document/source-creation/source-review 继续验证 history、invalid draft、Save
与多文件操作。源码 writer 的纯测试保留，它是现用组件而非旧文档副本。

CLI 完整回归为 85 tests/27 files，通过。测试数量下降来自删除旧实现重复测试，不作为
测试覆盖提升的计量。全仓 lint/typecheck、protocol 39 tests 和真实 VS Code Extension Host
通过，包括未保存模块、DAW 文本回写、共享历史、Save/autoSave、重启冲突和生产 CLI 启动。
系统盘第一次使 Vitest 启动报 ENOSPC；更改本轮 TMPDIR 到数据盘后完成测试，未删除其他项目文件。
迁移中修复两处 fixture 的旧 requestId 比较，以及重复边界不可编辑的旧测试假设。

检查日志位于 target/prerelease-*。测试音频为 offline/simulated，没有打开系统输出设备。

最终 Rust workspace 516 passed、0 failed、2 ignored（既有编辑/布局基准）；cargo fmt 与
git diff --check 通过。CLI dist 已核对不存在六个旧模块的 JS/declaration 残留。
开发 VSIX 更新为 target/oxitone-vscode.vsix，6 entries，归档 bundle 与已测构建字节一致；
SHA-256 为 890bae586d0b82396b9beeabc50209d6a97aa9fa2c39b8c76b247d25c1a98318。未发布。

统一工程基准 target/prerelease-bench.json：Apple M4/Node 26.5.0，100 notes、2 placements、
20 plugins，48 kHz/128 frames，1 warmup + 10 iterations。p95/p99：音符事务 185.81 ms，
配置 179.72 ms，rack review 202.74 ms，Save 49.74 ms，效果器重排 447.65 ms，重开 282.95 ms。
本轮重排超出 300 ms 小工程目标，保留为未通过性能门禁；没有为取得更快结果重复采样。
无 callback 改动，device/callback/xrun 未测，不能将该控制侧基准作为实时设备指标。

该重排性能缺口已在后续优化中改善：source-daw p95 157.43 ms，40 次专测 p95 258.92 ms，
均通过小工程 300 ms 目标；更长尾延迟与大工程仍需跟踪。见
[效果器性能报告](2026-09-09-effect-performance.md)，本节原始基线不覆盖删除。

## 范围

这是已被取代实现的实际删除，不代表完整 DAW 目标已交付。Engine 1.2、Preview 1.0 和
插件 ABI 1 仍是现用运行路径；其整体重设计必须把 SDK、Rust、header、插件和 fixtures
同步迁移，不能删除现用能力后仅改版本号。ABI 2、概率/时间域、资产编辑与包迁移回滚
仍按完整交付矩阵继续实现，迁移完成后也不要求保留当前版本的兼容分支。
