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

1. 补齐 TS mixer/Channel insert authoring，验证快照隔离、revision、非法路由和
   实际 native WAV 输出；复用现有协议 1.0，不改变 DSP callback。
2. 补齐 Sample/SampleClip 与导入 facade、fit helpers、Track 剩余配置。
3. 完成 insert 参数自动化、Session 换图/播放位置、项目持久化与预设。
4. 建立 macOS CI、npm 平台包和用户示例；跑持续有声负载性能与设备验收。
5. 实现 Preview，完成 fuzz/endurance/发布门禁。各项出口分别记录证据。
