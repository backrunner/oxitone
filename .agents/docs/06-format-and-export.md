# Oxitone 格式、资源和导出

## 项目文件

Phase 1 推荐项目文件为可版本控制的目录：

```text
my-track/
  oxitone.project.json       # ProjectSnapshot（canonical JSON）
  assets/
    <content-hash>.wav       # 原始或导入后缓存的媒体
  locks/                      # 可选工具生成，不属于语义模型
```

`oxitone.project.json` 必须包含 `formatVersion`、`protocolVersion`、`projectId`、`revision`、`seed` 和规范化数组。数组按 stable ID 排序，浮点使用有限十进制，bigint frame 使用十进制字符串。资源用相对 URI + SHA-256；绝对路径不得进入可移植项目文件。

Canonical JSON 的字节级规则（`@oxitone/protocol` 与 `oxitone-core` 的编码器必须逐字节一致，实现对拍 fixture 在 `schemas/fixtures/`）：

- 对象 key 按字典序排序（协议 key 均为 ASCII）。
- 元素全部带字符串 `id` 的数组按 `id` 升序排序；其他数组保持 authoring 顺序。
- 数值必须有限；整数值写在 JSON safe-integer 范围内、不带小数部分，其余浮点用 ECMAScript 最短 round-trip 十进制（与 `JSON.stringify` 一致）。
- `u64` frame/revision 一律无符号十进制字符串（无前置零）；Beat 为约分后的 `{ "numerator": <safe integer>, "denominator": <u32> }`。
- 2 空格缩进、LF 换行、文件末尾一个 LF。

未知字段读取时忽略但保留在 round-trip metadata（若实现支持）；未知 major version 拒绝。写入采用 temp file + fsync + rename，避免中断产生半个项目。

## Sample 导入策略

导入阶段识别 WAV、AIFF、FLAC 和 MP3/MP4 音频轨。压缩格式解码成规范化缓存 WAV：PCM source、sample rate、channel layout、decoder name/version、original hash 都写入 sample metadata。不能静默覆盖用户原文件；失败时给出 asset path、format 和稳定错误码：容器/编码/位深/声道布局无法表示时报 `SampleFormatUnsupported`，content hash 不匹配或资产不可读时报 `AssetUnavailable`。

编辑只写 `SampleEditSpec`，保持原始资源不可变。`normalize`、fade、crossfade、trim 等在 prepare/offline cache 阶段应用；实时播放引用已经准备好的 PCM segment。

当前交付的是只读导入入口：`@oxitone/samples#importSample` 调用 Rust `inspectSample`，
按文件签名识别 WAV/AIFF/FLAC/MP3/MP4/M4A，完整解码后报告源文件 SHA-256、解码维度、
decoder 名称及声道转换。PCM 的采样率保持原值，SRC/trim 在 prepare 应用；大于 2 个
声道按既有降混规则输出 stereo，帧数表示解码结果（压缩格式可包含 codec padding）。
WAV/AIFF decoder 标识为 `oxitone-wav-v1`/`oxitone-aiff-v1`，压缩格式沿用
`symphonia 0.5/<codec>`。文件系统路径支持 Unicode 和空格。

此入口不写原文件或缓存 WAV；每次调用暂存完整解码 PCM 后释放，prepare 会再次解码。
`provenance` 返回给调用方，但当前项目 `SampleRef` 不保留该字段。上文的规范化缓存
落盘、provenance 持久化和原子项目保存仍未交付。默认绝对 URI 适合内存项目；显式
`assetBaseDir` 可返回该目录内的相对路径，并在 render 时以相应 base 解析，尚不等同于
项目目录的完整保存/加载。

## WAV 导出

`renderWav` 默认 32-bit float little-endian WAV，支持 16-bit PCM 和 24-bit PCM。写入 RIFF/WAVE header、fmt、data；文件大小超过 RIFF 限制时返回 `WavTooLarge`，Phase 1 不隐式切 RF64。render options 中的 start/end 用 bar/beat/timecode/marker 之一，不能混用；`tailSeconds` 明确是否渲染效果尾音。

- **Dither**：bit depth 降低只在导出边界发生。16/24-bit 导出默认加 1 LSB TPDF dither（`dither: 'none'` 可关闭）；32-bit float 不加 dither。dither 使用项目 seed 驱动的 versioned PRNG，保证相同快照导出字节一致。
- **Stem 导出**：`stems: 'mixer-channels'|'tracks'` 时 `path` 是输出目录，每个 mixer channel（或 track 对应 channel）一个 WAV，从对应 bus 的 post-fader/post-inserts 位置 tap；master 文件始终附带。所有 stem 与 master 使用同一 RenderGraph、同一 seed、同一 transport 区间，stem 之和必须与 master 混音在数值上一致（容差内），PDC 对齐关系保持一致。
- **Render report**：每次导出返回每个文件的 `durationSeconds`、`peakDbfs`、4x oversampled `truePeakDbfs`、EBU R128 `integratedLufs`，以及图内部总延迟 `graphLatencyFrames`。loudness 计算与导出共用同一遍渲染，不允许二次渲染引入差异。
- Metronome 默认不进导出，`includeMetronome: true` 时混入 master 与所有 stem。

## MIDI 导出

输出标准 MIDI File Type 1，PPQ 默认 960（可配置但必须记录）。Conductor track 写 tempo/time-signature/name；markers 以 marker meta event 写入 conductor track。每个 Oxitone Track 写 note-on/off 和声明的 CC automation。Note 时间由 beat 按 `round-half-up(beat * PPQ)` 转 tick（与 transport 的 frame 换算同一确定性四舍五入规则，以精确有理数计算），并修正同 tick 的 event ordering：同 tick 上 note-off 先于 note-on；tick 舍入产生的零时长 note 保证其 note-on 先于自身 note-off。Note velocity 0..1 映射为 `1 + round-half-up(v * 126)`（即 1..127，0 不映射为 0 以避免被部分接收端当作 note-off）。MIDI export 不读取外部 MIDI，也不导出不可映射的 synth/effect 参数；报告 skipped automation diagnostics。

展开规则与 transport scheduler 一致（`loopCount`、exclusive `lastBeat` 裁剪、`transpose`、`velocityScale`、`enabled`、`probability` 使用同一 `hash64(project seed, clip id, loop iteration, note ordinal)` 派生，导出结果与渲染一致），但 swing 不进入 MIDI 导出：swing 是调度层行为，导出的是编排数据，使用未加 swing 的 beat 位置。含 note 的 Track 定义为 enabled 且展开后至少含一个 note 的 Track；只有这类 Track 分配 channel 并生成 SMF track（SMF track 顺序按稳定 track ID 排序）。

MIDI 只有 16 个 channel，分配规则如下：

- 含 note 的 Track 数不超过 16 时，按稳定 ID 顺序自动分配 channel 1..16；Track 显式 `midiChannel`（1..16，可共享）优先，自动分配取未被显式占用的最小编号。
- 超过 16 个时导出失败，错误码 `MidiChannelLimit`，错误详情列出全部未分配的 track ID。用户必须在 Track 上显式设置 `midiChannel`（1..16，允许多个 Track 显式共享同一 channel）后重试；只要存在未分配的 note Track 或 distinct channel 数超过 16，导出同样以 `MidiChannelLimit` 失败。
- channel 10 仅按 GM 惯例建议用于打击乐，engine 不强制，也不在自动分配中特殊处理。
- 自动分配结果必须写入导出诊断，保证用户能知道每个 Track 落在哪个 channel。

变速 tempo 的导出：SMF 只支持离散的 tempo event。导出使用有效 tempo——tempoMap，或存在 tempo lane 时的烘焙结果（`03-audio-runtime-spec.md`）。`step` 变化精确写入；连续变化（tempoMap 的 `linear`/`exponential` 段和 tempo lane）按 `tempoEventResolutionTicks`（默认 PPQ/8，即 32 分音符一个 event）重采样为离散 tempo event 序列，tick 换算与音频使用同一有效 tempo map 和同一确定性舍入规则。resolution 可在 export options 覆盖并记入导出诊断；文档必须说明接收端重放时是阶梯近似，这是 MIDI 格式的固有边界而非实现缺陷。

## npm 与原生包

公共入口 `oxitone` 提供 TS API 和按平台解析原生包。平台包命名：`@oxitone/native-darwin-arm64`、`@oxitone/native-darwin-x64`；包内带 ABI/protocol manifest 和校验 hash。安装失败需说明平台、架构和可用 fallback；禁止运行时从网络下载未知二进制。macOS notarization/signing 作为发布门禁，开发构建可明确标记 unsigned。

## 兼容与迁移

任何 format/protocol major bump 都提供迁移器和 changelog；minor 字段必须有默认值。旧项目导入时先验证，再迁移到内存中的最新 snapshot，写回由用户显式触发。项目文件不包含 native cache、设备路径、临时 meter 或 playback position。

## 预设（Preset）

预设是命名的参数包，文件为 `*.oxitonepreset.json`，canonical JSON 编码：

- 插件预设：`{ formatVersion, pluginId, pluginVersion, parameters, resources? }`；加载时校验 pluginId 与 ABI 版本，未知参数报错而非忽略。
- Channel 预设：在插件预设外加 `effectChain`、`level`、`pan`、`swing`，用于整条 channel 的保存/复用。
- 预设只描述参数，不包含音频资产本体；引用的资源用相对 URI + hash，与项目文件同一规则。
- 预设与应用都不触发 graph 之外的副作用；应用到正在播放的 engine 时与普通参数变化走同一队列和平滑路径。
