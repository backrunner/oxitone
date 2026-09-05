# Oxitone 全仓审查结果

**结论：现有实现存在明确缺陷，暂不能认定正确或满足实时线程要求。**

本轮按 `oxitone-guard` 和仓库规格审查 TS authoring/protocol、N-API、Rust graph/transport/DSP/instruments/mixer/samples/render、CoreAudio 和 benchmark。清点到 196 个源文件、33,430 行；这是一次广泛审查，不是对所有分支的形式化正确性证明。

原有 lint、typecheck、Rust 格式检查、Rust 全工作区测试全部通过。重新构建原生模块与 TS 后，130 个原有 TS 测试仍全部通过。但补充的 **17 个 Rust 定向断言全部失败**，另有 TS 实测确认 authoring revision 不更新。完整代码依据和修复建议见 [详细报告](review.md)，复现源代码见 [Rust 用例](review_probes.rs) 与 [TS 用例](authoring-probe.mjs)。

## 问题清单

P1 表示应优先修复、阻碍可靠使用；P2 表示确定的行为缺陷或验收缺口。“静态”表示从调用路径确认问题，未故意触发竞态或设备故障。

| # | 优先级 | 问题与实际影响 | 证据 |
| --- | --- | --- | --- |
| 1 | P1 | 控制线程与设备监控线程同时写同一 SPSC 队列，存在数据竞争、命令覆盖及未定义行为 | 静态调用路径 |
| 2 | P1 | 合法深度 64 的 restart chance 表达式编译成功，求值时越过 160 字节缓冲并 panic；direct 路径可能终止进程 | 实测越界至 167 字节 |
| 3 | P1 | Rust 小数 beat 转换改变数值，frame→beat 甚至倒退 | 1/24000 变成 0.005333；frame 2→3 对应 beat 下降 |
| 4 | P1 | 音频线程执行堆分配与释放；seek 还会重建 Sampler/Slicer voice pool | 首个音频块、首次参数入队均测到分配；其他路径静态确认 |
| 5 | P1 | 队列满时静默丢弃 transport/换图/shutdown；仍返回成功，dispose 可能永远等待 | 静态错误路径 |
| 6 | P1 | 播放中换图直接采用新图的 Stopped/frame 0 状态 | 实测播放停止 |
| 7 | P1 | 换图后 session tempo 不更新，设备 buffer/resampler 配置也未同步 | 120→60 bpm 后 beat 1 仍为 24000 frame |
| 8 | P1 | 参数 atFrame 和 automation 不连续点未在 block 内分段，audio-rate 参数也仅在块首求值 | frame 64 事件渲染至 127 后仍在队列；frame 120 gate 边界失效 |
| 9 | P1 | loop 只在整块渲染完后回绕，越界事件已经发声 | loop [0,64) 播出了 frame 96 的音符 |
| 10 | P1 | track stem 错误：空轨导出 Master、直连 Master 的有声轨导出静音、共享 bus 的轨无法隔离 | 空轨 -8.759 dBFS；有声轨 -∞ dBFS |
| 11 | P1 | MIDI 忽略 tempo lane，导出时钟与音频不同 | 音频 60 bpm，MIDI 120 bpm |
| 12 | P1 | Mixer/Master insert 的 mix、bypass 字段被忽略 | bypass=true 的极性反转效果仍生效 |
| 13 | P1 | WAV extensible 把有效位数当成容器宽度，解码错位 | 32-bit 容器/24 有效位的 4 帧变成 5 帧 |
| 14 | P2 | note.chance 被序列化和校验，却不参与调度或 MIDI 概率决策 | chance=0 仍有音符事件 |
| 15 | P2 | lastBeat 不截断跨边界的 note-off | clip 在 frame 24000 结束，note-off 位于 96000 |
| 16 | P2 | TS channel.level/pan setter 不增加 revision | 内容变化后 revision 仍为 "1" |
| 17 | P2 | 10 分钟 soak 的音符只覆盖前 16 秒；其余大部分时间没有合成器音符负载 | 静态检查 benchmark 项目长度与播放方式 |
| 18 | P2 | HAL listener 注册失败时，已创建的 IOProc 与 callback state 未释放 | 静态资源生命周期 |
| 19 | P2 | Channel insert 的 dry/wet 与 bypass 未补偿插件延迟，混合与并行路径可能错位 | 静态 DSP 路径 |

## 性能与验证边界

- 本机 Apple M4 / macOS 27.0 / Rust 1.98.0。
- 10 秒模拟输出测试：48 kHz / 128 frames / 8 tracks，0 xrun、0 deadline miss；worker p95/p99 均约 0.262 ms。这不是 CoreAudio callback 耗时，也不代表长时真实设备验收通过。
- 20 秒离线导出基准首次平均耗时 1.1214 秒（约 17.8 倍实时），独立复测 2.1091 秒（约 9.5 倍），未达到规格中的 20 倍目标。波动较大，不能据此精确归因代码性能回退。
- C Plugin ABI、完整 TS mixer/sample authoring 等仍有未完成部分，不能将 Rust 局部实现视为 SDK 已完整交付。
- 物理设备拔插/热切换、sanitizer/fuzz、修正负载后的长时 soak 尚未验证。工作区没有 Git 元数据，无法核查提交历史或归因。

## 本轮交付与后续顺序

本轮仅做审查，没有修复生产实现。临时失败测试已归档到这里，不进入常规 Cargo 测试发现。日志与模拟输出基准记录也保存在本目录，便于复查。

建议先修复并发队列、panic、beat 换算、实时分配/回收，再修复换图、块内分段和 loop；随后处理导出、采样解码、revision 与 benchmark。每项修复应将对应复现用例纳入正式测试，并按仓库要求执行完整检查。
