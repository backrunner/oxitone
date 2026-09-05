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
- `napi/command`: compile/transport command 往返延迟，不能用于 callback。

`dsp/*` benchmark 必须包含 denormal 语料（衰减中的滤波器/混响尾部），验证 FTZ/DAZ 与 denormal-safe 实现没有性能悬崖。

使用 Criterion 或等价 Rust harness；microbench 固定 seed 与输入 buffer，并包含 warmup。Realtime benchmark 采用独立高优先级线程、真实 block size 和预热后的 graph，禁止用仅测函数调用的 microbench 代替。

## 回归策略

主分支保存每个场景的 JSON baseline。p95/p99 超过 baseline 10%、peak memory 超过 15%、render ratio 下降超过 10% 或出现任何 xrun 时 CI 失败；硬件噪声较大时允许人工批准并记录原因。golden WAV 使用 SHA-256 加上 peak/RMS/true-peak 摘要，浮点比较需声明容差。

## 诊断

callback 只更新 atomic counters：`blocks`, `deadlineMisses`, `xruns`, `nanBlocks`, `queueDrops`；render worker 额外维护 `engineLoad`（block 耗时 / deadline 的 EMA）和 ring occupancy 水位。控制线程定期采样并输出结构化 JSON。性能日志不能使用每 block 一条日志的方式。

## Profiling 和验收

开发阶段使用 Instruments Time Profiler、Audio Unit/ CoreAudio diagnostics 和 `cargo flamegraph`（离线）。发布前至少进行 10 分钟实时测试、重复 seek/loop 测试、设备切换测试和 60 分钟 soak；报告必须归档到 `benchmarks/results/<date>-<machine>.json`，不提交大型音频产物。
