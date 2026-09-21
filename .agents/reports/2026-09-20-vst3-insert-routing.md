# VST3 Mixer 效果多路输入与辅助输出

完成此前未支持的 Mixer 效果多路输入选择和多输出工程路由。Engine snapshot 升为 1.6，
C ABI 1、VST3 stream 8 和配置格式保持不变。完整 VST3 接入仍有本文末尾的剩余项。

## 行为与实现

- `bus.routeInsert(instance, { inputs, outputs })` 按稳定实例 ID 连接物理辅助 bus 1…15。
  bus 0 保留串行链；多个输入可取不同 Mixer 的 post-fader 信号；多个输出可分别送往
  Mixer 或 Master。取消路由会停用辅助物理 bus，不静默丢弃默认启用的额外音频。
- 输入在 Mixer 入口 PDC 后补偿目标 insert 前的串行延迟；辅助输出补偿后续 inserts
  的延迟后再执行下游 PDC。修复 Channel 直接进入已有上游延迟的 Mixer 时漏补偿的问题，
  并为广播侧链补齐 insert 前的延迟。零延迟入口保留直接求和路径。
- 辅助输出绕过后续 inserts，共用所属 insert 的 mix/bypass 和 bus 的 fader/balance/
  mute/solo，不受 owner 的 masterSendRatio 控制。直接送往 Master 的辅助输出进入
  owner stem；其他输出进入下游 bus stem。Mixer stem tap 在 Master 处理之前，已修正
  导出文档中将它直接等同于最终 Master 文件的表述。
- 输入、输出、send 和侧链共同参与 DAG。拒绝循环、悬空引用、非本 owner 实例、非法
  物理索引和配置恢复后不存在的 bus。Master 只接收。失败保留已接受的播放图。
- 重排保留路由，删除实例清理路由；配置复制不复制连接。TS 快照/恢复、Rust wire、
  schemas 和 Preview 的双向连接投影已同步。旧 minor 携带新字段时拒绝，恢复数据不改写。
- 路由准备、激活与缓冲分配在控制侧完成；辅助处理仍限制在隔离执行图。设备 callback
  只消费已完成 PCM。新增模块分别负责 insert 构建、串行处理、路由准备与交付，未扩大
  现有大模块；本轮新增源码均低于 300 行。

## 验证

- `cargo test --workspace`：641 passed、3 ignored、90 suites；忽略的是已有 Preview
  release microbenchmarks。追加的 Mixer 专项最终为 5/5，包括逐帧 PDC、广播侧链、
  mix/bypass/fader/mute、stem、reset、非法图和处理期间 allocation/free 均为零。
- `pnpm test`：121 suites、492 tests 全部通过。首次 CLI watch 超时来自旧 Preview
  binary 不支持 Engine 1.6；重建后完整重跑通过。Rust 首轮的辅助输出测试 fixture
  共用了乐器/效果的激活计数；分开记录后完整重跑通过，没有修改 PCM 预期来放宽测试。
- format、lint、typecheck、Rust fmt、diff whitespace 检查通过；native addon、TS workspace、
  Wasm 与 debug Preview 已构建。
- `smoke-vst3-buses.mjs` 实际运行 1600 个 native bus 包，并覆盖旧乐器输出/侧链回归。
  新三输入三输出效果完成 8 项 PCM 比较，每项 48000 个交错样本，最大误差约 2.05e-8；
  覆盖独立输入、mono/stereo 转换、未连接输入停用、辅助 wet/bypass、fader/mute、求和、
  保存重开逐样本一致和 Master 前的 stem tap。
- 同一真实 fixture 验证配置源码事务、Undo/Redo、Save/reopen 保留实例 ID 与所有路由。
  模拟 sink 换图/seek/stop 处理 400 blocks，xruns/deadlineMisses/nanBlocks/queueDrops/
  plugin faults 为零；循环路由更新拒绝后，合法更新仍成功。
- 实际 Reaktor 6 FX 6.5.0 的 8 输入/8 输出全部路由，工程注册及离线 WAV 成功，图延迟
  264 frames。没有加载 ensemble，输出静音；不构成商业音色、资源恢复或听音验收。

## 性能与证据边界

Apple M4、Darwin 27、Rust 1.98.1，48 kHz、128 frames、stereo。Criterion 使用
20 samples、1 s warmup、2 s measurement。首轮暴露零延迟缓冲工作，已保留原始记录；
优化后在本轮 Rust/TS 测试结束后复测。2/8/16 bus 平均约 5.70/10.37/16.72 µs，
8 bus 各 1/4 inserts 平均约 44.62/148.36 µs。相对首轮明显下降；旧 Criterion 基线
源码来源未确认，不能据此清除之前的发布性能门禁，也没有关闭用户的其他进程。

真实 VST3 模拟运行的后台 block 直方图 p95/p99 桶均为 0.262144 ms；这是后台图成本，
不是硬件 callback 的 p95/p99。本轮没有打开系统音频输出，没有重新做 GUI 截图/人工交互。

- [真实总线、SDK、源码事务和模拟运行](../../benchmarks/results/2026-09-20-vst3-insert-routing.json)
- [Reaktor FX 注册与八路路由](../../benchmarks/results/2026-09-20-vst3-reaktor-fx-routing.json)
- [首轮性能原始记录](../../benchmarks/results/2026-09-20-vst3-insert-routing-before-fast-path.json)
- [优化后性能原始记录](../../benchmarks/results/2026-09-20-vst3-insert-routing-benchmark.json)

## 剩余范围

MIDI 输出路由、surround、上游 Channel 乐器/效果的外部音频输入不在当前 stereo Mixer
接口中。商业 ensemble/外部资源矩阵、长时压力、Intel 实机、厂商窗口人工交互和签名/
公证/远端发布验收仍待完成。本轮未提交或发布。
