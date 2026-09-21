# VST3 厂商手势日志与音频块定位

## 实现

stream control 1 新增 startEdits/readEdits/stopEdits/discardEdits。SDK 使用现有
Session.controlVst3Instance 与精确 graphGeneration/instanceId 操作播放实例的日志。
没有扩展 C ABI、引入设备 callback 工作或改变自动化回放优先级。新命令用显式 tag，
旧 helper 拒绝未知命令；现有控制应答不附加日志，也不改写历史持久化格式。

厂商 IComponentHandler 的 begin/value/end 按原顺序保留。日志在实际下一播放音频包
首帧绑定 audioSequence、完整 Transport 与 reset，保留 seek/loop 的非单调 Project
位置。VST3 回调没有 sample offset，这里是宿主处理块量化，不能声称鼠标采样级时间
或设备实际听到的时刻。零样本 flush 和暂停包都不定位事件。

每次 captureId 在 helper 生命周期内唯一；最多保留 4096 个未确认事件和 4096 个
待定位事件，每页最多 256 个。fromSequence 确认已收数据，重复游标可重试；续页游标
是 firstSequence + events.length，nextSequence 仅为累计事件数。过期游标、错误 ID
和旧图 generation 返回 PluginTaskConflict，不会因为游标错误终止音频。

stopEdits 后停止接收新手势，下一播放包定位已有事件并提供 endPosition；没有下一包
时维持 stopping，调用者可 discard。日志不能以虚构的终止位置变成成功。Unknown/
read-only/non-automatable 参数、非有限或越界值、待定位/未确认预算耗尽，以及 vendor
队列丢失都让整个 capture 失败；调用者必须丢弃此前积累的事件，不能接受部分结果。

本地 MIT loader 增加累积饱和 overflow counter，覆盖 gesture 与 DSP value 队列丢失。
不提供 counter 的 backend 明确拒绝开始采集。原 vendor 队列容量保持不变。TS/Rust
拒绝畸形游标、缺页、未来音频序号、非法阶段值、null 可选字段和不匹配的结束状态。

新增手写模块均小于 300 行。vendor 的既有长文件保留原布局与第三方格式例外，更新了
来源说明；没有清理、提交工作区中其他开发任务的修改。

## 验证

- Rust workspace：627 passed、3 ignored、88 suites；host/stream 专项：62 passed。
- vendor ComponentHandler 专项：7 passed，使用真实 COM 回调、无音频设备。首次独立
  运行因未缓存依赖/缺少 lockfile 未执行测试；解析独立测试依赖后通过，临时生成的
  vendor Cargo.lock 已移除，没有改动项目依赖锁来迁就测试。
- TS workspace：473 passed，含 protocol 56、native 28、VST3 SDK 16、core 189、CLI 146。
- workspace/native/Preview/Wasm 构建、schemas、format、lint、typecheck、cargo fmt 和
  git diff --check 通过。首轮格式检查指出新控制契约未格式化，修正后重跑通过。
- [真实手势 conformance](../../benchmarks/results/2026-09-20-vst3-gesture-journal.json)：
  11 项原生检查、10 项 SDK 检查；完整 brackets、暂停等待、seek/reset、停止边界、旧图
  拒绝、4097 值溢出、非法回调和 1024 个事件四页完整读取。SDK 使用实际 N-API、原生
  helper 和 simulated sink，48 kHz/128 frames；xruns/deadlineMisses/nanBlocks/
  queueDrops/pluginFaults 均为 0。原生包检查处理 640 帧，没有系统音频输出。
- 既有 [动态/Dense 配置与 DAW](../../benchmarks/results/2026-09-20-vst3-gesture-configuration.json)、
  [VestiGain 状态接受/保存](../../benchmarks/results/2026-09-20-vst3-gesture-live-daw.json)、
  [多总线与工程路由](../../benchmarks/results/2026-09-20-vst3-gesture-buses.json) 回归通过。
  多总线检查包含 1600 个真实 VST3 音频块。
- 新手势 smoke 已加入 macOS arm64/x64 CI 矩阵；本轮本机实际执行 arm64，未冒充 Intel 验收。

## 时间与性能边界

停止本轮其他 build/test 后执行原生 transport probe 与 focused insert_automation 基准。
[Transport 结果](../../benchmarks/results/2026-09-20-vst3-gesture-transport.json) 验证 400 包、
26,129 帧和 79 次 reset；完整往返 p95 0.15625 ms、p99 0.307958 ms，helper 处理最大
0.039 ms。此路径包含未启动日志时的队列收取，不等同于高频厂商 GUI 录制负载。
设备 callback p95/p99 未测，模拟 sink 的块耗时直方图也不用于替代设备测量。

[Insert 基准原始样本与 estimates](../../benchmarks/results/2026-09-20-vst3-gesture-benchmark.json)：
Apple M4、48 kHz/128 frames，bench 优化构建，1 秒预热、3 秒测量、30 个样本。
普通/typed insert render 均值为 175.256/173.863 µs，归一化批次 p95 为
183.779/179.719 µs、p99 为 198.283/189.706 µs。Criterion 相对缓存的历史基线未检出
显著 render 回退；compile 变化在噪声阈值内。此结果不关闭之前的 Preview telemetry
回退，也不证明高频 GUI 录制、商业插件或真实设备 callback 的性能。

## 后续

1. 把手势日志接入 SDK/DAW 的录制事务：touch/write 与现有 automation 回放的优先级、
   gesture 生命周期、seek/loop 分段、source.replaceRange 区间合并和 Undo/Redo/Save。
2. 运行中 I/O/latency/参数表变化的 state 交接、PDC 重编译与原子换图。
3. 效果器多输出、更多输入、MIDI 输出和 surround。
4. 实际厂商窗口点击、缩放、Apply/Cancel 和播放负载验收；当前测试回调由测试控制器
   主动触发，没有用鼠标操作厂商窗口，不声称 GUI 自动化录制已完成。
5. 商业资源/ensemble、长加载/soak、Intel 执行、发行签名/公证、受控性能基线与既有
   Preview telemetry 回退定位。历史比较的未决性能门禁继续保留。

本轮完成日志与定位基础，“完整 VST3 接入”仍在推进。
