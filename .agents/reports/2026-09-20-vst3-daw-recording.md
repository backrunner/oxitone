# VST3 DAW 自动化录制与可编辑 Playlist 源码

## 实现

Document 接通 Start/Stop/Cancel recording，Touch/Write、最多 32 个参数选择和会话状态。
捕获绑定源码 revision/read set、配置使用和 Preview graph generation；复用实际播放
实例及 SDK 分页收集器。取消可绕过正在等待的请求，迟到 start 先取得原 captureId 再清理，
不能遗留占用或把取消的结果接受。源码变化使旧任务失效；同 revision 的 Save 保持录制。
完整 take 接受失败后保留在会话中，可重试或取消；未完成的 capture 失败不发布部分结果。
插件元数据变化会清除窗口中已失效的参数选择；其他 VST3 面板也可取消文档的旧录制任务。

Engine 1.5 增加可选 lane.priority:u32，缺省 0。同目标按 priority、lane ID 组合；
同一优先级的多 lane 仍要求 combine，legacy/typed aliases 共享同一检查。携带字段
的旧版本拒绝；不改写不支持的恢复数据。排序在编译侧，音频回调未添加锁、分配或 IPC。

Project.recordAutomation 先验证 take 和全部曲线，再为有数据的每个参数建立一个 Track，
每个连续区间建立一个局部零点 step Source/Playlist lane/clip。后录入的重叠区间优先；
priority 高于已有 lanes，仅在录制片段内覆盖，空隙恢复原有自动化。原 global/playlist、
loop、combine、生成器和共享来源保持；Track M/S 沿用既有 Playlist 规则。已有启用的
solo Track 时，新录制 Track 加入同一 solo 组，避免刚接受的录制层被 solo 过滤。

DAW 回写普通 AutomationSource、param.automate、createAutomationClip 调用，各曲线
有独立源码边界，可继续编辑。局部包裹表达式保留原 Project 表达式一次求值和原注册，
不写实例/任务 UUID，不额外执行工厂；候选全工程比较后一次接受，支持 Undo/Redo/Save。
早期纯 recordAutomation 调用虽然回放正确，却没有可编辑 Source capture，已改为上述
普通 builder 表达式，并用实际范围编辑/撤销验证，不保留旧写入路径。

## 验证

- Rust workspace：630 passed、3 ignored、88 suites。包括 priority 排序、typed/legacy
  aliases、版本门禁、Playlist 精确边界和 64/128/256 block 的逐样本一致性与零分配。
  最终面板状态调整后 Preview 专项 78 passed、3 ignored 再次通过。
- TypeScript 分批回归合计：487 passed（protocol 58、native 28、VST3 16、core 197、CLI 150、MIDI 6、
  samples 11、web 9、SDK 2、VS Code 1、drums 9）。CLI 包含乐器/通道插入/总线插入、
  多参数/多间隔、连续录制、绑定重名与原工厂单次求值；生成 TS 通过 strict 和
  noUncheckedIndexedAccess 编译检查。Wasm/原生 PCM parity 含新优先层。
- [Debug Preview conformance](../../benchmarks/results/2026-09-20-vst3-daw-recording.json) 与
  [Release Preview conformance](../../benchmarks/results/2026-09-20-vst3-daw-recording-release.json)：
  Recording Gain 真实 VST3 helper，生产 DawRunner/PreviewConnection、模拟输出。
  录制循环及 seek，与原生 sample 包独立生成的参考 WAV 逐字节一致（192000 PCM 样本）；
  修改录制曲线、撤销该修改、撤销录制、重做、Save/reopen 都验证音频。
- 同一真实链路覆盖完整 take 的接受失败/重试、空 Touch 无修改、启动中取消、停止中取消、
  迟到 start 的精确清理与再次捕获、源码变化失效、录制中 Save。不启用系统音频输出。
- Workspace/native/Preview/Wasm 构建通过，Preview 生成 unsigned development bundle。
  format、lint、typecheck、cargo fmt、git diff --check 通过，schema 已更新。
  首次 Web 测试使用旧 1.4 Wasm；随后 workspace build 清空已重建的 dist/Wasm。
  按 workspace build → build:wasm 顺序重建后 Web 9 项通过，未改变拒绝旧产物的版本检查。
- macOS arm64/x64 CI 新增 DAW recording smoke，复用前置步骤构建的 fixture/helper。
  本机实际执行为 arm64；GUI 手动操作/视觉验收与 Intel 执行没有取得新证据。

## 性能

[Focused insert benchmark](../../benchmarks/results/2026-09-20-vst3-daw-recording-benchmark.json)
新增 recording_layers compile/render 场景，保留 ordinary/typed 历史场景。Apple M4，
48 kHz/128 frames，预热 1 秒、测量 3 秒、30 个 Criterion 样本；计时前本轮其余构建
和测试已结束。普通/typed/录制层 render 均值分别为 173.018/175.695/180.560 µs；
录制层归一化批次 p95/p99 为 195.704/221.400 µs，compile 均值 47.343 µs。

普通 render 相比缓存历史无显著变化；typed 均值增加约 1.72%，Criterion 判定在噪声
阈值内。录制层是新增额外 lane/placement 的不同负载，没有旧同场景基线。这些是内存
图渲染的批次均摊数据，不是设备 callback 分位数或 VST3 IPC/厂商 GUI 延迟；没有设备
的 xrun 数据。缓存历史不等于受控源码/环境基线，不关闭此前 Preview telemetry 回退。

## 剩余

1. 运行中 I/O、latency、参数表变化的 state 交接、PDC 重编译和原子发布。
2. 效果器多输出、更多输入、MIDI 输出与 surround。
3. 商业资源/ensemble/长加载/soak、实际厂商 GUI、Intel 执行、签名与公证。
4. 受控性能基线及此前 Preview telemetry 回退仍未关闭。

本轮完成 DAW 参数录制的源码与回放闭环；完整 VST3 接入目标仍进行中。
