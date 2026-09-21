# Oxitone 性能与 benchmark 规范

## 性能预算

基线为 macOS Apple Silicon、48 kHz、128 frames、stereo、release build：

| 场景                              | callback p99 |      CPU 总占用 | xruns |
| --------------------------------- | -----------: | --------------: | ----: |
| 空图 + Master                     |     < 0.5 ms |            < 1% |     0 |
| 32 voice synth + 4 inserts        |     < 1.5 ms |           < 10% |     0 |
| 64 voice + 8 mixer buses + reverb |     < 2.0 ms |           < 20% |     0 |
| 10 min offline render             |          n/a | >= 20x realtime |   n/a |

预算不是跨硬件的绝对承诺；每次 benchmark 必须记录 CPU 型号、OS、Rust/LLVM 版本、sample rate、block size、channel/voice 数、插件参数、warmup 和测量时长。

## 必备 benchmark

- `vst3-midi`：`node scripts/smoke-vst3-midi.mjs` 使用真实 VST3 fixture 和 400 次原生
  port 往返，覆盖通道消息、sample offset、reset、溢出和非法输出；工程离线 PCM 对拍
  验证依赖排序、扇出、静音、insert 输出与保存恢复，源码事务验证 Undo/Redo/Save/reopen。
  另包含 400 次 MIDI-only 往返和 400 次满载 SysEx 往返：每包 4 × 4096 bytes，保留
  0/13/63/127 帧位置；验证输出指针失效后的字节所有权、reset 和格式/预算故障。
  SysEx 图覆盖透传到音源、扇出、mute、保存恢复与独立 WAV 输入。
  模拟 sink 验证 loop/seek/stop 与故障计数；记录 IPC p95/p99，设备 callback 指标为 null。
  `OXITONE_VST3_MIDI_REPORT` 指定报告。macOS arm64/x64 CI 均运行，不打开系统音频输出。

- `vst3-installed-instrument`：`node scripts/smoke-vst3-instrument.mjs` 用显式安装的
  Vesti MIDI Synth + VestiGain，验证音符释放、复音、效果器增益、独立参数、运行中捕获、
  显式接受状态与保存恢复的 PCM。48 kHz/128 frames/stereo、16 块 render-ahead，
  模拟 loop/seek/pause/resume/stop 的诊断与插件故障均检查；设备 callback 指标为 null。
  `OXITONE_VST3_INSTRUMENT_REPORT` 指定报告，不替代任意商业音源资源和长负载验收。

- `vst3-daw-recording`：`node scripts/smoke-vst3-recording-daw.mjs` 使用已经构建的
  Recording Gain fixture/helper 与真实 headless Preview，验证可编辑曲线、原生包时钟的
  独立 PCM、Undo/Redo/Save、完整 take 重试、取消及源码失效；不测 callback 时延。
  `OXITONE_VST3_RECORDING_DAW_REPORT` 指定结果，`OXITONE_VST3_VIEWER` 可选 release
  Preview。该脚本必须在 workspace/Preview/fixture 构建之后运行。

- `vst3-recording`：`node scripts/smoke-vst3-edits.mjs --benchmark` 使用源构建 Recording Gain
  实际 helper，48 kHz/128 frames/stereo，Write 覆盖一个参数，入站自动化每包交替变化。
  100 包预热、1000 包测量、每 32 包确认一次日志，复核每个 PCM 样本与 1100 个 sample
  事件。记录 submit→receive IPC 往返原始样本和 p95/p99，poll 使用 yield_now，收页在
  计时区外。还包含 SDK 模拟 sink 的录制/seek/Source/WAV/save-reopen conformance。
  OXITONE_VST3_EDITS_REPORT 指定报告；不是设备 callback、高参数密度或厂商 GUI 负载验收。

- `vst3-live-instance`：`node scripts/smoke-vst3-instance.mjs` 使用本地 VestiGain 与真实
  Project/N-API 图实例，48 kHz/128 帧、3 个 insert、模拟 sink、16 块 render-ahead。
  3 次预热、30 次 setParameter + capture，记录 SDK Promise 往返 p95/p99 与同段模拟
  render worker diagnostics，测试首块值保持、独立实例、捕获写入工程后的 PCM、旧图
  Promise 拒绝及失败编译保留目标。OXITONE_VST3_INSTANCE_REPORT 指定报告；无厂商
  GUI 交互、设备 callback 或 CPU utilization 验证，不代替长负载与性能基线回归。

- `vst3-multibus`：`node scripts/smoke-vst3-buses.mjs` 编译真实 VST3 fixture，验证
  1600 块、48 kHz、最大 128 帧，主输入/mono 侧链、多输出、中间 inactive slot、显式激活、
  短段和 reset。报告各场景 IPC p95/p99；设备/callback/xrun 为 null，不代表设备期限保证。
  同时通过 native Project 验证侧链发送量、静音 detector、旁路和保存恢复的 PCM；
  三输出乐器覆盖独立路由、共用 fader/mute、关闭/切换路由、mixer/Track stems、逐样本
  保存恢复及模拟 sink 的 loop/换图/seek。IPC 数据与模拟 sink diagnostics 分开记录。
  `OXITONE_VST3_BUS_REPORT` 指定报告；两个 macOS 架构 CI 均运行无设备测试。

- `vst3-project-integration`：`node scripts/smoke-vst3-project.mjs` 用实际 VestiGain 验证
  stereo 工程 PCM、两级 insert、Master、干湿/bypass、非块对齐自动化、配置保存恢复，以及
  模拟 sink 的 direct→buffered 回退、pause/seek/短循环/stop。48 kHz、128 帧，明确记录
  background render block 直方图、xruns/deadlineMisses 和插件 faults；这不是设备 callback
  或听音。2026-09-20 的 stream 5 轻量 reset 后记录与原始数据见
  [集成记录](../reports/2026-09-20-vst3-engine-editor-integration.md)。

- `vst3-managed-lifecycle`：release 构建 `oxitone-vst3-host --features host,stream` 与
  `vst3-managed-probe`，显式 fixture 下执行 `node scripts/smoke-vst3.mjs --managed`。
  48 kHz/128 帧/stereo，manager capacity=2、queueDepth=4、latencyBlocks=2；32 次实例
  激活，每 epoch 40 块、每周期 17+111 帧，前 2 个 epoch 预热。记录 prepare、activate、
  process、reclaim、shutdown 和实际 caller interval，逐帧验证旧 epoch PCM 隔离并复核
  32 个 helper 全部退出。`OXITONE_VST3_MANAGED_REPORT` 指定 JSON。控制准备在每个新
  epoch 之前完成；可用 `OXITONE_VST3_MANAGED_LATENCY_BLOCKS` 显式改变延迟 2…16 块，
  queueDepth 随之取 max(4,latencyBlocks)，保留各配置的通过与失败数据。
  每 epoch 记录 IO 空闲等待/发送/响应等待与 helper 处理的最大墙钟耗时、完成块数和两条
  线程的 time-constraint 申请结果。`OXITONE_VST3_LOAD_THREADS=0…64` 可附加普通 CPU
  忙线程（默认 0）；记录实际线程数，结束或异常时 join，最长 45 秒自停。压力前后使用
  相同延迟，不能靠扩大缓冲隐藏迟到。普通调用线程可能也被延迟，须一起检查 caller interval。
  无设备，不能将结果当作并发工程编译/设备 callback/PDC 验收。

- `vst3-fixed-schedule`：release 构建 `oxitone-vst3-host --features host,stream` 和
  `vst3-schedule-probe` example，显式 fixture 下执行 `node scripts/smoke-vst3.mjs --schedule`。
  48 kHz/128 帧、每周期拆成 17+111 帧，显式 latencyBlocks=2（256 帧）、queueDepth=4。
  100 周期预热、1000 周期测量，记录每段 process p95/p99、原始耗时、caller interval 和
  deadlineMisses；逐帧校验 stereo 信号与两次全新 epoch 的启动静音。失败也先写报告。
  `OXITONE_VST3_SCHEDULE_REPORT` 指定 JSON 路径。调用方用普通线程尽力按周期唤醒，不
  补拉错过的周期；测量的是调度方法成本和 PCM 对齐，不是 HAL callback、全图 PDC 或
  音频设备 deadline 验收。CPU 占用、device、callback 耗时和 xruns 未测填 null。

- `vst3-transport-context`：`node scripts/smoke-vst3-transport.mjs` 构建仓库内 VST3 fixture，
  48 kHz、最大 128 帧，400 块交错 1/7/17/111/128 帧；PCM 编码插件实际处理计数及
  ProcessContext，验证初始位置、逐块跳转、暂停续块、tempo/meter、cycle 标志和短块时钟。
  stream 5 另覆盖 79 次段首 DSP reset，processor 内部计数清零而 sequence/连续帧保持。
  `OXITONE_VST3_TRANSPORT_REPORT` 指定 JSON。仅测无设备 IPC 往返；callback p95/p99、
  CPU 占用率与 xruns 不适用，不能替代 Engine/设备验收。
- `vst3-native-stream`：release 构建 `oxitone-vst3-host --features host,stream` 和
  `vst3-stream-probe` example，然后在显式 `OXITONE_VST3_FIXTURE` 下运行
  `node scripts/smoke-vst3.mjs --stream`。100 次预热、1000 次计时，48 kHz/128 帧、
  stereo 0.25 输入、bypass=1，分别记录实时端 submit/receive 和队列/helper 整体往返。
  probe 另验证短块、4 块并发排队、两次进程重建和 close/reap。调用方忙轮询，不创建设备；
  这些数字不是 callback、deadline 保证、PDC 或真实工程负载验收。
  `OXITONE_VST3_STREAM_REPORT` 指定包含原始样本与环境元数据的 JSON 路径。

- `vst3-offline-helper`：release 编译 `oxitone-vst3-host --features host` 并 build `@oxitone/vst3`，
  设置 `OXITONE_VST3_FIXTURE=/path/VestiGain.vst3`，执行 `node scripts/smoke-vst3.mjs --benchmark`。
  3 次预热、20 次完整 helper inspection 和一秒 WAV 渲染，48 kHz、128 帧、mono 0.25 输入、
  stereo 输出、bypass=1。包含两次 bundle hash、加载、状态恢复、DSP、WAV 发布与销毁；warm
  filesystem cache。另记录同次数预设 create-only 保存（含 fsync）与加载校验耗时；不启动
  helper，测试实际 opaque state 和归一化参数。`OXITONE_VST3_BENCH_OUTPUT` 指定 JSON 归档。设备、callback、CPU 占用、
  xrun 未测记 null；不能推导实时适配成本或 GUI 帧率。

- `preview/editing`：`cargo test --release -p oxitone-preview benchmark_editing_gestures -- --ignored --nocapture`。
  UI/control 线程测 128/10000 音符组变换与内容投影、128/4096 片段跨轨拖拽投影、
  128/4096 点原生曲线编译及 1024 点预览求值，
  100 次预热、1000 次采样，记录 p50/p95/p99。GPU 帧率、设备、sample rate、block size、
  callback、CPU utilization、xrun 未测记 null；不能用此基准宣称达到 FL Studio 完整交互/实时性能。

- `source-daw`：`pnpm --filter @oxitone/cli build && node packages/cli/bench/source-daw.mjs`。
  测完整工程 edit/原生校验/接受、实际 dirty journal Save、投影和重开。报告源码控制延迟，
  不代替 GPUI 帧率或 callback/xrun 实测。每次 Save 测量前先在计时区外修改文档。
  单 Pattern 文档及其 source-session 基准已删除；只维护实际工程使用的 source-daw 路径。

- `source-writing`：`pnpm --filter @oxitone/cli build && node packages/cli/bench/source-writing.mjs`。
  100/1000 音符的 sparse edit 文本补丁、含 import 的拆散和 literal 回写；3 次预热、20 次
  测量。包含 TS 解析/局部符号绑定/打印与 Pattern 校验，不包含文件发布、用户 TS 执行或音频。

- `effect-order`：先 build CLI，运行 `node packages/cli/bench/effect-order.mjs`。
  100 notes/2 placements/2 delays，5 次预热、40 次排序事务；报告 p50/p95/p99 和全部样本。
  内部 `oxitone.source.timing` diagnostics channel 仅在订阅时测量，发布 phase/耗时，
  不含源码、路径或插件值。分段涵盖 AST writing、所有 read validation 的累计耗时、bundle、
  disposable worker、native validation 与总事务；嵌套阶段不可重复相加。此基准验证控制延迟，
  不打开设备，不测 DSP/callback/xrun。记录 Node compile-cache 是否启用；采样期间禁止 build、
  包管理器或并行测试改变读集、占用 CPU。需同时跑 source-daw，覆盖累计编辑后的排序。

- `source-authoring`：`pnpm --filter @oxitone/core build && node packages/core/bench/source-authoring.mjs`。
  1k/100k 音符的单音与 128 音集合 edit、JSON DAG 重建；3 次预热、20 次测量，报告
  authoring p50/p95/p99 和包含 GC 的 heap net delta（非峰值）。没有设备、DSP 或 callback。

- `song-profile`：离线处理实际导出的 demo snapshot 与 hash-pinned 本地鼓机注册，
  不创建音频设备。命令 `cargo run --release -p oxitone-bench --bin song-profile --
target/examples/full-songs/after-the-horizon.snapshot.json
target/examples/full-songs/drums.json 24 80`。每个零基 bar seek 后预热 128 blocks，
  测 4000 blocks 的整图 process p95/p99/max、deadline exceedances 和插件 faults。
  使用工程自身 sample rate/block size，包含 DSP/automation/PDC，排除文件写入与
  compile；callback、CPU utilization、xrun 未测，不代替设备或 render-ahead 验收。

- `effects/electronic`: 26 种内置中的电子制作/动态效果，48 kHz/stereo/128 frames，
  continuous tone 与反复衰减语料；Criterion 测平均 DSP 成本。
  `cargo run --release -p oxitone-bench --bin effects-profile` 另测全部 26 种效果及
  262144-frame stereo IR：128 blocks 预热、3000 blocks 计时，包含 256-frame FFT
  分区的计算峰值，报告 process p95/p99/max。它是离线逐插件耗时，callback/设备/
  CPU utilization/xrun 未测记 null；不能替代整图或实时 callback 验收。

- `instruments/synth_motion`: 48 kHz/stereo/128 frames、8/32 个持续声部；default patch
  对比 full patch（A 7/B 3 unison、morph、sub/noise、五条 LFO 路由）。波表/声部起音
  在计时外准备，测试持续有声 process。`cargo bench -p oxitone-bench --bench synth_motion`。
  Criterion 记录平均处理时间；不是设备 callback，callback p95/p99、CPU 占用与 xrun 记 null。

- `compile/validate`: snapshot 大小、节点数、compile latency、峰值内存。
- `transport/schedule`: tempo changes、loop boundaries、automation density 下每 block event 数和调度时间。
- `automation/evaluate`: gate、polyline、sine/cos、triangle/saw、chance 在 control-rate/audio-rate 下的 evaluator 成本；记录 source 节点数、chance rate、每 block 求值次数和 PRNG 状态开销。
- `dsp/*`: oscillator、envelope、biquad、FFT/EQ、reverb、resampler 的 samples/sec。
- `mixer/routing`: bus 数、send 数、sidechain detector 和 meter 成本。
- `render/realtime`: callback wall time 分布（p50/p95/p99/max）、miss count、xrun count。
- `render/worker`: render worker 单 block 耗时分布、ring 深度扫描（2/4/8 blocks）、注入人工调度抖动（模拟抢占）时平均负载 < 70% 预算下 underrun 必须为 0。
- `render/offline`: render ratio、WAV writer throughput、peak memory。
- `plugin/c_abi_gain`: 真实 C 动态库在 48 kHz、64/128/256 frames 的实例适配成本，包含参数事件转换和非有限输出检查；不代替 realtime soak。
- `plugin/drums_native_entry`: 自带 drum cdylib 的同一 C 函数表静态链接测量，48 kHz、
  stereo、64/128/256 frames、四声部，每 8 blocks 重触发，decay=1、velocity=0.8。
  包括参数/note dispatch 与原生合成；不包含动态宿主适配器、整图或设备。
  `cargo bench -p oxitone-example-drums --bench process`；callback/xrun 记 null。
- `napi/command`: compile/transport command 往返延迟，不能用于 callback。
- `preview/plugin_layout`: release viewer 测试 harness，8 / 256 controls；100 次预热、1000 次
  控制线程解析/校验，记录 median/p95/p99。命令：`cargo test --release -p oxitone-preview
benchmark_layout_validation -- --ignored --nocapture`；包括 JSON 克隆/预算检查/descriptor
  绑定，不包含 GPUI 排版/绘制、设备、callback 或 DSP，未测指标记 null。
- `samples/inspect`: 控制线程的文件读取、SHA-256、解码、降混和 PCM 释放耗时；使用
  48 kHz stereo 的 1 秒 PCM16 / 10 秒 float32 WAV，固定 440 Hz 正弦。Criterion
  重复读取同一文件，代表 warm filesystem cache；不含 N-API、TS 或 prepare 的 SRC/编辑。
  此场景不使用音频设备、voice 或 callback，p95/p99 callback 与 xruns 记 null。
- `samples/playback`: 48 kHz/stereo/128 frames，单个 clip reset 后输出首个 block。
  `repitch_reset_128` 使用 rate 2，`stretch_reset_128` 使用 WSOLA ratio 0.5；样本为
  440 Hz、幅度 0.5 的一秒正弦，素材与 player 在测量外创建。覆盖启动和 reset 的
  RT 路径成本，仍不代替真实 callback/worker soak。
  `track_repitch_reset_128` / `track_stretch_reset_128` 在相同素材上测完整 ClipNode
  reset + 首块，Track BPM 120、全局 BPM 240、局部 duration 1 beat；包含窗口门控、
  局部 tempo 因子计算和 player 输出。未测真实设备 callback、CPU 利用率或 xrun。
- `instruments/slicer_tempo`: 48 kHz/128 frames、mono 440 Hz 一秒素材、单 slice/voice、
  Project BPM 90→153 的 linear ramp。完整 Channel/Master 路径（关闭保护 limiter）；
  测 reset + 首块，以及每 180 blocks 重触发的持续有声处理；不包含资产加载。
  此处是内存渲染 microbench，callback、设备、CPU 利用率与 xrun 记 null。
- `instruments/multisampler_39_regions_32_voices`: 48 kHz/stereo/128 frames，13 key zones × 3 dynamics，
  32 个不同音高声部使用 mono 440 Hz 一秒素材，力度 0.3/0.6/0.9。测 reset + 32 note-on + 首块，以及
  每 120 blocks 重触发的持续有声处理。资产/查表在计时前准备；不含 mixer、N-API、I/O 或设备。
  命令 `cargo bench -p oxitone-bench --bench multisampler`；callback p95/p99、CPU 利用率与 xrun 记 null。

`dsp/*` benchmark 必须包含 denormal 语料（衰减中的滤波器/混响尾部），验证 FTZ/DAZ 与 denormal-safe 实现没有性能悬崖。

模拟 worker 的 `JitterConfig` 注入隔离的调度暂停：仅在 ring 已恢复完整可用
horizon 后按 probability 决定是否暂停，并避免连续 block 暂停。恢复阶段不继续
注入，否则会把“可由 horizon 吸收的单次暂停”变为多个暂停累积的持续过载。
长于 horizon 的单次暂停仍由 extreme-jitter 测试验证 underrun 和 transport 连续性。

使用 Criterion 或等价 Rust harness；microbench 固定 seed 与输入 buffer，并包含 warmup。Realtime benchmark 采用独立高优先级线程、真实 block size 和预热后的 graph，禁止用仅测函数调用的 microbench 代替。realtime-soak 会循环完整 timeline，避免长测试在内容结束后只测静音；可用 `--plugin PATH --plugin-manifest PATH` 给每个 channel 添加已信任的动态效果器，并记录 hash 与 fault 数。

## 回归策略

`cargo bench -p oxitone-bench --bench synth_motion --bench electronic_effects`
新增电子合成/效果器内存基准：48 kHz、stereo、128 frames。Synth 8/32 个声部，
default/full/electronic 三种配置；electronic 含 A/B octave、非正弦 Sub、bank/warp、
FM、曲线包络及两条矩阵路由。Effects 单独测 distortion/multiband/delay 的连续
1 kHz 输入和反复衰减至 subnormal 的语料，保持相同 buffer/event storage。
此为 DSP microbench，device、CPU utilization、callback p95/p99、xruns 未测记 null；
不能冒充系统输出或 soak 验收。全曲另测 native/Wasm PCM parity、process timings 和内存。

`cargo bench -p oxitone-bench --bench insert_automation` 覆盖 insert 参数路径：
48 kHz / 128 frames，1 wavetable Track，3 mixer buses（含 Master），4 utility
inserts 和 4 条 polarity gate lanes（period 0.01 beat、duty 0.5、seed 7）。
分别测整图 compile/销毁、Master insert 参数解析、持续循环 [0,96000) frame
的内存 block render（含 Master limiter）。首个归档使用 1 s 预热、每场景 3 s
测量、30 个 Criterion samples。设备、CPU 使用率、callback p95/p99 和 xrun
未测，记 null；此 microbench 不代替 worker/callback soak。

`insert/recording_layers` 在 typed 场景的每个 target 增加 priority=1、constant=0.75
的 Playlist lane，clip 为 [0.01,0.81) beat。原 4 条 gate lanes 保留；另外测 compile
和 render_128。它与原场景负载不同，单独归档，不能拿两者差值当作同代码性能回退。

`node benchmarks/project-files.mjs` 测项目文件保存/加载：native 先生成 48 kHz stereo
一秒 float32 音源素材，测量外完成准备；预热 5 次，测量 30 次已有内容寻址资产的
重复 save 和 load（包含 SHA-256、canonical manifest、fsync）。记录 median/p95/p99
I/O 耗时；不测 callback 或 xrun，不用于实时验收。

`node benchmarks/project-restore.mjs` 测 `Project.fromSnapshot(...).snapshot()`：32 Tracks、
32 Channels/Patterns、每 Pattern 64 notes、32 lanes、3 buses/1 send，固定 seed 42。
预热 20 次、测量 100 次，归档 median/p95/p99 和原始样本。包括 TS schema、归属校验、
builder 恢复与快照生成；不含 native compile、素材 I/O、设备或 DSP，不作为 callback 验收。

主分支保存每个场景的 JSON baseline。p95/p99 超过 baseline 10%、peak memory 超过 15%、render ratio 下降超过 10% 或出现任何 xrun 时 CI 失败；硬件噪声较大时允许人工批准并记录原因。golden WAV 使用 SHA-256 加上 peak/RMS/true-peak 摘要，浮点比较需声明容差。

## 诊断

callback 只更新 atomic counters：`blocks`, `deadlineMisses`, `xruns`, `nanBlocks`, `queueDrops`；render worker 额外维护 `engineLoad`（block 耗时 / deadline 的 EMA）和 ring occupancy 水位。控制线程定期采样并输出结构化 JSON。性能日志不能使用每 block 一条日志的方式。

## Profiling 和验收

开发阶段使用 Instruments Time Profiler、Audio Unit/ CoreAudio diagnostics 和 `cargo flamegraph`（离线）。发布前至少进行 10 分钟实时测试、重复 seek/loop 测试、设备切换测试和 60 分钟 soak；报告必须归档到 `benchmarks/results/<date>-<machine>.json`，不提交大型音频产物。
