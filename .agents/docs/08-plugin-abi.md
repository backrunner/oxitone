# Oxitone Plugin ABI 与动态加载

新增静态 npm 插件目录格式和 GPUI/helper 验证入口见 [18-project-daw.md](18-project-daw.md)。
metadata format 1 不改变 ABI 1 的执行能力，不声称资源/state 已进入外部 C 插件。

发布前已批准迁移到 [ABI 2 与插件管理器](../designs/source-daw/05-plugins.md)：参数、资源、
configuration state、UI 事务与独立实例均进入同一 authoring 模型。下文仍记录现行 ABI 1，
header/host/真实 C fixtures 必须一起迁移；不能让 ABI 1 插件被误认为支持新配置能力。

Wasm host 静态链接自带 `example.drums`，调用相同 `oxitone_plugin_entry_v1` 和
`plugins::from_entry` 校验/适配路径。Wasm 目标不编译 libloading/CoreAudio；
原生 .dylib 不能上传后执行。第三方需重编译并静态集成；动态 Wasm plugin linker
不在当前交付中，详见 `12-wasm-web-audio.md`。

本文定义第三方 Instrument/Effect 的注册、C ABI 和生命周期。实现位于
`oxitone-graph::abi_c`（记录与 descriptor 校验）、`oxitone-render::plugins`
（加载和实例）、`oxitone-napi` 和 `oxitone` TypeScript facade。

当前 ABI v1 不提供插件自带 UI 入口。Preview 已能从已验证 descriptor 为内置/动态
插件生成多窗口只读详情，并通过 Project.registerPluginUi 注册自定义 GPUI 原生布局。
已实现的布局协议、后续独立 native UI ABI 与实时反馈规划见 `11-plugin-ui.md`。
UI 不改变此处 audio ABI；宿主尚不加载 native UI companion symbol。

## 注册与所有权

```ts
import { createEngine, registerPlugin, compile, renderWav, dispose } from "oxitone";

const engine = createEngine({ allowPlugins: "any" }); // 本地 unsigned fixture
try {
  const registered = registerPlugin(engine, {
    libraryPath: plugin.pluginPath(),
    manifest: plugin.manifest,
    expectedHash: plugin.sha256, // 可选，64 位十六进制 SHA-256
  });
  compile(engine, project.snapshot());
  renderWav(engine, project.snapshot(), { path: "mix.wav" });
} finally {
  dispose(engine);
}
```

- 注册是同步的控制线程操作；只加载显式路径，不扫描目录、不联网。
- `manifest` 必填，格式由 `PluginManifest` 与
  `schemas/plugin-manifest.schema.json` 定义：pluginId、pluginVersion、
  abiMajor/abiMinor、minHostVersion、kind、输入/输出布局、完整有序参数表、
  sidechainInput、reportsTail 和可选 maxPolyphony。
- 宿主先检查 manifest 版本、读取文件 hash、执行签名策略，再加载库和解析
  `oxitone_plugin_entry_v1`。库的初始化器可能在 descriptor 校验之前运行。
- C descriptor 与 manifest 必须一致，包括参数顺序、标签、范围、默认值、
  smoothing、rate、mapping、automation。automation 缺省与 false 等价。
- one-pole smoothing 的 wire 拼写统一为 `one-pole`（与 TS/manifest schema 一致）；
  Rust 与 TS 均只接受 `one-pole`，旧内部拼写 `onePole` 已删除，fixtures 同步使用公开拼写。
- 每个 engine 有独立注册表。相同 ID/version 且相同文件 hash 重复注册幂等；
  同 ID/version 的不同二进制报错。内置插件 ID 不允许被动态库覆盖。
- compile 和 renderWav 使用该 engine 的注册表；后者仍按传入 snapshot 独立编译。
  `Project.compile()` 创建新 engine；使用第三方插件时，通过上述 facade 显式注册。
- descriptor 字符串被复制为宿主拥有的数据。实例持有动态库引用，先调用
  dispose 再释放库；注册表销毁不会使存活实例的函数指针失效。
- 实时换图将旧图送入有界回收队列，由控制线程执行插件销毁和卸载。
  回收队列满时延后取命令；命令队列满时报 RealtimeFault，不返回成功确认。
  当前实时 session 不接受 sampleRate/blockSize 变化，需要重建 session。
  graph swap 保留 transport 和 automation origin，声音状态从已 prepare 的新图开始，
  当前不跨图延续旧实例尾音。离线导出长度由 tailSeconds 指定，暂不根据
  动态实例的 tail_frames 自动延长。

## ABI v1

权威 C 声明在 `include/oxitone_plugin.h`，Rust 镜像在
`crates/graph/src/abi_c.rs` 和 `abi_c/runtime.rs`。使用平台原生对齐，
禁止 packing；边界上没有 Rust trait object、String、Vec 或泛型。

```c
const OxiPluginEntryV1 *oxitone_plugin_entry_v1(void);
```

入口返回静态元数据和函数表；前缀是 `abi_major, abi_minor, struct_size`。
major 必须为 1，struct_size 至少覆盖宿主 v1 记录，minor 扩展只能追加兼容字段，
使用 `minHostVersion` 声明所需宿主版本（严格 SemVer）。descriptor/entry/manifest
的 ABI 版本必须互相匹配。所有 C 字符串是有效、NUL 结尾的 UTF-8。

生命周期：

| 方法 | 线程与约束 |
| --- | --- |
| create | 控制线程，接收仅在调用期间有效的 host context；返回独占实例，NULL 表示失败 |
| prepare | 控制线程，可分配；参数为 sample rate、最大 block frames，返回 0 成功 |
| process | 实时路径，返回 0 成功；非 0 触发节点静音 |
| reset | 实时安全的 flush，seek/loop/换图可调用；保留参数设置，清空 voices/delay state |
| tail_frames | 实时安全；仅 reportsTail 为 true 时读取 |
| latency_frames | 实时安全；仅 prepare 可改变，宿主参与 PDC |
| dispose | 控制线程，释放实例；此时库仍已加载 |

每个实例的调用串行化，但不同实例可能来自不同控制线程；实例必须能在线程间转移。
process/reset/tail/latency 不得分配或释放 heap、加锁、阻塞、I/O、日志、读取时钟或调用 JS。
禁止跨 C 边界 unwind；Rust 插件应使用不会 unwind 的入口并在插件内部处理错误。

- 音频是宿主拥有的 non-interleaved f32 buffer；所有表的 count 都是实际可访问长度。
  process 的指针只在该次调用期间有效，不得保留或越界访问。
- 宿主图是 stereo。mono 输入取 (L+R)/2，mono 输出复制到 L/R；
  sidechain 存在时固定提供两个声道，缺省时 count 为 0。
- note 事件含 frame_offset、kind（0 on / 1 off）、pitch（0..127）和 velocity（0..1）。
- 参数通过 descriptor 索引传递物理值，frame_offset 小于 frames；
  同帧事件保持输入顺序。插件自行实现 smoothing。跨 note/parameter 列表的同帧优先级
  为 note-off、parameter、note-on。
- v1 最多 256 个参数、每次 process 最多 256 个参数事件；宿主先校验事件和缓冲区。
  prepare 最大 block 为 65,536，sample rate 上限 768 kHz；latency 上限为 10 秒。
- create/prepare 必须建立 descriptor 默认参数，缺省参数不会另发事件。
  v1 C ABI 不传递 sample resources 或 structured state；这些能力需要后续 ABI 扩展。

Channel/Mixer/Master insert 的初始参数、host events 和 automation 在每个 render
segment 内按参数索引合并，最后的值生效。暂存空间在 compile 时按 descriptor
参数数预分配；ABI 事件借用的参数 ID 仅在 process 调用期间有效。Rust trait 的
320 参数队列测试不改变 C ABI v1 的 256 参数上限。

## 故障与信任边界

`getPluginDiagnostics(engine)` 返回按 ID/version 排序的
`{ pluginId, pluginVersion, faults }` 数组。faults 是该注册插件所有实例累计的
静音故障次数，包含 offline render；一次实例故障只计数一次，reset/成功 prepare
解除 latch 后的新故障重新计数。

- process 返回非零、输出出现 NaN/Inf、处理期间 latency 改变或宿主 context 不合法：
  当前实例输出静音并 latch，其余实例和图继续处理。动态边界始终检查有限输出，
  成本由 plugin_process benchmark 覆盖。
- create 返回 NULL 或 prepare 返回非零：compile 报 RealtimeFault，保留旧可播放图。
- hash、签名、descriptor/manifest 不匹配：PluginManifestMismatch。
- ABI major、最小宿主版本、入口 record 太短、入口 symbol 缺失：PluginAbiMismatch。
- 路径或动态库加载失败：AssetUnavailable。

原生插件与宿主同进程，是显式信任的本机代码。ABI 校验不能防止任意无效指针、
越界写、崩溃或无限循环；SHA-256 校验也不是沙箱，不涵盖库的依赖或加载期间被
并发替换的文件。库文件及依赖在加载和使用期间必须保持可信、不可变。
逐节点 deadline watchdog 和进程外/WASM 隔离尚未实现；现有 worker deadline
诊断不等于插件超时隔离。

`allowPlugins` 默认在 release 为 signed-only，在 debug 为 any；显式 any 允许
unsigned 开发库。macOS signed-only 使用 `codesign --verify --strict` 及 Apple
信任锚要求，拒绝 unsigned/ad-hoc 签名。此校验不证明 notarization 或依赖链策略；
发布签名和公证仍是独立门禁。非 macOS 平台目前拒绝 signed-only。

## npm 分发格式

```text
@acme/oxitone-osc/
  package.json
  index.js / index.d.ts             # manifest、sha256、pluginPath()
@acme/oxitone-osc-darwin-arm64/
  lib/libacme_osc.dylib
  manifest.json
```

插件 npm 包提供 manifest、参数类型和平台路径；不得要求在 TS 内编写音频回调。
平台包选择遵循 `06-format-and-export.md`，禁止运行时下载二进制。
macOS 发布库必须签名并完成公证流程；浏览器下载文件可能带 quarantine。
本仓库目前提供 ABI 和 fixture，没有发布第三方平台包或自动公证流水线。

## Conformance 与验证

仓库自带 `example.drums@1.0.0` 开发示例（`crates/example-drums`），通过真实
Rust cdylib 注册，不占用内置 ID。四个固定 one-shot 声部：MIDI 36 底鼓、38 军鼓、
42 闭镲、46 开镲；同 pad 重触发，闭镲终止开镲，忽略 note-off。`volume` 为 0..1
线性音量（无平滑），`decay` 为 0.5..2 的时长倍数（note-on 时采样）。reset
保留参数、清空声部并重置固定 noise seed；process 不分配，声音不依赖 block size。
这是随仓库提供的可重建示例，不是已发布或已签名的内置产品包。

`pnpm example:drums` 构建并显式注册上述音源与参考 C `fixture.gain` 效果器，
执行初始参数、host 参数、automation 和无故障检查，然后输出 16 小节歌曲、
鼓机独奏、MIDI、可恢复项目、snapshot 与 hash/peak/LUFS 报告。无需采样下载或设备；
重新加载项目后仍需在同一 engine 注册两个库，库路径不写入项目。

- `crates/render/tests/fixtures/gain.c`：纯 C 参考效果器；同一源码可静态编译为
  runner 或动态库，测试逐样本 bitwise parity、参数排序、mono、sidechain、
  null factory、prepare 失败和故障 latch。
- `fixtures/instrument.rs`：真正通过 rustc 编译的 Rust cdylib，验证 note、tail/reset。
- `plugins_validation.rs`：ABI prefix、缺失 symbol、兼容 minor、错误缓冲区，
  以及 process/reset 的宿主 heap 分配/释放计数。
- realtime retirement tests 覆盖 direct 和 worker 换图、设备链回收以及满队列关闭。
- `packages/native/test/plugins.test.ts` 经过真实 N-API 注册、引擎隔离、compile、
  WAV parity/gain 和故障诊断；不需要音频设备。
- 命令：`cargo test -p oxitone-render --test plugins --test plugins_validation --lib`，
  `pnpm --dir packages/native test`，
  `cargo bench -p oxitone-bench --bench plugin_process`。

内置 Rust 插件继续通过 Plugin/PluginInstance trait 静态注册并共享生命周期和
descriptor 规则；它们并非逐一导出 C symbol。纯 C 静态/动态 fixture 验证实际 C
边界，不能用只测试 Rust trait 的结果代替。
