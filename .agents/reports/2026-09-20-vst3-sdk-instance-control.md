# VST3 工程实例与异步 SDK 控制

## 本轮实现

Session.vst3Instances 返回已接受图的版本与稳定实例目录；controlVst3Instance 通过异步
N-API task 控制同一个播放 helper。支持打开/关闭厂商窗口、参数修改、轮询与 state 捕获。
目录覆盖乐器、Channel insert、Mixer/Master insert；相同插件的实例独立，重排不改变身份。

每次编译生成独立的 u64 graphGeneration；prepared 图不能控制，换图边界退役旧目录，
销毁图不因外部句柄存活而保留可编辑状态。命令执行前后及 Promise 回到 JS 时分别复核
当前 generation，拒绝过期请求和迟到结果。编译失败保留旧目录；设备 callback 仅更新
原子状态，不执行 JSON、字符串查找、锁、厂商 UI 或 N-API。

VST3 adapter 已负责初始化参数，渲染图不在首块重复发送初始值。helper 在 Ready 前
零样本排空预设队列；实时修改也先排空已有事件，避免 4096 参数配置吞掉新修改。
保留 ABI 1 默认首块行为和后续时间线自动化。生成绑定与三个 instance control schemas
均由构建脚本生成；browser 返回 PluginCapabilityUnsupported。

实时修改不改 authoring snapshot，离线导出仍使用最后接受的工程状态。将 capture 结果
显式写入 owning configuration 后重新编译，才能让保存/离线 WAV 使用该状态。

## 验证

- Rust workspace：623 passed，3 ignored，88 suites；原生 host/stream：58 passed。
- TypeScript workspace：468 passed；native 28、protocol 54。
- workspace build、Wasm build、生成 schemas、lint、format 和 typecheck 已执行；异步返回
  类型生成最初为 unknown，现通过 N-API ts_return_type 声明为 Promise<string>，重建后
  typecheck 通过，未手改生成文件。
- Rust mock 图覆盖所有实例位置、初始化参数优先级、无身份旧快照、销毁期间迟到回复；
  N-API 测试覆盖版本/身份/超时的独立验证、失败编译保留目标、播放/暂停期间换图与浏览器拒绝。
- 真实 VestiGain：三个实例独立、播放前修改保持、连续循环期间控制、只读源码快照、
  捕获状态显式接受后的逐样本 WAV、重排/删除/失败编译及旧 Promise 拒绝全部通过。
  [原始报告](../../benchmarks/results/2026-09-20-vst3-sdk-instance.json)。
- Apple M4 / Darwin 27 / 48 kHz / 128 frames，3 个 insert、16 块 render-ahead、模拟 sink。
  216 个处理块，xruns/deadlineMisses/nanBlocks/queueDrops 与插件 faults 均为 0。
  3 次预热、30 次 SDK setParameter + capture：p95 0.447 ms，p99 0.466 ms；同段
  background block 直方图 p95/p99 上界 1.049 ms。这不是设备 callback 或 GUI 交互指标。
- 原生配置 fixture 19 项通过，包含完整 4096 参数预设后首次实时修改的 DSP state 保存/
  恢复，以及 SDK 通过 N-API 控制两个真实 Dense 实例、隔离参数与拒绝旧 Promise。
  此路径已接入两个 macOS CI 架构的现有 configuration 检查，本机只执行 arm64；零音频帧、
  无设备。[原始报告](../../benchmarks/results/2026-09-20-vst3-instance-configuration.json)。
- 增加启动零样本 flush 后，真实 transport fixture 的 400 块/26129 帧仍逐样本通过，
  验证控制刷新不推进音乐时钟。[报告](../../benchmarks/results/2026-09-20-vst3-instance-transport.json)。
- insert_automation 在停止本轮 build/test 后单独运行：1 秒预热、3 秒测量、30 样本。
  mean compile 42.82 µs、render 173.32 µs；typed compile 43.67 µs、render 173.58 µs。
  本次未重现此前约 319/274 µs 的渲染回归，但没有可重建的相同源码旧基线，不能把不同
  环境下的差值全部归因于本轮代码。保留历史失败数据与本轮原始 estimates/sample、编译器
  信息和二进制 hash。[报告](../../benchmarks/results/2026-09-20-vst3-sdk-instance-benchmark.json)。

## 仍需完成

本轮实现 SDK 与原生图控制，尚未将 DAW 的独立配置编辑器迁移到 Preview 实际播放实例。
后续需连接 Node Document Service 的播放实例命令与捕获接受事务，再实现厂商 GUI
begin/perform/end 手势、时间坐标和自动化录制。控制进程内 UI/DSP 仍串行，厂商慢调用
可能导致音频超时；需持续播放的 GUI 负载验证。

动态 I/O/latency/参数表变化仍显式使旧流失效，尚无状态交接、PDC 重编译和发布事务。
效果器多输出/更多输入与 MIDI 输出路由、完整厂商兼容矩阵、资源/ensemble、真实 GUI
Apply/resize、Intel 机器执行、签名/公证和长负载验收仍未完成。原性能基线回归问题
也不能由本轮控制通道的低延迟结果代替。因此不将“完整 VST3 接入”标为完成。
