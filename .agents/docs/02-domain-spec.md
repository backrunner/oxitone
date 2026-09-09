# Oxitone 领域规格

生成式 Pattern 的新实现契约见 [15-source-authoring.md](15-source-authoring.md)：保留 chord/
arp 规则、输出坐标与不可变局部 edit，并新增 concat/repeat/slice/transpose/velocity。
完整编排、typed plugin instance 与 automation range 目标见 [统一设计](../designs/source-daw/01-authoring.md)。

本文定义 TypeScript authoring API 的语义。字段名称是建议的 Phase 1 契约；具体实现可以拆成多个文件，但不能改变单位和边界规则。

## Project 与时间轴

`Project` 是渲染和导出的根。它拥有唯一 `id`、`name`、`sampleRate`（默认 48_000）、`blockSize`（默认 128）、`tempoMap`、`timeSignatureMap`、markers、tracks、channels、mixer 和 automation lanes。

`Project.fromSnapshot`/`Project.load` 恢复可编辑工程，保持 stable ID、有理数时序、音符
顺序和原 revision；加载本身不视为 authoring mutation。`revisionBigInt` 覆盖 wire u64
范围，后续每次成功修改递增一次，耗尽时拒绝修改；API 与目录资源语义见 04/06。

- `Project.tempoMap` 是全局权威时钟，支持 beat 级变速：每个 segment 为 `{ startBeat, bpm, curve }`，`curve` 描述该 segment 到下一个 segment 的过渡方式——`step`（阶跃，默认）、`linear`（BPM 线性斜坡）、`exponential`（乘性斜坡，用于听觉均匀的 rit./accel.）。三种 curve 都有 beat↔秒的闭式积分与逆变换（公式见 `03-audio-runtime-spec.md`）。BPM 有效范围为 20..999 的有限数；segment 按 startBeat 严格递增，首个 segment 必须位于 beat 0。time signature 变化仍只允许在 bar 边界。
- Track 的 `tempo` 是独立预览/编译上下文的静态 convenience override，不支持变速，也不会隐式改变同一 Project 其他 Track。Track tempo 会把该 Track 的相对 bar/beat 编排先换算到 Project beats，只有显式 `tempoMap` 才改变 Project 全局时钟。
- 全局 tempo 是可自动化 target：lane 绑定 `target: { entityId: <projectId>, parameterId: 'tempo' }`，输出的 0..1 系数按 log 映射到 20..999 BPM。规则：
  - 至多一条 tempo lane；多余 lane 或 combine 冲突报 `TempoAutomationConflict`。
  - 存在 tempo lane 时它取代 tempoMap 成为有效时钟；不存在时使用 tempoMap。
  - tempo lane 的 source 必须 transport-invariant：允许 `constant`/`curve`/`polyline`/`line`/`gate`/`wave` 及其组合；禁止 `chance` 和一切依赖 play/loop 状态的 `restart` 语义，违反报 `AutomationTempoRestriction`。这保证有效 `bpm(t)` 是 beat 的确定性函数，compiler 能在编译期烘焙整条 beat→frame 映射（见 `03-audio-runtime-spec.md`），调度保持 sample-accurate。
  - tempo lane 可 `loop`/`lastBeat`，规则与其他 lane 一致；烘焙范围覆盖 Project timeline 全长加 tail。
- bar 从 1 开始，beat offset 从 0 开始。时间签名 `n/d` 中 `d` 必须是 1、2、4、8、16 等 2 的幂。
- 一切位置和长度都用 `Beat`（有限、非负、有理数语义）；API 可以接受 number，但序列化时必须规范化为 decimal string 或 `{numerator, denominator}`，避免浮点漂移。
- Project timeline 的结束点是所有 clips、samples、tails、automation 的最大结束点并向上取整到 bar；可以显式提供 `lengthBars`，但不能截断未声明的内容。
- `markers` 是命名的 beat 位置，用于 play 起点、render range 和 loop region 的引用与编排导航；不参与音频语义，MIDI 导出时写入 conductor track。

## Track、Pattern、PatternClip

编译后的 Session 保留独立快照：authoring 修改通过 `session.update()` 换入，失败时
保留旧图。Project.play 自动提交新 revision；Session 直接 play/export 则使用当前编译
版本。bar+beat/marker 位置按该编译版本解析，seconds/frame 在 Rust  transport 边界转换。

Track 是编排容器，不直接产生声音，允许 Pattern、Sample、Automation 重叠排布。Engine 1.2 的 Pattern.parts 保存各 Channel 的独立 leaf Pattern，调度按 Pattern 自身路由；旧 leaf Pattern 与 SampleClip 继续采用 Track.channelIds。一个 PatternClip 只能属于一个 Track。parts 长度、注册/恢复和编辑约束见 [18](18-project-daw.md)。

TS Track 已暴露 `enabled`（默认 true）、`mute`/`solo`（默认 false）和 `midiChannel`（可选的 1..16 整数）；setter
先校验再增加 revision。disabled Track 保留编排数据，但不参与音频调度和 MIDI note track
分配。Track M/S 独立于共享 Channel，过滤本 Track 的所有 placements，保持时间线长度；
优先级及播放/导出策略见 [23](23-daw-controls.md)。`use(channel)` 只接受同 Project 的 Channel 对象。Track `tempo` 支持 20..999
静态 BPM，赋 undefined 恢复 Project 时钟。局部拍位以项目时间零点为锚点，先按
`seconds = localBeat * 60 / trackTempo` 换算，再用 Rust 有效时钟逆变换为 Project beat。
bar/beat、duration、loop/last 均先在局部编排域解释，保留原 clip ID 和概率 seed。
time signature 仍取 Project 的小节表；映射后的 swing 和全部 automation 使用 Project beats。
SampleClip 的 off 窗口、repitch 速率和 stretch 比例均使用局部静态时钟；效果器和
音源的 beat-synced 参数仍使用共享 Project 时钟（同一 Channel 可以绑定多个 Track）。
`fitToContent` 无音乐长度时按 Track BPM 折算局部长度。

```ts
track.pattern(pattern).at({ bar: 1, beat: 0 }).loop(4).last({ bar: 17 })
```

- `Pattern` 是不可变 Note/automation 片段，长度 `lengthBeats > 0`。
- `PatternClip` 有 `startBeat`、可选 `durationBeats`、`loopCount` 或 `lastBeat`，`loopCount` 与 `lastBeat` 不能同时指定。
- `lastBeat` 是 exclusive end；循环在每个 pattern 长度边界重新计算，超出 `lastBeat` 的 Note 被裁掉。
- DAW 移动片段同步平移 `lastBeat`，同 Track 不改变 membership 顺序；复制保留全部 placement
  选项并创建新实例。右边缘长度与 Sample fit 的事务及限制见 [23](23-daw-controls.md)。
- Pattern 内 Note 的 `start` 可以为 0，但不能为负；`duration > 0`。同一时间的 Note 使用稳定排序：start、channel voice hint、创建序号。
- Clip 允许 transpose（半音整数）、velocity scale（0..2）、probability（0..1，使用项目 seed 确定性采样）；Phase 1 不随机改变 MIDI export，export 必须使用 seed 固定结果。PatternClip 和 SampleClip 都有 `enabled`（默认 true），`false` 时不参与调度但保留数据，用于编排中的快速 mute/unmute。

## Note 与编排高阶函数

Note 最小字段为 `pitch`（MIDI 0..127 或明确的频率模式，Phase 1 默认 MIDI pitch）、`start`、`duration`、`velocity`（0..1），以及可选 `chance`、`offVelocity`、`voice`、`tags`。Note 不包含音色；音色由 Channel/Instrument 决定。

高阶函数是纯函数，输入不可变，输出新的 Pattern 或 Note 数组：

- `Chord(root, quality, options)`：支持 major/minor/dim/aug/sus、inversion、voicing、duration、velocity。
- `Arp(notes, order, rate, options)`：支持 up/down/upDown/random（random 需 seed）、gate、octaves、velocity curve。
- `Euclidean(pulses, steps, rotation)`、`Humanize(seed, timing, velocity)`、`Quantize(grid, strength)` 和 `Strum(spreadBeats, order)` 可作为后续 helpers；不得在 Rust callback 执行。

## Instrument 与 Channel

Channel、MixerChannel 和 Master 的有序 insert 均暴露 `insert.<index>.mix`、
`insert.<index>.bypass` 与 `insert.<index>.parameter.<pluginParameterId>`。
初始参数先应用，随后同帧 host 参数按入队顺序应用，最后由该帧求值的 automation
覆盖。beat-unit effect 参数保留拍数，在调度路径按有效 BPM 换算为 seconds；
索引与参数描述在 compile 期解析，实时端只使用整数索引和预分配事件缓冲。

`Channel` 是声音生成和效果链的宿主。每个 Channel 绑定一个 `Instrument`，可绑定多个 insert effects；它有 `level`（线性 gain，0..2）、`pan`（-1..1）、`swing`（0..1，默认 0）、`mute`、`solo`、`mixerChannelId`。

标准音源必须声明：`id`、`version`、单/多声道布局、最大 polyphony、参数 specs（stable parameter ID、unit、range、default、smoothing）、是否接受 MIDI note/automation，以及 tail 行为。处理链按声明顺序执行，instrument 输出先经过 inserts 再发送到 mixer bus。

内置 `WavetableSynth`：每个 voice 至少有 OSC A/B、wavetable ID、unison/detune、oscillator mix、pan、filter type/cutoff/resonance、amp ADSR、可选 filter envelope、voice mode（poly/mono/legato）和 glide。Wavetable 读取和 mip level 选择在 prepare 完成，process 只读预分配表。

增量音色能力保持 `oxitone.wavetable@1.0.0` 的旧参数及默认声音：新增 Organ/Glass 两种谐波波形；
每 OSC 的 morphTo + position 把源波形与目标波形在同相位的 mip 表之间线性渐变，position 默认 0。
phase / phaseSpread（0…1 cycles，默认 0）在新声部 note-on 时设置各 unison 相位，legato 保持相位。
OSC A/B 支持独立 octave −4…4（默认 0，与 pitch 半音相加）和 level 0…1（默认 1）。
Sub 的 wave 为 sine/triangle/saw/square/pulse/rounded，默认 sine；level 默认 0，
octave −4…4（默认 −1），在滤波后混入并共用 amp envelope；Sub 不跟随 A/B 的 octave。
Noise 为确定性白噪声，level 默认 0，在滤波前混入。每次非 legato note-on 重置 noise/LFO/sub。
每声部 LFO 支持 sine/triangle/ramp/square、0.01…30 Hz、初相位，以及 pitch（±12 st）、cutoff（±48 st）、
OSC A/B position（±1）、amp level（0…1）五个固定目标。所有调制深度默认 0；相位以 f64 逐 sample 积分。
该 LFO 随音符触发，Hz 为物理速率；工程节拍同步可用 bpm/60/periodBeats 明确换算，tempo map 自动跟随
仍使用既有工程 automation。它不是新的 automation source，不改变 07 的 golden vectors。

追加的波表库、warp、FM/ring、第二 LFO、曲线包络和八槽调制矩阵，以及 distortion、
multiband、delay 与电子鼓参数遵循 `13-electronic-production.md`；A/B unison 为 1…16。

内置 `Sampler`：把 Sample 映射为可用 Note 演奏的音源。note pitch 相对 `rootKey`（默认 60）决定 playback rate（varispeed 变调），velocity 按可调灵敏度映射到 level，带 amp ADSR、loop 模式（off/forward）和切片起点；复用 sample 解码/编辑链与 varispeed 路径，其播放参数同样可自动化。v1 为单采样；velocity layer、round-robin、键盘分区等多采样能力留待 descriptor 版本演进。

新增独立 `oxitone.multisampler@1.0.0`，保持旧 Sampler 语义：1…256 个互不重叠的键盘/力度区域，
每区 resource key、rootKey、闭区间 keyRange 0…127 / velocityRange 1…127、gain 0…4。
力度索引为 round(clamp(velocity,0,1)×127)，夹到 1…127；空白区域静音，重叠/倒置区域拒绝。
原始 velocity 仍用于灵敏度增益。prepare 建立 128×128 u16 查表，32 个预分配声部各保存区域索引；
process/reset 无分配、锁或 Arc 复制。播放音高差为 note - region.rootKey + transpose（±48 半音），其余 ADSR/loop/start/
velocitySensitivity/level/pan 与 Sampler 一致；rootKey 不再是全局参数。暂不支持 round-robin 或踏板事件。

内置 `Slicer`：把 Sample 切成 slice 并映射到连续 note 上演奏（Slicex/Fruity Slicer 类）。

- slice 定义在编辑后的内容上，来源三种：显式 marker 列表（frame 或 beat 位置）、`grid: n` 等分、`onset` 瞬态检测。onset 检测算法必须确定性并版本化（`onsetAlgorithm`），与 stretch 同一规则：改进算法用新 id。
- `triggerNote`（默认 60）触发 slice 0，向上每半音一个 slice；超出 slice 数的 note 不发声。`playMode: 'oneshot'|'gate'`：oneshot 忽略 note-off 播到 slice 末尾，gate 由 note-off 触发 ADSR release。
- 每个 slice 可声明可选覆盖：`level`、`pan`、`rate`、`reverse`（reverse 由 player 倒放实现，不产生新资产）。
- `tempoSync` 支持 `off` 和 `repitch`（语义与 SampleClip 相同）；v1 不支持逐 slice 保调 stretch——slice 边界随 tempo 变化的重拉伸需要 per-slice WSOLA，作为算法演进候选。切片型 loop 需要跟 tempo 时推荐用 `repitch`。
- slice 表是插件的结构化 state（见 `04-api-contracts.md` 的 `InstrumentRef.state`），compile 期定稿，播放中不可变；改 slice 需要重新 compile 该节点。

`@oxitone/core` 提供 `wavetable(options?)`、`sampler(sample, options?)` 与
`slicer(sample, options)`、`multisampler(regions, options?)`，返回可序列化 InstrumentRef，供 `addChannel({instrument})`
或 `channel.instrument = ref` 使用。它们不持有 PCM 或 native instance。选项省略时保留
Rust descriptor 默认值；非法选项报 `InvalidProject`。资源 ID 必须属于目标工程。

Slicer `state.tempoSync` 缺省为 off。repitch 的原生 BPM 为编辑后内容的
`60 * musicalLengthBeats / durationSeconds`；未声明音乐长度时固定使用编译后有效
Project tempo 在 beat 0 的 BPM。每个处理段起点用有效 Project BPM（含 tempo lane）
除以原生 BPM，更新已经发声和新触发的 slice。Channel 可被多个 Track 共享，因此
Track 静态 tempo 只控制其音符时间，不改变 Channel 的音源时钟。
tempo 因子支持 0.25..4，完整有效 tempo map 超出此范围时报 `InvalidProject`，
不静默钳制；与 slice rate 相乘后沿用 voice 的 0.125..16 安全范围和平滑。
`tempoFactor` 是宿主保留参数，不能在快照参数表、automation 或 setParameter 中赋值。

## Sample

`Sample` 是可复用的音频资产引用和非破坏性编辑描述，而非内存中的 PCM。它包括 `assetUri`、content hash、原始格式、声道数、sampleRate、帧数和编辑链：

- `startFrame`/`endFrame`（半开区间）、`level`（线性）、`tone`（-1..1 的受限 tilt 参数）。
- `normalize`（目标 peak，默认 -1 dBFS）、`fadeIn`、`fadeOut`、`crossfade`（帧数/曲线）。
- 可选元数据 `musicalLengthBeats`：声明素材的原始音乐长度（如一段 loop 原生为 2 bars），是 fit/stretch 计算的基准；未声明时按 clip 位置的项目 tempo 折算。
- `reverse` 属于后续可选操作；拉伸与变调以 SampleClip 的 `tempoSync`/`rate` 机制在 Phase 1 支持（见下），更高质量的离线拉伸算法作为后续增强。
- SampleClip 放在 Track 时间轴上，支持 one-shot、loop、切片起点、gain/pan、playback rate；loop 边界必须做 crossfade 或 zero-crossing 策略。同一 Track 上重叠的 SampleClip 默认相加叠混，需要交叉淡化时在 clip 边界显式声明 fade。

### Sample 的 tempo 跟随（tempoSync）

SampleClip 的内容长度在秒域，编排在 beat 域；`tempoSync` 定义全局 tempo（含 tempoMap 变速和 tempo lane）变化时二者的对齐方式：

- `off`（默认）：内容按原始速率播放，tempo 变化不影响声音；clip 的 beat 长度只决定编排范围，内容超出则截断、不足则静音或按 `loop` 重复。
- `stretch`：内容经保调 time-stretch，使 `musicalLengthBeats`（或显式 `durationBeats`）精确填满 clip 的 beat 长度；tempo 变化时 beat 域编排不变，秒域自适应，音高不变。拉伸算法版本化为 `stretchAlgorithm`，默认 `wsola-v1`；替换或改进算法必须用新 id 或 protocol major bump，不得在 minor 版本里改变既有渲染结果。
- `repitch`：用 playback rate 跟随 tempo（变速变调，tape 式）；有效 rate = tempo 因子 × clip 的 `rate` 参数，`rate` automation 在其上叠乘。

### 长度设置与 bar 对齐

Authoring 层提供纯函数 helper，把 clip 长度对齐到音乐尺度（允许分数值）：

```ts
clip.fitBars(2)        // 长度 = clip 位置 time signature 下的 2 个 bar
clip.fitBars(0.5)      // 1/2 bar
clip.fitBeats(3.5)
clip.fitToContent()    // 反向：按内容原始长度折算 beat 长度
```

- `bars → beats` 的换算使用 clip startBeat 处的 time signature；跨 time signature 变化的长 clip 按 beat 累计，不按 bar 数线性外推。
- fit 只设置 beat 域长度；音频行为由 `tempoSync` 决定：`stretch`/`repitch` 时内容缩放到该长度，`off` 时截断或 loop。
- helper 在 TS 端计算并写入快照，Rust 侧只看到最终的 `durationBeats` 与 `tempoSync`，不重复实现换算逻辑。

WAV/AIFF 可以直接解码。MP3（以及包含音频轨的 MP4/M4A）在导入/prepare 阶段转换成缓存 WAV/PCM，并记录原始 hash 与 decoder 版本；实时 callback 不读压缩数据。

TS authoring 提供 `project.addSample(options)` 和 `track.sample(sample).at(position, options)`。
Sample 只保存资源 URI/hash/格式/帧数/编辑描述；SampleClip 保存 beat 位置、gain/pan/rate、
`tempoSync`、loop 和 enabled。`fitBeats`、`fitBars`、`fitToContent` 只写入 beat-domain
`durationBeats`；Rust 仍负责资源 hash 验证、解码、编辑烘焙、SRC 和实时播放。

Sample frames 接受正 safe-integer number 或正 u64 bigint；`options.id` 可指定稳定 ID，
已占用 ID 报 `InvalidProject`。trim 必须满足 `0 <= startFrame < endFrame <= frames`，
`musicalLengthBeats` 和显式 clip duration 必须大于 0。失败的添加/放置不注册实体、不增加
revision，SampleClip draft 可重试；ID 生成器可消耗序号。`fitBars` 始终使用起始拍号，
不因 clip 的 beat offset 缩短整小节长度。`fitToContent` 优先使用声明的音乐长度，否则
使用 trim 后帧数，并通过 Rust 有效时钟计算
`endBeat = secondsToBeat(beatToSeconds(startBeat) + contentSeconds)`。该同步只读查询
覆盖 step/linear/exponential tempo map，以及替代它的 tempo lane（含 loop/lastBeat），
不读取音频资产、不创建 engine。音乐长度已声明时仍优先使用该长度。

文件导入入口为 `@oxitone/samples` 的同步 `importSample(path, { assetBaseDir?, cacheDir? })`。
它通过版本化 native 命令让 Rust 读取、识别和完整解码源文件，返回可传给 `addSample`
的 descriptor（frames 为 bigint），附带随 SampleRef 持久化的 source/decoder provenance。
默认只读导入；指定 cacheDir 时发布内容寻址的 float32 WAV（详见 `06-format-and-export.md`）。
默认 URI 是绝对本地路径；指定 base 时输入相对该目录解析，并返回目录内的相对 URI。
导入不创建 engine、不打开音频设备，PCM 不跨 native 边界；prepare 校验实际播放资产的
hash 并解码。缓存保留原采样率和有效 loop，不烘焙 Sample edits。

## Mixer 与 routing

TS authoring 已提供 `project.master`、`project.addMixerChannel(options)`、
`channel.mixerChannelId` 路由 setter 和 `bus.send(destination, options)`。
Master 与其他 bus 的 level/balance/mute/solo/inserts 可编辑；Channel 暴露
effectChain/swing/mute/solo。所有 setter 先校验，再替换内部值并增加 revision；
输入的嵌套参数、getter 和快照均与内部状态隔离。send 对同一 destination 为替换语义，
跨 Project 对象与 Master 发送立即报 `InvalidProject`，完整 audio/detector 环在 Rust
compile 阶段报 `InvalidProject` 并提供 cycle path。

Mixer 总是包含不可删除的 `Master`。MixerChannel 有输入布局、insert chain、level、balance（-1..1）、mute/solo、meter 和 sends。Send 字段：`source`、`destination`、`ratio`（0..1）、`preFader`、可选 `sidechain` 标记。

- 每个 MixerChannel 有独立的 `masterSendRatio`（默认 1，post-fader）控制发送到 Master 的音量；到 Master 的路由不占用 `sends`，`sends` 的 destination 不得为 Master。
- `sends` 的 destination 是其他 MixerChannel；`sidechain: true` 的 send 只驱动 destination insert chain 中声明 sidechain 输入的效果器 detector，不进入该 bus 的音频求和。
- 路由图必须是 DAG；sidechain detector 边也参与拓扑排序，禁止任何音频或 detector 反馈环。编译器用拓扑排序并报告完整 cycle。
- Master 不能作为 send 的 source；Master 输出不能再次路由。
- `ratio` 是线性振幅系数，panning 使用 equal-power law。Level automation 在 fader 前后按 send 的 pre/post 语义生效。
- Solo 是监听策略，不改变导出图；导出时默认忽略 solo，除非 render options 显式 `respectSolo: true`。

Phase 1 内置效果器：`EQ`（至少 4 biquad bands）、`Limit`、`Clipper`、`Filter`、`Phaser`、`Reverb`、`Compressor`（必须支持 sidechain detector 输入）、`Delay`（beat-synced time、feedback、feedback 路径 filter）、`Gate`、`Chorus`、`Saturator`、`Utility`（gain/width/polarity/mono）。每个效果器共享标准 parameter/tail/bypass contract；效果器可以声明 sidechain 输入。

电子制作扩展在 `14-effects-production.md`：新增非线性滤波、Compactor、Multiband
Dynamics、Resonator、频移、移调、Flanger、Convolver、Bitcrush、Tape、Spreader 和
4x mastering Limiter。总线无侧链路由时使用自身检测器；已连接的静音侧链仍是外部输入。
Convolver 通过 EffectRef.resources.impulse 绑定工程 SampleRef，编译期解码/重采样和
FFT 分区，IR 变化走完整换图。界面、API 与预设只使用自有的描述性名称。

每个 insert 节点（Channel 和 MixerChannel 上的 EffectRef）除插件自身参数外，还暴露内建 `mix`（dry/wet 0..1，默认 1，并联处理）和 `bypass` 参数，均可自动化。时间类效果参数可声明 `unit: 'beats'`（如 Delay 的 time），引擎经 tempo map 换算为帧数，tempo 变化（含变速）自动跟随。

任何音源/效果器都可以在 descriptor 上报 `latencyFrames`（lookahead limiter、线性相位 EQ、oversampling 级等）；compiler 对汇聚到同一 bus 的各路径自动插入补偿 delay，按最长路径做 plugin delay compensation，sidechain detector 路径同样对齐。PDC 只移动音频落点，不改变 automation 的 beat 域定义；图内部总延迟在 render report 和诊断中列出。

## Automation

Automation lane 绑定一个 `target`（entity ID + stable parameter ID），输出 0..1 系数。它可以由控制点、内置函数或函数组合构成；所有高阶函数在 TypeScript 端只生成可序列化的 source AST，不能把 JS 闭包交给 Rust 或在 audio callback 中解释 JavaScript。

**可自动化实体与内建参数**：插件（Instrument/Effect）通过 descriptor 声明参数；内建图节点由引擎按同一 `ParameterSpec` schema 暴露固定参数集，二者走完全相同的映射、平滑和调度机制：

- `Project`：`tempo`。
- `Channel`：`level`（0..2 linear）、`pan`（-1..1 bipolar）、`mute`（0|1 step）。
- `MixerChannel`：`level`、`balance`（-1..1）、`mute`、`masterSendRatio`（0..1）、`send.<destinationId>.ratio`（0..1）。
- `SampleClip`：`level`（0..2）、`tone`（-1..1）、`gain`（0..2）、`pan`（-1..1）、`rate`（0.25..4，log）。

边界规则：Sample 的结构化编辑（`startFrame`/`endFrame`/`normalize`/fade 长度）在 prepare 阶段烘焙进 PCM segment，**不可自动化**；`tone`（运行时 tilt filter）、`level`、`rate`（varispeed）是播放期 DSP 参数，可自动化。绑定不存在的参数或未声明 `automation: true` 的参数，validator 报 `AutomationTargetInvalid` 并附 target path。

完整公式、随机性、边界、错误码和测试向量见 `07-automation-spec.md`；本文只定义公共领域语义。

### 控制点和基本曲线

点为 `{beat, value, curve}`；value 超出 0..1 在 validation 阶段拒绝。支持曲线：step、linear、smooth、exponential（要求正值域）、bezier（控制点有限且可序列化）。`polyline(points)` 是多段折线的快捷构造，连续段使用每段自己的 interpolation；`line(from, to, duration)` 是单段线性 ramp。

### 函数型 automation

所有函数的时间输入都是 Project beat，输出都被限定为 0..1。函数返回 immutable `AutomationSource`，可以通过 `map`、`clamp`、`invert`、`scale`、`offset`、`mix`、`add`、`multiply`、`min`、`max` 和 `quantize` 组合；组合树必须有深度和节点数上限（默认 64/256），避免失控的实时成本。除 `map` 的目标区间和显式 `scale/offset` 中间结果外，绑定到 lane 的最终 source 必须 clamp 到 0..1。

- `gate(options)`：周期矩形门。`periodBeats > 0`、`duty` 在 0..1、`phase` 为 beat offset；在每个周期的半开区间 `[phase, phase + duty * period)` 输出 on，其余区间输出 off。on/off 默认 1/0，也可提供 0..1 的值。`periodBeats` 可以写成 `1 / subdivisions` 的 beat 分辨率，因此 gate 能和 bar/beat 编排对齐；`duty: 0` 永远 off，`duty: 1` 永远 on。
- `chance(options)`：确定性的随机采样保持源，不是每个 sample 重新抛硬件随机数。`rate`/`frequency` 是每 beat 的决策次数，或改用 `intervalBeats` 指定决策间隔；三者必须且只能提供一个，`frequency` 只是面向用户的 `rate` 别名。每个决策按 `probability` 输出 1 或 0，结果保持到下一次决策；可选 `smoothBeats` 将跳变插值。每个 lane 必须有显式 `seed`，实际种子由 `project.seed + lane.seed + sourcePath` 派生，Phase 1 固定使用 versioned `pcg32`。相同 snapshot、seed、transport 起点和 loop 次数必须产生相同结果；seek/loop 按定义的 `randomPhase: 'absolute'|'restart'` 重置或继续序列。频率/间隔必须为有限正数；`probability` 必须在 0..1。
- `wave(kind, options)`：周期波形源。`kind` 支持 `sine`、`cos`、`triangle`、`saw`、`ramp`、`square`；`periodBeats > 0`、`phase`、`min`/`max`（默认 0/1）和 `pulseWidth`（square，0..1）可配置。内部先生成 -1..1，再按 min/max 映射并 clamp 到 0..1；要求 `min <= max`。`phase` 按 beat offset 解释，增加一个完整 `periodBeats` 后结果相同。
- `sine(options)` 与 `cos(options)` 是 `wave('sine'|'cos', options)` 的类型安全别名；另提供 `triangle`、`saw`、`ramp`、`square` 别名。sine 在 phase=0 时从中点上升，cos 在 phase=0 时从 1 开始，文档和 golden tests 固定该约定。
- `curve(points, interpolation)`：控制点曲线的显式构造；`interpolation` 为 step/linear/smooth/exponential/bezier。`bezier` 控制点必须位于相邻 beat 区间内，禁止产生未定义或 NaN 输出。

函数选项的建议 TS 形状：

```ts
const cutoff = automation.sine({ periodBeats: 8, phase: 0, min: 0.2, max: 0.9 });
const rhythmicGate = automation.gate({ periodBeats: 0.5, duty: 0.5 });
const humanChance = automation.chance({ frequency: 2, probability: 0.72, seed: 17, smoothBeats: 0.04 });
channel.automate('filter.cutoff', automation.map(cutoff, { min: 0.2, max: 0.9 }));
```

`map` 的区间必须位于 0..1，物理范围映射由 target parameter spec 再次完成；source 仍只输出 0..1。`chance` 适合 gate、mute、trigger-like 参数和稀疏事件；对音高、连续滤波器等参数应显式使用 `smoothBeats` 或其它平滑组合，避免非预期 zipper noise。

- lane 可以 `loop`，也可以有 exclusive `lastBeat`；边界规则与 PatternClip 一致。
- 参数 spec 把 0..1 映射到物理值（linear/log/bipolar/enumerated）。
- 编译时把 automation source AST 校验并编译成 Rust evaluator；内置函数不经过 N-API，也不在 callback 中分配。automation 采样为每 block 的 segment；需要 audio-rate 的参数由 DSP 在 block 内逐样本插值，需要 control-rate 的参数只在 block 边界更新。`chance` 的 PRNG 状态和 wave phase 都是预分配的 evaluator state。
- 多个 lane 指向同一 target 时必须声明 `combine: replace|add|multiply|max`，默认 `replace` 且同优先级冲突报错。
- source 在 beat 域计算，tempo change 只改变 beat 到 sample-frame 的映射，不改变同一 source 的相位/随机序列定义。`absolute` random phase 使用 Project beat 作为索引，`restart` 在每次 play/loop 起点重新播种。
- 组合器语义固定：`invert(x) = 1 - x`；`clamp` 使用给定 0..1 边界；`scale` 和 `offset` 对中间值执行乘法/加法并在 lane 输出前 clamp；`mix(a,b,t)` 为 `(1-t)*a + t*b`，t 默认 0.5；`quantize(x,n)` 映射到 n 个等距阶梯，n 必须是大于 1 的整数。

## Playback 与 Export 语义

Transport 状态：`stopped|playing|paused|rendering`，游标为 sample frame 和 beat 双表示但 sample frame 是 Rust 权威。`play({ from: { bar|beat|timecode|marker }, loop })` 在下一个安全 block 边界开始；seek 会 flush voices、delay tails（可选保留 reverb tail）并发送 acknowledgement。实时预览默认经 render-ahead ring 输出，transport/参数命令存在 horizon 控制延迟（语义与配置见 `03-audio-runtime-spec.md`），但事件落点仍然 sample-accurate。

引擎内置 Rust 合成的 metronome click（engine option，无需音频资产，accent 规则来自 time signature map），在最终 output limiter 之前混入输出；默认不进入 WAV export，render option 可显式包含。`renderWav` 支持 stem 导出（按 mixer channel 或 track 分文件）和 loudness 报告，规则见 `06-format-and-export.md`。

Offline render 使用相同 RenderGraph 和 block renderer，显式给出 `sampleRate`、`blockSize`、`start`、`end`、`tailSeconds`、`respectSolo`、`seed`，写标准 PCM WAV（16/24/32-bit float）。实时和离线必须共享 DSP 代码路径。

MIDI export 使用 SMF Type 1：tempo/time-signature 在 conductor track，所有 Track 按 channel/track 输出，Note velocity 映射 1..127，automation 只对声明 MIDI CC 的参数导出。16 channel 上限与显式 `midiChannel` 分配规则见 `06-format-and-export.md`，超限导出报 `MidiChannelLimit`。禁止 `importMidi`；未来 converter 另立包。
