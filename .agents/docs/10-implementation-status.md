# 计划与实现核对（更新至 2026-09-07）

最初以 `9423ad3` 为核对基线；下表更新为当前实现，后文保留各阶段证据。
提交继续使用 `BackRunner <dev@backrunner.top>` 与 `type(scope): description`。
表格依据源码、正式测试和分发目录核对；“已有”不等于整个里程碑通过验收。

| 里程碑 | 已有实现与依据 | 尚未完成或缺少验收证据 |
| --- | --- | --- |
| M0 工程与协议 | pnpm/Cargo workspace、版本协议、canonical fixtures、N-API smoke tests | `.github/workflows` 缺失；干净 macOS 环境安装/构建门禁未建立 |
| M1 时间轴/MIDI | Project/Track/Pattern/Clip、Chord/Arp、tempo/time-signature、Track tempo/enabled/midiChannel、确定性 SMF writer 与边界测试 | 当前已识别的 authoring 缺口已关闭；持续维护确定性/边界回归 |
| M2 音源/采样 | Rust synth/Sampler/Slicer、解码/编辑/SRC、SampleClip stretch/repitch、C ABI、TS Sample/Clip/fit 和三种内置音源入口、缓存/provenance；Slicer repitch tempo map/lane 跟随 | 当前已识别的功能缺口已关闭；全规格 golden 和发布环境验证继续追踪 |
| M3 Mixer/Automation/导出 | TS mixer/insert authoring，Rust mixer/PDC/12 effects、完整 insert 自动化/host 参数路径、tempo bake、WAV/stem/loudness 与回归测试 | 全规格 golden/PDC/export 的自动化发布门禁仍需建立和复核 |
| M4 实时与设备 | CoreAudio HAL、render-ahead/direct、transport/loop、设备适配/诊断、Session 换图及 bar/beat/marker/timecode 入口、换图回收与模拟设备测试 | 修正循环负载后的 10/60 分钟 soak、真实设备切换/拔插和 callback 指标仍需验收 |
| M5 npm/DX/插件 | CLI render/export-midi/doctor、显式动态插件注册/校验/故障计数，最新提交有 C/Rust/N-API 测试 | 平台包/发布/签名公证、逐节点 deadline watchdog、示例/API reference/迁移说明；`oxitone` 当前仅导出底层 facade，未提供仅安装它即可使用 Project 的包结构 |
| M6 Preview | `09-preview-app.md` 规格 | runner/watch、IPC、GPUI viewer、CLI preview 和分发均未建立 |
| M7 稳定性/发布 | 定向回归、插件 conformance、基准 harness | fuzz/sanitizer、持续负载 endurance、故障注入/资源上限、SBOM/签名公证和自动发布门禁 |

规格中的项目目录保存/读取（formatVersion、资产相对路径、原子写入）已在后续阶段提供，
详见下文；preset 仍未提供。canonical snapshot 编解码本身不能替代这些功能。

## 审查与性能记录的解释

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
4. 建立 macOS CI、npm 平台包和用户示例；跑持续有声负载性能与设备验收。
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

当前剩余重点：预设；macOS CI/npm 分发/示例；Preview runner/GPUI；
持续有声负载及设备拔插、资源预算/watchdog、fuzz/sanitizer/SBOM/签名公证等发布门禁。
这些项仍未完成，逐项实现和记录出口证据后才能关闭对应里程碑。
