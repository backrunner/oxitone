# 计划与实现核对（2026-09-05）

本次以 `9423ad3` 为核对基线。工作区干净；已有 5 个提交均使用
`BackRunner <dev@backrunner.top>` 和 `type(scope): description` 格式，无待补提交的实现。
下表依据源码、正式测试和分发目录核对；“已有”不等于整个里程碑通过验收。

| 里程碑 | 已有实现与依据 | 尚未完成或缺少验收证据 |
| --- | --- | --- |
| M0 工程与协议 | pnpm/Cargo workspace、版本协议、canonical fixtures、N-API smoke tests | `.github/workflows` 缺失；干净 macOS 环境安装/构建门禁未建立 |
| M1 时间轴/MIDI | Project/Track/Pattern/Clip、Chord/Arp、tempo/time-signature、确定性 SMF writer 与边界测试 | TS Track 未暴露 `tempo`、`enabled`、`midiChannel`；不能据底层 wire 字段认定 authoring 已交付 |
| M2 音源/采样 | Rust synth/Sampler/Slicer、解码/编辑/SRC、SampleClip stretch/repitch、C ABI/动态插件与 conformance tests | `packages/samples` 缺失；Project 的 samples/sampleClips 固定为空；缺 Sample/SampleClip builders、fit helpers 和内置音源便捷入口 |
| M3 Mixer/Automation/导出 | Rust mixer/PDC/12 effects、automation evaluator/tempo bake、WAV/stem/loudness 与回归测试 | TS 仅固定 Master、Channel effectChain 固定为空；effect 插件参数和 Mixer/Master insert 自动化缺乏完整 binding；全规格 golden/PDC/export 门禁需逐项复核 |
| M4 实时与设备 | CoreAudio HAL、render-ahead/direct、transport/loop、设备适配/诊断、换图回收及模拟设备测试 | SDK 的 marker/timecode 起播、Session 换图入口未完整暴露；修正循环负载后的 10/60 分钟 soak、真实设备切换/拔插和 callback 指标仍需验收 |
| M5 npm/DX/插件 | CLI render/export-midi/doctor、显式动态插件注册/校验/故障计数，最新提交有 C/Rust/N-API 测试 | 平台包/发布/签名公证、逐节点 deadline watchdog、示例/API reference/迁移说明；`oxitone` 当前仅导出底层 facade，未提供仅安装它即可使用 Project 的包结构 |
| M6 Preview | `09-preview-app.md` 规格 | runner/watch、IPC、GPUI viewer、CLI preview 和分发均未建立 |
| M7 稳定性/发布 | 定向回归、插件 conformance、基准 harness | fuzz/sanitizer、持续负载 endurance、故障注入/资源上限、SBOM/签名公证和自动发布门禁 |

规格中的项目目录保存/读取（formatVersion、资产相对路径、原子写入）与 preset
也尚无公共实现；canonical snapshot 编解码本身不能替代这些功能。

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
2. **进行中（2026-09-06）**：Sample/SampleClip 与 fit helpers 已补齐 TypeScript
   authoring 和 snapshot 连接；Track `enabled`/`midiChannel` 与只读文件导入 facade 已完成。
   Track `tempo` 只有 wire 校验，独立时钟换算尚未执行，继续列为缺口。
3. 完成 insert 参数自动化、Session 换图/播放位置、项目持久化与预设。
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
- 表格仍保留起始基线的缺口以便对照；M3 的 TS mixer/Channel insert 空入口在本轮
  关闭，其余列出的缺口继续追踪。下一步优先 Sample/SampleClip authoring 与导入 facade。

## Sample/Track 后续推进（2026-09-06）

- 已提供 Track `enabled`/`midiChannel`，用真实 native MIDI 导出验证禁用 Track 和显式
  channel 分配；跨项目同 ID 的 Channel 不再能被误绑定。
- 修复 Sample bigint 帧数、显式 ID、trim 范围及音乐长度校验，失败构造不注册实体；
  draft 可重试。`fitBars` 使用起始拍号且保留完整长度，`fitToContent` 使用 trim 后帧数。
- fit helpers 尚不能据此视为完整验收：`fitToContent` 的 tempo ramp/lane 换算待实现；
  `repitch` 使用内容音乐长度作为分母，显式 duration 的缩放语义仍需补齐并做音频验证。

- 新增 `@oxitone/samples#importSample` 和无 engine 的版本化 `inspectSample` 命令。
  Rust 返回源 hash、格式、解码维度与 provenance，JS 不持有 PCM。完成 WAV/AIFF 识别、
  6 声道降混、AAC/M4A 真文件、24→48 kHz SRC/trim、相对目录迁移、文件变更/缺失、
  损坏容器及 native 协议错误测试；两次有声 WAV 渲染逐字节一致。
- 目前只提供元数据 descriptor 导入，未实现缓存 WAV 写入或 provenance 持久化；
  继续追踪项目保存/加载、内置音源便捷入口和原表中的其余缺口。
- `pnpm schemas`、`pnpm build`（release native addon）、`pnpm lint`、`pnpm typecheck`、
  TS 测试（164 tests）、`cargo fmt --all --check`、`cargo test --workspace`（377 tests）
  通过。TS 全量测试先通过 162 tests，新增的 native 请求及 AAC 测试随后在各自包中通过。
- Apple M4 / macOS 27.0 / Rust 1.98.0，48 kHz stereo，30 samples：1 秒 PCM16
  inspector slope estimate 为 0.835 ms，10 秒 float32 为 10.477 ms。测量包含文件读取、
  hash、完整解码和释放，使用 warm filesystem cache；首次建立该场景基线，无回退结论。
  [基准归档](../../benchmarks/results/2026-09-06-sample-import.json) 记录置信区间、实际
  测量时长和环境；未测设备/callback/xrun 的值为 null，不替代长期实时验收。
