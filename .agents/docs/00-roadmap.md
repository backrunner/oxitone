# Oxitone 实现路线图

## 产品边界

Oxitone Phase 1 是 macOS 优先的编程化 DAW SDK，不是图形编辑器，也不承诺读取 MIDI 或加载 VST3。用户用 TypeScript 创建项目，Rust 编译并执行音频图，npm 提供安装、类型和原生二进制分发。

目标平台：macOS 13+，Apple Silicon 为首要验证目标，Intel macOS 保持可构建。输出设备默认使用系统默认设备，但 API 必须保留显式选择设备的字段。Phase 1 不接收麦克风或其他输入。

## 阶段与出口条件

### M0：工程基线与协议

- 建立 pnpm workspace、Cargo workspace、变更日志和 CI。
- 固定包名、crate 边界、协议版本、ID 规则、采样单位和错误码。
- 建立 N-API smoke test：TypeScript 创建空 Project，Rust 返回可校验 snapshot。
- 出口：全新 macOS 机器可执行 `pnpm install && pnpm build && cargo test --workspace`。

### M1：时间轴、Pattern 和 MIDI 导出

- 实现 Project、Track、Pattern、PatternClip、Note、支持 step/linear/exponential 变速的 tempo map 和 time-signature map。
- 实现 `Chord`、`Arp` 等无副作用的 TypeScript 编排函数。
- Rust 实现 beat/sample-frame 转换、loop/last 边界、确定性事件排序。
- 实现只导出 MIDI 的 SMF Type 1 writer。
- 出口：同一输入快照产生字节相同的 MIDI；边界和循环测试通过。

### M2：音源、Sample 和基础 DSP

- 实现音源/效果器的稳定参数契约、内置 wavetable synth 和 Plugin ABI v1（C ABI 函数表与参数事件，见 `08-plugin-abi.md`）。
- 实现内置 `Sampler` 音源（rootKey 变调、velocity 灵敏度、ADSR、loop 模式）。
- 实现内置 `Slicer` 音源：显式 marker/grid/onset 三种切片来源、oneshot/gate、per-slice 覆盖、off/repitch 跟随。
- 实现 WAV/AIFF 读取、MP3/MP4 音频轨离线转 WAV、采样率转换和非破坏性 Sample 编辑。
- 实现 SampleClip 的 `tempoSync`（off/stretch/repitch）、`wsola-v1` 保调拉伸器和 `fitBars`/`fitBeats`/`fitToContent` 对齐 helper。
- 实现 oscillator、envelope、voice allocation、gain/pan、filter、unison。
- 固定数值精度规则：f32 音频通路、f64 滤波器系数/低频 state、相位累加禁 f32、导出边界 TPDF dither。
- 出口：离线生成可听 WAV；没有在 callback 中分配、加锁或调用 N-API；stretch/repitch 在 tempo 阶跃与斜坡下均有 golden 测试；Slicer 的 slice 解析与 oneshot/gate 有 golden 测试。

### M3：Mixer、Automation 和侧链

- 实现 Master、MixerChannel、send/return、pre/post-fader、sidechain detector。
- 实现全图 PDC（plugin delay compensation）：效果器上报 `latencyFrames`，compiler 按最长路径自动补偿对齐。
- 实现 EQ、Limiter、Clipper、Filter、Phaser、Reverb、Compressor（含侧链）、beat-synced Delay、Gate、Chorus、Saturator、Utility 的最小可用版本；非线性处理器 2x/4x oversample 抗混叠。
- 实现 insert 内建 `mix`/`bypass`、Channel `swing` 调度和 `unit: 'beats'` 的 tempo 跟随参数。
- 实现 0-1 automation、曲线和参数快照，支持 block 内 sample-accurate 分段；支持全局 tempo lane（编译期烘焙为分段线性 bpm 表）；内建节点参数（Channel/MixerChannel/SampleClip，含 tone、send ratio）全部可自动化。
- 提供 `gate`、`chance`、`wave`、`sine`、`cos`、`polyline/curve` 等可组合高阶 automation source，并在 Rust 中确定性求值。
- 离线 render 支持 stem 导出（mixer channel/track）和 loudness 报告（peak/true-peak/LUFS）。
- 出口：路由图非法时在编译期报错；自动化和侧链有 golden WAV 与数值测试；PDC 对齐有 sample 级测试。

### M4：实时播放与 macOS I/O

- CoreAudio HAL 输出适配（禁用 input scope，不用 AudioQueue/AVAudioEngine），系统默认设备，设备枚举和显式设备 ID 预留。
- 设备不匹配处理：buffer frame size 协商、`adapt-device`/`resample` 采样率策略、声道/格式适配、设备热切换与诊断。
- Render-ahead 架构：专用 realtime worker 线程 + 预分配 SPSC ring，HAL callback 只做拷贝；`renderAheadBlocks` 默认 4。
- play/pause/stop/seek、从 bar/timecode/marker 开始播放、loop region、transport state、内置 metronome。
- callback 使用预分配的双缓冲 render graph 和无锁 command queue；FTZ/DAZ 与 denormal-safe DSP。
- 出口：48 kHz/128 frame 在目标硬件上 10 分钟无 underrun/xrun，render worker p99 满足性能预算；注入调度抖动时平均负载 < 70% 预算下保持 0 underrun。

### M5：npm 分发与开发者体验

- CLI foundation is implemented in `@oxitone/cli` (`render`, `export-midi`,
  `doctor`). Platform native package resolution remains compatible with the
  reserved `@oxitone/native-<platform>-<arch>` naming scheme.
- Realtime transport loop regions are exposed through the versioned transport
  command and `Session.play(..., { startFrame, endFrame })`; the end is
  exclusive and applied at block boundaries without rendering past it.
- `oxitone` facade、平台包（`@oxitone/native-darwin-arm64` 等）、postinstall 选择器和 ABI 检查。
- 第三方插件动态加载已实现：显式 registerPlugin、控制线程加载、SHA-256/签名/ABI/manifest 校验、C 实例适配、插件 fault 计数和实时延迟回收。纯 C 静态/动态 parity、Rust cdylib 和 N-API WAV 测试覆盖。逐节点 deadline watchdog、发布平台包与公证流水线仍待完成。
- 项目诊断、结构化错误、日志级别和最小 CLI（render、export-midi、doctor）。
- 示例项目、API reference、版本迁移说明。
- 出口：干净 npm 项目可以只安装 `oxitone`，无需 Rust 工具链即可播放/渲染。

### M6：GPUI Preview App

- runner：watch 模式执行 TS 工程、防抖重建 snapshot、诊断转发；viewer：GPUI 只读视图与内嵌引擎。
- Workspace/piano roll/channel rack/mixer/scopes（波形、频谱、XY）、transport 条与延迟补偿播放头。
- `oxitone preview` 启动路径与平台二进制分发。
- 出口：示例工程改代码后不停播放完成换图；代码报错时旧图继续播放并显示诊断；集成冒烟通过（`09-preview-app.md`）。

### M7：稳定性与发布

- fuzz：项目 manifest、Pattern、automation、routing graph、WAV/MIDI writer。
- 长时播放、重复 seek、设备断开、坏 sample、OOM 前置错误处理。
- 签名/校验原生包，生成 SBOM 和 macOS notarization 材料。
- 出口：发布清单中的所有门禁通过，性能基线没有回归。

## Phase 2 预留

VST3 只作为 `PluginHost` 的新 adapter，复用 `08-plugin-abi.md` 的 manifest 与参数模型。不得把 Steinberg SDK、VST 参数 ID、编辑器线程模型或厂商二进制放入 Phase 1 核心接口。Phase 2 还需要新增进程外沙箱/WASM 隔离、VST3 生命周期、参数映射、尾音和线程安全规范，并保持现有 `Instrument`/`Effect` contracts 与 Plugin ABI 不变。

## 依赖顺序

`协议与图模型 -> 时间轴 -> DSP 原语 -> 音源/样本 -> Mixer/Automation -> offline render -> CoreAudio playback -> npm/release`。每个阶段都必须先完成上一阶段的测试和文档出口条件。

## 未决但不阻塞 M0

- 默认 polyphony 上限：M2 先固定为 64 voices/channel，之后以配置项开放。
- Reverb 的算法：M3 采用 Schroeder/FDN 之一，接口不暴露算法细节。
- MP3 转码实现：优先 Rust 内置解码器；若采用系统工具，必须在导入阶段执行，不得进入实时路径。
