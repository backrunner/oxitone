# Oxitone 性能与 benchmark 规范

## 性能预算

基线为 macOS Apple Silicon、48 kHz、128 frames、stereo、release build：

| 场景 | callback p99 | CPU 总占用 | xruns |
| --- | ---: | ---: | ---: |
| 空图 + Master | < 0.5 ms | < 1% | 0 |
| 32 voice synth + 4 inserts | < 1.5 ms | < 10% | 0 |
| 64 voice + 8 mixer buses + reverb | < 2.0 ms | < 20% | 0 |
| 10 min offline render | n/a | >= 20x realtime | n/a |

预算不是跨硬件的绝对承诺；每次 benchmark 必须记录 CPU 型号、OS、Rust/LLVM 版本、sample rate、block size、channel/voice 数、插件参数、warmup 和测量时长。

## 必备 benchmark

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

`dsp/*` benchmark 必须包含 denormal 语料（衰减中的滤波器/混响尾部），验证 FTZ/DAZ 与 denormal-safe 实现没有性能悬崖。

模拟 worker 的 `JitterConfig` 注入隔离的调度暂停：仅在 ring 已恢复完整可用
horizon 后按 probability 决定是否暂停，并避免连续 block 暂停。恢复阶段不继续
注入，否则会把“可由 horizon 吸收的单次暂停”变为多个暂停累积的持续过载。
长于 horizon 的单次暂停仍由 extreme-jitter 测试验证 underrun 和 transport 连续性。

使用 Criterion 或等价 Rust harness；microbench 固定 seed 与输入 buffer，并包含 warmup。Realtime benchmark 采用独立高优先级线程、真实 block size 和预热后的 graph，禁止用仅测函数调用的 microbench 代替。realtime-soak 会循环完整 timeline，避免长测试在内容结束后只测静音；可用 `--plugin PATH --plugin-manifest PATH` 给每个 channel 添加已信任的动态效果器，并记录 hash 与 fault 数。

## 回归策略

`cargo bench -p oxitone-bench --bench insert_automation` 覆盖 insert 参数路径：
48 kHz / 128 frames，1 wavetable Track，3 mixer buses（含 Master），4 utility
inserts 和 4 条 polarity gate lanes（period 0.01 beat、duty 0.5、seed 7）。
分别测整图 compile/销毁、Master insert 参数解析、持续循环 [0,96000) frame
的内存 block render（含 Master limiter）。首个归档使用 1 s 预热、每场景 3 s
测量、30 个 Criterion samples。设备、CPU 使用率、callback p95/p99 和 xrun
未测，记 null；此 microbench 不代替 worker/callback soak。

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
