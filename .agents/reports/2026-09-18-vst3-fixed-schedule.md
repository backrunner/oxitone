# VST3 固定延迟 native 调度

在常驻 helper / 有界端口之上增加 `stream::schedule::ScheduledPort`。本次实现仍属于 SDK
native 层，没有把 VST3 processor 接入 RenderGraph、开放 DAW Assign 或加载厂商 editor。

## 行为与边界

- TS 先声明 `Vst3StreamSchedule` 并生成 schema：scheduleVersion 1、显式 epoch 和 2…16
  个 latencyBlocks；Rust 独立校验版本、范围、全新端口和容量。helper stream protocol 不变。
- 控制线程预分配所有输入、输出和暂存。1…blockSize 的短段拼成插件整块，重定位参数和
  MIDI 偏移；每个插件块最多 256 事件，累积超限终止，不能丢事件后继续旧状态。
- 结果只在确定的帧区间输出，启动静音长度为 latencyBlocks × blockSize。报告总延迟加上
  插件固有 latency；模拟 7 帧插件延迟证明没有漏算或额外重复插入 delay。
- 缺失到期响应时锁存 DeadlineMissed，静音整个当前调用和以后调用，原子终止会话。
  迟到响应、已缓存后续块以及失败前已拷贝的半段均不能泄漏到输出。无等待、补播或 dry bypass。
- epoch/frame 不连续及显式 invalidate 都终止旧调度器。新会话重新准备，旧声音状态、输入
  半块、事件和输出槽不复用；实时侧不销毁对象，由控制线程 close/reap 并回收全部存储。
  现有 loader 没有绝对 seek 接口，所以不声称无缝 seek/loop 或工程 transport 已映射。

实现按职责分为 `stream_schedule.rs`（生命周期和公开入口）、`stream_schedule_process.rs`
（拼块和输出调度）、`schedule_wire.rs`（控制契约）以及对应测试文件，每份均小于 250 行。

## 验证

- `pnpm format:check`、`pnpm lint`、`pnpm typecheck`、`cargo fmt --all --check` 通过。
- `pnpm test`：443 项通过；`cargo test --workspace`：557 项通过，3 项 benchmark 忽略。
  工作区仍提示既有 napi `cache_sample` dead-code 和依赖的 future-incompatibility 警告。
- `cargo test -p oxitone-vst3-host --features host,stream`：27 项通过。
- `cargo check -p oxitone-vst3-host --no-default-features --features stream` 通过，客户端不链接 loader。
- 单元验证不规则分段、2/4/16 块调度延迟与模拟固有延迟的逐样本 PCM、事件偏移/顺序、
  过期 epoch、帧跳跃/溢出、非法缓冲/事件/样本、迟到及故障后静音。20,000 次短段处理及
  失效路径均为 0 次 heap 分配/释放；迟到、事件超预算和非法输入路径也独立计数。
- 进程测试新增挂起 helper 的调度 deadline 静音、首故障保留和控制 close/reap，早于其
  10 秒 IO watchdog；沿用现有启动/崩溃/滴流/非法响应等回归。
- release VestiGain smoke `--schedule` 通过：两次独立进程、281,600 帧 stereo 信号逐样本
  对齐、短段和参数事件、256 帧固定延迟与新 epoch 启动静音。无音频设备或扬声器输出。

## 聚焦 benchmark

结果：[2026-09-18-vst3-fixed-schedule.json](../../benchmarks/results/2026-09-18-vst3-fixed-schedule.json)。
Apple M4 / Darwin 27 / Rust 1.98.1 / LLVM 22.1.8，release，48 kHz / 128 帧 / stereo。
VestiGain bypass=1，queueDepth=4，latencyBlocks=2；每周期拆成 17+111 帧。
预热 100 周期后计时 1000 周期的 2000 次 process 调用，第二个进程复核新 epoch。

| 测量                    | p95         | p99         | 最大值      |
| ----------------------- | ----------- | ----------- | ----------- |
| 单段 ScheduledPort 调用 | 0.000584 ms | 0.000833 ms | 0.008458 ms |
| 实际 caller 周期间隔    | 3.337958 ms | 3.339875 ms | 3.343708 ms |

本次 deadlineMisses=0。caller 使用普通线程 sleep，实际周期中位数 3.335916 ms，比目标
128/48000 秒（2.666667 ms）更慢；这提供方法成本和 PCM 对齐证据，不能证明真实 callback
deadline、全工程负载或设备无 xrun。device、callback p95/p99、CPU 占用和 xruns 均记 null。
原始耗时、插件 hash、环境、失败字段和启动时间保留在报告内。

## 后续

需要实现 Engine 的实例绑定、全图 PDC、绝对 transport/tempo 映射、无缝 seek/loop 与
控制线程回收协议，再开放 DAW 实时插槽。此前源码 Save p95 回归未在本次重新评估；
本次不修改源码保存链路，也不把调度 benchmark 当成其性能修复。
