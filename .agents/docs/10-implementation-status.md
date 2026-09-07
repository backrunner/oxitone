# 计划与实现核对（更新至 2026-09-07）

最初以 `9423ad3` 为核对基线；下表更新为当前实现，后文保留各阶段证据。
提交继续使用 `BackRunner <dev@backrunner.top>` 与 `type(scope): description`。
表格依据源码、正式测试和分发目录核对；“已有”不等于整个里程碑通过验收。

| 里程碑 | 已有实现与依据 | 尚未完成或缺少验收证据 |
| --- | --- | --- |
| M0 工程与协议 | pnpm/Cargo workspace、版本协议、canonical fixtures、N-API smoke tests、macOS arm64/x64 CI（locked install/build/schema/tests/examples/bench） | 远端 CI 首跑尚无记录；最低 macOS 13 runtime 验收仍待完成 |
| M1 时间轴/MIDI | Project/Track/Pattern/Clip、Chord/Arp、tempo/time-signature、Track tempo/enabled/midiChannel、确定性 SMF writer 与边界测试 | 当前已识别的 authoring 缺口已关闭；持续维护确定性/边界回归 |
| M2 音源/采样 | Rust synth/Sampler/Multisampler/Slicer、解码/编辑/SRC、SampleClip stretch/repitch、C ABI、TS Sample/Clip/fit 和四种内置音源入口、缓存/provenance；Slicer repitch tempo map/lane 跟随；CC0 钢琴 demo | 当前已识别的功能缺口已关闭；全规格 golden 和发布环境验证继续追踪 |
| M3 Mixer/Automation/导出 | TS mixer/insert authoring，Rust mixer/PDC/12 effects、完整 insert 自动化/host 参数路径、tempo bake、WAV/stem/loudness 与回归测试 | 全规格 golden/PDC/export 的自动化发布门禁仍需建立和复核 |
| M4 实时与设备 | CoreAudio HAL、render-ahead/direct、transport/loop、设备适配/诊断、Session 换图及 bar/beat/marker/timecode 入口、换图回收与模拟设备测试 | 修正循环负载后的 10/60 分钟 soak、真实设备切换/拔插和 callback 指标仍需验收 |
| M5 npm/DX/插件 | 统一 `oxitone` authoring/native/sample 入口，Project/Session 动态插件注册，descriptor 查询，instrument/effect/Channel preset；CLI、便携工程与有声示例 | npm 平台包/发布/签名公证、干净安装验收和逐节点 deadline watchdog |
| M6 Preview | runner/watch、带版本 IPC、GPUI arrangement/piano/channel rack/mixer/scopes/transport、原生换图与错误恢复、插件多窗口与声明式原生布局/固定 Mix、CLI preview、unsigned 开发 app bundle | 完整物理交互验收、独立 NSView companion/effective 参数遥测、正式 npm 平台包、签名分发及大工程虚拟列表继续追踪 |
| M7 稳定性/发布 | 定向回归、插件 conformance、基准 harness | fuzz/sanitizer、持续负载 endurance、故障注入/资源上限、SBOM/签名公证和自动发布门禁 |

规格中的项目目录保存/读取（formatVersion、资产相对路径、原子写入）已在后续阶段提供，
详见下文；preset 已在 2026-09-07 后续阶段提供。canonical snapshot 编解码本身不能替代这些功能。

## 审查与性能记录的解释

本轮新增两首完整 demo 与钢琴（2026-09-07）：

- `examples/drum-machine/src/full/`：Lofi 80 BPM / 60 bars / 183 s，Melodic Dubstep
  140 BPM / 104 bars / 181.286 s（均含 3 s tail）。原创主题、段落/收尾、独立轨道、return、
  sidechain 与自动化均以 SDK authoring 表达；WAV/MIDI/snapshot/report 写入 ignored target。
- `multisampler()` 与 native `oxitone.multisampler`：1…256 区域、32 声部、键位/力度查表、
  transpose/ADSR/loop。非法范围、重叠、缺失资源、音高/力度、保存恢复音频一致性与零分配 seek/抢占有测试。
- 显式 `pnpm example:songs:prepare` 下载固定 commit、逐文件 SHA-256 校验的 CC0 VSCO upright，
  13 key zones × 3 dynamics（MIDI 39…91），普通 build/test 不下载。钢琴和动态鼓机有 GPUI 面板，
  两个工程入口支持既有 watch 与最后有效状态恢复。
- `pnpm build/lint/typecheck/test/schemas`、`cargo fmt --all --check`、`cargo test --workspace`
  通过（217 TS / 447 Rust tests）；两首实际 WAV 的 LUFS 为 -18.23 / -16.14、true peak 为
  -4.22 / -3.68 dBTP，native drum faults 均为 0。GPUI 工程截图及钢琴/效果器独立窗口重开检查通过。
- 39 区域/32 声部/128 frames 微基准：reset+首块 1.509 ms、持续处理 1.137 ms；
  [归档](../../benchmarks/results/2026-09-07-full-songs-multisampler.json)。这是内存 microbench，
  未测物理设备 callback/CPU 占用/xrun，不代替 M4 实时验收。

- `.agents/reviews/2026-09-05/` 是历史审查，保留当时失败证据，不代表当前 HEAD
  仍有全部 19 项缺陷。`crates/render/tests/regressions.rs` 已纳入原有 17 个定向
  复现；回收/队列和插件用例在其余正式测试中。当前通过情况由新一轮检查记录确认。
- CHANGELOG 已说明旧 M4 soak 在有限内容结束后主要渲染静音，不能作为持续 DSP
  负载验收。当前 harness 循环内容，但尚无新的长期验收记录。
- 本表不把源码存在视为性能达标，也不把本机检查视为干净 npm 安装验收。

## 推进顺序

1. **本轮已完成（2026-09-06）**：TS mixer/Channel insert authoring，覆盖快照隔离、
   revision、非法路由和实际 native WAV 输出；复用现有协议 1.0，不改变 DSP callback。
2. **已完成入口（2026-09-06）**：Sample/SampleClip 与 fit helpers 已补齐 TypeScript
   authoring 和 snapshot 连接；Track `enabled`/`midiChannel` 与只读文件导入 facade 已完成。
   Track `tempo` 的 authoring、独立时钟换算及音频/MIDI 验证已于后续阶段补齐。
3. insert 参数自动化、Session 换图/播放位置、项目持久化与可编辑恢复已完成；继续预设。
4. macOS CI 和用户示例已建立；继续首跑记录、npm 平台包与持续有声负载/设备验收。
5. 实现 Preview，完成 fuzz/endurance/发布门禁。各项出口分别记录证据。

## 本轮落地与验证（2026-09-06）

- 已新增可编辑的 `project.master`、`addMixerChannel`、Channel 路由/效果链、
  pre/post-fader send、sidechain send、bus/send automation 和 swing/mute/solo。
  Master 与跨项目引用规则在 authoring 边界校验，完整反馈环由 Rust compiler 拒绝。
- 新增 5 个 authoring 测试和 5 个真实 native WAV/compile 集成测试；后者验证极性效果、
  dry/wet 与 bypass、发送增益、pre/post-fader、detector 不混入音频、send 自动化、
  stems、cycle path 和无效插件参数。具体 API 见 `04-api-contracts.md`。
- `pnpm build`（含 release native addon）、`pnpm lint`、`pnpm typecheck`、
  `pnpm test`（142 tests）、`cargo fmt --all --check`、`cargo test --workspace`
  （372 tests）通过。性能敏感 DSP/实时路径未修改；混音微基准不替代长期设备验收。
- Apple M4 / macOS 27.0 / Rust 1.98.0，48 kHz / 128 frames：8 bus、sidechain、
  每 bus 4 EQ 的 Criterion slope estimate 分别为 10.01 / 14.69 / 159.24 μs；
  warmup 1 s、measurement 3 s、30 samples，各场景未检测到显著回退。
  详见 [基准归档](../../benchmarks/results/2026-09-06-mixer-authoring.json)。callback
  p95/p99、设备与 xrun 未在微基准中测量，记录为 null，不视为 0。
- 本节保留起始基线的缺口以便对照；M3 的 TS mixer/Channel insert 空入口在本轮
  关闭，其余列出的缺口继续追踪。下一步优先 Sample/SampleClip authoring 与导入 facade。

## Sample/Track 后续推进（2026-09-06）

- 已提供 Track `enabled`/`midiChannel`，用真实 native MIDI 导出验证禁用 Track 和显式
  channel 分配；跨项目同 ID 的 Channel 不再能被误绑定。
- 修复 Sample bigint 帧数、显式 ID、trim 范围及音乐长度校验，失败构造不注册实体；
  draft 可重试。`fitBars` 使用起始拍号且保留完整长度，`fitToContent` 使用 trim 后帧数。
- 此阶段最初遗留的 `fitToContent` tempo ramp/lane 换算和 repitch 显式 duration
  缩放，已在 `ab36241` 关闭，并添加有效时钟、onset 和 WSOLA reset 验证。

- 新增 `@oxitone/samples#importSample` 和无 engine 的版本化 `inspectSample` 命令。
  Rust 返回源 hash、格式、解码维度与 provenance，JS 不持有 PCM。完成 WAV/AIFF 识别、
  6 声道降混、AAC/M4A 真文件、24→48 kHz SRC/trim、相对目录迁移、文件变更/缺失、
  损坏容器及 native 协议错误测试；两次有声 WAV 渲染逐字节一致。
- 目前只提供元数据 descriptor 导入，未实现缓存 WAV 写入或 provenance 持久化；
  继续追踪项目保存/加载、内置音源便捷入口和原表中的其余缺口。
- `pnpm schemas`、`pnpm build`（release native addon）、`pnpm lint`、`pnpm typecheck`、
  TS 测试（168 tests）、`cargo fmt --all --check`、`cargo test --workspace`（382 tests）
  通过。新增 timing query、tempo-lane loop 和 sample playback reset 测试也已通过。
- Apple M4 / macOS 27.0 / Rust 1.98.0，48 kHz stereo，30 samples：1 秒 PCM16
  inspector slope estimate 为 0.835 ms，10 秒 float32 为 10.477 ms。测量包含文件读取、
  hash、完整解码和释放，使用 warm filesystem cache；首次建立该场景基线，无回退结论。
  [基准归档](../../benchmarks/results/2026-09-06-sample-import.json) 记录置信区间、实际
  测量时长和环境；未测设备/callback/xrun 的值为 null，不替代长期实时验收。
- Apple M4 / macOS 27.0 / Rust 1.98.0，48 kHz stereo / 128 frames：repitch reset +
  首块输出 97.05 µs，WSOLA reset + 首块输出 311.65 µs；reset/replay 分配计数为 0。
  [播放基准归档](../../benchmarks/results/2026-09-06-tempo-sample-playback.json) 记录了
  测量参数和未覆盖的 callback/xrun 限制。

## Track 独立 tempo（2026-09-06）

- Track setter、Rust 局部时钟、Pattern scheduler/MIDI 映射、SampleClip 三种播放模式、
  fitToContent、loop 截止点与 timeline end 已接通。step/linear/exponential/tempo lane
  下的 MIDI conductor 实际重放时间与音频事件以明确容差对拍；全局斜坡改变时局部
  SampleClip 音频逐样本一致。callback reset/seek 的分配和释放均为 0。
- native release build、lint、typecheck、TS 170 tests、Rust 384 tests、fmt 通过。
- [基准记录](../../benchmarks/results/2026-09-06-track-tempo.json)：30 samples，Track
  repitch reset/首块 173.27 µs、stretch 447.15 µs。首次测量受并发负载显著影响，
  同二进制复测仍高于早期基线；回归验收暂不下结论，需受控环境复测。未测 callback/xrun。
- M1 原表中 Track 三个 authoring 属性缺口均已关闭，M2 的缓存导入、内置音源便捷入口，
  M3 insert automation 与 M4 Session 等其余能力仍按推进顺序执行。

## Session 更新与位置（2026-09-06）

- `Session.update` 保留 engine ID；编译拒绝保留旧版本和导出。Project.play 自动更新
  authoring revision，Session 直接操作使用编译版本。释放幂等且 Project 不再复用已释放 Session。
- bar+beat、marker ID、seconds、frame(s) 起播/seek 均已暴露，位置按编译快照解析；
  Rust 按实际 engine sample rate 转换秒数，检查越界/互斥，失败保留游标。首次 play 前
  重编译现在也保留 transport 状态。真实设备切换和持续负载验收仍是独立缺口。
- release native build、TS 175 tests、Rust 385 tests、lint、typecheck、fmt 通过。
  4 个 Session 测试与 native smoke 覆盖快照隔离、失败更新、位置和生命周期；复用
  Rust worker/direct 的换图、游标、队列和回收测试。SDK play 转发用 spy，不视为设备 soak。
- [控制线程基准](../../benchmarks/results/2026-09-06-session-update.json)：16 tracks /
  32 lanes 编译计划 2.843 ms（95% CI 2.789..2.898），未测 native 往返、换图延迟或 callback。

## 项目目录与资源迁移（2026-09-06）

- `Project.save`、`saveProject`、`loadProject` 提供 formatVersion/projectId、canonical
  manifest、内容寻址资产、hash 校验与 temp/fsync/原子发布。目录移动且源素材删除后，
  native compile 和 WAV 输出保持可用；二次保存的 manifest 与音频逐字节一致。
- compile、Project.compile、Session.update/renderWav 支持或保留 assetBaseDir。加载
  拒绝路径逃逸及损坏资源；保存失败保留已发布 manifest。TS 不解码 PCM，不改变 callback。
- release native build、schemas、lint、typecheck、fmt、Rust 385 tests 与 TS 178 tests
  通过。首轮 TS 有 6 项默认 5 秒超时；单 worker / 30 秒重跑全部 core/native 通过。
  其后系统临时卷 ENOSPC 阻止 MIDI suite 启动，迁到 BRData 临时目录后其余 suites 通过。
- [文件 I/O 基准](../../benchmarks/results/2026-09-06-project-files.json)：1 秒 stereo
  float32 资源、5 次预热、30 次测量，保存 median/p95 为 54.00/175.74 ms，加载为
  5.59/45.39 ms。BRData 卷、并发主机负载；首次基线，不代表回退或 realtime 验收。
- `Project.fromSnapshot` 与 `Project.load` 已恢复可编辑 builders，并保留稳定 ID、音符
  ID/顺序、有理数拍点、clip 默认字段和 tempo/automation wire state。预设、标准化 WAV
  缓存及 provenance 持久化继续推进。原表的 Preview、分发、长时设备与发布门禁仍未完成。

## 可编辑工程恢复（2026-09-06）

- `Project.load` 直接返回可编辑 Project，保留资源目录；compile、Session.update、导出与
  再保存默认复用该目录。省略的 Master、未放置 Pattern、note ID/顺序、精确 rational
  beat、插件 state、send/insert 顺序、automation loop/hold 与显式默认值均可往返。
- 使用 `revisionBigInt` 保存完整 u64 revision；旧 number getter 超出安全整数范围时
  明确报错。u64 耗尽时拒绝编辑且不修改状态。生成 ID 跳过恢复实体，显式重复 ID 拒绝。
- 新增 6 个恢复测试和 1 个 snapshot 测试，扩展目录迁移集成：有声 WAV、seeded MIDI
  恢复前后字节一致；移动目录后加载、编译、修改 gain、更新 Session 和保存到新目录通过。
  缺失资源/归属、重复 ID、无效 trim/duration 和版本边界均有拒绝测试。
- `pnpm build`（release native）、`pnpm lint`、`pnpm typecheck`、`pnpm test`
  （185 tests）、`cargo fmt --all --check`、`cargo test --workspace`（385 tests）通过。
  本阶段 TS 使用默认 timeout；临时目录仍在 BRData 卷。
- [恢复基准](../../benchmarks/results/2026-09-06-project-restore.json)：Apple M4 / Node
  26.5.0，32 Tracks/2048 notes/32 lanes，20 次预热、100 次测量；恢复加 snapshot
  median/p95/p99 为 11.44/12.61/12.80 ms。首个控制线程基线，不包含 native compile、
  callback 或 xrun；Rust 执行路径未变，不替代 M4/M7 的实时验收。

## Insert 参数自动化（2026-09-06）

- Channel/MixerChannel/Master 的 `insert.<index>.mix/bypass` 与
  `insert.<index>.parameter.<pluginParameterId>` 已统一支持 automation 和
  `setParameter`，控制线程校验 descriptor、索引、automation 标志和物理值范围。
  同时修复 send ratio host target 误拒绝与 Master outgoing ratio 的错误接受。
- 初始值先于 host event 和 automation；beat 参数可随有效 tempoMap/tempo lane
  换算，并支持 seconds/beat 模式切换。新目标只携带整数索引，参数暂存空间按
  descriptor 预分配并合并同帧更新，避免原 Channel 64 事件截断。
- 新增 10 项 Rust integration 场景覆盖三种宿主、首块优先级、64/128/256 block
  的 frame 120/240 边界、PDC、beat/seconds、范围和零 allocation/free。
  320 参数队列测试通过 Miri；TS native WAV 与真实 C 动态插件测试覆盖 facade parity。
- release native build、TS 187 tests、lint/typecheck、fmt 和专项 benchmark 通过。
  Rust workspace 的 `jitter_within_horizon_causes_no_underrun` 在当前机器失败；
  原提交 4bcc217 的隔离工作树也出现 6 xruns，独立 render 包测试曾通过。
  该既有模拟调度问题单独追踪，不能记作全量检查通过或实时 soak 验收。
  对该项显式 skip 后，其余 Rust workspace 检查全部通过；没有放宽断言或隐藏失败。
- [专项基准](../../benchmarks/results/2026-09-06-insert-automation.json)：Apple M4，
  1 Track / 3 buses / 4 inserts / 4 lanes，compile mean 32.03 µs、resolve 51.60 ns、
  render 128 frames mean 168.99 µs。没有设备、callback 或 xrun 测量。

## 实时缓冲深度与模拟调度修复（2026-09-06）

- 同采样率 worker 现在可以填满配置的 4-block ring；此前无条件预留 SRC 余量，
  导致实际最多填入 3 blocks。重采样分支仍保留额外帧余量，设备链重建后判断随之更新。
- 模拟 sink 调度落后时等待新 period，不连续补拉。隔离 jitter 注入在 horizon
  恢复后才继续，避免把恢复期的暂停累积成持续过载；超过 horizon 的单次暂停仍
  由 extreme-jitter 测试验证错误计数与 transport 连续性，没有放宽 xrun 断言。
- 新增 4 项确定性测试验证完整填满/补满、SRC 余量和正常/延迟时钟调度。
  原先失败的 jitter 用例现已通过；`cargo test --workspace` 全部 400 tests 通过，
  无 skip/ignore。`pnpm build`、187 项 TS tests、lint/typecheck、fmt 全部通过。
- 修复短基准 `--tracks 1` 时重复 lane target 的配置，并记录实际 automation lane 数；
  simulated 报告不再把系统默认设备标成受测设备。
- [10 秒模拟基准](../../benchmarks/results/2026-09-06-buffered-horizon.json)：
  Apple M4 / 48 kHz / 128 frames，1 Track / 4 lanes，隔离 jitter probability 0.3、
  max 2 periods、seed 11；3,761 blocks，xruns/deadlineMisses/NaN/queueDrops 均为 0。
  worker p99 直方图桶上界 1.049 ms，deadline 2.667 ms，ring 观测达到 512 frames。
  这不关闭真实 CoreAudio、10/60 分钟长测、设备拔插或资源预算的验收缺口。

## 标准化采样缓存与来源持久化（2026-09-07）

- `importSample(path, { cacheDir, assetBaseDir? })` 与版本化 native `cacheSample`
  在 Rust 控制线程生成内容寻址的 float32 WAV；原采样率、解码 PCM、有效 forward loop
  保留，源文件不变。发布使用 temp/fsync/hard-link，拒绝损坏/链接缓存，失败清理临时文件。
- 两种导入的 provenance 现在经 SampleRef、TS/Rust snapshot、可编辑恢复和项目文件
  保存；原 hash/format 与缓存 hash/format 分开，无机器路径。实际 AAC 文件转码后删除
  源文件、移动并再次保存工程，原始/缓存/恢复后的 native WAV 逐字节一致。
- 修正 WAV smpl 的 inclusive endpoint 到内部半开区间转换，避免少播放最后一帧。
  覆盖 mono/stereo/downmix、AIFF、并发发布、写入中断、hash 损坏和非法 metadata。
- `pnpm schemas`、release native build、lint/typecheck、191 TS tests、fmt 和
  `cargo test --workspace` 的 406 tests 全部通过，无 skip/ignore。
  本机全局 pnpm 11.25.0 的自动切换缓存损坏，使用任务缓存中安装的仓库指定 11.11.0
  完成检查，未修改项目 packageManager 或全局安装。
- [控制线程基准](../../benchmarks/results/2026-09-07-sample-cache.json)：Apple M4 /
  48 kHz stereo PCM16 1 秒，30 samples；缓存复用 10.49 ms、首次发布 7.25 ms。
  包含完整读/解码、WAV 写入、hash 与 fsync；复用另校验已有文件。首次基线，主机并发
  负载下测量，不代表 realtime 或设备验收；callback、设备、xrun 未测。

## 内置音源入口与 Slicer 速度跟随（2026-09-07）

- `wavetable`、`sampler`、`slicer` 提供类型化选项与严格校验；Channel instrument 可替换。
  帧标记保持 u64 精度，完整选项经真实 native compile/render 验证，Slicer 工程保存、
  删除源文件后恢复，WAV 逐字节一致。
- Slicer repitch 更新新触发和活跃 voice，支持 step/linear/exponential 和 tempo lane；
  显式音乐长度、trim、reverse/rate、缺省时钟和 Track override 规则均有验证。
  超范围 tempo 因子、非法模式和宿主保留参数赋值提前拒绝。
- 修复 Sampler/Slicer seek 未清零 ADSR 导致重放跳过 attack；重放逐样本一致，
  seek/process allocation/free 均为 0。音源参数队列按 descriptor 预分配，128 参数
  加同帧 host/automation 优先级通过，原先 64 事件截断已移除。
- release native build、schemas、lint/typecheck、196 TS tests、fmt 和 Rust workspace
  的 412 tests 全部通过，无 skip/ignore。新增 6 个 Rust integration 和 5 个 TS 测试。
- [专项基准](../../benchmarks/results/2026-09-07-slicer-tempo.json)：Apple M4 / 48 kHz /
  128 frames，单 voice 的整图 reset/首块为 49.24 µs，持续有声段为 55.35 µs。
  首轮与构建/测试重叠，已在结束后复测；新场景首次基线，无前提交回退结论，未测设备
  callback/xrun，不替代 10/60 分钟验收。

## CI、离线示例与便携 CLI（2026-09-07）

- 新增 macOS 15 arm64/Node 24、Intel/Node 22 的 CI；构建原生 addon/SDK/示例，
  检查 schema 漂移，执行全部 TS/Rust tests、示例及 Slicer/insert benchmark。
  CoreAudio 冒烟使用明确选定的 BlackHole 虚拟输出；不替代真实设备验收。
- `examples/offline` 已生成 synth/automation/delay 的两小节 WAV/MIDI，再缓存并
  Slicer 重排素材，经历 150→180 BPM ramp。保存/恢复后的有声 WAV hash 一致。
  README、API 使用指南、迁移和 CI 说明已建立，明确当前 workspace 与 npm 分发的区别。
- CLI 读取工程目录/标准 manifest 或 snapshot。相对素材基于输入 JSON 的目录解析，
  移动工程、异 cwd 调用、WAV/MIDI 与 SDK parity、版本/损坏/缺失输入、失败保留输出
  均有真实子进程/native 验证；stderr 现在提供结构化 JSON 错误。
- actionlint、shellcheck、frozen install、schema drift、native build、lint/typecheck、
  198 TS tests、412 Rust tests、fmt 与离线示例全部通过，无 skip/ignore。
  远端 CI 尚未执行，不记作干净 runner 验收。
- [文件加载专项基准](../../benchmarks/results/2026-09-07-cli-project-files.json)：
  1 秒 stereo float32 资产，5 次预热/30 次测量，保存 median/p95 17.30/64.60 ms，
  加载 0.59/0.66 ms。验证 CLI 复用的项目 I/O 层，未测子进程启动或设备 callback，
  不作为实时或远端 CI 验收证据。

该阶段剩余重点（历史记录）：预设；CI 首跑及 npm 单包/平台分发；Preview runner/GPUI；
持续有声负载及设备拔插、资源预算/watchdog、fuzz/sanitizer/SBOM/签名公证等发布门禁。
这些项仍未完成，逐项实现和记录出口证据后才能关闭对应里程碑。

## 动态音源/效果器联调与自带鼓机音乐（2026-09-07）

- 增加未发布示例 crate `oxitone-example-drums`，输出真实 Rust cdylib。四个固定
  原生合成声部支持力度、同 pad 重触发、开闭镲 choke、volume/decay 参数与 reset；
  无外部采样。C 函数表复用公开 ABI 类型，process/reset/tail 分配与释放计数均为 0。
- `examples/drum-machine` 经真实 N-API → Rust → drum cdylib → C gain 动态库 → WAV
  验证：unity 与 dry PCM 一致，音源音量/效果器增益减半均为 6.0205999 dB；初始值、
  host、automation 的输出一致，automation 覆盖同帧 host，64/128/256 block hash
  一致，衰减参数影响音频，非法参数拒绝，两插件 fault 均为 0。
- `pnpm example:drums` 输出原创《Midnight Circuit / 午夜回路》：112 BPM、16 小节、
  约 36.29 秒，含鼓机、贝斯、和声、旋律、breakdown、过门和淡出；另有鼓机独奏、
  MIDI（鼓为 channel 10）、snapshot、可恢复项目及报告。恢复 WAV 逐字节一致。
  原生分析 -17.05 LUFS / -2.05 dBTP，FFmpeg 独立复核 -17.0 LUFS / -2.0 dBTP。
- release native build、lint、typecheck、fmt、199 TS tests、415 Rust tests 与
  actionlint 全部通过，无 skip/ignore。根 test 和 macOS CI 纳入动态链验证与示例。
- [专项基准与验证归档](../../benchmarks/results/2026-09-07-dynamic-drums.json)：
  Apple M4 / 48 kHz / 128 frames，真实动态 C gain adapter 约 0.301 µs，四声部
  drum C entry 约 7.33 µs；后者静态链接同一函数表，不含宿主 adapter，置信区间较宽。
  两项均为 microbenchmark；未测真实设备 callback、worker 长测或 xrun，不关闭
  上述发布门禁。鼓机为仓库自带开发示例，尚未发布为签名平台包。

## SDK 补齐与 GPUI 代码预览（2026-09-07）

- 核对 SDK public source 后补齐统一 `oxitone` 入口、Project/Session 插件注册与
  descriptor 元数据，以及 canonical instrument/effect/Channel preset。底层包改名
  `@oxitone/native` 避免依赖环；旧 `oxitone` 原生函数继续 re-export。采样预设目录
  移动、hash/路径校验、失败应用原子性、恢复前后有声 WAV 一致均有测试。
- 修复 TS/Rust 的 signed-only、adapt-device、follow-default 与 one-pole wire
  拼写差异；Rust 保留旧 camelCase 解码别名。Project 注册保留实际 hash，未来
  compile/render 和已有 Session 使用同一验证结果；动态鼓机链已验证高低层 parity。
- `oxitone preview` 使用 esbuild 本地依赖 watch、独立 tsx worker、150 ms 防抖、
  generation 取消、10 秒超时与 64 MiB 帧限制。Unix IPC 一帧 in-flight，慢编译时
  合并待发状态；重复内容跳过，native 拒绝后允许下一次 watch 重试。
- GPUI 链接原生 Rust 引擎，展示轨道/clip、钢琴窗、Channel/Mixer 路由和 peak/RMS、
  Master scope true peak、波形/频谱/XY，以及 marker/ruler/位置输入/loop transport。
  authoring 只读；有效 tempo/Track tempo 和 sample clip 编译边界用于呈现。
  Opt-in telemetry 预分配，FFT/true-peak 在 UI；满 ring 丢分析帧并计数。
- Native IPC 冒烟验证播放中换图、失败保持旧图且游标前进、旧 revision 拒绝、代码
  错误恢复、重复内容去重/失败重试、超时/过期执行和显式 runtime asset watch。
  macOS accept 继承 O_NONBLOCK 曾导致消息间隙断线，已修正并由多帧测试覆盖。
- Release GPUI 和 unsigned `.app` 已构建；鼓机工程在真实 GPUI 进程接受了 revision 1，
  IPC 返回 4 Tracks / 54 Patterns。当时机器锁屏、界面不完整，未记作视觉或真实设备播放验收。
  后续确认另有 GPUI `font-kit` 未启用导致文字后端为空，不能仅归因于锁屏（见下方修复记录）。`docs/preview.md` 提供直接运行的入口与限制。
- `pnpm build`（release native）、lint、typecheck、209 TS tests、421 Rust tests、
  rustfmt、schema 再生成无漂移、frozen install、actionlint/plist 检查通过，无 skip。
  CLI 用实际 `.app` executable 启动，保留 viewer PID 以保证退出时回收；真实 GPUI
  接受工程和进程/socket 清理通过。独立 1 秒 headless 鼓机播放游标达到 48,128，
  xruns/pluginFaults 均为 0，仅是模拟输出启动冒烟。
- [专项基准](../../benchmarks/results/2026-09-07-preview.json)：Apple M4 / 48 kHz /
  128 frames、4 Channels，30 samples；关闭采集 30.047 µs，启用并模拟约 30 Hz 消费
  31.514 µs（增加约 4.9%）。PCM 逐样本一致，process/seek allocation/free 均为 0。
  没有设备 callback p95/p99 或长期 xrun 测量，不关闭 M4/M7 的发布门禁。

当前仍需独立完成：远端 CI 首跑、macOS 最低版本和完整视觉交互、签名 npm 平台分发；
10/60 分钟有声负载、设备拔插、资源预算/watchdog、fuzz/sanitizer/SBOM/公证。
这些验收不能由本地源码与模拟输出测试替代。

## Preview 自绘标题区与系统外观（2026-09-07）

- 隐藏独立系统标题栏/标题文字，56 px 自绘区域整合工程名、只读标识和构建状态。
  保留原生交通灯，非交互区调用 AppKit 原生拖动，双击遵循 macOS 偏好；Scopes
  按钮不在拖动目标内，全屏回收交通灯留白。原生对象仅在 UI 线程短暂持有。
- 启动读取窗口 appearance，并订阅系统 Light/Dark（含 Vibrant）变化；所有面板、
  琴键、轨道颜色、meter/scope、诊断和 hover/active/focus 共用语义主题。
  外观更新只通知重绘，不修改 snapshot、编译图或音频状态。
- 两套主题通过文字 ≥ 4.5:1、分析信号 ≥ 3:1 的对比度检查，覆盖 clip 混色背景。
  `pnpm lint`、`pnpm typecheck`、`cargo fmt --all --check`、422 Rust tests 均通过，
  无失败/忽略。Release unsigned app 已重建，Info.plist 校验通过。
- 新 bundle 经 CLI 加载鼓机工程，IPC 返回 revision 1、4 Tracks / 54 Patterns；
  显式退出后 viewer 和 socket 目录正常回收。此冒烟未启动真实设备播放。
- 专项基准：Apple M4 / 48 kHz / 128 frames / 4 Channels，1 秒预热、3 秒测量、
  30 samples；baseline 30.167 µs、telemetry 31.629 µs。Criterion 分别报告无明显
  变化/噪声阈值内变化；不测 UI 布局、设备 callback p95/p99 或长期 xrun。
- 当前桌面仍锁屏，原生拖动/交通灯/全屏及运行中主题切换的视觉交互验收待未锁屏
  桌面完成；配色测试和进程冒烟不能替代这些项目。

## Preview 文字修复、Playlist 与 Mixer 设计及浏览交互（2026-09-07）

- 确认并修复文字缺失根因：关闭 GPUI 默认 features 后没有启用 `font-kit`，落入 dummy
  文字后端；现在启用原生字体与中文 fallback，并在 GUI 启动时检查字体目录非空。
  锁屏另外会抑制 display-link 刷新，两者不能混为同一问题。
- Playlist 使用固定 Track 侧栏/小节标尺、真实 note 缩略图、彩色 Pattern 标题，按
  首次出现为未命名 Pattern 编号并缓存标签，首次打开适配工程；长时间轴按可见范围生成刻度。
- 钢琴窗覆盖全部 128 MIDI keys，独立琴键/局部拍标尺/velocity lane，适配短音符及
  鼓机音符范围；双轴滚动、可拖动滚动条、时间/行高缩放、快捷键和 Loop clip 可用。
- Mixer 改为紧凑通道条、固定 Master、pan/level 只读图示和 peak/RMS 分段表；独立
  效果器/路由 inspector、双轴滚动、键盘选择并显示通道。上下/左右面板分隔线可拖动，
  Scope 增加网格、友好名称和暂停状态。浅/深色共同通过含 Pattern 标题混色的对比度检查。
- Opt-in 截图模式驱动真实 NSView 绘制后用系统窗口截图，不生成假界面、不启动播放、
  不改系统主题；深色 1440×920、浅色 1060×720 实际截图完成。GPUI 键盘分发与按
  实测布局坐标执行的滚动/拖动控制器冒烟通过，验证缩放、滚动条、Mixer 选择及自动滚动。
  物理鼠标/触控板、原生拖动/交通灯/全屏、运行中系统主题切换仍待未锁屏桌面验收。
- 全套测试暴露既有 extreme-jitter 测试竞态：xrun atomic 与事件队列独立采样，首次读到
  xrun 不能假定同批含 Underrun。修正测试为限时等待两者并保留游标继续推进断言；音频处理逻辑未变。
- [专项基准](../../benchmarks/results/2026-09-07-preview-ui.json)：Apple M4 / 48 kHz /
  128 frames / 4 Channels，baseline 29.628 µs、telemetry 31.390 µs；1 秒预热、3 秒
  测量、30 samples。未测 GPUI 布局/绘制或设备 callback p95/p99/xrun，不据此推断 UI 帧率。
- `pnpm lint`、`pnpm typecheck`、`cargo fmt --all --check` 与 427 项 Rust 工作区测试通过，
  无失败/忽略。包含 10 项 viewer 测试与扩展后的双主题对比度覆盖。
- Release `.app` 已重建并通过最终深/浅色截图与最小窗口导航冒烟，Info.plist 校验通过。
  同时修复开发 bundle 原地覆盖旧 Mach-O 后的 `SIGKILL (Code Signature Invalid)`：
  构建脚本使用临时文件 + rename 创建新 inode。连续两次构建后由 CLI 直接启动 bundle
  executable、加载鼓机与截图退出均通过；仍是未做 Developer ID 签名/公证的本地开发包。

## 音源/效果器多窗口详情与自定义 UI 规划（2026-09-07）

- Mixer 音源条双击、音源 inspector 入口和 Channel/Mixer/Master 的效果器槽位可打开
  独立详情窗口；同地址复用窗口，不同地址可并排查看。沿用自绘标题区、原生交通灯和
  系统亮暗色。支持参数筛选、标签页、复制引用 JSON、滚动条及方向/Page/Home/End。
- 控制线程从已接受图的 registry 复制权威 descriptor、动态库路径与实际 SHA-256。
  显示 source/default 参数、范围/单位/平滑/rate/mapping、自动化绑定、host mix/bypass、
  资源与 structured state、插件能力和通道上下文；数值明确为初始配置，不冒充 live 参数。
  窗口不持有 DSP/动态库指针，不创建第二个音频实例，不改变音频 ABI 或处理路径。
- 有效 watch 更新所有详情；编译失败保留上次合法数据。效果器没有独立实例 ID，窗口
  按 owner + slot index 寻址；重排跟随槽位，删除显示未挂载，再出现恢复。详情关闭释放
  自己的订阅，主窗口关闭退出整个配对 session。
- 431 项 Rust 工作区测试（含 14 项 viewer）、lint/typecheck、rustfmt 均通过，无失败/忽略。
  新测试覆盖 source/default、自动化作用域、同插件多槽位/Master、重排/删除/返回及拒绝换图。
- 真实 GPUI 深/浅色截图检查了动态音源、gain 效果器/库信息与 27 参数 Wavetable；
  键盘滚动、重复打开、关闭重开均通过。截图模式按 AppKit 生命周期分步操作，避免创建同一
  dispatch 立即销毁造成旧窗口的初始原生绘制残留；不修改 GPUI 依赖或隐藏错误日志。
- `scripts/smoke-preview-details.mjs` 在真实鼓机/gain dylib 详情均打开后修改临时 TS 入口，
  两窗口跟随 revision 2，volume 从 0.85 变为 0.42。Release unsigned `.app` 重建及 plist
  检查通过，最终 bundle 再执行 watch 冒烟；主窗口的 AppKit close 路径可在详情仍打开时退出。
- [专项基准](../../benchmarks/results/2026-09-07-plugin-details.json)：Apple M4 / 48 kHz /
  128 frames / 4 Channels，baseline 30.999 µs、telemetry 32.359 µs。相对历史记录 Criterion
  报告 +4.74%/+3.21%；基准代码/二进制未变，且不链接 viewer，不能归因于 UI 变更或据此
  宣称帧率表现。没有测多窗口并发音频负载、真实 callback p95/p99/xrun 或物理桌面交互。
- `11-plugin-ui.md` 明确当前仅交付通用详情 P0；后续 P1 声明式 GPUI 布局、P2 独立版本
  native companion UI C ABI、P3 有界实时反馈分别列出契约/所有权/主题/watch/fallback 与出口。
  自定义 UI 注册、schema、symbol、第三方 UI 进程隔离与签名分发仍是规划，未声称已实现。

## Mixer 路由呈现与整体视觉优化（2026-09-07）

- Mixer 通道条统一为 92 px，重新绘制 pan、推子和 peak/RMS 表，改善名称、读数、
  选中态、边框及留白。固定 Master，底部显示输出/send 数，关联通道标为 IN/OUT。
  右侧详情按宽度适配，Chain 卡片展示音源/效果器、mix/bypass，Routing 优先展示 sends。
- 路由来自已接受快照：直接输出、独立 Master 比例（含 0）、aux/sidechain、输入来源、
  比例/dB、pre/post tap 与自动化标识。Channel 下游发送注明所属 bus，侧链正确显示
  post-insert/pre-fader detector tap。点击连接跳转并显示目标；反向输入可回到来源。
  路由模型随快照缓存，watch 接受时更新、拒绝时保留，测试覆盖旧快照与反向输入稳定性。
- Expand/Split 切换整个编辑区的 Mixer/钢琴窗，保留浏览状态；宽屏空余区展示最多三条
  连接的 signal-flow 图，完整列表在 Routing。右侧独立滚动条及键盘导航不修改音乐。
  钢琴窗窄布局使用两行工具栏，分隔线考虑自身宽度并保留 Piano ≥420 px、Mixer ≥520 px。
- 统一更轻的控件边框、圆角、线条图标与标签页；压缩 Transport/Playlist/Scope 的占用。
  插件参数改为双列数值卡片，保留来源、范围和默认值；Specs 展开完整技术元数据及自动化绑定。
- 新增可运行 `examples/drum-machine/src/mixer-preview.ts`，复用真实鼓机/gain dylib，包含
  Drum/Music bus、混响/延迟返回、pre/post send、自动化与侧链；原歌曲导出入口独立保留。
- lint/typecheck、rustfmt、433 项 Rust 工作区测试（含 16 项 viewer）通过，无失败/忽略；
  最后布局调整后再次通过全部 viewer 测试。未修改音频回调、DSP、协议或 authoring API。
- Release unsigned `.app` 重建及 plist 校验通过；真实 GPUI 深色展开、浅色最小窗口截图
  与发送跳转/反向输入/右侧独立滚动冒烟通过。最小窗口钢琴缩放/滚动与 Mixer 导航通过；
  最终 bundle 的多窗口 watch 冒烟验证两个 dylib 详情跟随 revision 2、volume 更新到 0.42。
- [专项基准](../../benchmarks/results/2026-09-07-mixer-design.json)：Apple M4 / 48 kHz /
  128 frames / 4 Channels，baseline 31.459 µs、telemetry 33.119 µs。Criterion 报告历史变化
  +2.43%（噪声阈值内）/+1.73%（回退）；基准二进制未变且不链接 viewer，不能归因于 UI
  或据此宣称界面帧率。仍未测真实设备 callback p95/p99/xrun、并发窗口有声长测。
  桌面锁定，物理鼠标/触控板、原生窗口操作与运行中系统主题切换仍待未锁屏验收。

## Preview 任意位置播放与鼠标/键盘快捷键（2026-09-07）

- Playlist 标尺从按刻度起点定位改为按鼠标坐标精确定位，扩展到 clip/空白轨道；
  钢琴窗标尺、音符区和 velocity 区均支持单击定位，双击/Option 点击直接播放。
  定位按当前有效 tempo map 与 Track tempo 换算，考虑 clip 当前重复轮次与截断边界。
- 新增 viewer cue：单击/Go 记录起点，Space 播放/暂停，Enter 从 cue 重播，Stop/Shift+Space
  停止并回到 cue。定位到循环区外关闭循环；区外启用循环则定位其起点，避免跳回旧区域。
  Option 方向键逐拍、加 Shift 逐小节；Command/Control+Home/End 到首尾，[/] 跳 Marker，L 切换循环。
- G 聚焦 Go，支持 bar.beat.tick（960 ticks）、秒数及 mm:ss；Enter 定位、Shift+Enter 播放，
  输入屏蔽全局快捷键，完成/取消恢复工作区焦点。播放切换忽略键盘自动重复，定位键可连续重复。
  音源/效果器窗口通过弱引用共享 transport 快捷键，原有无修饰滚动键保持可用。
- 顶部 Keys ? 和 ? 快捷键打开内置操作说明，深/浅色与最小窗口布局通过实际 GPUI 截图检查。
  Native 暂停/停止游标不再减去输出延迟；首次 play 前的 watch 换图保留 frame cursor/state/loop。
  所有改动位于 viewer/UI/控制线程，不改 authoring snapshot、TS 协议或音频 callback/DSP。
- 436 项 Rust 工作区测试（含 19 项 viewer）、lint/typecheck、rustfmt 通过，无失败/忽略。
  新测试覆盖小数定位、拍号/tempo 变化、重复/截断 clip、Marker 顺序、原生 seek→watch→play→pause。
  最后循环和焦点调整后再次通过 viewer 测试，Release unsigned bundle 已重建、plist 校验通过。
- 新 `OXITONE_PREVIEW_CAPTURE_TRANSPORT=1` 使用 GPUI 键盘分发、实测布局的鼠标控制器及真实
  Rust engine + simulated sink，验证定位、play/cue/stop、held-key、输入隔离、循环和多窗口控制，
  并断言 snapshot 不变；最终 release 在最小窗口通过，旧 piano/Mixer 导航与独立 inspector 冒烟通过。
  Debug 并行负载下曾报告 simulated underrun，Release 单独运行截图为 0；不据此推断设备长测。
  GPUI 首次复杂画面有自动扩大 instance buffer 后重绘的日志，最终截图正常；不隐藏该日志。
- 最终 bundle 的多窗口 watch 冒烟通过：鼓机与 gain 详情均跟随 revision 2，volume 更新到 0.42。
- [专项基准](../../benchmarks/results/2026-09-07-preview-transport.json)：Apple M4 / 48 kHz /
  128 frames / 4 Channels，baseline 30.809 µs、telemetry 34.489 µs。Criterion 历史变化
  -3.45%/+6.88%，telemetry 含 4 个 high-severe outliers；基准二进制未改且不链接 viewer，
  不用于归因 UI 性能或判断输入延迟。物理鼠标命中、真实设备播放/callback p95/p99/xrun 仍未验收。

## 紧凑插件面板、自定义布局与 watch 恢复（2026-09-07）

- `Project.registerPluginUi` / PluginUiManifest / 生成 JSON schema 已交付；布局独立于音乐
  快照和 DSP 注册，支持精确 plugin ID/version、分页、自动换行分组、旋钮、水平推子、
  enum 选项/开关、读数及 source ADSR 示意。范围/默认值/mapping 仍由权威 descriptor 提供。
- 效果器固定显示 host Mix/bypass、dry/wet 与自动化标识；Inspect 改为紧凑行表，
  移除重复大标题和常驻说明。Wavetable 覆盖全部参数；真实鼓机/gain dylib 示例注册自定义面板。
- UI-only 变化在控制线程比较音乐源摘要，复用 graph、telemetry、音频实例和 transport；
  不重置 DSP、不清空分析历史。布局失败保留同身份的兼容旧布局，否则局部回退；合法音乐仍可更新。
  修改代码时窗口显示 Building/Last good，直到 native 接受才显示 Synced；语法、runtime、
  超时、缺失插件和 native 拒绝都保留上份合法数据。页面 ID、用户滚动/尺寸按兼容规则保留。
- 442 项 Rust 工作区测试通过；另有 1 项默认忽略的 release 布局 benchmark 已显式运行通过。
  Protocol/Core/CLI 共 181 项 TS 测试、lint/typecheck、rustfmt 通过；schema 已生成。
  Release `.app` 与 plist 校验通过；实际亮暗主题、440 px 窄窗口/包络分页、键盘滚动、
  多窗口关闭重开及 source→语法错误→runtime 错误→native 拒绝→坏布局→恢复冒烟通过。
  冒烟期间发现并修复窗口创建时重复读取父 Entity 的 panic，以及效果器默认高度遮住数值的问题。
- [专项基准](../../benchmarks/results/2026-09-07-plugin-panels.json)：布局 8 / 256 controls 的
  median 为 4.666 / 95.125 µs，p95 为 5.084 / 134.417 µs；只测控制线程解析/校验，
  不代表 GPUI 绘制或 callback。音频 microbench 的历史 baseline 报告回退，telemetry 无显著变化；
  该二进制没有修改且不链接 viewer，原样归档，不将噪声数字解释为 UI 性能结论。
- P1 本次交付原生向量组件范围；独立 AppKit/Metal UI companion、图片资源、effective 参数
  遥测、物理桌面操作与真实设备长测仍单独追踪，未声称 VST hosting 或第三方 native UI 隔离。
