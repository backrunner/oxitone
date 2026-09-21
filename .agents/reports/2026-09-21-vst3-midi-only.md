# VST3 MIDI-only 接入

Engine 1.7 / stream 10 支持物理音频输入与输出均为空、具有主 MIDI 输出的 VST3。
包括接收乐谱/上游 MIDI 的转换器，以及没有 MIDI 输入、按工程时钟运行的自主生成器。

## 行为与边界

- inspection、Ready、目录验证和状态捕获保留真实的空 audioBuses 与零 input/outputChannels。
  必须声明 noteOutput；有音频输入却没有音频输出的分析器仍拒绝。
- 即使厂商类别为 Fx，纯 MIDI 插件也作为 Channel instrument 使用，经 routeMidi 连接下游。
  图适配器输出静音，原生插件处理时仍是零音频 bus，不伪造厂商布局。
- stream 10 的回复允许 busCount=0，保留 frames、transport、reset 与 MIDI，省去 PCM。
  无音频输入的请求沿用静音占位 bus；旧 stream 1…9 明确拒绝。Engine 快照和 C ABI 版本不变。
- 无输入的生成器绑定有效乐谱或成为 MIDI 路由目标时，在编译能力检查中拒绝，避免静默丢音符。
  独立 stereo WAV API 与只消费 PCM 的 ScheduledPort/Manager 拒绝零音频输出；
  工程图可将 MIDI 连接音源后导出 WAV。
- 配置恢复、运行中捕获、换图、源码编辑、Undo/Redo/Save/reopen 沿用既有实例生命周期。
  图节点仍在隔离 helper/后台执行；设备 callback 不进入插件、JS、JSON 或 N-API。
- 新增实现和测试文件保持单一职责；新增 graph MIDI 校验、metadata、原生 probe 与集成模块
  均小于 250 行。生成 schema 通过生成命令更新。

## 验证

- `pnpm test`：124 suites、499 tests 全部通过。
- `cargo test --workspace`：92 suites，656 passed、3 ignored、0 failed；ignored 为已有跳过项。
- `cargo test -p oxitone-vst3-host --features host,stream --locked`：70 passed，7 suites。
  零总线回复保留短块 frames 与 MIDI，错误 stereo receive 不消费完成项；连续 receive 不复用
  旧事件。端口提交/接收过程中 allocation/free 均为零。
- `pnpm build`、Preview debug build 和 `pnpm build:wasm` 完成；协议 schema 已重新生成。
- `pnpm format:check`、`pnpm lint`、`pnpm typecheck`、`cargo fmt --all --check` 通过。
- 原生集成覆盖 inspection/configuration、两级 MIDI-only 链、无路由静音、mute 后事件交付、
  工程保存恢复、自主生成器采样时序、模拟播放 loop/seek/update、运行中 capture。
  四项新增 PCM 对拍各 48000 个交错样本，最大误差均为零。
- 文档层执行真实插件验证，再检查零总线目录、配置源码编辑、Undo/Redo/Save/reopen、
  稳定实例与路由。无 MIDI 输入生成器绑定乐谱及独立 WAV 调用均明确拒绝。
- macOS arm64/x64 CI 的 MIDI 集成步骤已包含 MIDI-only 检查；未触发远端 CI。

## 性能证据

所有测试结束后，依次独立运行 Playlist benchmark 与真实 MIDI 集成。
Apple M4、Darwin 27 arm64、Rust 1.98.1 / LLVM 22.1.8，48 kHz、128 frames。
音频/MIDI 与纯 MIDI 端口各执行 400 次往返；纯 MIDI 的 p95/p99 为
0.156042/0.259125 ms。MIDI-only 模拟播放 263 blocks，xruns、deadlineMisses、nanBlocks、
queueDrops 与插件 faults 全部为零；后台块时间 p95/p99 直方图桶均为 0.524288 ms。
这些不是硬件 callback 测量，callback p95/p99 保留 null。

`cargo bench -p oxitone-bench --bench playlist -- --warm-up-time 1 --measurement-time 2
--sample-size 20` 完成。32/1024 placements 的估算分别为 118.74/118.30 µs，
2048-block DSP profile 的 p95/p99 分别为 124.625/128.500 µs 和 133.375/137.834 µs。
Criterion 对前次本地运行的均值比较，32 placements 报告 +6.06%，1024 为 +1.14%。
前者的 DSP p95/p99 则低于前次记录；保留全部结果，不把该局部比较认定为发布基线门禁通过。
历史发布基线的同源性仍未确认。

- [真实 MIDI、MIDI-only、PCM、状态与源码事务](../../benchmarks/results/2026-09-21-vst3-midi-only.json)
- [Playlist 环境、日志及 Criterion 原始样本](../../benchmarks/results/2026-09-21-vst3-midi-only-playlist.json)

## 尚未完成

SysEx、Note Expression、辅助事件总线、Mixer/Master 反馈 MIDI、内置/C ABI 1 MIDI 接收、
硬件 MIDI 输出和运行时 MIDI 录制到 SMF 仍不支持。surround、Channel 外部音频输入、
商业插件与外部资源恢复、厂商 GUI 人工交互、长时负载、Intel 实机、签名/公证与发布验收
仍需独立完成。本轮没有打开硬件音频输出，也没有提交或发布。
