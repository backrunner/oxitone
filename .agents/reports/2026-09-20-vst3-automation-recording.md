# VST3 Touch/Write 录制与 SDK Source 回放

## 实现

stream control 1 新增显式 startRecording(mode, parameterIds)，复用已有日志的捕获身份、
分页、停止边界及取消。纯观察 startEdits 保持原优先级。最多选择 32 个可写且可自动化
参数；helper 内按实际播放包执行 Touch/Write 覆盖，并产生带 frames 的 sample 事件。
sample 是宿主应用值的记录，不冒充厂商 begin/value/end 回调。

Write 从启动到停止边界优先；Touch 从 begin 持续到 end 所在块结束，下一块恢复已有
自动化。同块取最后修改值，未选参数及 MIDI 不变。保存被抑制的最新自动化值，使松手、
停止、失败或取消后，即使上游已去重而不重发参数，也能恢复正确值。暂停保留待处理
手势；seek/loop 记录实际 Project 坐标，连续音频序号不回退。Touch 非法 brackets、
事件丢失和预算耗尽使整个采集失败。

SDK Session.recordVst3Automation 自动收页，stop 返回不可变的 take，cancel 等待清理。
分页使用已返回的事件数量推进游标。异步请求期间取消或停止超时后，迟到的完成页不能
发布 take；清理只能访问原 generation/captureId。take 合并相邻同值区间，最多 32768
段；source(parameterId, base) 使用 step/replaceRange，后录入的重叠段优先，未触及
区间保留原生成器。该 API 只生成 Project-beat Source，不自动修改作者数据或源码。

修复 VST3 graph 事件转换中的独立问题：同段 A→B→A 必须保留返回 A 的事件，不能
只与前段缓存比较而丢掉它。超事件预算的失败不提前更新参数缓存。该路径在隔离后台
执行，设备 callback 未增加录制、JSON、锁、分配或厂商调用。

新 Recording Gain 测试类从实际 inputParameterChanges 按 sample offset 计算增益，
用 PCM 独立观察参数送达。测试回调由真实 VST3 控制器主动调用 IComponentHandler；
未用鼠标操作厂商窗口。

## 验证

- Rust workspace：629 passed、3 ignored、88 suites；host/stream 专项：63 passed。
- TypeScript workspace：479 passed，含 protocol 57、native 28、VST3 16、core 194、CLI 146。
- workspace/native/Preview/Wasm 构建、schemas、format、lint、typecheck、cargo fmt、
  git diff --check 通过。新增手写模块均小于 300 行。
- [真实录制 conformance](../../benchmarks/results/2026-09-20-vst3-recording.json)：
  原手势日志 11 项 native + 10 项 SDK 检查继续通过；新增 9 项 native 录制检查和
  10 项 SDK 录制检查。Native 逐样本验证 Touch 释放、Write、暂停、seek/reset、取消
  与非法手势恢复。SDK 自动收集后生成 Source，192000 个 PCM 样本与独立按整数帧
  枚举生成的阶梯曲线完全相同，Save/reopen 后 WAV 逐字节相同。
- 增益 oracle 起初未计入主限幅器，出现区间结束后的预期外差异；确认其 264 帧延迟与
  FIR 过渡后修正比较位置。过渡区由独立阶梯曲线的完整 WAV 对拍覆盖；区间外增益
  最大误差 1.1642e-8。没有改变 Source evaluator 或降低全段 WAV 对拍要求。
- SDK 使用 simulated sink，xruns/deadlineMisses/nanBlocks/queueDrops/pluginFaults
  均为 0。测试不打开系统输出设备，也不代表主观听音或设备 callback 验收。
- [动态/Dense 配置与 DAW](../../benchmarks/results/2026-09-20-vst3-recording-configuration.json)、
  [多总线与工程路由](../../benchmarks/results/2026-09-20-vst3-recording-buses.json)、
  [VestiGain 实例接受](../../benchmarks/results/2026-09-20-vst3-recording-live-daw.json) 回归通过。
  多总线验证 1600 个真实包；DAW 状态接受含精确实例、Undo/Redo/Save/reopen。
  这些是已有配置事务的回归，不是新录制 take 的 DAW 源码事务验收。
- macOS arm64/x64 CI 已有的 smoke-vst3-edits 步骤包含新录制 conformance。本机实际
  执行 arm64；Intel 执行与厂商窗口可视交互未在本轮完成。

## 性能

其他测试及构建结束后，Recording Gain 使用 100 包预热、1000 包计时、每 32 包确认
日志。单参数 Write 覆盖每包交替的入站自动化，140800 帧 PCM 与 1100 个 sample
事件均核对；submit→receive 往返 p95 0.132125 ms、p99 0.1465 ms、max 0.205209 ms。
收页不在计时区内，poll 使用 yield_now。没有测设备 callback、高参数密度、厂商 GUI
交互或商业插件资源负载，不能将此数字用于这些场景的性能保证。

[Focused insert 基准](../../benchmarks/results/2026-09-20-vst3-recording-benchmark.json) 使用
Apple M4、48 kHz/128 frames、1 秒预热、3 秒测量、30 个 Criterion 样本。普通/typed
render 均值为 172.888/172.730 µs，归一化批次 p95 为 182.721/178.783 µs、p99 为
186.589/183.841 µs。相对缓存的历史基线未检出显著 render 回退；这些是整批均摊的
内存渲染数据，不能冒充 callback 分位数，也不关闭已有 Preview telemetry 性能门禁。

## 后续工作

1. DAW Record/Stop/Cancel 入口、选择录制参数，复用当前 Preview 实例；把 take 接入
   完整源码事务，处理 lane/clip 的时间域与共享来源，覆盖 Undo/Redo/Save。
2. 运行中 I/O、latency 和参数表变化的 state 交接、PDC 重编译与原子换图。
3. 效果器多输出、更多输入选择、MIDI 输出与 surround。
4. 商业资源/ensemble、长加载与 soak、真实原生窗口操作、Intel 执行及发行签名/公证。
5. 受控性能基线与此前 Preview telemetry 回退仍未关闭，不能用本轮较窄的检查代替。

当前完成 Touch/Write helper 和 SDK take/Source；完整 VST3 接入目标继续保持进行中。
