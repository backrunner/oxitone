# VST3 完整接入审计与推进记录

目标是 VST3 音源/效果器能在真实工程中分配、播放、自动化、导出、保存恢复，并具备
正确的定位/循环、PDC、插件窗口和可安装的宿主分发。当前尚未完成这个目标。
本记录以当前源码为准，保留此前未提交实现和失败基准，不把 SDK probe 当作 Engine 验收。

## 已有能力

- 可选 `oxitone/vst3` / `@oxitone/vst3`，显式 bundle/class/hash/签名校验，独立 helper
  检查与离线 WAV 渲染；不在 JavaScript 中处理 PCM。
- 不透明 component/controller state、独立预设文件、DAW 离线工作台，以及把渲染结果
  经 Document Service 事务导入源码工程；支持既有 Undo/Redo/Save/恢复。
- 常驻隔离 helper、有界原生 PCM/event 端口、固定帧延迟调度、迟到静音、实例 epoch
  门禁和控制线程回收。音频方法有零分配/释放测试。

## 本次完成

1. 修复短块推进错误。upstream 的 process 长度来自 channel Vec 长度，旧 helper 只改
   block_size，17 帧调用可能处理完整 128 帧；增益/bypass fixture 无法观察内部时钟偏差。
   现在离线和 stream 统一调整 channel 长度并保留预分配 capacity，不处理虚构样本。
2. 新增严格 `Vst3Transport` 契约，stream 协议升为 3。初始化可指定真实位置，原生
   submit_at / ScheduledPort.process_at / AudioSlot.process_at 可发送完整块的精确上下文。
   工程与连续帧独立，quarter-note/bar position 不从“当前 BPM × 总秒数”反推；支持
   tempo、拍号、playing 与 cycle flags。短段混入新的完整块上下文明确拒绝。
3. helper 每块写入真实 VST3 ProcessContext；省略下一上下文时按同一位置锚点和整数
   累计帧重新计算。暂停冻结工程位置，连续处理时钟仍前进。位置改变不冒充 DSP reset。
4. vendored upstream `vst3-host` 0.9.0 的 MIT 源码并加入最小 ProcessPosition setter，
   保留 LICENSE 与来源说明。原依赖没有绝对位置入口；未修改用户 Cargo cache。
5. 修复 macOS helper 退出时的错误读取：SO_RCVTIMEO 在 peer 已关闭时可能报 EINVAL，
   导致尚在 socket 中的具体错误丢失。只有 poll 确认 hangup 后继续读缓存，保留原 deadline。
6. 新增可从仓库构建的真实 VST3 conformance fixture。processor 把实际处理帧数和
   ProcessContext 编码为 PCM，验证 400 个交错短块、26,129 帧、初始位置、回跳、暂停
   续块、变速/拍号、cycle 清除；不是仅回显协议字段。

关键实现：

macOS arm64/x64 CI 已增加 host/stream feature 测试与真实 conformance fixture，并把
结果收入已有 CI artifact。使用锁定依赖，不要求安装商业插件；远端 CI 尚未执行。

- [短块缓冲长度](../../crates/vst3-host/src/process_buffers.rs)
- [原生 transport 数据](../../crates/vst3-host/src/transport_wire.rs)
- [transport 帧编码](../../crates/vst3-host/src/transport_codec.rs)
- [helper 执行](../../crates/vst3-host/src/stream_server.rs)
- [固定延迟调度](../../crates/vst3-host/src/stream_schedule.rs)
- [真实处理器验证脚本](../../scripts/smoke-vst3-transport.mjs)
- [扩展依赖来源](../../vendor/vst3-host/OXITONE.md)

## 尚未完成的开发工作与验收条件

| 部分                  | 当前缺口                                                                            | 完成条件                                                                                                                         |
| --------------------- | ----------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| 工程 authoring / 注册 | Project 的实时注册仍是 C ABI；VST3 只有会话工作台条目，Assign 被拒绝                | 可序列化 VST3 配置、独立 instrument/insert 身份、显式本机绑定与信任策略；SDK 和源码/DAW 同一事务模型                             |
| RenderGraph adapter   | `build_plugins.rs` 只创建内置/C 插件；尚不依赖 VST3 原生流                          | 接入真实音源、channel/bus/master insert，参数/MIDI 事件、bypass/mix 和明确故障诊断；不得在 process/reset 中执行 loader 或 socket |
| PDC / 干湿 / 路由     | 通用 PDC 已存在，但从未消费 VST3 的调度延迟+插件固有 latency                        | 多条并行/串联路径和 send、干湿/bypass 逐样本对齐；latency 改变须控制侧重建，不能在 callback 偷改                                 |
| 离线全工程导出        | renderVst3Wav 只处理一个插件；全图离线 tight loop 不适合异步到期检查                | 实际工程离线渲染能等待异步工作而不放宽 realtime process 不变量；导出、tail/stems、超时清理和实例状态独立验证                     |
| Transport / loop      | 新 SDK 能传精确位置，但 Engine 未发出这些上下文；graph reset 与异步实例重建尚未衔接 | play/stop/pause/seek、非块对齐 loop、tempo map、graph swap 与 epoch 生命周期协同；不会回放旧 PCM/MIDI 或永远静音                 |
| 状态 / 实时参数       | 只有 inactive inspection state 和离线参数覆盖；活跃实例尚无状态回传                 | 两个相同插件独立；编辑、快照、保存/重开和 Undo 不串实例；未知格式与缺失插件保留恢复数据                                          |
| 插件原生窗口          | 没有 IPlugView 窗口、run loop、resize/scale 或 GUI gesture 回传                     | helper 所属主线程拥有窗口；关闭/崩溃能回收；参数与状态经 Document Service 回写，不能由 GUI 直接改源码/Rust 图                    |
| 发现与能力            | 用户手工填精确 class ID；仅主 mono/stereo bus；sidechain、多输出、MIDI 输出未接入   | class 列表/缓存与隔离扫描；支持的布局实际参与路由，其余明确错误；兼容矩阵不可用 VestiGain 一项替代                               |
| npm / 发布            | 已安装 SDK 仍需显式提供本地 helper；没有随平台包交付                                | macOS arm64/x64 helper 打包、解析、manifest/hash、版本匹配、doctor、签名/公证与安装 smoke                                        |
| 集成验收              | 底层 smoke、GUI 离线工具不等于实时工程完成                                          | 使用模拟 sink 做 SDK→native→RenderGraph→DAW 全链路、多个实例、loop/恢复/异常和负载测试；设备/听音另记未测范围                    |

下一条关键路径是 RenderGraph 的异步插件节点生命周期：需要同时设计控制侧准备、
实时侧消费、离线侧等待和 transport reset，才能把现有 native stream 用进真实工程。
不应把 mutex/controller 路径直接包装进同步 PluginInstance::process，也不能仅去掉
DAW 的 Assign 拒绝来宣称接入完成。

## 验证记录

基准输出见 [transport conformance](../../benchmarks/results/2026-09-19-vst3-transport-context.json)。
所有本次验证均不打开系统音频输出。CPU/平台、IPC p95/p99 与原始诊断写入报告；
设备 callback p95/p99、CPU 占用和 xruns 未测，不能据此声称任意负载下实时合格。

- `pnpm format:check`、`pnpm lint`、`pnpm typecheck`、`cargo fmt --all --check` 通过。
- `cargo test --workspace`：559 通过，3 个基准忽略。
- `cargo test -p oxitone-vst3-host --features host,stream`：49 通过，包括实时端口/调度/
  managed process_at 零分配/释放、坏上下文、旧版本、短块容量和已关闭 socket 最后响应。
- `cargo check -p oxitone-vst3-host --no-default-features --features stream` 通过，客户端
  不依赖 VST3 loader；protocol / vst3 包测试通过，schema 已重新生成。
- Apple M4 / Darwin 27、release、48 kHz / 128 帧：transport conformance 往返
  p95/p99 为 0.143208 / 0.169125 ms，400 块、26,129 帧实际 processor 校验通过。
- [VestiGain stream 3](../../benchmarks/results/2026-09-19-vst3-stream-v3.json)：2,208 块、
  两次进程生命周期，往返 p95/p99 为 0.127208 / 0.179000 ms。
- [固定调度](../../benchmarks/results/2026-09-19-vst3-schedule-v3.json)：281,600 帧对齐，
  固定 256 帧延迟，deadlineMisses=0；process p95/p99 为 0.004416 / 0.021000 ms。
  普通调用线程 interval 最大 52.801667 ms，不能解释成真实设备 callback 已达标。
- [实例切换](../../benchmarks/results/2026-09-19-vst3-managed-v3.json)：32 epochs、
  163,840 帧、31 次替换回收、32 个 helper 全部退出；process p95/p99 为
  0.003000 / 0.009792 ms，无附加压力线程。本次未重复完整 GUI/负载/听音验收。
