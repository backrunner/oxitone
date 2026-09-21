# VST3 Channel MIDI 输出路由

Engine 1.7 / stream 9 接通 Channel 乐器及 insert 的主 MIDI 输出到下游原生乐器。
本轮完成中断后的验证、故障清理回归与 CI 接入；这不是完整 VST3 兼容性或发布验收。

## 行为与边界

- `channel.routeMidi(instance, [destination])` 以稳定实例身份保存连接；空数组断开。
  验证源归属、输入输出能力、重复目标、悬空引用及环；effect 重排保留路由，删除清理路由。
- Channel 按 MIDI 拓扑处理，事件在同段交付并保留 sample offset，不额外延迟一个 block。
  音频仍按 Channel ID 求和。MIDI 扇出各目标只收到一次；音频 mute、solo、mix、bypass
  不截断 note-off。Channel insert 不自动接收所属 Channel 的乐谱音符。
- 主事件 bus 0 支持 MIDI 1.0 通道消息。输入（含参数与乐谱）和输出各最多 256 条/段；
  无效事件、队列溢出及不支持的事件报错，不丢弃最早事件继续播放。普通无路由实例不捕获输出。
- 同段参数 A→B→A 保留返回 A 的事件；接收可能映射参数的 MIDI 后，显式 authored 值
  仍发送给插件，不因宿主旧缓存而丢失恢复值。
- seek、loop、reset 清空 MIDI 暂存。中途其他节点失败时，离线/后台处理整块静音、
  停止 transport 并重置 MIDI 图，未执行目标排队的音符不会在下次播放重现。
- 插件继续在隔离 helper/后台图执行。新增 MIDI 图路径使用预分配 PCM 与事件容量；
  设备 callback 不执行厂商处理、JSON、JS 或 N-API。C ABI 1 不变。
- 快照、恢复、生成 schema、配置源码编辑与 Undo/Redo/Save/reopen 保持连接与实例身份。
  Preview 显示可导航的连接；路由编辑使用源码 API。拒绝旧 stream 1…8 和不支持的新字段版本。

## 验证

- 中断前启动的 `cargo test --workspace` 已完成：650 passed、3 ignored，91 suites；
  ignored 为已有显式跳过项。随后新增/强化的 MIDI 图专项 3/3 通过，覆盖短段、loop、seek、
  两种执行域失败清理、callback 隔离；成功处理与 reset 的 heap allocation/free 均为零。
- `cargo test -p oxitone-vst3-host --features host,stream --locked`：68 passed，7 suites。
- `pnpm test` 完整重跑：123 suites、497 tests 全部通过。此前 helper 与源码执行超时
  未再出现；未放宽时间上限、跳过测试或修改功能预期。
- `pnpm format:check`、`pnpm lint`、`pnpm typecheck`、`cargo fmt --all --check`、
  `git diff --check` 通过。上轮 native release build 已完成；真实 MIDI 集成使用该构建。
- `smoke-vst3-midi.mjs` 完成 400 次真实 VST3 port 往返，验证通道/采样位置、reset、
  关闭捕获、非法输出与溢出故障。四项工程 PCM 对拍各 48000 个交错样本，最大误差
  1.41e-45，覆盖逆 ID 依赖顺序、扇出、目标静音和工程保存恢复。
- 同一集成验证 muted/bypassed insert MIDI、取消连接、实例删除及能力拒绝；源码事务
  验证配置编辑、Undo/Redo、Save/reopen 和稳定路由。模拟 sink 377 blocks，
  xruns、deadlineMisses、nanBlocks、queueDrops、plugin faults 全部为零。
- macOS arm64/x64 CI 已增加同一 MIDI 集成脚本并归档 JSON；未触发远端 CI。

## 性能证据

Apple M4、Darwin 27、Rust 1.98.1 / LLVM 22.1.8，48 kHz、128 frames、stereo。
真实 MIDI port 往返 p95=0.325458 ms、p99=0.539917 ms；模拟后台图直方图
p95/p99 桶为 1.048576/2.097152 ms。它们均不是硬件 callback 指标；没有打开系统音频输出。

完成全套测试后独立运行 `cargo bench -p oxitone-bench --bench playlist -- --warm-up-time 1
--measurement-time 2 --sample-size 20`。32/1024 placements 的 Criterion 估算分别为
113.78/117.08 µs；2048-block DSP profile 的 p95/p99 分别为
148.792/178.250 µs 与 131.250/136.208 µs。较前一轮并行构建期间的结果明显回落，
但该对照不能代替已知源码、相同环境下的发布基线；不宣称历史性能门禁已清除。

- [真实 MIDI、PCM、模拟播放和源码事务](../../benchmarks/results/2026-09-21-vst3-midi-routing.json)
- [前一轮并行构建期间的 Playlist 测量](../../benchmarks/results/2026-09-21-vst3-midi-playlist-before.json)
- [独立复测、环境与 Criterion 原始样本](../../benchmarks/results/2026-09-21-vst3-midi-playlist.json)

## 尚未完成

SysEx、Note Expression、辅助事件总线、零音频输出的 MIDI-only 插件、Mixer/Master
反馈 MIDI、内置/C ABI 1 MIDI 接收、硬件 MIDI 输出和运行时 MIDI 录制到 SMF 仍不支持。
surround、Channel 外部音频输入、更多商业插件与外部资源恢复、厂商 GUI 人工交互、
长时负载、Intel 实机，以及签名/公证/远端发布仍需独立完成。历史性能基线尚未证明同源，
发布性能门禁不能由本次功能通过代替。本轮未提交或发布。
