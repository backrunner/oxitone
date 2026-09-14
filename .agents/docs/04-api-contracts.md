# Oxitone TypeScript / Rust API 契约

`ProjectDocument`、Document control 2.0、完整工程 GPUI 音符编辑和静态插件目录的增量
契约见 [18-project-daw.md](18-project-daw.md)，该 control 版本独立于 Engine snapshot 1.2。
实例参数与效果器排序 API 见 [20-plugin-instances.md](20-plugin-instances.md)，插件 ABI 保持 1。
`Project.configure`、扩展的 `Project.arrange`、`withPluginRegistration` 与 Document
`assignPlugin` 的行为、范围、错误和回写约束见 [23-daw-controls.md](23-daw-controls.md)。
Engine 1.2 的 TrackSpec 新增可选 mute/solo boolean；Track configure 接受 enabled/mute/solo
中至少一个字段，未提供字段保持原值。低于 1.2 的快照携带这些字段时拒绝，不静默忽略。

发布前已移除单 Pattern 文档/evaluator，源码编辑只使用 ProjectDocument；DocumentView 必须
提供 projectRoot，请求身份只接受单调 stream。ParameterSmoothing 的 one-pole 拼写在 TS/Rust
完全一致，不再接受 onePole 内部别名。未发布旧格式/旧接口不要求提供兼容 shim。

新增独立 authoring `PatternSourceDocument` format 1 与 Pattern edit/组合 API，精确定义见
[15-source-authoring.md](15-source-authoring.md)。引擎 1.2 同时支持 leaf PatternSpec 与引用独立 Channel leaf 的 parts，详见 [18](18-project-daw.md)；这不等于
已支持 wire 2、持久随机 origin 或完整 AuthoringDocument。发布前已授权按
[统一设计](../designs/source-daw/README.md) 重定义 API/协议/ABI，无需保留旧 major 兼容层。

新增 `@oxitone/web`：复用本文件 ProjectSnapshot 与 authoring 合约，通过独立 ABI v1
提供 WasmEngine 和 WebAudioSession。内存资产、帧游标、WAV/MIDI bytes、生命周期、
浏览器宿主约束和错误边界见 `12-wasm-web-audio.md` 与 `docs/web.md`；N-API 的
路径/设备方法保持原生语义，不做隐式浏览器降级。

以下是 Phase 1 的公共形状。实现时建议用 zod/JSON schema 或等价的运行时校验生成 Rust 类型；手写类型必须与 schema 同步。

`EngineOptions.audioBackend?: 'device' | 'simulated'` 默认为 device；simulated 使用现有
Rust realtime worker/ring/sink，按 graph 的采样率和 blockSize 消费 PCM，不打开 CoreAudio。
simulated 的设备 latency/safety offset 为 0，outputDeviceId 不使用。原生 createEngine
response 回显 audioBackend；TS facade 请求 simulated 时必须收到确认，否则在播放前释放
引擎并报 ProtocolVersionUnsupported（防止旧 addon 忽略新字段后误开系统设备）。
所有自动测试必须选模拟/无设备 sink；真实扬声器输出不作为自动测试环节。

## 基础类型

```ts
export type EntityId = string;
export type Beat = number; // finite, >= 0 at authoring edge; canonicalized on wire
export type Pitch = number; // integer 0..127 in Phase 1
export type Timecode = { seconds: number } | { frames: bigint };

export interface ParameterSpec {
  id: string; label: string; unit: 'normalized'|'db'|'hz'|'semitones'|'seconds'|'beats'|'enum';
  min: number; max: number; default: number; smoothing: 'none'|'linear'|'one-pole';
  rate: 'control'|'audio'; automation?: boolean;
  mapping?: 'linear'|'log'|'bipolar'|'enum'; // 0..1 → 物理值的映射律；tempo 使用 'log'
}
// unit 'beats': 以 beat 为单位的时间参数（如 Delay.time），引擎随 tempo map 换算，变速自动跟随
export interface AutomationPoint { beat: Beat; value: number; curve?: Curve; }
export type Curve = { kind: 'step'|'linear'|'smooth'|'exponential' } |
  { kind: 'bezier'; out: [number, number]; in: [number, number] };

export type WaveKind = 'sine'|'cos'|'triangle'|'saw'|'ramp'|'square';
export interface WaveSourceSpec {
  kind: 'wave'; wave: WaveKind; periodBeats: Beat; phase?: Beat;
  min?: number; max?: number; pulseWidth?: number;
}
export interface GateSourceSpec {
  kind: 'gate'; periodBeats: Beat; duty: number; phase?: Beat;
  on?: number; off?: number;
}
export interface ChanceCommonSpec {
  kind: 'chance'; probability: number; seed: number;
  smoothBeats?: Beat; randomPhase?: 'absolute'|'restart';
}
export type ChanceSourceSpec = ChanceCommonSpec &
  ({ rate: number; intervalBeats?: never } | { rate?: never; intervalBeats: Beat });
export interface PolylineSourceSpec {
  kind: 'curve'; interpolation: Curve['kind']; points: AutomationPoint[];
}
export type AutomationSourceSpec =
  | { kind: 'constant'; value: number }
  | PolylineSourceSpec
  | GateSourceSpec
  | ChanceSourceSpec
  | WaveSourceSpec
  | { kind: 'map'; input: AutomationSourceSpec; min: number; max: number }
  | { kind: 'unary'; op: 'clamp'|'invert'|'quantize'|'scale'|'offset'; input: AutomationSourceSpec; steps?: number; amount?: number; min?: number; max?: number }
  | { kind: 'binary'; op: 'mix'|'add'|'multiply'|'min'|'max'; left: AutomationSourceSpec; right: AutomationSourceSpec; amount?: number };

/** Opaque authoring value; serializes to AutomationSourceSpec during compile. */
export interface AutomationSource { readonly __automationSource: unique symbol; }
export interface GateOptions {
  periodBeats: Beat; duty: number; phase?: Beat; on?: number; off?: number;
}
/** Authoring-only options; frequency is normalized to rate in the wire source. */
export type ChanceOptions = {
  probability: number; seed: number; smoothBeats?: Beat; randomPhase?: 'absolute'|'restart';
} & ({ rate: number; frequency?: never; intervalBeats?: never } |
     { rate?: never; frequency: number; intervalBeats?: never } |
     { rate?: never; frequency?: never; intervalBeats: Beat });
export interface WaveOptions {
  periodBeats: Beat; phase?: Beat; min?: number; max?: number; pulseWidth?: number;
}
export interface AutomationNamespace {
  constant(value: number): AutomationSource;
  curve(points: AutomationPoint[], interpolation?: Curve['kind']): AutomationSource;
  polyline(points: AutomationPoint[]): AutomationSource;
  line(from: number, to: number, durationBeats: Beat): AutomationSource;
  gate(options: GateOptions): AutomationSource;
  chance(options: ChanceOptions): AutomationSource; // frequency/rate = decisions per beat
  wave(kind: WaveKind, options: WaveOptions): AutomationSource;
  sine(options: WaveOptions): AutomationSource;
  cos(options: WaveOptions): AutomationSource;
  triangle(options: WaveOptions): AutomationSource;
  saw(options: WaveOptions): AutomationSource;
  ramp(options: WaveOptions): AutomationSource;
  square(options: WaveOptions): AutomationSource;
  map(input: AutomationSource, range: { min: number; max: number }): AutomationSource;
  clamp(input: AutomationSource, range?: { min?: number; max?: number }): AutomationSource;
  invert(input: AutomationSource): AutomationSource;
  quantize(input: AutomationSource, steps: number): AutomationSource;
  scale(input: AutomationSource, factor: number): AutomationSource;
  offset(input: AutomationSource, amount: number): AutomationSource;
  mix(left: AutomationSource, right: AutomationSource, amount?: number): AutomationSource;
  add(left: AutomationSource, right: AutomationSource): AutomationSource;
  multiply(left: AutomationSource, right: AutomationSource): AutomationSource;
  min(left: AutomationSource, right: AutomationSource): AutomationSource;
  max(left: AutomationSource, right: AutomationSource): AutomationSource;
}

export interface TempoSegment {
  startBeat: Beat; bpm: number; // 20..999, finite
  curve?: 'step'|'linear'|'exponential'; // 到下一 segment 的过渡方式，default 'step'
}
export interface TimeSignatureSegment { startBar: number; numerator: number; denominator: number; }
export interface TrackSpec {
  id: EntityId; name?: string; channelIds: EntityId[]; tempo?: number;
  patternClipIds: EntityId[]; sampleClipIds: EntityId[]; enabled?: boolean;
  midiChannel?: number; // 1..16; required for MIDI export when note tracks exceed 16, may be shared explicitly
}
export interface LoopSpec { startBeat?: Beat; lengthBeats: Beat; count?: number; lastBeat?: Beat; }
export interface MarkerSpec { id: EntityId; name?: string; startBeat: Beat; }
export interface FadeSpec { lengthFrames: bigint; curve?: 'linear'|'equalPower'|'exponential'; }
export interface SampleRef {
  id: EntityId; assetUri: string; sha256: string; format: 'wav'|'aiff'|'flac'|'mp3'|'mp4'|'m4a';
  sampleRate: number; channels: 1|2; frames: bigint; edits?: SampleEditSpec;
  musicalLengthBeats?: Beat; // 素材原始音乐长度，fit/stretch 计算基准
}
export interface InstrumentRef {
  pluginId: string; pluginVersion: string; parameters: Record<string, number>;
  resources?: Record<string, string>;
  state?: unknown; // 插件声明 schema 的结构化状态（如 Slicer 的 slice 表），compile 期定稿，播放中不可变
}
export interface EffectRef {
  pluginId: string; pluginVersion: string; parameters: Record<string, number>;
  resources?: Record<string, string>; bypass?: boolean; mix?: number; // dry/wet 0..1, default 1
}
```

Electronic effects retain this reference format. The typed `effect(kind, parameters?,
{mix?, bypass?})` helper uses physical units and returns `EffectRef`; supported names
and ranges are in `packages/protocol/src/authoring/effects.ts`. `convolver(sampleId, parameters?,
options?)` adds `resources: {impulse: sampleId}`. Compile validates the reference and
prepares 1..262144 resampled impulse frames before graph publication. Invalid helper
values/resource graphs return `InvalidProject`; unavailable decoded assets return
`AssetUnavailable`. The new processors and precise limitations are specified in
`14-effects-production.md`. Existing raw EffectRefs remain supported.

## Authoring interfaces

Sample authoring 同样只产生 wire descriptors：

```ts
const sample = project.addSample({ assetUri: 'assets/loop.wav', sha256, format: 'wav',
  sampleRate: 48000, channels: 2, frames: 96000, musicalLengthBeats: 8 });
const clip = track.sample(sample).at({ bar: 1 }, { tempoSync: 'stretch' });
clip.fitBars(2);
```

`Sample` 的 getter 和 `SampleClip.toSpec()` 返回副本；`fitBeats`、`fitBars`、`fitToContent`
仅更新 beat 长度并增加项目 revision。资源不存在或 hash 不匹配由 Rust 返回 `AssetUnavailable`。

`addSample` 接受可选稳定 `id`，并拒绝已占用 ID。`frames` 可传正 safe-integer number
或正 u64 bigint；越界、不精确数字、非法 trim 范围和非正音乐长度报 `InvalidProject`。
添加失败不注册实体或改变 revision，SampleClip draft 可在放置失败后重试。
`fitBars` 按起点拍号设置长度；`fitToContent` 未指定音乐长度时按 trim 后内容折算，
通过 Rust 有效 tempo map/lane 的积分和逆变换计算完整内容区间。
`project.beatsForSeconds(startBeat, durationSeconds)` 提供同一只读查询，返回 beat 长度；
不增加 revision，不访问资产或设备。参数必须有限且非负，错误为 `InvalidProject`。
tempo lane 查询的烘焙上界为 `startBeat + durationSeconds * 999 / 60`，超出 65,536 个
segment 时提前报 `TempoMapComplexity`。查询完成后再调用 fit helper 更新 duration/revision。

Native facade `resolveBeatDuration(snapshot, startBeat, durationSeconds): number` 使用
版本化 ProjectSnapshot 与 `{startBeat: BeatWire, durationSeconds}` query JSON，返回
`{protocolVersion, durationBeats: BeatWire}`。复用 Rust 时钟及 tempo source validator；
版本校验先于查询解析，不要求图完整或资源可读。

Track 的 `enabled` 和 `midiChannel` 支持读写、revision 和快照序列化。`enabled` 默认 true，
false 会关闭该 Track 的音频调度与 MIDI note track；`midiChannel` 为 1..16，可赋 undefined
恢复自动分配。非法值或跨 Project 的 `use(channel)` 报 `InvalidProject`，且不改变 revision。
Track `tempo?: number` 支持读写、revision 和序列化；有效范围 20..999，非法值报
`TempoRange` 且不增加 revision。undefined 恢复 Project 时钟。局部拍位从项目零点按
固定 BPM 换算，音频和 MIDI 使用同一 Rust 有效时钟；不修改其他 Track 的节奏。

### 文件采样导入（已实现）

```ts
import { Project } from '@oxitone/core';
import { importSample } from '@oxitone/samples';

const project = new Project();
const assetBaseDir = '/music/song';
const imported = importSample('assets/loop.wav', { assetBaseDir });
const sample = project.addSample({ ...imported, musicalLengthBeats: 8 });
const track = project.addTrack('audio').use(project.addChannel());
track.sample(sample).at({ bar: 1 }).fitBeats(8);
await project.renderWav({ path: '/music/out.wav', assetBaseDir, tailSeconds: 0 });
```

`importSample(path, options?: { assetBaseDir?: string }): ImportedSample` 同步执行。
默认相对路径基于 cwd，输出绝对 `assetUri`；指定 base 时，相对输入基于 base 解析，
绝对输入保持原意，输出 URI 相对 base，目录外的词法路径报 `InvalidProject`。路径是
本地文件系统路径，不接受 file URL。相对 descriptor 用于 render 时必须传入相应的
`assetBaseDir`。`Project.compile({ assetBaseDir, ...engineOptions })` 已支持相对资源；
Session.update 和 Session.renderWav 默认沿用它，render options 可显式覆盖 base。

`ImportedSample` 包含 `assetUri`、`sha256`、`format`、`sampleRate`、`channels`、bigint
`frames`，与 `Project.addSample` 结构兼容；可通过展开对象加入 `edits`/`musicalLengthBeats`。
附加 `provenance: { sourceChannels, sourceBitDepth?, decoder, channelLayoutAction }`
只在返回 descriptor 中，当前 `SampleRef` 快照不会保存该字段。导入只读取并解码，
不复制、转码到磁盘或复用 prepare 缓存；这些功能由后续项目持久化阶段补齐。

`oxitone` 和 `@oxitone/samples` 均导出同步 `inspectSample(path): SampleInfo`，不需要 engine。
底层 N-API 为 `inspectSample(requestJson): responseJson`：

```ts
interface InspectSampleRequest { protocolVersion: string; path: string; }
interface SampleInfo {
  protocolVersion: string;
  sha256: string;
  format: 'wav'|'aiff'|'flac'|'mp3'|'mp4'|'m4a';
  sampleRate: number;
  channels: 1|2;             // 解码后维度，>2 个源声道会降混
  frames: string;           // wire u64 十进制字符串；importSample 转为 bigint
  sourceChannels: number;
  sourceBitDepth?: number;  // compressed decoder 不一定提供
  decoder: string;
  channelLayoutAction: 'kept'|'downmixed-to-stereo';
}
```

请求与响应均使用协议 1.0。先检查版本，再读文件。
空/NUL 路径或错误请求结构报 `InvalidProject`；文件不可读报 `AssetUnavailable`；
未知容器、无法解码或零帧音频报 `SampleFormatUnsupported`，文件相关错误带 `details.path`。
prepare 仍按原始文件 hash 校验，源文件修改后必须重新导入。PCM 始终留在 Rust 控制线程，
不进入 JSON，也不触发 audio callback。

`cacheSample(path, cacheDir)` 为独立 native facade；N-API 输入为
`{ protocolVersion, path, cacheDir }`，输出为
`{ protocolVersion, path, sha256, format: 'wav', sampleRate, channels, frames, provenance }`。
输出 path 是绝对缓存文件路径，frames 为 wire u64。空/NUL cacheDir 以 InvalidProject
拒绝，缓存 I/O/hash/symlink 错误为 AssetUnavailable，RIFF 溢出为 WavTooLarge。
`importSample` 的 cacheDir 选项把该结果转换为 bigint frames 和 assetUri。
`SampleOptions`/`SampleRef` 新增可选 provenance，含 sourceSha256/sourceFormat/
sourceSampleRate/sourceChannels/sourceBitDepth?、decoder、channelLayoutAction、
cacheEncoding?（当前唯一值 wav-f32-v1）。Sample.provenance 返回防御性副本。
该字段通过 TS/Rust snapshot、可编辑恢复和目录保存/读取保留，不改变 DSP。

`@oxitone/core` 的混音 builder 复用以下协议 1.0 wire contracts：

```ts
const project = new Project();
const keys = project.addMixerChannel({ name: 'Keys', level: 0.8 });
const fx = project.addMixerChannel({ name: 'Reverb', inserts: [reverbRef] });
const channel = project.addChannel({ mixerChannelId: keys.id, effectChain: [eqRef] });
keys.send(fx, { ratio: 0.25, preFader: false });
keys.automate(`send.${fx.id}.ratio`, automation.sine({ periodBeats: 8 }));
project.master.addEffect(limiterRef);
channel.swing = 0.2;
```

- `MixerChannelOptions`：name、level（0..2）、balance（-1..1）、masterSendRatio（0..1）、
  mute、solo、inserts（有序 EffectRef 数组）。`project.mixerChannels` 包含 Master。
- `bus.send(destination, { ratio = 1, preFader?, sidechain? })` 添加或完整替换该 destination
  的 send；`removeSend(destination)` 删除。Master 无 outgoing route，不能设置
  masterSendRatio（getter 返回 undefined），也不能作为 send source/destination。
- Channel 的 effectChain 与 bus 的 inserts 可整体替换，`addEffect(ref)` 追加；getter
  返回副本，编辑后必须重新赋值。Channel 另支持 level/pan/swing/mute/solo 与路由 setter。
- 所有成功变更更新 revision，范围错误或未知路由以 `InvalidProject` 拒绝且不改变快照。
  插件 descriptor/参数范围与完整 DAG 由 Rust compile 校验。修改 authoring 后需要重新编译
  才影响当前 session；离线导出读取当前快照。
- `bus.automate` 支持 bus 参数及 `send.<destinationId>.ratio`；Channel、MixerChannel
  和 Master 的 insert 支持 `insert.<index>.mix/bypass` 以及
  `insert.<index>.parameter.<pluginParameterId>`。
  这些路径统一用于 Channel、MixerChannel 和 Master 的 `automate`/`setParameter`；
  index 为从 0 开始、无前置零的十进制整数，插件参数 ID 可含点号。未知 insert/参数报
  `AutomationTargetInvalid`，插件参数绑定要求 descriptor 声明 `automation: true`。
  `setParameter` 接收物理值并按 descriptor 范围校验（`AutomationRange`）。
  effect 的 `<name>Beats` 参数在存在 `<name>Seconds` 时按有效 BPM 换算；
  显式设置 seconds 切换为秒值，设置 beats 恢复跟随 tempo。

```ts
export interface NoteSpec {
  id?: EntityId; pitch: Pitch; start: Beat; duration: Beat;
  velocity: number; offVelocity?: number; chance?: number; voice?: number; tags?: string[];
}
export interface PatternSpec { id: EntityId; name?: string; lengthBeats: Beat; notes: NoteSpec[]; }
export interface PatternClipSpec {
  id: EntityId; patternId: EntityId; trackId: EntityId; startBeat: Beat;
  durationBeats?: Beat; loopCount?: number; lastBeat?: Beat;
  transpose?: number; velocityScale?: number; probability?: number; enabled?: boolean;
}
export interface SampleEditSpec {
  startFrame?: bigint; endFrame?: bigint; level?: number; tone?: number;
  normalize?: { peakDb: number }; fadeIn?: FadeSpec; fadeOut?: FadeSpec; crossfade?: FadeSpec;
}
export interface SampleClipSpec {
  id: EntityId; sampleId: EntityId; trackId: EntityId; startBeat: Beat;
  durationBeats?: Beat; gain?: number; pan?: number; rate?: number; loop?: LoopSpec;
  tempoSync?: 'off'|'stretch'|'repitch'; // default 'off'
  stretchAlgorithm?: string;             // default 'wsola-v1'
  enabled?: boolean;                     // default true; false 时不调度但保留数据
}
export interface ChannelSpec {
  id: EntityId; name?: string; instrument: InstrumentRef; effectChain: EffectRef[];
  level: number; pan: number; swing?: number; mixerChannelId: EntityId; mute?: boolean; solo?: boolean;
}
export interface MixerChannelSpec {
  id: EntityId; name?: string; level: number; balance: number;
  masterSendRatio?: number; // default 1, post-fader; controls the dedicated route to Master
  inserts: EffectRef[]; sends: SendSpec[]; mute?: boolean; solo?: boolean;
}
// destinationId must be another MixerChannel, never Master; sidechain: true routes to the
// destination's sidechain detector inputs only and does not enter the bus audio sum.
export interface SendSpec { destinationId: EntityId; ratio: number; preFader?: boolean; sidechain?: boolean; }
export interface AutomationLaneSpec {
  id: EntityId; target: { entityId: EntityId; parameterId: string };
  source: AutomationSourceSpec; combine?: 'replace'|'add'|'multiply'|'max'; loop?: LoopSpec; lastBeat?: Beat;
}
export interface ProjectSnapshot {
  protocolVersion: string; revision: bigint; id: EntityId; name?: string;
  sampleRate: number; blockSize: number; seed: number;
  tempoMap: TempoSegment[]; timeSignatureMap: TimeSignatureSegment[]; markers: MarkerSpec[];
  tracks: TrackSpec[]; patterns: PatternSpec[]; patternClips: PatternClipSpec[];
  sampleClips: SampleClipSpec[]; samples: SampleRef[]; channels: ChannelSpec[];
  mixerChannels: MixerChannelSpec[]; automation: AutomationLaneSpec[];
}
```

`InstrumentRef`/`EffectRef` 包含 `pluginId`, `pluginVersion`, 参数值和资源引用；具体插件不得让 TS 传任意 JSON 到 realtime。未知参数和版本在 compile 时返回错误。

`InstrumentRef.state` 是插件结构化状态的受控通道：descriptor 必须声明 versioned state schema，compile 时校验并固化为不可变 plugin state，实时路径只读、不可通过 `setParameter` 修改；未声明 state schema 的插件携带 `state` 在 compile 期报错。Phase 1 只有内置 `Slicer` 使用该通道：

```ts
// Slicer 的 state 形状（pluginId: 'oxitone.slicer'）
interface SlicerState {
  sampleId: EntityId;
  slices: { start: { frames: bigint } | { beat: Beat }; end?: { frames: bigint } | { beat: Beat };
            level?: number; pan?: number; rate?: number; reverse?: boolean }[]
          | { grid: number }                 // 等分编辑范围
          | { onset: { algorithm: string; sensitivity?: number } }; // 版本化瞬态检测
  triggerNote?: Pitch;                      // default 60
  playMode: 'oneshot'|'gate';
  tempoSync?: 'off'|'repitch';               // default off; host-owned tempoFactor
}
```

内置音源便捷入口（从 `@oxitone/core` 导出）：

```ts
const lead = wavetable({ oscA: { wave: 'saw', unison: 4, detune: 12 },
  mix: 0.2, filter: { type: 'lowpass', cutoff: 4000, resonance: 0.3 },
  amp: { attack: 0.01, release: 0.2 }, voiceMode: 'poly' });
const keys = sampler(sample, { rootKey: 60, loop: 'forward', startSeconds: 0.1 });
const piano = multisampler([
  { sample: softC4, rootKey: 60, keyRange: [58, 62], velocityRange: [1, 79] },
  { sample: loudC4, rootKey: 60, keyRange: [58, 62], velocityRange: [80, 127] },
], { amp: { attack: 0.001, sustain: 1, release: 0.25 } });
const chops = slicer(sample, { slices: { grid: 8 }, tempoSync: 'repitch' });
channel.instrument = chops;
```

Wavetable 可配置 oscA/oscB 的 wave/pitch/unison/detune/spread、mix、filter、
filterEnvelope（ADSR + amount）、amp、voiceMode、glide、level/pan。Sampler 提供
rootKey、velocitySensitivity、amp、loop、startSeconds、level/pan。ADSR 时间单位
为 seconds，wave/type/mode 字符串转换为 descriptor 枚举。Slicer slices 支持显式
frame/beat 标记、grid 1..64 或 onset-v1，以及 triggerNote/playMode/tempoSync、level/pan。
frame authoring 接受 safe-integer number 或 u64 bigint，wire 统一为十进制字符串。
schema/类型从 protocol 导出；返回值独立且不修改 Sample。Channel 替换 instrument
增加一次 revision，需 Session.update 或重新 compile 生效；失败保留旧 authoring 状态。

Wavetable 新增参数追加在原 descriptor 索引之后，保留 `oxitone.wavetable@1.0.0`
与原参数默认输出，保留前 46 个参数索引。以下 options 由 helper 映射到相同 dotted parameter ID：

| Options | 范围、默认值与语义 |
| --- | --- |
| `oscA/oscB.wave`、`morphTo` | sine/saw/square/triangle/organ/glass → enum 0…5；wave 仍映射 `.wavetable`，morphTo 默认 triangle |
| `.position` | 0…1，默认 0；source 到 morphTo 的同相位线性渐变 |
| `.phase`、`.phaseSpread` | 0…1 cycles，默认 0；非复用声部起音的相位与 unison 相位铺开 |
| `oscA/oscB.octave`、`.level` | octave 整数 −4…4 默认 0；叠加 pitch 半音；level 0…1 默认 1 |
| `sub.level`、`sub.octave`、`sub.wave` | level 0…1 默认 0；octave 整数 −4…4 默认 −1；sine/triangle/saw/square/pulse/rounded → 0…5，默认 sine，滤波后独立层 |
| `noise.level` | 0…1 默认 0，滤波前确定性白噪声层 |
| `lfo.shape`、`.rateHz`、`.phase` | sine/triangle/ramp/square → 0…3，默认 sine；0.01…30 Hz 默认 1；phase 0…1 默认 0 |
| `lfo.pitch`、`.cutoff` | 分别 ±12 / ±48 semitones，默认 0；双极调制深度 |
| `lfo.positionA`、`.positionB` | ±1，默认 0；调制后 position 夹紧到 0…1 |
| `lfo.level` | 0…1 默认 0；单极 tremolo，满深度增益 0…1 |

LFO 随音符触发，legato 保留 phase；是合成器内部固定路由，不新增工程 automation AST。
可用 `rateHz: bpm / 60 / beatsPerCycle` 在代码声明节奏；当前不自动跟随 tempo map，
扩展支持八槽调制矩阵、第二 LFO、FM/ring、三种八帧 bank 与 warp、ADSR curves、四个 macro，
完整范围和默认值见 `13-electronic-production.md` 与 protocol schema；不支持导入任意波表。
参数可由既有 Channel automation 绑定。
非法字符串、非有限值、越界/非整数枚举由 helper 报 `InvalidProject`，Rust descriptor
仍为权威验证来源。UI 显示的是这些 source/default 参数。

```ts
const motionLead = wavetable({
  oscA: { wave: 'saw', morphTo: 'glass', position: 0.25, unison: 5,
    detune: 8, spread: 0.75, phaseSpread: 0.62 },
  sub: { level: 0.07, octave: -1 }, noise: { level: 0.01 },
  lfo: { shape: 'sine', rateHz: 4.7, pitch: 0.045, positionA: 0.07 },
});
```

`multisampler(regions, options?)` 返回 `oxitone.multisampler@1.0.0`；SampleRegion 使用 Sample、rootKey、
keyRange、可选 velocityRange（默认 [1,127]）/gain（默认 1）。Options 为 SamplerOptions 去掉 rootKey、
增加 transpose（-48…48）。Wire state 为 `{version:1,regions:[{resource,rootKey,keyRange,velocityRange,gain}]}`，
声明 schema `oxitone.multisampler.regions@1`；resource 是 InstrumentRef.resources 的 key，资源值仍为 Sample ID，
因此便携工程和 preset 的既有资源 remap 有效。TS 与 Rust 拒绝重叠/倒置区域（InvalidProject），
缺失 state、resource key 或工程 Sample ID 为 InvalidProject，缺失已准备 PCM 为 AssetUnavailable；
不在 TypeScript 解码音频。JSON schema 为 multisampler-state.schema.json。
transpose 为连续半音偏移，在新 note-on 时取值；区域选择仍使用未转调的 MIDI key。
最终播放速率沿用 Sampler 的 0.125…16 范围，`clamp(2^((note-rootKey+transpose)/12),0.125,16)`；
极端跨区或转调会饱和到该范围。常规采样库应选择靠近实际演奏音高的 rootKey。
完整 native 参数顺序为 transpose、velocitySensitivity、amp.attack/decay/sustain/release、loop、start、level、pan；
后九项沿用 Sampler 的范围/默认值/自动化语义，transpose 的默认值为 0。键位与力度区域不可在 callback 修改。

`grandPiano(bank, options?)` / `softPiano(bank, options?)` 是这个原生 multisampler 的录音钢琴映射 helpers，
不引入新的 wire schema 或 JS DSP。`PianoBank` 包含 `keyRange: [low,high]` 与按弱到强排列的
2…8 个 `layers`，每层是 `{sample: Sample, rootKey, gain?}` 数组。各层根音集合必须一致且无重复，
总区域数 ≤256；helper 不修改输入，按根音中点创建不重叠键区，平分力度 1…127。
Grand 使用所有录音层；Soft 只使用最弱两层并映射完整力度范围。默认 Grand 的 velocitySensitivity=.7、
attack=.002s、release=.38s；Soft 为 .45/.007s/.75s；两者 decay=0、sustain=1，允许 MultisamplerOptions 覆盖。
非法层、Sample、键区或 options 报 `InvalidProject`；音频文件依旧由 Project 提供，helper 不下载资源。

`Record<string, number>` 只表示经过 schema 校验的参数表，不代表插件可以接收任意键；每个 plugin descriptor 必须提供完整 `ParameterSpec[]`，缺失、未知或越界参数都在 compile 阶段拒绝。

Project 自身也是可自动化实体：它暴露参数 `tempo`（20..999 BPM，`mapping: 'log'`，`rate: 'control'`），lane 以 `target: { entityId: <projectId>, parameterId: 'tempo' }` 绑定，语义与烘焙规则见 `02-domain-spec.md` 和 `03-audio-runtime-spec.md`。

内建图节点的 stable parameter ID 全集（与插件参数共用同一 automation/参数机制，语义见 `02-domain-spec.md`）：

```text
Project:      tempo
Channel:      level, pan, mute, swing
MixerChannel: level, balance, mute, masterSendRatio, send.<destinationId>.ratio
SampleClip:   level, tone, gain, pan, rate
EffectInsert: mix, bypass   // 每个 insert 节点内建，叠加在插件自身参数之上
```

## Facade 与 commands

CLI `render <input> <output>` / `export-midi <input> <output>` 接受 snapshot JSON、
便携工程目录或其中的 `oxitone.project.json`。目录/manifest 使用 loadProject 校验版本、
相对 URI 和 hash；snapshot 资源相对输入 JSON 的目录解析，输出路径仍相对调用者 cwd。
读取不修改输入；渲染始终把 assetBaseDir 传给 Rust。成功 stdout 为 JSON report，错误
stderr 为 `{code?, message, details?}` JSON，exit 1；用法错误 exit 2。doctor 列出设备。

```ts
export interface EngineOptions {
  sampleRate?: number;        // default 48000
  blockSize?: number;         // default 128; 允许 64/256
  renderAheadBlocks?: number; // default 4, 范围 2..16; 决定 ring 深度与控制延迟
  latencyMode?: 'buffered'|'direct'; // default 'buffered'
  allowPlugins?: 'signed-only'|'any';
  outputDeviceId?: string;    // 缺省跟随系统默认输出
  audioBackend?: 'device'|'simulated'; // default device; simulated 不打开任何系统输出
  deviceRatePolicy?: 'adapt-device'|'resample';     // default 'adapt-device'
  deviceChangePolicy?: 'follow-default'|'pause';    // default 'follow-default'
  metronome?: { enabled: boolean; level?: number }; // accent 规则来自 time signature map
}
// 注意: adapt-device 会把设备 nominal rate 设为项目采样率,该修改对全系统生效;
// 不支持或设置失败时自动回落 resample 并产生诊断。
export interface OutputDeviceInfo {
  id: string; name: string; nominalSampleRates: number[];
  bufferFrameSizeRange: [number, number]; isDefault: boolean;
}
export interface OutputLatency {
  frames: bigint; seconds: number;
  breakdown: { ring: bigint; resampler: bigint; deviceBuffer: bigint; safetyOffset: bigint; deviceLatency: bigint };
}
export interface RenderOptions {
  path: string;
  start?: { bar: number } | { beat: Beat } | Timecode | { marker: EntityId }; // 四选一
  end?:   { bar: number } | { beat: Beat } | Timecode | { marker: EntityId };
  sampleRate?: number; blockSize?: number;
  tailSeconds?: number; respectSolo?: boolean; seed?: number;
  bitDepth?: 16|24|'float32';               // default 'float32'
  dither?: 'tpdf'|'none';                   // 仅 16/24-bit 有效，default 'tpdf'
  stems?: 'none'|'mixer-channels'|'tracks'; // default 'none'; 非 none 时 path 是输出目录
  includeMetronome?: boolean;               // default false
  assetBaseDir?: string;                    // 相对 assetUri 的解析基准目录
}
export interface RenderReport {
  files: { path: string; stem?: EntityId; durationSeconds: number;
           peakDbfs: number; truePeakDbfs: number; integratedLufs: number }[];
  graphLatencyFrames: bigint;               // PDC 引入的图内部总延迟
}
export interface MidiExportOptions {
  path?: string;                            // 设置时 native 落盘（temp+fsync+rename），report 返回 path/bytes
  ppq?: number;                             // default 960, 1..32767；记入 diagnostics
  tempoEventResolutionTicks?: number;       // default max(1, ppq/8)；连续 tempo 段的重采样步长，记入 diagnostics
}
export interface MidiChannelAssignment { trackId: EntityId; channel: number; source: 'explicit'|'auto'; }
export interface MidiDiagnostics {
  ppq: number; tempoEventResolutionTicks: number; tempoEventCount: number; noteTrackCount: number;
  channelAssignments: MidiChannelAssignment[];              // 按 track ID 排序
  skippedAutomation: { laneId: EntityId; targetEntityId: EntityId;
                       targetParameterId: string; reason: string }[]; // 按 lane ID 排序
}
export interface MidiExportReport {
  path?: string; bytes?: number;            // options.path 模式
  bytesBase64?: string;                     // 无 path 时返回
  diagnostics: MidiDiagnostics;
}
```

建议 `@oxitone/core` 暴露：

```ts
const project = new Project({ id, sampleRate: 48_000 });
const track = project.addTrack('drums').use(channel);
track.add(pattern).at({ bar: 1 }).loop(8);
await project.compile();
const session = await project.play({ bar: 1 });
await session.renderWav({ path, tailSeconds: 2 });
await project.exportMidi({ path });
```

Native facade 的最小命令：`createEngine(options?: EngineOptions)`、`registerPlugin(engine, { libraryPath, manifest, expectedHash? })`、`getPluginDiagnostics(engine)`、`compile(snapshot)`、`enqueueTransport(command)`、`renderWav(options)`、`exportMidi(engine, snapshot, options)`（返回 `MidiExportReport`；`MidiChannelLimit` 错误的 `details.unassignedTrackIds` 列出全部未分配 track ID）、`listOutputDevices`、`getOutputLatency(engine)`（返回 ring horizon + 设备延迟的 frame/秒双表示；frame 以**项目采样率**计）、`getDiagnostics(engine)`、`dispose`。所有异步方法在控制线程执行；`registerPlugin` 的 dlopen/校验和 `enqueueTransport` 的 queue 复制都不许进入 audio callback。

### 已实现的命令语义（M4）

`Session.update(): Promise<Session>` 在同一 engine 编译当前 authoring snapshot，成功后
更新 `session.revision: bigint`。失败保留旧图、revision 和 Session 导出快照。实时更新
保留 cursor/state/loop 并在 block boundary 生效；首次 play 前的更新同样保留 frame
cursor/state/loop。`Project.play(position?, loop?)` 自动更新有新 revision 的已有 Session；
直接 `Session.play` 播放已编译版本。`Project.compile(options?)` 仍显式新建引擎并释放旧 Session。

`Session.play/seek` 与 Project 转发入口接受 `{bar, beat?}`（bar 1-based、beat 0-based）、
绝对 `{beat}`、`{marker: markerId}`、`{seconds}`、`{frame}` 或 `{frames}`。frame(s) 可为
safe integer number 或 u64 bigint；秒数由 Rust 按实际编译采样率 round-half-up 换算。
marker ID 和小节表来自最后一次成功编译的快照，未提交的 authoring 改动不影响定位。
一次只能使用一种位置形式（bar+beat 例外）；无效/越界位置报 `InvalidProject`，不移动游标。
loop 仍使用 `{startFrame,endFrame}`，end exclusive。

`Session.renderWav/exportMidi` 使用最后一次成功编译的 snapshot；要导出当前 authoring
可先 update，或调用 Project 上的导出入口。`Session.dispose()` 幂等；`session.disposed`
变为 true，Project.session 返回 undefined，后续 Session 操作报 `InvalidProject`。
Project 再 compile/play 会创建新 Session。上述定位和换图不要求 TS 参与音频 callback。

transport wire 新增可选 `seconds`，与 `frame`/`beat` 互斥；旧命令仍兼容。

native `compile(engine, snapshot, options?: {assetBaseDir?: string})` 在控制线程以该目录
解析相对 Sample URI，省略时使用 cwd；旧的两个参数调用兼容。N-API 第三个参数是
可选 compile options JSON，版本仍由 snapshot 先行校验。`assetBaseDir` 必须为非空本地路径。

项目文件入口：`Project.save(directory, options?: {assetBaseDir?: string}): Promise<void>`，
或 `saveProject(snapshot, directory, options?)`；`loadProject(directory)` 返回
`Promise<{snapshot: ProjectSnapshot, assetBaseDir: string}>`。格式、原子写入和错误码见
`06-format-and-export.md`，schema 为 `project-file.schema.json`。

`Project.fromSnapshot(snapshot, {assetBaseDir?}): Project` 恢复 authoring builders；
`Project.load(directory): Promise<Project>` 同时加载、校验素材 hash，并把绝对资源目录保留
在 `project.assetBaseDir`。后续 compile/update/renderWav/save 默认使用它，显式参数可覆盖。
恢复不创建 engine、不打开播放设备或执行插件；结构、ID、clip 归属、资源引用和 builder
范围错误报 `InvalidProject`；DSP 路由 DAG、插件 descriptor/参数的完整校验仍在 Rust compile。

恢复保留 ID、note ID/顺序、精确 rational beat、send/insert 顺序、插件 state 和可选默认值，
无编辑的 canonical snapshot 往返一致。省略的 Master 保持省略，显式修改 Master 时才写入。
`project.patterns` 可访问已恢复、尚未放置的 Pattern；NoteInput 新增可选 `id`。
`PatternClip.durationBeats` 支持正数或 undefined setter，修改保持其他已保存的时序字段。
`revisionBigInt: bigint` 精确表示 u64 revision；既有 `revision: number` 在安全整数范围内
继续可用，超限报 `InvalidProject` 并提示使用 bigint。到达 u64 最大值时允许读/导出，
后续编辑以 `InvalidProject` 拒绝且保持原状态。恢复后创建实体跳过已有 ID，重复显式 ID 拒绝。

- `registerPlugin(engineId, optionsJson)`：返回 `{ pluginId, pluginVersion, sha256 }` JSON；manifest 必填，按 engine 独立注册并用于 compile/renderWav。相同 ID/version/hash 幂等，不同 hash 冲突；完整 ABI、签名与信任边界见 `08-plugin-abi.md`。
- `getPluginDiagnostics(engineId)`：返回 `{ pluginId, pluginVersion, faults }[]` JSON，累计各实例的故障静音次数；无需启动播放。不是逐节点 deadline watchdog。
- `compile(engineId, snapshotJson)`：完整编译 `RenderGraph` 并由 engine registry 持有（互斥锁保护的控制线程状态；编译在锁外进行）。后续 `enqueueTransport`/`setParameter` 作用于该图。重新 compile 替换图并清空已排队的 parameter events；实时播放中重新 compile 时，新图经命令队列在 block boundary 换入 render worker（或 direct callback），旧图经有界队列交给控制线程销毁。实时 session 内 sampleRate/blockSize 变化报 InvalidProject；命令队列满报 RealtimeFault，拒绝的 compile 保留旧图和参数索引。
- `enqueueTransport(engineId, commandJson)`：命令为 `{type:'transport', command:'play'|'pause'|'stop'|'seek', frame?, beat?, loopRegion?: {startFrame,endFrame}}`（`frame`/`beat` 二选一；loopRegion 使用项目采样率 frame，end exclusive）；返回 `{"state":"stopped|playing|paused|rendering","cursor":"<frame string>"}`。首次 `play` 启动实时输出：解析设备（`outputDeviceId` 或系统默认）、协商采样率/buffer、启动 render-ahead worker 与 CoreAudio HAL stream；此后 play/pause/stop/seek 经 lock-free command queue 在 **ring horizon** 生效，`seek`/`play(from)` flush voices。首次 `play` 前 transport 只推进离线状态（不发声）。设备不可用时不启动 session，transport 置 paused 并报 `DeviceUnavailable`；engine 未 compile 时报 `InvalidProject`。
- `setParameter(engineId, entityId, parameterId, value, atFrame?)`：`value` 为**物理值**，按目标 `ParameterSpec.min..max` 校验；越界/非有限值报 `AutomationRange`，未知 entity/parameter 报 `AutomationTargetInvalid`。控制线程对照 compile 期固化的 target 快照同步校验后经 worker 命令队列投递，在 ring horizon 生效（`atFrame` 缺省 = 当前 cursor）；离线渲染经过该 frame 时同样生效。Channel `swing` 仅可自动化、不接受 runtime `setParameter`，报 `AutomationTargetInvalid`。
- `listOutputDevices()`：返回 CoreAudio 输出设备列表（默认设备在前），`id` 为设备 UID（可作 `outputDeviceId`），含 `nominalSampleRates`、`bufferFrameSizeRange`、`isDefault`。
- `getOutputLatency(engineId)`：返回 `OutputLatency`（frame 以项目采样率计，秒双表示）：`ring`（render-ahead horizon，direct 模式为 0）+ `resampler`（polyphase 群延迟）+ `deviceBuffer` + `safetyOffset` + `deviceLatency`。engine 未在播放时报 `DeviceUnavailable`。
- `getDiagnostics(engineId)`：返回结构化诊断快照 `{state, cursor, blocks, deadlineMisses, xruns, nanBlocks, queueDrops, performanceWarnings, engineLoad, ringOccupancyFrames, blockTimeNs:{p50,p95,p99,max}, events}`；`events` 是上次调用以来产生的诊断事件（`Underrun`、`PerformanceWarning`、`RealtimeFault`、`DeviceChange`、`DeviceUnavailable`、`ModeFallback`），读取即清空。engine 未在播放时报 `DeviceUnavailable`。
- `renderWav(engineId, snapshotJson, optionsJson)`：按传入 snapshot 重新编译离线渲染（返回 `RenderReport` JSON，`graphLatencyFrames` 为无符号十进制字符串）。`RenderOptions` 扩展字段 `assetBaseDir`：相对 `SampleRef.assetUri` 相对它解析；资产缺失报 `AssetUnavailable`。engine 上通过 `setParameter` 排队的 parameter events 会应用到渲染（事件在渲染推进到其 frame 时生效）。响度值出现 `-inf`（数字静音或文件过短）时在边界钳到 `-144.0` dB（JSON 无法表示非有限值）。

`AutomationNamespace` 是纯 TypeScript builder，返回值不可直接求值；`compile` 时必须将其降级为 `AutomationSourceSpec`。builder 应在创建时拒绝 NaN/Infinity、负 beat、空 points、非正 period/duration、超出 0..1 的 source 输出范围和组合树超限。`map` 只在 0..1 内做归一化区间映射；物理范围由 target `ParameterSpec` 负责，lane 在进入 wire snapshot 前必须保留 source 的 0..1 语义和 parameter mapping。

`ChanceOptions` 的类型保证 `frequency`、`rate` 和 `intervalBeats` 三选一；`frequency` 是面向用户的 `rate` 别名，序列化时统一为 `rate`。运行时还要检查所选频率/间隔为有限正数。`GateOptions` 的 `on`、`off`、`WaveOptions.min/max` 和所有组合结果必须在 0..1，除非它们只作为未绑定 target 的中间 source 并在 `clamp` 前收敛。

## Wire messages

```ts
type NativeCommand =
  | { type: 'compile'; revision: bigint; snapshot: ProjectSnapshot }
  | { type: 'transport'; command: 'play'|'pause'|'stop'|'seek'; frame?: bigint; beat?: Beat }
  | { type: 'setParameter'; entityId: EntityId; parameterId: string; value: number; atFrame?: bigint };

interface NativeEvent {
  protocolVersion: string; revision: bigint;
  type: 'compiled'|'transport'|'meter'|'diagnostic'|'fault';
  payload: unknown;
}
```

Events include stable `code`, `severity`, and `path` where applicable. N-API errors map to `OxitoneError` with `code`, `message`, `details`, and optional `cause`; callers must not parse human-readable messages.

## Compatibility

`oxitone` 统一导出 core authoring、native facade 和 importSample；底层包改为
`@oxitone/native`，避免 core 与统一入口循环依赖。既有低层函数签名保留。

`Project.registerPlugin(options, {allowPlugins?})` 显式校验/注册动态库并保留绝对路径
与实际 SHA-256（不进入 snapshot）。后续 compile/play/renderWav 创建的引擎都注册
这些库；已有 Session 同步注册。未指定策略时仍遵循 native 默认策略，不隐式放宽。
`Project.registeredPlugins`、`pluginPolicy` 提供只读控制配置，供 preview runner 传递。
`Session.registerPlugin`、`pluginDiagnostics()` 作用于该 Session 的引擎。
`getPluginInfo(engine, pluginId, pluginVersion)` 返回版本化的 kind/parameter/state schema
元数据，复用 Rust registry 的权威 descriptor，未知 ID/version 报 PluginManifestMismatch。
EngineOptions 的 signed-only/adapt-device/follow-default 与 ParameterSpec.one-pole
使用公开的连字符拼写；Rust 解码保留旧 camelCase 别名兼容，序列化只输出公开拼写。

`Channel.applySettings({instrument?, effectChain?, level?, pan?, swing?})` 校验并按单个
revision 应用一组 authoring 设置。`Project.importSampleRef(ref, id?)` 导入 detached
SampleRef（默认生成新 ID），保留精确 rational 音乐长度；不在 TS 中读取/解码 PCM。
预设格式、资源迁移、候选图验证与显式 Session.update 语义见 `06-format-and-export.md`。

Protocol major changes require a new npm major and native ABI tag. Minor additions are optional and must have defaults. Rust rejects snapshots with a newer minor version unless the field is explicitly marked ignorable. Generated declarations include protocol version constants and are checked in CI against schemas.
