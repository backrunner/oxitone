# Oxitone TypeScript / Rust API 契约

以下是 Phase 1 的公共形状。实现时建议用 zod/JSON schema 或等价的运行时校验生成 Rust 类型；手写类型必须与 schema 同步。

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

## Authoring interfaces

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
}
```

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

```ts
export interface EngineOptions {
  sampleRate?: number;        // default 48000
  blockSize?: number;         // default 128; 允许 64/256
  renderAheadBlocks?: number; // default 4, 范围 2..16; 决定 ring 深度与控制延迟
  latencyMode?: 'buffered'|'direct'; // default 'buffered'
  allowPlugins?: 'signed-only'|'any';
  outputDeviceId?: string;    // 缺省跟随系统默认输出
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
const session = await project.play({ from: { bar: 1 } });
await session.renderWav({ path, tailSeconds: 2 });
await project.exportMidi({ path });
```

Native facade 的最小命令：`createEngine(options?: EngineOptions)`、`registerPlugin({ libraryPath, expectedHash? })`、`compile(snapshot)`、`enqueueTransport(command)`、`renderWav(options)`、`exportMidi(engine, snapshot, options)`（返回 `MidiExportReport`；`MidiChannelLimit` 错误的 `details.unassignedTrackIds` 列出全部未分配 track ID）、`listOutputDevices`、`getOutputLatency(engine)`（返回 ring horizon + 设备延迟的 frame/秒双表示；frame 以**项目采样率**计）、`getDiagnostics(engine)`、`dispose`。所有异步方法在控制线程执行；`registerPlugin` 的 dlopen/校验和 `enqueueTransport` 的 queue 复制都不许进入 audio callback。

### 已实现的命令语义（M4）

- `compile(engineId, snapshotJson)`：完整编译 `RenderGraph` 并由 engine registry 持有（互斥锁保护的控制线程状态；编译在锁外进行）。后续 `enqueueTransport`/`setParameter` 作用于该图。重新 compile 替换图并清空已排队的 parameter events；实时播放中重新 compile 时，新图经命令队列在 block boundary 换入 render worker（或 direct callback），旧图在 worker 线程退役。
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

Protocol major changes require a new npm major and native ABI tag. Minor additions are optional and must have defaults. Rust rejects snapshots with a newer minor version unless the field is explicitly marked ignorable. Generated declarations include protocol version constants and are checked in CI against schemas.
