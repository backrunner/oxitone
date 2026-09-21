# VST3 SDK 实例交接与回收

后续调度/IPC 修复与 stream protocol 2 迁移见
[迟到修复报告](2026-09-19-vst3-deadline-fix.md)；下文保留本阶段原始检查和失败记录。

增加 `stream::managed::Controller` / `AudioSlot`。控制侧准备独立 helper 与预分配的
ScheduledPort；Rust 音频侧在 block 边界换入，旧实例经有界队列交回控制侧关闭/回收。
本次没有接入 RenderGraph、DAW 实时插槽或厂商 editor。

## 关键行为

- TS `Vst3StreamManager` 与 Rust `ManagerOptions` 独立校验版本、固定采样率/blockSize 和
  2…16 的 capacity，schema 由协议包生成。stream/schedule 协议版本保持不变。
- capacity 包含准备完成待换入、正在使用和等待回收的全部实例；满容量在 spawn 前拒绝，
  不覆盖队列。初始化失败保留当前实例，成功发布的 epoch 严格递增。
- 总延迟和插件 Ready 在激活前可由控制侧查询，给后续全图 PDC 编译提供输入。本层仅报告
  延迟，不代替图补偿，也不允许在 manager 内更换采样率/blockSize。
- `require_epoch` 设置单调最低版本：失效旧 active、丢弃旧 pending，并拒绝初始化期间已
  过期的 prepare。后两者在控制线程回收，不能因旧加载晚完成而回退时间轴。
- 激活时已观察到的候选故障不替换旧 active；激活后崩溃仍由调度故障静音处理。
  新 epoch 从 frame 0 起步，不延续旧 PCM、半块输入、事件或 processor 实例。
- 音频侧只移动对象和原子标志。Controller 独占进程句柄，reclaim 关闭/reap 后再释放存储。
  shutdown 不依赖音频队列空位，能中断挂起 IO；active buffers 保留到 AudioSlot 返回控制侧。

依赖检查确认 `vst3-host 0.9.0` 没有绝对位置 setter，而且 set_playing(false) 仍推进内部位置。
因此本次没有把该开关错误包装成 DAW pause/seek；绝对 transport、无缝 loop 与图集成仍需后续实现。

## 验证

- `pnpm format:check`、`pnpm lint`、`pnpm typecheck`、`cargo fmt --all --check` 通过。
- `cargo test --workspace`：558 项通过、3 项基准忽略；保留既有 napi `cache_sample` dead-code
  与依赖 future-incompatibility 警告，没有新增警告。
- `cargo check -p oxitone-vst3-host --no-default-features --features stream` 通过，客户端无需 loader。
- VST3 host,stream feature：39 项通过，包括新增生命周期和初始化竞态测试。
- TypeScript protocol / vst3：55 项通过；schema、类型导出和现有 SDK helper 回归通过。
- 零分配/释放计数覆盖 16 个 FIFO 候选换入、过期队列清理、候选故障、未准备/关闭和旧 PCM
  隔离；另检查控制侧 reclaim 确实释放存储，避免以泄漏误充实时安全。
- 真实子进程测试覆盖容量满的提前拒绝、准备失败/epoch 重用/格式变化、排队后崩溃、挂起 IO
  时并发关闭、控制端先于 AudioSlot 清理，以及初始化期间 epoch 变化或 AudioSlot 销毁。
  挂起实例使用 10 秒 IO timeout，shutdown 必须在 1 秒内中断并回收。
- 初始化竞态 fixture 使用文件 gate 确定时序。排队后崩溃测试首次并行运行暴露了原 fixture
  退出时机不确定：有时退出早于握手读取完成，正确返回 PluginHostCrashed。改为 prepare
  成功后显式 SIGKILL 该测试 helper，准确覆盖候选已排队的故障阶段；未放宽产品错误检查。

所有测试不打开系统输出设备。手写实现/测试按生命周期、实时交接、协议和进程竞态拆分，
每个文件均不超过 250 行；真实插件 benchmark 独立于自动测试。

## 真实插件基准与限制

Apple M4 / Darwin 27 / Rust 1.98.1，release，48 kHz / 128 帧 / stereo，VestiGain bypass=1。
manager capacity=2，每周期拆成 17+111 帧。控制准备在新 epoch 的第一次音频调用前完成，
方法计时不含控制 prepare/reclaim；这些另行记录。没有打开音频设备，CPU 占用、设备 callback
p95/p99 和 xruns 均未测，不能把普通线程唤醒节拍当作设备 callback。

首次配置 latencyBlocks=2、queueDepth=4（256 帧 / 5.33 ms）在 epoch 2、frame 3072 出现
DeadlineMissed，已校验 13,312 帧；故障调用输出静音，4 个已启动 helper 全部回收。这是
实际失败结果，保存在 [两块延迟报告](../../benchmarks/results/2026-09-19-vst3-managed-latency-2.json)。
当次 caller interval p99 为 12.729166 ms；两块延迟没有通过本机这次运行的时限要求。
这份失败报告的 epochs 字段统计已准备进程数，包含尚未激活的下一实例；后续 probe 已明确
区分 planned/prepared/activated/completedEpochs。不修改旧记录来伪装全部完成。

probe 允许显式配置 `OXITONE_VST3_MANAGED_LATENCY_BLOCKS=2…16`，并记录实际 queueDepth
与延迟；不在故障后自动扩大缓冲或继续重放旧输出。
