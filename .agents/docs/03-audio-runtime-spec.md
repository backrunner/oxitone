# Oxitone 音频运行时规格

## 时间和调度

Rust 将 tempo map 编译成可查询的 beat <-> sample-frame segments。使用 64-bit sample frames 和 64-bit fixed/rational beat，禁止用累计 f32 seconds 作为 transport 游标。每个 audio block 计算 `[frameStart, frameEnd)`，调度该区间内的 note-on、note-off、parameter、transport 事件；事件按 frame、优先级（stop/note-off/param/note-on）、序号排序。

**变速 tempo 的数学定义**：设 segment 起点 beat `x0`、BPM `b0`，下一 segment 起点 `x1`、BPM `b1`，`L = x1 - x0`、`u = (x - x0)/L ∈ [0, 1)`：

- `step`：`bpm(x) = b0`；`seconds(x) = 60·(x - x0)/b0`。
- `linear`：`bpm(x) = b0 + (b1 - b0)·u`；`seconds(x) = 60L/(b1 - b0) · ln(bpm(x)/b0)`（`b1 == b0` 时退化为 step）。
- `exponential`：`bpm(x) = b0 · (b1/b0)^u`；`seconds(x) = 60L/(b0·ln(b1/b0)) · (1 − b0/bpm(x))`。

三种 curve 的 beat→seconds 和 seconds→beat 都是闭式解，两个方向都必须实现且单调可逆。实现规则：编译时预计算每个 segment 边界的累计 f64 seconds；查询时定位 segment、用闭式局部积分加边界累计值，乘 sampleRate 后统一 round-half-up 到 `u64` frame；禁止逐 block 累加秒数。同一 beat 的所有查询路径（事件调度、timecode 显示、offline render、MIDI tick 换算）必须得到同一 frame。校验错误码：`TempoRange`（BPM 越出 20..999 或非有限）、`TempoMapOrder`（startBeat 非严格递增、首段不在 beat 0 或 segment 重叠）。

**Tempo automation 烘焙**：snapshot 含 tempo lane 时，compiler 先求有效 `bpm(t)`（lane 输出 0..1 经 log 映射到 20..999），再烘焙为分段线性 bpm 表：

- 采样网格为固定有理数 1/64 beat，并叠加 lane 的全部不连续点（curve 控制点、gate/square 边界、loop 边界）作为额外汇段边界，保证跳变落在精确 frame。对值发生实质跳变（|Δ| > 1e-3）的不连续点，烘焙器在其前 ε = 1/65536 beat 处额外插入一个左极限探针边界：探针段以 `step` 保持跳前值，跳变本身成为边界上的精确阶跃；连续控制点（斜率连续）不插入探针，避免虚增段数。
- 每段用前述 `linear` 闭式积分；分段线性逼近的计时误差随网格密度二次收敛，golden 测试声明容差并覆盖 1/64 网格的最坏情况。
- 烘焙范围覆盖 Project timeline 全长加 `tailSeconds`；分段总数上限 65_536，超出报 `TempoMapComplexity`。
- source transport-invariant 意味着该表与 seek/loop/pause 无关，播放期间不重建。

在 tempo change、pattern loop、automation loop 的边界，事件必须落在精确的 sample frame。跨 block 的事件存入预分配 event ring；不得因为 block 边界重新分配或丢失事件。

Channel 的 `swing` 在 beat 域作用于 note 事件调度：以 1/16（0.25 beat）为网格，落在弱位（网格奇数位）的 note 后移 `swing × 0.125 beat`；swing 可自动化（control-rate，逐事件读取当前值），只影响 note 事件，不影响 sample clip 和 automation 求值，也不参与 PDC。

Automation source AST 在 compile 阶段转换为固定 evaluator。`gate`、waveforms 和 polyline 不分配；`chance` 使用 versioned `pcg32` 和预分配状态，不能调用系统随机源。对 control-rate target，每 block 最多求值一次；对 audio-rate target，evaluator 在既定 SIMD/scalar 路径中逐 sample 求值。source 深度、节点数、chance rate 和 audio-rate source 数量超过预算时，validator 必须返回稳定错误码。

## Realtime callback 不变量

Track 静态 tempo 由 Rust `TrackClock` 在控制线程解析：局部拍位转换为绝对秒，
音频 round-half-up 到 frame，MIDI 经有效 Project clock 逆变换到 beat/tick。
loop/last 先在局部拍域裁剪；图的 content end 转换到 Project beat。tempo lane 的
烘焙范围按最高合法 BPM 999 对局部时长作保守上界估计，仍执行 segment 数量上限。

`process_block` 及其所有直接调用必须：

- 不分配/释放 heap，不获取 mutex/RwLock，不等待条件变量，不进行文件、网络、系统调用或 N-API。
- 不解析 JSON、不格式化日志、不读取环境变量/时钟、不使用不可预测的 lazy initialization。
- 只使用 prepare 阶段创建的缓冲区、DSP 状态和插件指针。参数通过 lock-free queue 或 atomic snapshot 传入。
- 处理固定 channel count/block size；不支持时由 adapter 在 callback 外做转换。
- 在检测到内部异常时输出静音并设置 atomic fault flag；callback 不 panic/throw。

Rust 可用 `#[deny(unsafe_op_in_unsafe_fn)]` 和 clippy lint 约束；任何 `unsafe` 必须局部、注释不变量并有测试。

## RenderGraph 生命周期

1. TS 提交 snapshot/revision。
2. Rust validator 检查 IDs、类型、路由 DAG、参数、资源引用和能力。
3. compiler 在控制线程加载/解码 sample、prepare 插件、建立拓扑和预分配池。
4. 新图在 block boundary 原子发布；旧图进入 deferred reclamation，直到没有 callback 使用。
5. compile error 不替换当前可播放图；返回稳定 error code/path。

当前实现：换图和设备链替换使用预分配的回收队列；只有控制线程销毁旧实例、
缓冲和库。回收队列满时实时端延后消费命令，控制命令队列满报 RealtimeFault。
shutdown 使用独立 atomic 标志，不依赖队列空位。实时 session 内禁止改变
sampleRate/blockSize；新图通过 seek 对齐游标并保留 automation origin/loop iteration。
旧图的尾音不会跨换图延续。

N-API compile 在首次 play 前也保留旧图的 frame cursor、state 和 loop；Session 已开始
实时播放时沿用 worker 换图路径。timecode seconds 在控制线程按实际编译采样率转换，
必须先检查有限、非负和 u64 frame 可表示范围，再进行 round-half-up；无效输入不取走
engine 持有的图、不启动设备。新查询不增加 callback 工作。

Graph 节点包括 instrument、sample player、effect、mixer bus、meter 和 output sink。节点不通过全局单例互相查找，依赖在编译时以索引/句柄解析。

## DSP 处理顺序

Wavetable 的 A/B 各支持六种 source cycle（sine/saw/square/triangle/organ/glass），
prepare 在控制线程构造并共享同采样率 mip tables。position 在相同 phase/mip 读取
source 与 morph target 后线性插值，只前进一次 f64 phase；position=0 走旧读取路径。
unison start phase/spread 在新声部起音时应用，不在 callback 构造波表。
确定性 xorshift32 noise 进入滤波器，正弦 Sub 位于滤波器后、amp/velocity 前；
noise seed 随 note pitch 重置，seek/reset 重建固定声部状态而不分配。

合成器内部 LFO 为逐声部 note-triggered Hz source（不是工程 AutomationSource）：
四种双极形状，固定路由到 pitch/cutoff/两路 position/amp。position 与 tremolo 逐样本求值；
pitch/cutoff 以 32-frame 声部控制周期更新，pitch/glide 改变时重选 mip。
开启 LFO 的控制周期跨 process block 保留，note/parameter 更新使缓存失效；
legato 保留 LFO phase/包络并更新音高，mono retrigger 重触发 LFO。关闭调制保留旧控制路径。
level 使用 `1 − depth * (1 − lfo) / 2`，position 夹紧到 0…1，cutoff 在叠加
filter envelope 与 LFO 半音值后夹紧到 10 Hz…Nyquist−1。新增控制参数除 LFO
逐样本内部计算外不另加参数平滑；工程 automation 仍遵守既有 control-rate 语义。
跨 64/128/256 frames、非控制周期对齐起音、mono/legato glide 与零分配 seek/抢占有集成测试。

每 block：transport event dispatch -> track note/sample events -> voice/instrument render -> channel inserts -> channel fader/pan -> pre sends -> mixer bus inserts -> post sends -> Master inserts -> output limiter/format -> meter enqueue。Effect tail 按节点报告的 tail length 保持；offline render 额外渲染 `tailSeconds`。

compiler 依据各 plugin 上报的 `latencyFrames` 执行全图 PDC（plugin delay compensation）：汇聚到同一 bus 的各路径用补偿 delay 对齐到最长路径，sidechain detector 路径同样对齐；补偿节点是普通预分配 delay 节点，参与同一拓扑序。PDC 不改变 automation 的 beat 域定义，只移动音频落点；图内部总延迟列入 render report 和诊断。

## 数值精度与保真

- 内部音频通路统一 non-interleaved `f32` buffer；所有 summing 的顺序由 graph compiler 的拓扑序固定，任何重构不得改变求和顺序，保证 bitwise deterministic。
- 滤波器系数用 `f64` 计算；低 cutoff biquad 的 state 用 `f64`（防止低频系数发散），其余 DSP state 可用 `f32`。相位累加（oscillator、LFO、automation wave phase）一律用 `f64` 或从绝对整数 sample frame 推导，禁止累加 `f32` phase。
- Headroom 依赖 `f32` 动态范围；Master 出口前有默认启用的最终 limiter/clip 保护节点（仅诊断场景可显式关闭），实时设备输出与 WAV export 共用该保护。
- 非线性处理器（Clipper、带饱和级的 Limit、waveshaper 类、含反馈的 Phaser）必须 2x/4x oversample 抑制 aliasing；oversampling 引入的 latency 经 `latencyFrames` 上报并参与 PDC。
- 所有重采样（sample decode、device rate 适配、rate/pitch 播放）目标质量 ≥ 100 dB SNR（windowed-sinc polyphase）；质量和成本都进 benchmark。
- bit depth 降低只发生在导出边界：16/24-bit WAV export 默认加 1 LSB TPDF dither（可用 render option 关闭）；32-bit float 导出与实时设备输出不 dither。
- Meter：每个 MixerChannel 提供 peak/RMS，Master 额外提供 4x oversampled true-peak 估计；meter 在音频线程只写 atomic/ring，展示层自行节流。

默认内部格式为 non-interleaved `f32`，headroom 至少 6 dB。动态 C 插件边界逐样本检查 NaN/Inf，故障只静音该实例并累计插件 fault；该额外开销必须由插件适配层 benchmark 验证。其余数值检查的开销同样需要符合 block 预算。

Sample player 的运行时可自动化参数按 DSP 实现：`tone` 是每 clip 预分配的 tilt filter，`level`/`gain`/`pan` 是平滑增益级，`rate` 是 varispeed（resampler 质量遵循 ≥100 dB SNR 规范，rate 变化经 smoother，播放位置用 f64 逐 sample 积分）。`startFrame`/`endFrame`/`normalize`/fade 在 prepare 阶段烘焙，运行时不可变；对它们发起 automation 绑定在 validation 阶段拒绝。

`tempoSync` 的运行时规则：

- `stretch`：WSOLA 类保调拉伸器，所有窗口/缓冲在 prepare 预分配；拉伸比由 beat 域映射推导（内容 beat 长度 ÷ clip beat 长度），tempo 变化时按 control-rate 每 block 从烘焙的 beat↔frame 表重新计算并平滑，不允许逐 sample 突变。拉伸比有效范围 0.25..4，clip 配置超出范围时 compile 报错 `SampleStretchRange`。realtime 与 offline 使用同一算法、同一参数和同一 ratio 序列（offline parity 适用）。
- `repitch`：复用 varispeed 路径，tempo 因子与 `rate` 参数相乘后经同一 smoother；播放位置仍用 f64 积分。
  内容帧数按最终 `durationBeats` 分配，block 的有效 rate 为
  `contentFrames / durationBeats * (beat(endFrame) - beat(startFrame)) / blockFrames`
  再乘 clip rate 和自动化 rate，因此显式缩短/拉长 clip 会缩放完整内容。启动先以
  rate 1 预卷足够的零帧，再直接设定已知起始 rate；仅后续改变走 smoother，避免启动
  ramp 造成固定时间偏移。有效 rate 的既有 varispeed 范围为 0.125..16。
- WSOLA 的 seek/loop reset 清空流状态和已有缓冲，不重建 stretcher 或释放内存。
  reset 后继续 render 必须与新实例逐样本一致，分配/释放计数均为 0。
- 默认 tempo 因子来自有效 Project tempo 表。独立 Track 的 SampleClip plan 保存静态
  BPM 和局部 duration；repitch 的每 block beat 增量为 `frames * BPM / (60 * sampleRate)`，
  stretch 使用该 BPM。所有窗口和 loop 截止点在编译期转换为绝对 frame；callback
  只做标量运算，不创建第二份 tempo map，不分配/释放。

`Slicer` 的每个 slice 是一个预分配的 varispeed player voice：compile 期把显式 marker/grid/onset 统一解析成 frame 区间的不可变 slice 表（onset 检测在 compile/prepare 执行，不得在 callback 中运行），note-on 按 `triggerNote` 偏移索引 slice；oneshot 忽略 note-off，gate 用 note-off 触发 release。slice 播放的全部参数路径与 Sampler 相同，PDC、平滑和确定性规则不变。

Slicer repitch 在 compile 解析原生 BPM 与参数索引并校验完整有效 tempo map 的
0.25..4 因子范围。处理段起点在 note dispatch 前暂存 tempoFactor，更新活跃 voice
与随后触发的 voice；所有数据预分配，不读取 JSON、资产或 descriptor 字符串表。
seek/loop 后重新使用目标 frame 的有效 BPM，复用 SampleVoice 的 rate smoother。
Sampler/Slicer 共享 voice pool 在 seek/loop 清零 ADSR 状态，保留参数，避免旧 sustain
让重放跳过 attack。音源事件暂存按 descriptor 大小预分配，同段同参数后写覆盖前写，
初始值 → host → automation，不再依赖 64 事件上限；seek 清除待处理事件，不释放缓冲。

## 低延迟和设备

**输出路径选型**：macOS adapter 直接使用 CoreAudio HAL 输出（AudioUnit v2 `HALOutput` 或 `AudioDeviceCreateIOProcID`），输入 scope 显式禁用；不使用 AudioQueue 或 AVAudioEngine——二者会引入不可控的中间缓冲和 graph 开销。adapter 启动时：

1. 解析目标设备（默认系统输出或显式 `deviceId`），查询其支持的 nominal sample rates、buffer frame size 范围、`kAudioDevicePropertyLatency` 和 safety offset。
2. 把 buffer frame size 设为允许范围内最接近 `blockSize` 的受支持值（目标 128，不低于 64）；设置失败则回落到设备当前值并产生诊断。
3. 注册监听：默认输出设备变化、设备移除、nominal sample rate 变化、buffer size 变化；所有重建都在控制线程完成，HAL callback 只读 atomic 状态。

公开 API：`listOutputDevices()`、`getDefaultOutputDevice()`、`setOutputDevice(deviceId)`（Phase 1 可返回 unsupported/requiresRestart）。无 input stream、无 input permission 请求。

**项目规格与设备规格不匹配的处理**（render graph 以 `Project.sampleRate`/`blockSize` 编译）：

- **Sample rate**：由 `deviceRatePolicy: 'adapt-device'|'resample'` 控制，默认 `adapt-device`。adapt-device 在设备支持时将其 nominal rate 设为项目采样率（该修改对全系统生效，必须在文档明示）；设备不支持或设置失败时自动回落到 resample。resample 模式在 render worker 内放置实时安全的 polyphase resampler，把项目 rate 的 block 转成设备 rate 后写入 ring；HAL callback 仍是纯拷贝，resampler 的群延迟计入 `getOutputLatency()`。
- **Buffer size**：ring 以 frame 为单位而非 block。worker 按固定 `blockSize` 渲染，HAL callback 按设备实际的 frames-per-slice 拉取（允许每次 callback 帧数不同）。ring 深度取 `max(renderAheadBlocks, ceil((deviceBufferFrames + deviceLatencyFrames) / blockSize))`，保证安全垫不被大缓冲设备（蓝牙/AirPlay）吃掉。`latencyMode: 'direct'` 要求设备 frame size 与 `blockSize` 兼容，否则自动回落 buffered 并诊断。
- **声道与格式**：图内部为 stereo non-interleaved f32；worker 写 ring 前转成设备原生布局（interleaved f32，按 `PreferredChannelsForStereo` 映射；mono 设备输出 `(L+R)*0.5`；声道数大于 2 的设备其余声道写 0）。HAL callback 不做格式转换。
- **设备热切换**：由 `deviceChangePolicy: 'follow-default'|'pause'` 控制，默认 `follow-default`。默认设备变化或被拔出时，在控制线程重建输出链路，transport 保持播放并产生诊断事件；无法打开任何设备时 transport 置为 paused，报 `DeviceUnavailable`。

**延迟口径**：对外报告的输出延迟 = ring horizon + resampler 群延迟 + 设备 buffer + safety offset + `kAudioDevicePropertyLatency`，以 frame 和秒双表示；`doctor` CLI 必须能逐项解释该构成。

目标默认配置为 48 kHz / 128 frames / stereo；允许 44.1/96 kHz 和 64/256 frames。实时 deadline 是 `blockSize / sampleRate`，它约束 render worker 的单 block 耗时：目标 p99 < 70% deadline；HAL callback 的 copy 路径开销必须远低于此。Hard failure 是任何 ring underrun，或 worker 连续 3 次超过 deadline（预示即将 underrun，必须先产生 `PerformanceWarning` 诊断）。

## 线程模型与平滑播放

Phase 1 没有音频输入，延迟敏感点只有 transport 响应和参数变化，因此默认采用 render-ahead 架构换取对调度抖动的免疫：

- 专用 realtime render worker 线程把 graph 渲染进预分配的 SPSC ring buffer；CoreAudio HAL callback 只做 ring → output 的 copy，不做 DSP。worker 使用 time-constraint 线程策略（或由 CoreAudio workgroup 授予优先级），不与控制线程共享任何锁。
- `renderAheadBlocks` 默认 4（48 kHz/128 frames 下约 10.7 ms 安全垫），可配 2..16。只要平均 CPU 占用低于容量，单点调度抖动被 ring 吸收，不产生爆音。
- 控制延迟语义：transport/parameter 命令在 ring horizon 生效，`controlLatency ≈ renderAheadBlocks * blockSize / sampleRate + 设备输出延迟`；facade 暴露当前 horizon frame 供调用方对齐 UI。
- Underrun（ring 被掏空）：callback 输出静音、`xruns++`、transport 不停止；恢复后从当前 transport 位置继续，不追帧。每次 underrun 产生诊断事件，并归因到最近的高负载 node。
- 诊断计数与事件队列是独立采样：单次 snapshot 不保证新增 xrun 和对应事件同时可见；消费者/测试必须持续消费有界队列，不能在首次读到计数后停止等待事件。
- `latencyMode: 'direct'` 作为 engine option 回到在 HAL callback 内直接渲染，供未来 input 或超低延迟场景使用；同一 graph 在两种模式下输出必须 sample-accurate 一致（golden parity 测试）。
- Offline render 不经过 ring，直接驱动 block renderer。

模拟设备线程用于测试时，按名义 period 拉取 ring。线程调度落后时从当前时刻等待
  一个完整 period 再拉取，禁止连续补拉旧 period；测试机器的 consumer 延迟不能被
  转换成人造的 producer underrun。真实 HAL callback 仍由设备时钟驱动。

Worker 仅在设备重采样开启时保留 SRC 额外输出帧余量；相同采样率每次写入恰好
blockSize 帧，必须使用完整配置的 ring horizon，不能因额外余量损失一整块缓冲。

## CPU 尖峰防线

- 音频线程启动时设置 FTZ/DAZ；所有滤波器、混响和 delay 必须 denormal-safe，denormal 语料进入 DSP benchmark。
- Voice pool 在 prepare 阶段预分配并有编译期上限；超限按 quietest-then-oldest 策略 steal，steal 过程不分配内存。
- 每 block 更新一次 atomic `engineLoad`（block 渲染耗时 / deadline 的 EMA）；控制线程采样上抛，接近预算时先产生 `PerformanceWarning` 诊断，让调用方有机会降级（减 voice/效果），而不是等到 xrun。
- Phase 1 单线程执行整图；graph compiler 必须保留 DAG 分层拓扑信息，并行 executor 作为不改变契约的后续优化。是否引入并行由 benchmark 数据决定，不以假想负载先行。

## 错误与恢复

- `InvalidProject`：不建立图，附带 JSON path 和稳定 code。
- `AssetUnavailable`：保留图但相关 node 输出静音，meter/diagnostic 标记；用户可修复后重新 compile。
- `DeviceUnavailable`：transport 可保持 paused，offline render 不受影响。
- `RealtimeFault`：当前 block 静音、置 fault flag，控制线程收到后停止 transport 并提供可读上下文。
- 任何用户可修复错误都不能导致 Rust panic 或 Node 进程崩溃。

## Offline parity

Preview telemetry 为显式 opt-in：graph prepare 时按 Channel/Mixer bus 数创建固定容量
PCM ring、note echo queue 和 atomic meter。每段只复制音频、累加 peak/RMS、写 note
事件；满 ring 丢帧计数，不阻塞音频。UI 消费后执行加窗/FFT/XY 和 Master true-peak
分析；telemetry 不调用 JS。seek 增加 epoch 并清空 pending note queue，viewer 清除
旧高亮；换图获得新的独立 telemetry。默认关闭时不分配这些缓冲。

Offline renderer 使用同一 `RenderGraph`、event scheduler、automation evaluators 和 DSP implementations，只替换 output sink 和 clock。golden tests 比较固定 seed、sample rate、block size、transport 起点和 loop 策略下的 WAV hash/peak/RMS，并允许在不同 CPU 上配置极小浮点容差。必须单独测试 sine/cos phase、gate duty、chance seed/restart/absolute、tempo change 和 block-boundary continuity。为保证 block size 无关性，voice 在**非 legato 的（重）触发**（含 steal）时重置 oscillator 相位与滤波器 state——release 尾音结束后 voice slot 的释放发生在 segment（block）边界，残留的相位/滤波器记忆会随 block size 变化；包络电平保留（retrigger 从当前电平起音，避免爆音），legato/glide 路径不重置。
