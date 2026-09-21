# Oxitone 实现路线图

新的发布目标是完整代码/GPUI DAW 双向 authoring，按 [P0–P5](../designs/source-daw/06-delivery.md)
推进，包含外部插件 ABI 2 与 GPUI 插件管理器；下文 SDK-only 定位和 M0–M8 作为已有基线保留。
当前开始实现生成式 Pattern 核心，契约见 [15](15-source-authoring.md)，不代表整个 P1 完成。

当前落地状态与缺口见 [实现核对](10-implementation-status.md)。下文是目标与出口条件，
不能仅根据某一层已有代码推定整个里程碑完成。

## 产品边界

Oxitone Phase 1 是 macOS 优先的编程化 DAW SDK，不是图形编辑器，也不承诺读取 MIDI 或加载 VST3。用户用 TypeScript 创建项目，Rust 编译并执行音频图，npm 提供安装、类型和原生二进制分发。

目标平台：macOS 13+，Apple Silicon 为首要验证目标，Intel macOS 保持可构建。输出设备默认使用系统默认设备，但 API 必须保留显式选择设备的字段。Phase 1 不接收麦克风或其他输入。

## 阶段与出口条件

### M0：工程基线与协议

- 建立 pnpm workspace、Cargo workspace、变更日志和 CI。
  macOS 双架构 workflow 已建立，覆盖 locked install/build、全部测试、schema drift、
  离线示例与专项基准；远端首跑和最低 macOS 版本验收仍需记录。
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
  统一 SDK 入口、Project/Session 插件注册和 portable presets 已实现；平台包发布与
  无工具链干净安装验收仍待完成。
- 第三方插件动态加载已实现：显式 registerPlugin、控制线程加载、SHA-256/签名/ABI/manifest 校验、C 实例适配、插件 fault 计数和实时延迟回收。纯 C 静态/动态 parity、Rust cdylib 和 N-API WAV 测试覆盖。逐节点 deadline watchdog、发布平台包与公证流水线仍待完成。
- 项目诊断、结构化错误、日志级别和最小 CLI（render、export-midi、doctor）。
- 示例项目、API reference、版本迁移说明。
  `examples/offline`、README 与 `docs/api.md`/`docs/migrations.md` 已提供开发入口，
  示例经 native 生成有声 WAV/MIDI，并验证采样工程保存/恢复。CLI 已能读取工程目录。
- 出口：干净 npm 项目可以只安装 `oxitone`，无需 Rust 工具链即可播放/渲染。

### M6：GPUI Preview App

- runner：watch 模式执行 TS 工程、防抖重建 snapshot、诊断转发；viewer：GPUI 只读视图与内嵌引擎。
- Workspace/piano roll/channel rack/mixer/scopes（波形、频谱、XY）、transport 条与延迟补偿播放头。
- `oxitone preview` 启动路径与平台二进制分发。
- 出口：示例工程改代码后不停播放完成换图；代码报错时旧图继续播放并显示诊断；集成冒烟通过（`09-preview-app.md`）。
  当前 runner/watch、GPUI 只读视图、IPC/transport、CLI 及 unsigned 开发 bundle
  已建立，真实原生 headless 冒烟覆盖换图/错误恢复；完整视觉交互与正式平台分发
  仍待验收，详见 `10-implementation-status.md`。

### M7：稳定性与发布

- fuzz：项目 manifest、Pattern、automation、routing graph、WAV/MIDI writer。
- 长时播放、重复 seek、设备断开、坏 sample、OOM 前置错误处理。
- 签名/校验原生包，生成 SBOM 和 macOS notarization 材料。
- 出口：发布清单中的所有门禁通过，性能基线没有回归。

## Phase 2 预留

VST3 使用独立 helper 与工程后台执行器，现行能力见 [`24-vst3-sdk.md`](24-vst3-sdk.md)。
已支持精确 class/hash 注册、工程乐器/insert、参数/音符、transport/seek/loop reset、
图 PDC、预设/工程状态、离线 WAV 和模拟 sink 验证。厂商配置编辑器通过 Apply 原子回写源码，
DAW 实例面板可控制播放实例窗口并显式接受当前状态，catalog 仍使用独立配置窗口。
stream 11 保留完整总线 metadata、显式激活和按索引的多总线 PCM；
工程 bus insert 支持一个侧链输入；Engine 1.4 的 Channel.outputRoutes 将乐器辅助输出
独立路由到 mixer，并保留共用事件、轨道控制与 PDC。设备 callback 始终只读取完成的 PCM。
本轮实现、测试和未清除的性能门禁见 [多总线接入记录](../reports/2026-09-20-vst3-multibus-output-routing.md)。
配置窗口和 `configureVst3Plugin` 已统一多总线零样本处理，支持配置阶段的 I/O/latency/
参数表重查与有界重启；预设先恢复再校验，并合并参数覆盖。工作台刷新保留原始恢复数据，
发布重新解析后的配置。详见 [配置协商记录](../reports/2026-09-20-vst3-configuration-negotiation.md)。
原生 Session 已提供有界 live controller：厂商窗口绑定同一个处理实例，可实时修改参数、
捕获 state、关闭/重开窗口；超时和 session 关闭回收 helper，旧句柄失效。capture 不重置
已发声音符。SDK/DAW 已接通精确实例定位和试听状态的源码事务，包含 Undo/Redo/Save。
实现与证据见 [常驻实例控制记录](../reports/2026-09-20-vst3-live-control.md)。
厂商 begin/value/end 已通过有界游标日志绑定实际下一播放音频块，支持 SDK 读取、停止、
取消和丢失检测；Touch/Write 覆盖、停止/失败释放及 SDK take/source 区间合并已接通。
DAW 录制按钮、参数选择、Playlist 时间映射和源码事务已接入，曲线支持后续范围编辑。
真实 Preview、独立 PCM、Undo/Redo/Save、取消与重试证据见
[DAW 录制记录](../reports/2026-09-20-vst3-daw-recording.md)。
实现与验证见 [手势日志记录](../reports/2026-09-20-vst3-gesture-journal.md)。
录制覆盖、SDK Source/WAV 与保存恢复验证见 [自动化录制记录](../reports/2026-09-20-vst3-automation-recording.md)。

stream 11 在运行中 I/O/latency/参数表变化时冻结旧实例并保留恢复状态。显式接受状态后，
每个实例独立恢复和协商描述，再校验参数/路由、重算 PDC、准备并换图；候选失败保留旧图。
DAW 参数与自动化面板使用已编译实例的描述。实现与验证见
[运行中恢复记录](../reports/2026-09-20-vst3-runtime-recovery.md)。

Engine 1.6 已接入 Mixer 效果实例的多路输入选择与辅助输出，包括 PDC、stems 和 Preview 路由投影。
Engine 1.7 / stream 11 已接入 Channel 乐器及 insert 的主 MIDI 输出到下游原生乐器。
stream 11 同时保留纯 MIDI 处理器的零音频总线布局，支持 Channel MIDI 变换链和自主生成器。
同段采样位置、扇出、故障清理和源码保存恢复的验证见
[MIDI 路由记录](../reports/2026-09-21-vst3-midi-routing.md)。
零音频总线、MIDI 变换链、自主生成与配置事务见
[MIDI-only 接入记录](../reports/2026-09-21-vst3-midi-only.md)。
stream 11 新增主总线 SysEx：有界 payload、采样位置、输入/输出、图路由与独立 WAV 接入。
实现与验证见 [SysEx 与效果器/音源验收](../reports/2026-09-21-vst3-sysex.md)。
后续优先保障效果器和音源的加载、发声、参数、状态恢复与播放；高级事件协议按实际需要推进。
剩余：Note Expression、辅助事件总线、Mixer 反馈 MIDI 与 surround，
以及商业插件、原生窗口和 Intel 的兼容性验收。
商业插件资源/负载矩阵、真实原生窗口操作、
Intel 实机及发布签名/公证仍需独立验收；inspection/离线测试不等于这些项目完成。

## 依赖顺序

`协议与图模型 -> 时间轴 -> DSP 原语 -> 音源/样本 -> Mixer/Automation -> offline render -> CoreAudio playback -> npm/release`。每个阶段都必须先完成上一阶段的测试和文档出口条件。

## 未决但不阻塞 M0

- 默认 polyphony 上限：M2 先固定为 64 voices/channel，之后以配置项开放。
- Reverb 的算法：M3 采用 Schroeder/FDN 之一，接口不暴露算法细节。
- MP3 转码实现：优先 Rust 内置解码器；若采用系统工具，必须在导入阶段执行，不得进入实时路径。
